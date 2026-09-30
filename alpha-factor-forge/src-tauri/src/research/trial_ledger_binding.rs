//! P12b-2a: workspace evidence of the registry prefix it last observed.
//! Call `check_binding` before adopting a registry and write the returned
//! current head inside the workspace transaction that creates attempts.

use super::{
    admission_in, genesis, head, verify_chain, Admission, AdmissionBlocked, AdmissionCount,
    LedgerError, LedgerIntegrity, TrialLedger,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};

const BINDING_KEY: &str = "trial_ledger_binding";

fn valid_registry_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LedgerBinding {
    pub registry_id: String,
    pub seq: u64,
    pub chain_head: String,
}

impl LedgerBinding {
    fn validate(&self) -> Result<(), LedgerError> {
        if !valid_registry_id(&self.registry_id)
            || self.seq > i64::MAX as u64
            || self.chain_head.len() != 64
            || !self
                .chain_head
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || (self.seq == 0 && self.chain_head != genesis(&self.registry_id))
        {
            return Err(LedgerError::StateInvalid(
                "invalid trial_ledger_binding".into(),
            ));
        }
        Ok(())
    }
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
}

impl BindingCheck {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Current(_) => "current",
            Self::RegistryRolledBack => "registry_rolled_back",
            Self::RegistryDiverged => "registry_diverged",
            Self::RegistryReplaced => "registry_replaced",
            Self::RegistryChainBroken => "registry_chain_broken",
        }
    }
}

/// Read but never default a malformed binding. Existing `app_settings` has no
/// new schema requirement; the first verified binding is inserted later.
pub fn read_workspace_binding(conn: &Connection) -> Result<Option<LedgerBinding>, LedgerError> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [BINDING_KEY],
            |row| row.get(0),
        )
        .optional()?;
    raw.map(|text| {
        let value: LedgerBinding = serde_json::from_str(&text).map_err(|error| {
            LedgerError::StateInvalid(format!("invalid trial_ledger_binding: {error}"))
        })?;
        value.validate()?;
        Ok(value)
    })
    .transpose()
}

/// Use inside the same owner-checked workspace transaction that freezes new
/// attempts. It intentionally does not begin or commit a transaction itself.
pub fn write_workspace_binding(
    conn: &Connection,
    binding: &LedgerBinding,
) -> Result<(), LedgerError> {
    binding.validate()?;
    let value = serde_json::to_string(binding)?;
    conn.execute(
        "INSERT INTO app_settings (key, value_json) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json,
             updated_at = datetime('now')",
        params![BINDING_KEY, value],
    )?;
    Ok(())
}

impl TrialLedger {
    /// Compare the workspace's saved prefix against one consistent registry
    /// read. A changed registry ID is accepted only when import left the exact
    /// old prefix checkpoint and its event is present (§7.3) — or, for an
    /// empty prefix, verified genesis evidence of that origin (§20) — and the
    /// origin has no recorded registry conflict (§8.2).
    pub fn check_binding(
        &self,
        saved: Option<&LedgerBinding>,
    ) -> Result<BindingCheck, LedgerError> {
        if let Some(saved) = saved {
            saved.validate()?;
        }
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let check = self.prefix_check(&tx, saved)?;
        tx.commit()?;
        Ok(check)
    }

    /// The §7 prefix decision inside the caller's read transaction, so the
    /// admission fence can read the prefix and the count as one state.
    fn prefix_check(
        &self,
        tx: &Transaction<'_>,
        saved: Option<&LedgerBinding>,
    ) -> Result<BindingCheck, LedgerError> {
        if !matches!(
            verify_chain(tx, &self.registry_id)?,
            LedgerIntegrity::Intact { .. }
        ) {
            return Ok(BindingCheck::RegistryChainBroken);
        }
        let (seq, chain_head) = head(tx, &self.registry_id)?;
        let current = LedgerBinding {
            registry_id: self.registry_id.clone(),
            seq,
            chain_head,
        };
        if let Some(saved) = saved {
            if saved.registry_id == self.registry_id {
                if seq < saved.seq {
                    return Ok(BindingCheck::RegistryRolledBack);
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
                if old_chain.as_deref() != Some(saved.chain_head.as_str()) {
                    return Ok(BindingCheck::RegistryDiverged);
                }
            } else {
                // A recorded divergence is permanent: checkpoints that were
                // valid before it can no longer vouch for this origin.
                let conflicted = tx
                    .query_row(
                        "SELECT 1 FROM registry_conflicts WHERE origin_registry_id = ?1 LIMIT 1",
                        [&saved.registry_id],
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
                        [&saved.registry_id],
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
                    return Ok(BindingCheck::RegistryReplaced);
                }
            }
        }
        Ok(BindingCheck::Current(current))
    }

    /// The §6.4 freshness fence for a decision made from `earlier`, read as
    /// one registry state: first prove every event it saw is still present
    /// (same registry: the chain at its seq; replacement: §7.3 evidence), then
    /// compare the family's effective count. Callers must pass it before they
    /// act on an `ELIGIBLE` decision; a stale `NOT_ELIGIBLE` cannot improve.
    pub fn fence_admission(
        &self,
        batch_id: &str,
        earlier: &AdmissionSnapshot,
    ) -> Result<AdmissionFence, LedgerError> {
        let binding = LedgerBinding {
            registry_id: earlier.registry_id.clone(),
            seq: earlier.seq,
            chain_head: earlier.chain_head.clone(),
        };
        binding.validate()?;
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        match self.prefix_check(&tx, Some(&binding))? {
            BindingCheck::Current(_) => {}
            unproven => return Ok(AdmissionFence::Blocked(FenceBlocked::Prefix(unproven))),
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
                // Events are append-only and effectiveness is frozen at
                // registration, so with the prefix proven none of these can
                // change; if one did, the registry is inconsistent.
                if count.family_id != earlier.family_id
                    || count.tests_per_trial != earlier.tests_per_trial
                    || count.batch_effective_trials != earlier.batch_effective_trials
                    || count.family_effective_trials < earlier.family_effective_trials
                {
                    AdmissionFence::Blocked(FenceBlocked::Inconsistent)
                } else if count.family_effective_trials == earlier.family_effective_trials {
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
    /// Prefix proven, no new effective trial in the family.
    Unchanged(AdmissionCount),
    /// Prefix proven, the family grew: re-evaluate P12a from this count.
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
    /// The family shrank, or its family, protocol or batch size changed.
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
}
