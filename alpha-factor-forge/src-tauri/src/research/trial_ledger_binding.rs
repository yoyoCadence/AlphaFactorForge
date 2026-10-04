//! P12b-2a: workspace evidence of the registry prefix it last observed.
//! Call `check_binding` before adopting a registry and write the returned
//! current head inside the workspace transaction that creates attempts.
//!
//! §23 (2026-10-04): a binding also carries the restrictive evidence observed
//! with that head — quarantined families, conflicted origins and each family's
//! test-count high-water mark. That evidence lives outside the event chain, so
//! the chain alone cannot see it disappear when an older registry copy comes
//! back. The admission fence keeps using the event prefix only.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    admission_in, genesis, head, verify_chain, Admission, AdmissionBlocked, AdmissionCount,
    LedgerError, LedgerIntegrity, TrialLedger, TRIAL_FAMILY_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const BINDING_KEY: &str = "trial_ledger_binding";
/// The stored binding format since §23. The pre-§23 format has no `version`.
pub const LEDGER_BINDING_VERSION: &str = "trial-ledger-binding-v2";

fn valid_registry_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_lower_hex(text: &str, len: usize) -> bool {
    text.len() == len
        && text
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_family_id(id: &str) -> bool {
    id.strip_prefix(TRIAL_FAMILY_VERSION)
        .and_then(|rest| rest.strip_prefix(':'))
        .is_some_and(|hash| valid_lower_hex(hash, 64))
}

fn invalid_binding(detail: &str) -> LedgerError {
    LedgerError::StateInvalid(format!("invalid trial_ledger_binding: {detail}"))
}

/// The event-chain part of a binding or of an admission snapshot.
struct EventPrefix<'a> {
    registry_id: &'a str,
    seq: u64,
    chain_head: &'a str,
}

impl EventPrefix<'_> {
    fn validate(&self) -> Result<(), LedgerError> {
        if !valid_registry_id(self.registry_id)
            || self.seq > i64::MAX as u64
            || !valid_lower_hex(self.chain_head, 64)
            || (self.seq == 0 && self.chain_head != genesis(self.registry_id))
        {
            return Err(invalid_binding("bad event prefix"));
        }
        Ok(())
    }
}

/// Restrictive evidence a workspace has observed (§23). Each part only grows
/// in a registry (append-only tables, upward-only test counts) and a complete
/// export carries all of it, so a registry that no longer covers it has lost
/// history the workspace already saw.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LedgerEvidence {
    /// Families with a recorded conflict (§20): permanently quarantined.
    pub quarantined_families: BTreeSet<String>,
    /// Origins with a recorded registry divergence (§20): they can never
    /// authorize a replacement again.
    pub conflicted_origins: BTreeSet<String>,
    /// Each pinned family's effective tests per trial (§22) — a high-water
    /// mark observed, not an allowed maximum.
    pub family_tests: BTreeMap<String, u64>,
}

impl LedgerEvidence {
    fn validate(&self) -> Result<(), LedgerError> {
        if !self
            .quarantined_families
            .iter()
            .all(|id| valid_family_id(id))
            || !self
                .conflicted_origins
                .iter()
                .all(|id| valid_registry_id(id))
            || !self
                .family_tests
                .iter()
                .all(|(id, tests)| valid_family_id(id) && (1..=i64::MAX as u64).contains(tests))
        {
            return Err(invalid_binding("bad evidence"));
        }
        Ok(())
    }

