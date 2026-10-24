//! Bounded original-byte observations of a sealed historical SQL graph.
//! These digests do not authenticate semantics, files, ownership or registry state.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use super::{OriginalJson, SourceRowsObservation};
use crate::error::{AppError, AppResult};

const MAX_WORK_BYTES: usize = 33_554_432;
const MAX_OUTPUT_TEXT_BYTES: usize = 786_560;

#[cfg(test)]
thread_local! {
    static SHA_UPDATES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static DENY_SHA: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn invalid() -> AppError {
    AppError::Other("original_digests_invalid".into())
}

#[derive(Clone, Eq, PartialEq)]
pub(in crate::research::confirmation_inputs) struct RawDigestObservation {
    campaign_document_hash: String,
    engine_manifest_hash: String,
    candidates: Vec<CandidateRawDigests>,
}

#[derive(Clone, Eq, PartialEq)]
pub(in crate::research::confirmation_inputs) struct CandidateRawDigests {
    index: i64,
    definition_hash: String,
    execution_hash: String,
    lineage_hash: String,
}

impl RawDigestObservation {
    pub(in crate::research::confirmation_inputs) fn campaign_document_hash(&self) -> &str {
        &self.campaign_document_hash
    }

    pub(in crate::research::confirmation_inputs) fn engine_manifest_hash(&self) -> &str {
        &self.engine_manifest_hash
    }

    pub(in crate::research::confirmation_inputs) fn candidates(&self) -> &[CandidateRawDigests] {
        &self.candidates
    }
}

impl CandidateRawDigests {
    pub(in crate::research::confirmation_inputs) fn index(&self) -> i64 {
        self.index
    }

    pub(in crate::research::confirmation_inputs) fn definition_hash(&self) -> &str {
        &self.definition_hash
    }

    pub(in crate::research::confirmation_inputs) fn execution_hash(&self) -> &str {
        &self.execution_hash
    }

    pub(in crate::research::confirmation_inputs) fn lineage_hash(&self) -> &str {
        &self.lineage_hash
    }
}

#[derive(Clone, Copy)]
enum Domain {
    Definition,
    Execution,
    Lineage,
    Engines,
}

impl Domain {
    fn bytes(self) -> &'static [u8] {
        match self {
            Self::Definition => b"p13-definition-original-v1",
            Self::Execution => b"p13-execution-original-v1",
            Self::Lineage => b"p13-lineage-original-v1",
            Self::Engines => b"p13-engine-manifest-original-v1",
        }
    }
}

struct HashWork {
    bytes: usize,
}

impl HashWork {
    // Calculate locally first: failure never changes prior usage.
    fn add(&mut self, bytes: usize) -> AppResult<()> {
        let next = self.bytes.checked_add(bytes).ok_or_else(invalid)?;
        if next > MAX_WORK_BYTES {
            return Err(invalid());
        }
        self.bytes = next;
        Ok(())
    }

    fn frame(&mut self, domain: Domain, lengths: &[usize]) -> AppResult<()> {
        u64::try_from(lengths.len()).map_err(|_| invalid())?;
        let mut bytes = domain.bytes().len().checked_add(9).ok_or_else(invalid)?;
        for length in lengths {
            u64::try_from(*length).map_err(|_| invalid())?;
            bytes = bytes
                .checked_add(8)
                .and_then(|n| n.checked_add(*length))
                .ok_or_else(invalid)?;
        }
        self.add(bytes)
    }
}

struct CandidateParts<'a> {
    index: i64,
    index_bytes: [u8; 8],
    definition: &'a OriginalJson,
    hypothesis: &'a OriginalJson,
    input: &'a OriginalJson,
    engine: &'a OriginalJson,
    event: &'a str,
}

struct Prepared<'a> {
    campaign: &'a OriginalJson,
    config: &'a OriginalJson,
    candidates: Vec<CandidateParts<'a>>,
    work_bytes: usize,
    output_text_bytes: usize,
}

