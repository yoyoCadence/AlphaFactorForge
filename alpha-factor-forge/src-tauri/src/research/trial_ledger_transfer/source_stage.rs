//! Exact source reconstruction in a fresh disposable database. Ordinary import
//! rebases chains and can skip quarantined rows, so it cannot validate a source.
use super::*;
use rusqlite::Transaction;

impl ValidatedExport {
    /// Caller owns a fresh private schema and transaction, never a destination.
    /// No provenance markers, conflict completion or import receipts are made.
    pub(in crate::research::trial_ledger) fn stage_source_in(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<(), LedgerError> {
        let occupied: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM registry_meta) OR EXISTS(SELECT 1 FROM trial_families) OR EXISTS(SELECT 1 FROM trial_batches) OR EXISTS(SELECT 1 FROM trial_events) OR EXISTS(SELECT 1 FROM registry_imports)",
            [], |r| r.get(0),
        )?;
        if occupied {
            return Err(integrity("source staging requires a fresh private schema"));
        }
        self.validate_kinds_resolving(|_| Ok(None))?;
        tx.execute(
            "INSERT INTO registry_meta (key,value) VALUES ('registry_id',?1)",
            [self.registry_id()],
        )?;
        for (id, (key, protocol)) in &self.families {
            tx.execute("INSERT INTO trial_families (family_id,family_key,protocol_json,created_at) VALUES (?1,?2,?3,'source-validation')",params![id,key,protocol])?;
        }
        for (family, tests) in &self.upgrades {
            tx.execute("INSERT INTO family_protocol_upgrades (family_id,tests_per_trial,protocol_json,recorded_at) VALUES (?1,?2,?3,'source-validation')",params![family,tests,protocol_json(*tests)?])?;
        }
        for (id, (family, origin, members)) in &self.batches {
            tx.execute("INSERT INTO trial_batches (batch_id,family_id,members_json,origin_registry_id,created_at) VALUES (?1,?2,?3,?4,'source-validation')",params![id,family,canonical_text(&serde_json::to_value(members)?)?,origin])?;
        }
        for (id, payload, origin, batch, seq, chain) in &self.events {
            tx.execute("INSERT INTO trial_events (event_id,seq,chain,family_id,idempotency_key,kind,effective,batch_id,payload_json,origin_registry_id,registered_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'source-validation')",params![id,seq,chain,payload["familyId"].as_str(),payload["idempotencyKey"].as_str(),payload["kind"].as_str(),payload["effective"].as_bool(),batch,canonical_text(payload)?,origin])?;
        }
        for ((batch, origin), (before, effective)) in &self.receipts {
            tx.execute("INSERT INTO batch_receipts (batch_id,receipt_registry_id,family_effective_before,batch_effective_trials) VALUES (?1,?2,?3,?4)",params![batch,origin,before,effective])?;
        }
        for ((origin, seq), (event, chain)) in &self.checkpoints {
            tx.execute("INSERT INTO origin_checkpoints (origin_registry_id,origin_seq,event_id,origin_chain) VALUES (?1,?2,?3,?4)",params![origin,seq,event,chain])?;
        }
        for origin in &self.genesis {
            tx.execute("INSERT INTO origin_genesis (origin_registry_id,recorded_at) VALUES (?1,'source-validation')",[origin])?;
        }
        for (family, kind, detail) in &self.family_conflicts {
            tx.execute("INSERT INTO family_conflicts (family_id,kind,detail_json,recorded_at) VALUES (?1,?2,?3,'source-validation')",params![family,kind,detail])?;
        }
        for (origin, detail) in &self.registry_conflicts {
            tx.execute("INSERT INTO registry_conflicts (origin_registry_id,detail_json,recorded_at) VALUES (?1,?2,'source-validation')",params![origin,detail])?;
        }
        if !matches!(
            verify_chain(tx, self.registry_id())?,
            LedgerIntegrity::Intact { .. }
        ) || head(tx, self.registry_id())?
            != (self.header.head_seq, self.header.head_chain.clone())
        {
            return Err(integrity("staged source chain differs"));
        }
        Ok(())
    }
}