    /// Whether this (current) evidence still contains everything `saved`
    /// recorded. A family quarantined now satisfies its saved test count:
    /// it cannot be admitted at any count.
    fn covers(&self, saved: &LedgerEvidence) -> bool {
        saved
            .quarantined_families
            .is_subset(&self.quarantined_families)
            && saved.conflicted_origins.is_subset(&self.conflicted_origins)
            && saved.family_tests.iter().all(|(family, tests)| {
                self.quarantined_families.contains(family)
                    || self
                        .family_tests
                        .get(family)
                        .is_some_and(|now| now >= tests)
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LedgerBinding {
    pub registry_id: String,
    pub seq: u64,
    pub chain_head: String,
    /// The evidence observed with this head (§23). `None` only for a pre-§23
    /// binding read back from a workspace: it still proves the event prefix,
    /// is replaced by the next adoption, and cannot be written.
    pub evidence: Option<LedgerEvidence>,
}

impl LedgerBinding {
    fn prefix(&self) -> EventPrefix<'_> {
        EventPrefix {
            registry_id: &self.registry_id,
            seq: self.seq,
            chain_head: &self.chain_head,
        }
    }

    fn validate(&self) -> Result<(), LedgerError> {
        self.prefix().validate()?;
        if let Some(evidence) = &self.evidence {
            evidence.validate()?;
        }
        Ok(())
    }
}

/// The pre-§23 stored shape, accepted exactly (no `version`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredBindingV1 {
    registry_id: String,
    seq: u64,
    chain_head: String,
}

/// Sorted, duplicate-free arrays: the canonical stored form of the sets.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredEvidence {
    quarantined_families: Vec<String>,
    conflicted_origins: Vec<String>,
    family_tests: BTreeMap<String, u64>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredBindingV2 {
    version: String,
    registry_id: String,
    seq: u64,
    chain_head: String,
    evidence: StoredEvidence,
}

fn strictly_ascending(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn parse_binding(text: &str) -> Result<LedgerBinding, LedgerError> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| invalid_binding(&error.to_string()))?;
    let binding = if value.get("version").is_some() {
        let stored: StoredBindingV2 =
            serde_json::from_value(value).map_err(|error| invalid_binding(&error.to_string()))?;
        if stored.version != LEDGER_BINDING_VERSION {
            return Err(invalid_binding(&format!(
                "unsupported version {}",
                stored.version
            )));
        }
        let evidence = stored.evidence;
        if !strictly_ascending(&evidence.quarantined_families)
            || !strictly_ascending(&evidence.conflicted_origins)
        {
            return Err(invalid_binding("evidence sets must be sorted and unique"));
        }
        LedgerBinding {
            registry_id: stored.registry_id,
            seq: stored.seq,
            chain_head: stored.chain_head,
            evidence: Some(LedgerEvidence {
                quarantined_families: evidence.quarantined_families.into_iter().collect(),
                conflicted_origins: evidence.conflicted_origins.into_iter().collect(),
                family_tests: evidence.family_tests,
            }),
        }
    } else {
        let stored: StoredBindingV1 =
            serde_json::from_value(value).map_err(|error| invalid_binding(&error.to_string()))?;
        LedgerBinding {
            registry_id: stored.registry_id,
            seq: stored.seq,
            chain_head: stored.chain_head,
            evidence: None,
        }
    };
    binding.validate()?;
    Ok(binding)
}

/// `Current` carries the registry head to persist after legacy backfill (or
/// with new attempts). All other states block qualification; they never erase
/// the workspace's last good binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BindingCheck {
    Current(LedgerBinding),
    RegistryRolledBack,
    RegistryDiverged,
    RegistryReplaced,
    RegistryChainBroken,
    /// The event prefix is still proven, but quarantine or conflict evidence
    /// the workspace saw is gone, or a family's test count fell below its
    /// saved high-water mark (§23).
    RegistryEvidenceRolledBack,
}

impl BindingCheck {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Current(_) => "current",
            Self::RegistryRolledBack => "registry_rolled_back",
            Self::RegistryDiverged => "registry_diverged",
            Self::RegistryReplaced => "registry_replaced",
            Self::RegistryChainBroken => "registry_chain_broken",
            Self::RegistryEvidenceRolledBack => "registry_evidence_rolled_back",
        }
    }
}

/// Read but never default a malformed binding. Existing `app_settings` has no
/// new schema requirement; the first verified binding is inserted later. A
/// pre-§23 binding (no `version`) reads with `evidence: None`; an unknown
/// version or a malformed v2 binding is an error, never an empty snapshot.
pub fn read_workspace_binding(conn: &Connection) -> Result<Option<LedgerBinding>, LedgerError> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [BINDING_KEY],
            |row| row.get(0),
        )
        .optional()?;
    raw.map(|text| parse_binding(&text)).transpose()
}