// Resolve only sealed graph references. No source data or parsed trees are copied.
fn preflight(rows: &SourceRowsObservation) -> AppResult<Prepared<'_>> {
    let count = rows.attempts.len();
    if !(1..=4096).contains(&count) {
        return Err(invalid());
    }
    let output_text_bytes = count
        .checked_mul(3)
        .and_then(|n| n.checked_add(2))
        .and_then(|n| n.checked_mul(64))
        .ok_or_else(invalid)?;
    if output_text_bytes > MAX_OUTPUT_TEXT_BYTES {
        return Err(invalid());
    }
    let strategies: BTreeMap<_, _> = rows.strategies.iter().map(|r| (r.id, r)).collect();
    let hypotheses: BTreeMap<_, _> = rows.hypotheses.iter().map(|r| (r.id, r)).collect();
    let campaign = &rows.roots.campaign.document_json;
    let config = &rows.roots.run.config_json;
    let mut work = HashWork { bytes: 0 };
    work.add(campaign.as_bytes().len())?;
    let mut candidates = Vec::with_capacity(count);
    for (offset, attempt) in rows.attempts.iter().enumerate() {
        let index = i64::try_from(offset).map_err(|_| invalid())?;
        if attempt.candidate_index != Some(index) {
            return Err(invalid());
        }
        let definition = &strategies
            .get(&attempt.strategy_id)
            .ok_or_else(invalid)?
            .original_definition_json;
        let hypothesis = &hypotheses
            .get(&attempt.hypothesis_id)
            .ok_or_else(invalid)?
            .content_json;
        let input = &attempt.input_fingerprint_json;
        let engine = &attempt.engine_fingerprint_json;
        let event = attempt.trial_event_id.as_deref().ok_or_else(invalid)?;
        work.frame(Domain::Definition, &[definition.as_bytes().len()])?;
        work.frame(
            Domain::Execution,
            &[config.as_bytes().len(), engine.as_bytes().len()],
        )?;
        work.frame(
            Domain::Lineage,
            &[
                hypothesis.as_bytes().len(),
                input.as_bytes().len(),
                engine.as_bytes().len(),
                event.len(),
            ],
        )?;
        candidates.push(CandidateParts {
            index,
            index_bytes: u64::try_from(index).map_err(|_| invalid())?.to_be_bytes(),
            definition,
            hypothesis,
            input,
            engine,
            event,
        });
    }
    let engine_lengths: Vec<_> = candidates
        .iter()
        .flat_map(|c| [c.index_bytes.len(), c.engine.as_bytes().len()])
        .collect();
    work.frame(Domain::Engines, &engine_lengths)?;
    Ok(Prepared {
        campaign,
        config,
        candidates,
        work_bytes: work.bytes,
        output_text_bytes,
    })
}

fn update(hasher: &mut Sha256, bytes: &[u8]) {
    #[cfg(test)]
    {
        DENY_SHA.with(|deny| assert!(!deny.get(), "SHA update before successful whole preflight"));
        SHA_UPDATES.with(|count| count.set(count.get() + 1));
    }
    hasher.update(bytes);
}

fn frame(domain: Domain, parts: &[&[u8]]) -> AppResult<String> {
    let count = u64::try_from(parts.len()).map_err(|_| invalid())?;
    let mut hasher = Sha256::new();
    update(&mut hasher, domain.bytes());
    update(&mut hasher, &[0]);
    update(&mut hasher, &count.to_be_bytes());
    for part in parts {
        let length = u64::try_from(part.len()).map_err(|_| invalid())?;
        update(&mut hasher, &length.to_be_bytes());
        update(&mut hasher, part);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub(in crate::research::confirmation_inputs) fn observe_raw_digests(
    rows: &SourceRowsObservation,
) -> AppResult<RawDigestObservation> {
    let prepared = preflight(rows)?;
    let mut campaign = Sha256::new();
    update(&mut campaign, prepared.campaign.as_bytes());
    let campaign_document_hash = hex::encode(campaign.finalize());
    let mut candidates = Vec::with_capacity(prepared.candidates.len());
    for c in &prepared.candidates {
        candidates.push(CandidateRawDigests {
            index: c.index,
            definition_hash: frame(Domain::Definition, &[c.definition.as_bytes()])?,
            execution_hash: frame(
                Domain::Execution,
                &[prepared.config.as_bytes(), c.engine.as_bytes()],
            )?,
            lineage_hash: frame(
                Domain::Lineage,
                &[
                    c.hypothesis.as_bytes(),
                    c.input.as_bytes(),
                    c.engine.as_bytes(),
                    c.event.as_bytes(),
                ],
            )?,
        });
    }
    let engine_parts: Vec<&[u8]> = prepared
        .candidates
        .iter()
        .flat_map(|c| [c.index_bytes.as_slice(), c.engine.as_bytes()])
        .collect();
    let engine_manifest_hash = frame(Domain::Engines, &engine_parts)?;
    Ok(RawDigestObservation {
        campaign_document_hash,
        engine_manifest_hash,
        candidates,
    })
}

#[cfg(test)]
#[path = "original_digests/tests.rs"]
mod tests;