/// Use inside the same owner-checked workspace transaction that freezes new
/// attempts. It intentionally does not begin or commit a transaction itself.
/// Only a binding with evidence (one `check_binding` returned) can be written.
pub fn write_workspace_binding(
    conn: &Connection,
    binding: &LedgerBinding,
) -> Result<(), LedgerError> {
    binding.validate()?;
    let Some(evidence) = &binding.evidence else {
        return Err(LedgerError::StateInvalid(
            "a trial_ledger_binding without evidence cannot be written (§23)".into(),
        ));
    };
    let value = serde_json::to_string(&StoredBindingV2 {
        version: LEDGER_BINDING_VERSION.into(),
        registry_id: binding.registry_id.clone(),
        seq: binding.seq,
        chain_head: binding.chain_head.clone(),
        evidence: StoredEvidence {
            quarantined_families: evidence.quarantined_families.iter().cloned().collect(),
            conflicted_origins: evidence.conflicted_origins.iter().cloned().collect(),
            family_tests: evidence.family_tests.clone(),
        },
    })?;
    conn.execute(
        "INSERT INTO app_settings (key, value_json) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json,
             updated_at = datetime('now')",
        params![BINDING_KEY, value],
    )?;
    Ok(())
}

/// The registry's restrictive evidence, read inside the caller's transaction.
fn observed_evidence(tx: &Transaction<'_>) -> Result<LedgerEvidence, LedgerError> {
    let mut evidence = LedgerEvidence::default();
    {
        let mut stmt = tx.prepare("SELECT DISTINCT family_id FROM family_conflicts")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            evidence.quarantined_families.insert(row?);
        }
    }
    {
        let mut stmt = tx.prepare("SELECT DISTINCT origin_registry_id FROM registry_conflicts")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            evidence.conflicted_origins.insert(row?);
        }
    }
    {
        let mut stmt = tx.prepare(
            "SELECT f.family_id, f.protocol_json,
                    (SELECT MAX(u.tests_per_trial) FROM family_protocol_upgrades u
                      WHERE u.family_id = f.family_id)
               FROM trial_families f WHERE f.protocol_json IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })?;
        for row in rows {
            let (family, protocol, upgraded) = row?;
            let pinned = serde_json::from_str::<Value>(&protocol)?["testsPerTrial"]
                .as_u64()
                .ok_or_else(|| {
                    LedgerError::StateInvalid(format!("{family} protocol has no testsPerTrial"))
                })?;
            let tests = upgraded.map_or(pinned, |value| pinned.max(value as u64));
            evidence.family_tests.insert(family, tests);
        }
    }
    Ok(evidence)
}

/// Step 1 of a binding check or an admission fence.
enum PrefixOutcome {
    Proven { seq: u64, chain_head: String },
    Blocked(BindingCheck),
}

impl TrialLedger {
    /// Compare the workspace's saved binding against one consistent registry
    /// read. A changed registry ID is accepted only when import left the exact
    /// old prefix checkpoint and its event is present (§7.3) — or, for an
    /// empty prefix, verified genesis evidence of that origin (§20) — and the
    /// origin has no recorded registry conflict (§8.2). With the prefix
    /// proven, the registry must still cover the saved evidence (§23); the
    /// `Current` binding carries the evidence observed now, to be persisted.
    pub fn check_binding(
        &self,
        saved: Option<&LedgerBinding>,
    ) -> Result<BindingCheck, LedgerError> {
        if let Some(saved) = saved {
            saved.validate()?;
        }
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let check = match self.event_prefix(&tx, saved.map(LedgerBinding::prefix))? {
            PrefixOutcome::Blocked(check) => check,
            PrefixOutcome::Proven { seq, chain_head } => {
                let evidence = observed_evidence(&tx)?;
                let lost = saved
                    .and_then(|saved| saved.evidence.as_ref())
                    .is_some_and(|saved| !evidence.covers(saved));
                if lost {
                    BindingCheck::RegistryEvidenceRolledBack
                } else {
                    BindingCheck::Current(LedgerBinding {
                        registry_id: self.registry_id.clone(),
                        seq,
                        chain_head,
                        evidence: Some(evidence),
                    })
                }
            }
        };
        tx.commit()?;
        Ok(check)
    }

    /// The §7 event-prefix decision inside the caller's read transaction,
    /// shared by the binding check and the admission fence so each can read
    /// the prefix with its own state as one registry read.
    fn event_prefix(
        &self,
        tx: &Transaction<'_>,
        saved: Option<EventPrefix<'_>>,
    ) -> Result<PrefixOutcome, LedgerError> {
        if !matches!(
            verify_chain(tx, &self.registry_id)?,
            LedgerIntegrity::Intact { .. }
        ) {
            return Ok(PrefixOutcome::Blocked(BindingCheck::RegistryChainBroken));
        }
        let (seq, chain_head) = head(tx, &self.registry_id)?;
        if let Some(saved) = saved {
            if saved.registry_id == self.registry_id {
                if seq < saved.seq {
                    return Ok(PrefixOutcome::Blocked(BindingCheck::RegistryRolledBack));
                }
                let old_chain: Option<String> = if saved.seq == 0 {
                    Some(genesis(&self.registry_id))
                } else {
                    tx.query_row(
                        "SELECT chain FROM trial_events WHERE seq = ?1",
                        [saved.seq as i64],
                        |row| row.get(0),
                    )
                    .optional()?
                };
                if old_chain.as_deref() != Some(saved.chain_head) {
                    return Ok(PrefixOutcome::Blocked(BindingCheck::RegistryDiverged));
                }
            } else {
                // A recorded divergence is permanent: checkpoints that were
                // valid before it can no longer vouch for this origin.
                let conflicted = tx
                    .query_row(
                        "SELECT 1 FROM registry_conflicts WHERE origin_registry_id = ?1 LIMIT 1",
                        [saved.registry_id],
                        |_| Ok(()),
                    )
                    .optional()?
                    .is_some();
                let checkpoint: Option<(String, String)> = tx
                    .query_row(
                        "SELECT c.origin_chain, c.event_id FROM origin_checkpoints c
                      WHERE c.origin_registry_id = ?1 AND c.origin_seq = ?2",
                        params![saved.registry_id, saved.seq as i64],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;
                let accepted = if conflicted {
                    false
                } else if saved.seq == 0 {
                    // `validate` pinned the chain head to the origin genesis;
                    // there is no event to checkpoint, so require proof that
                    // a complete export of that origin was validated here.
                    tx.query_row(
                        "SELECT 1 FROM origin_genesis WHERE origin_registry_id = ?1",
                        [saved.registry_id],
                        |_| Ok(()),
                    )
                    .optional()?
                    .is_some()
                } else if let Some((chain, event_id)) = checkpoint {
                    chain == saved.chain_head
                        && tx
                            .query_row(
                                "SELECT 1 FROM trial_events WHERE event_id = ?1",
                                [&event_id],
                                |_| Ok(()),
                            )
                            .optional()?
                            .is_some()
                } else {
                    false
                };
                if !accepted {
                    return Ok(PrefixOutcome::Blocked(BindingCheck::RegistryReplaced));
                }
            }
        }
        Ok(PrefixOutcome::Proven { seq, chain_head })
    }

    /// The §6.4 freshness fence for a decision made from `earlier`, read as
    /// one registry state: first prove every event it saw is still present
    /// (same registry: the chain at its seq; replacement: §7.3 evidence), then
    /// compare the family's effective count. It checks the event prefix only:
    /// the snapshot carries no §23 evidence, and its own count comparison
    /// covers this batch's family (a quarantine blocks the admission, a fallen
    /// test count is `Inconsistent`). The read transaction ends before return:
    /// P13 must synchronize this final check with confirmation admission, so
    /// intervening registration/import cannot invalidate its observation.
    pub fn fence_admission(
        &self,
        batch_id: &str,
        earlier: &AdmissionSnapshot,
    ) -> Result<AdmissionFence, LedgerError> {
        let prefix = EventPrefix {
            registry_id: &earlier.registry_id,
            seq: earlier.seq,
            chain_head: &earlier.chain_head,
        };
        prefix.validate()?;
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        if let PrefixOutcome::Blocked(unproven) = self.event_prefix(&tx, Some(prefix))? {
            return Ok(AdmissionFence::Blocked(FenceBlocked::Prefix(unproven)));
        }
        let family: Option<Option<String>> = tx
            .query_row(
                "SELECT family_id FROM trial_batches WHERE batch_id = ?1",
                [batch_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(family) = family else {
            return Err(LedgerError::BatchNotFound(batch_id.to_string()));
        };
        let admission = admission_in(
            &tx,
            batch_id,
            family.as_deref(),
            &self.registry_id,
            &self.integrity,
        )?;
        tx.commit()?;
        Ok(match admission {
            Admission::Blocked(reason) => AdmissionFence::Blocked(FenceBlocked::Admission(reason)),
            Admission::Count(count) => {
                // Events are append-only, effectiveness is frozen at
                // registration and protocols only rise (§22), so with the
                // prefix proven none of these can move backwards; if one
                // did, the registry is inconsistent. A raised test count, like
                // a larger family, enlarges m: the decision must be redone.
                if count.batch_id != earlier.batch_id
                    || count.family_id != earlier.family_id
                    || count.tests_per_trial < earlier.tests_per_trial
                    || count.batch_effective_trials != earlier.batch_effective_trials
                    || count.family_effective_trials < earlier.family_effective_trials
                {
                    AdmissionFence::Blocked(FenceBlocked::Inconsistent)
                } else if count.family_effective_trials == earlier.family_effective_trials
                    && count.tests_per_trial == earlier.tests_per_trial
                {
                    AdmissionFence::Unchanged(count)
                } else {
                    AdmissionFence::Grew(count)
                }
            }
        })
    }
}

/// What a stored admission decision was based on (§6.4): the registry state
/// and counts of the `admissionCount` it used. The fence only compares it;
/// it never becomes a count or a precision-plan input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdmissionSnapshot {
    pub registry_id: String,
    pub batch_id: String,
    pub family_id: String,
    pub family_effective_trials: u64,
    pub batch_effective_trials: u64,
    pub tests_per_trial: u64,
    pub seq: u64,
    pub chain_head: String,
}

impl AdmissionCount {
    /// The part of this count a decision must store for its later fence.
    pub fn snapshot(&self) -> AdmissionSnapshot {
        AdmissionSnapshot {
            registry_id: self.registry_id.clone(),
            batch_id: self.batch_id.clone(),
            family_id: self.family_id.clone(),
            family_effective_trials: self.family_effective_trials,
            batch_effective_trials: self.batch_effective_trials,
            tests_per_trial: self.tests_per_trial,
            seq: self.seq,
            chain_head: self.chain_head.clone(),
        }
    }
}

/// §6.4 outcome. Only `Unchanged` lets an earlier decision stand as made.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdmissionFence {
    /// Prefix proven: no new effective trial and the same test count.
    Unchanged(AdmissionCount),
    /// Prefix proven, and the family grew or its test count rose (§22):
    /// re-evaluate P12a from this count.
    Grew(AdmissionCount),
    /// The earlier decision cannot be acted on.
    Blocked(FenceBlocked),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FenceBlocked {
    /// Step 1 failed: the earlier prefix is not provably present.
    Prefix(BindingCheck),
    /// The batch has no admission count any more (e.g. quarantined since).
    Admission(AdmissionBlocked),
    /// The family or its test count shrank, or its family, batch identity or
    /// batch size changed.
    Inconsistent,
}

impl FenceBlocked {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Prefix(check) => check.code(),
            Self::Admission(reason) => reason.code(),
            Self::Inconsistent => "admission_count_inconsistent",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::research::trial_ledger::{
        TrialBatchInput, TrialEventInput, TrialKind, TrialOrigin, REGISTRY_FILE_NAME,
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Dirs {
        root: PathBuf,
        registry: PathBuf,
        workspace: PathBuf,
    }

    impl Dirs {
        fn new() -> Self {
            let n = NEXT.fetch_add(1, Ordering::SeqCst);
            let root =
                std::env::temp_dir().join(format!("aff-ledger-binding-{}-{n}", std::process::id()));
            let workspace = root.join("workspace");
            std::fs::create_dir_all(&workspace).unwrap();
            Self {
                registry: root.join("registry"),
                root,
                workspace,
            }
        }

        fn open(&self) -> TrialLedger {
            TrialLedger::open(&self.registry, &self.workspace).unwrap()
        }
    }

    impl Drop for Dirs {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn batch(request: &str) -> TrialBatchInput {
        TrialBatchInput {
            workspace_id: "ws".into(),
            instrument_id: None,
            tests_per_trial: 1,
            events: vec![TrialEventInput {
                kind: TrialKind::Variant,
                origin: TrialOrigin::Request {
                    request_id: request.into(),
                    candidate_index: 0,
                },
                hypothesis_hash: None,
                strategy_hash: Some(format!("strategy-{request}")),
                dataset_hash: Some("dataset".into()),
                snapshot_id: None,
                split_hash: Some("split".into()),
                seeds_hash: Some("seeds".into()),
                engine_fingerprint_hash: Some("engine".into()),
                benchmark_id: None,
                benchmark_params_hash: None,
                reproduction_of: None,
                benchmark_evidence: None,
            }],
        }
    }

    fn current(ledger: &TrialLedger, saved: Option<&LedgerBinding>) -> LedgerBinding {
        match ledger.check_binding(saved).unwrap() {
            BindingCheck::Current(binding) => binding,
            other => panic!("expected a current binding, got {other:?}"),
        }
    }

    #[test]
    fn bound_open_never_creates_a_missing_or_empty_registry() {
        let dirs = Dirs::new();
        assert_eq!(
            TrialLedger::open_existing(&dirs.registry, &dirs.workspace)
                .err()
                .unwrap()
                .code(),
            "registry_missing"
        );
        assert!(!dirs.registry.exists());
        std::fs::create_dir_all(&dirs.registry).unwrap();
        let file = dirs.registry.join(REGISTRY_FILE_NAME);
        std::fs::write(&file, []).unwrap();
        assert_eq!(
            TrialLedger::open_existing(&dirs.registry, &dirs.workspace)
                .err()
                .unwrap()
                .code(),
            "registry_missing"
        );
        assert_eq!(std::fs::metadata(&file).unwrap().len(), 0);
    }

    #[test]
    fn bound_open_accepts_the_original_registry_without_changing_its_identity() {
        let dirs = Dirs::new();
        let original = dirs.open();
        let registry_id = original.registry_id().to_string();
        drop(original);
        let reopened = TrialLedger::open_existing(&dirs.registry, &dirs.workspace).unwrap();
        assert_eq!(reopened.registry_id(), registry_id);
    }

    #[test]
    fn same_registry_accepts_the_saved_prefix_but_detects_rollback_and_divergence() {
        let dirs = Dirs::new();
        let ledger = dirs.open();
        ledger.register_batch(&batch("one")).unwrap();
        let saved = current(&ledger, None);
        ledger.register_batch(&batch("two")).unwrap();
        assert_eq!(current(&ledger, Some(&saved)).seq, 2);

        let future = LedgerBinding {
            seq: 3,
            ..saved.clone()
        };
        assert_eq!(
            ledger.check_binding(Some(&future)).unwrap(),
            BindingCheck::RegistryRolledBack
        );
        let divergent = LedgerBinding {
            chain_head: "0".repeat(64),
            ..saved
        };
        assert_eq!(
            ledger.check_binding(Some(&divergent)).unwrap(),
            BindingCheck::RegistryDiverged
        );
    }

    #[test]
    fn replacement_needs_the_exact_imported_prefix_checkpoint() {
        let source_dirs = Dirs::new();
        let source = source_dirs.open();
        source.register_batch(&batch("one")).unwrap();
        let saved = current(&source, None);
        let export = source.export_json_lines().unwrap();

        let replacement_dirs = Dirs::new();
        let replacement = replacement_dirs.open();
        assert_eq!(
            replacement.check_binding(Some(&saved)).unwrap(),
            BindingCheck::RegistryReplaced
        );
        replacement.import_json_lines(&export).unwrap();
        let accepted = current(&replacement, Some(&saved));
        assert_eq!(accepted.registry_id, replacement.registry_id());
        assert_eq!(accepted.seq, 1);
    }

    #[test]
    fn binding_check_reverifies_a_chain_damaged_after_open() {
        let dirs = Dirs::new();
        let ledger = dirs.open();
        ledger.register_batch(&batch("one")).unwrap();
        let saved = current(&ledger, None);
        {
            let conn = ledger.lock().unwrap();
            conn.execute_batch("DROP TRIGGER trial_events_no_update;")
                .unwrap();
            conn.execute(
                "UPDATE trial_events SET chain = ?1 WHERE seq = 1",
                ["0".repeat(64)],
            )
            .unwrap();
        }
        assert_eq!(
            ledger.check_binding(Some(&saved)).unwrap(),
            BindingCheck::RegistryChainBroken
        );
    }

    #[test]
    fn workspace_binding_is_strict_and_can_be_written_in_the_callers_transaction() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE app_settings (key TEXT PRIMARY KEY, value_json TEXT NOT NULL, updated_at TEXT);").unwrap();
        assert_eq!(read_workspace_binding(&conn).unwrap(), None);
        let dirs = Dirs::new();
        let saved = current(&dirs.open(), None);
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        write_workspace_binding(&conn, &saved).unwrap();
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(read_workspace_binding(&conn).unwrap(), None);
        write_workspace_binding(&conn, &saved).unwrap();
        assert_eq!(read_workspace_binding(&conn).unwrap(), Some(saved));
        conn.execute(
            "UPDATE app_settings SET value_json = '{\"registryId\":\"bad\"}' WHERE key = ?1",
            [BINDING_KEY],
        )
        .unwrap();
        assert_eq!(
            read_workspace_binding(&conn).err().unwrap().code(),
            "registry_state_invalid"
        );
    }

    fn settings() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE app_settings (key TEXT PRIMARY KEY, value_json TEXT NOT NULL, updated_at TEXT);").unwrap();
        conn
    }

    fn store_raw(conn: &Connection, value: &str) {
        conn.execute(
            "INSERT INTO app_settings (key, value_json) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
            params![BINDING_KEY, value],
        )
        .unwrap();
    }

    #[test]
    fn r23_the_stored_binding_is_versioned_and_never_defaults_its_evidence() {
        let dirs = Dirs::new();
        let ledger = dirs.open();
        ledger.register_batch(&batch("one")).unwrap();
        let saved = current(&ledger, None);
        assert!(
            saved.evidence.is_some(),
            "a checked binding carries evidence"
        );
        let conn = settings();

        // v2 round trip, in its canonical form.
        write_workspace_binding(&conn, &saved).unwrap();
        let stored: String = conn
            .query_row("SELECT value_json FROM app_settings", [], |row| row.get(0))
            .unwrap();
        let stored: Value = serde_json::from_str(&stored).unwrap();
        assert_eq!(stored["version"], LEDGER_BINDING_VERSION);
        assert_eq!(
            stored["evidence"],
            serde_json::json!({"quarantinedFamilies": [], "conflictedOrigins": [], "familyTests": {}})
        );
        assert_eq!(read_workspace_binding(&conn).unwrap(), Some(saved.clone()));

        // A pre-§23 binding reads exactly, with no evidence, and still proves
        // its prefix; it cannot be written back without evidence.
        let legacy = serde_json::json!({
            "registryId": saved.registry_id, "seq": saved.seq, "chainHead": saved.chain_head
        });
        store_raw(&conn, &legacy.to_string());
        let read = read_workspace_binding(&conn).unwrap().unwrap();
        assert_eq!(read.evidence, None);
        assert!(matches!(
            ledger.check_binding(Some(&read)).unwrap(),
            BindingCheck::Current(_)
        ));
        assert_eq!(
            write_workspace_binding(&conn, &read).err().unwrap().code(),
            "registry_state_invalid"
        );

        // Unknown versions and malformed v2 bindings are errors, never an
        // empty snapshot.
        let family = format!("trial-family-v1:{}", "a".repeat(64));
        for (label, change) in [
            (
                "unknown version",
                serde_json::json!({"version": "trial-ledger-binding-v3"}),
            ),
            ("missing evidence", serde_json::json!({"evidence": null})),
            (
                "unsorted families",
                serde_json::json!({"evidence": {"quarantinedFamilies": [format!("trial-family-v1:{}", "b".repeat(64)), family], "conflictedOrigins": [], "familyTests": {}}}),
            ),
            (
                "duplicate origins",
                serde_json::json!({"evidence": {"quarantinedFamilies": [], "conflictedOrigins": ["c".repeat(32), "c".repeat(32)], "familyTests": {}}}),
            ),
            (
                "bad family id",
                serde_json::json!({"evidence": {"quarantinedFamilies": ["not-a-family"], "conflictedOrigins": [], "familyTests": {}}}),
            ),
            (
                "zero test count",
                serde_json::json!({"evidence": {"quarantinedFamilies": [], "conflictedOrigins": [], "familyTests": {(format!("trial-family-v1:{}", "d".repeat(64))): 0}}}),
            ),
            ("unknown field", serde_json::json!({"extra": true})),
        ] {
            let mut v2 = stored.clone();
            for (key, value) in change.as_object().unwrap() {
                if value.is_null() {
                    v2.as_object_mut().unwrap().remove(key);
                } else {
                    v2[key] = value.clone();
                }
            }
            store_raw(&conn, &v2.to_string());
            assert_eq!(
                read_workspace_binding(&conn)
                    .err()
                    .map(|error| error.code()),
                Some("registry_state_invalid"),
                "{label}"
            );
        }
    }
}
