//! Monotonic archival union. Every incoming representation is validated before
//! duplicate replay can retain an earlier byte/audit representation.
use super::*;

pub(super) fn capacity(i: &ConfirmationInventory) -> Result<(), LedgerError> {
    let mut count = i.blobs.len();
    let mut bytes = 0usize;
    for spec in DOCS {
        let values = section(i, &spec);
        count = count
            .checked_add(values.len())
            .ok_or_else(|| bad("input capacity exceeded"))?;
        for raw in values {
            if raw.len() > MAX_DOCUMENT_BYTES {
                return Err(bad("input document too large"));
            }
            bytes = bytes
                .checked_add(raw.len())
                .ok_or_else(|| bad("input capacity exceeded"))?;
        }
    }
    for blob in &i.blobs {
        bytes = bytes
            .checked_add(blob.text().len())
            .ok_or_else(|| bad("input capacity exceeded"))?;
    }
    if count > MAX_HISTORY_ROWS
        || bytes > MAX_HISTORY_BYTES
        || i.unknown_origins.len() > MAX_UNKNOWN_ORIGINS
        || !i.unknown_origins.windows(2).all(|p| p[0] < p[1])
        || i.unknown_origins.iter().any(|v| !origin(v))
    {
        return Err(bad("input capacity or provenance invalid"));
    }
    Ok(())
}
/// Overlay includes incoming duplicate texts so strict codecs see their exact
/// original JSON. Retained union keeps first-observed text for semantic replay.
pub(super) fn union(
    old: &ConfirmationInventory,
    input: &ConfirmationInventory,
) -> Result<(ConfirmationInventory, ConfirmationInventory), LedgerError> {
    capacity(input)?;
    let mut retained = old.clone();
    let mut overlay = old.clone();
    for spec in DOCS {
        let mut originals = BTreeMap::new();
        for raw in section(old, &spec) {
            let value = json(raw)?;
            originals.insert(document_id(&value)?.to_string(), (raw.clone(), value));
        }
        let mut seen = BTreeSet::new();
        let mut verification = originals
            .iter()
            .map(|(id, (raw, _))| (id.clone(), raw.clone()))
            .collect::<BTreeMap<_, _>>();
        for raw in section(input, &spec) {
            let value = json(raw)?;
            let id = document_id(&value)?.to_string();
            if !seen.insert(id.clone()) {
                return Err(bad("duplicate incoming record"));
            }
            if let Some((_, prior)) = originals.get(&id) {
                if prior != &value {
                    return Err(bad("existing record body differs"));
                }
            } else {
                section_mut(&mut retained, &spec).push(raw.clone());
            }
            verification.insert(id, raw.clone());
        }
        *section_mut(&mut overlay, &spec) = verification.into_values().collect();
    }
    let mut blobs = old
        .blobs
        .iter()
        .map(|b| (b.sha256().to_string(), b))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for blob in &input.blobs {
        if !seen.insert(blob.sha256()) {
            return Err(bad("duplicate incoming blob"));
        }
        if let Some(prior) = blobs.get(blob.sha256()) {
            if *prior != blob {
                return Err(bad("existing blob bytes differ"));
            }
        } else {
            retained.blobs.push(blob.clone());
            blobs.insert(blob.sha256().into(), blob);
        }
    }
    overlay.blobs = retained.blobs.clone();
    let unknown: BTreeSet<_> = old
        .unknown_origins
        .iter()
        .chain(&input.unknown_origins)
        .cloned()
        .collect();
    retained.unknown_origins = unknown.into_iter().collect();
    overlay.unknown_origins = retained.unknown_origins.clone();
    Ok((retained, overlay))
}
fn record_ids(i: &ConfirmationInventory, spec: &DocSpec) -> Result<BTreeSet<String>, LedgerError> {
    section(i, spec)
        .iter()
        .map(|r| Ok(document_id(&json(r)?)?.into()))
        .collect()
}

/// The caller owns the surrounding transaction. Propagate failures and roll
/// back that transaction; this helper never commits it or grants execution.
pub(in crate::research::trial_ledger) fn append_in(
    tx: &Transaction<'_>,
    registry: &str,
    input: &ConfirmationInventory,
    ctx: &GraphContext,
) -> Result<ConfirmationWriteSummary, LedgerError> {
    let old = load_in(tx, registry, ctx)?;
    let (retained, overlay) = union(old.inventory(), input)?;
    // Verification must precede deduplication of raw duplicate representations.
    complete_confirmation_graph(&overlay, ctx).map_err(|_| bad("invalid incoming graph"))?;
    let proposed =
        complete_confirmation_graph(&retained, ctx).map_err(|_| bad("invalid proposed graph"))?;
    let proposed = proposed.inventory();
    let summary = persist_in(tx, old.inventory(), proposed)?;
    let written = load_in(tx, registry, ctx)?;
    if written.inventory() != proposed {
        return Err(bad("written graph differs from preflight"));
    }
    Ok(summary)
}

/// Insert only the difference of a fully checked proposed inventory. Both
/// callers validate/finally reload in the same transaction; no commit here.
pub(super) fn persist_in(
    tx: &Transaction<'_>,
    old: &ConfirmationInventory,
    proposed: &ConfirmationInventory,
) -> Result<ConfirmationWriteSummary, LedgerError> {
    let stamp = chrono::Utc::now().to_rfc3339();
    let mut summary = ConfirmationWriteSummary {
        added_records: 0,
        added_blobs: 0,
        added_intervals: 0,
        added_unknown_origins: 0,
    };
    let mut new_exposures = ConfirmationInventory::default();
    for spec in DOCS {
        let ids = record_ids(old, &spec)?;
        for raw in section(proposed, &spec) {
            let value = json(raw)?;
            let id = document_id(&value)?;
            if ids.contains(id) {
                continue;
            }
            let mut values = projection::values(raw, &spec)?;
            values.push(SqlValue::Text(raw.clone()));
            values.push(SqlValue::Text(stamp.clone()));
            let columns = spec
                .columns
                .iter()
                .map(|c| c.0)
                .chain(["document_json", "recorded_at"])
                .collect::<Vec<_>>()
                .join(",");
            let marks = vec!["?"; values.len()].join(",");
            tx.execute(
                &format!("INSERT INTO {} ({columns}) VALUES ({marks})", spec.table),
                rusqlite::params_from_iter(values.iter()),
            )?;
            summary.added_records += 1;
            if spec.table == "confirmation_exposures" {
                new_exposures.exposures.push(raw.clone());
            }
        }
    }
    let old_blobs: BTreeSet<_> = old.blobs.iter().map(|b| b.sha256()).collect();
    for blob in &proposed.blobs {
        if old_blobs.contains(blob.sha256()) {
            continue;
        }
        let kind = match blob.kind() {
            BlobKind::Statistics => "statistics",
            BlobKind::Binding => "binding",
            BlobKind::SourceWitness => {
                tx.execute("INSERT INTO confirmation_source_witnesses (witness_sha256,byte_length,content_bytes,recorded_at) VALUES (?1,?2,?3,?4)",params![blob.sha256(),blob.text().len() as i64,blob.text().as_bytes(),stamp])?;
                summary.added_blobs += 1;
                continue;
            }
        };
        tx.execute("INSERT INTO confirmation_blobs (blob_sha256,kind,byte_length,content_bytes,recorded_at) VALUES (?1,?2,?3,?4,?5)",params![blob.sha256(),kind,blob.text().len() as i64,blob.text().as_bytes(),stamp])?;
        summary.added_blobs += 1;
    }
    for ((id, index), values) in projection::intervals(&new_exposures)? {
        let mut row = vec![SqlValue::Text(id), SqlValue::Integer(index)];
        row.extend(values);
        tx.execute("INSERT INTO confirmation_intervals (exposure_id,interval_index,instrument_id,from_ms,to_exclusive_ms) VALUES (?1,?2,?3,?4,?5)",rusqlite::params_from_iter(row.iter()))?;
        summary.added_intervals += 1;
    }
    let old_unknown: BTreeSet<_> = old.unknown_origins.iter().collect();
    summary.added_unknown_origins = proposed
        .unknown_origins
        .iter()
        .filter(|v| !old_unknown.contains(v))
        .count();
    record_unknown_origins(tx, &proposed.unknown_origins.iter().cloned().collect())?;
    Ok(summary)
}
impl TrialLedger {
    /// Monotonic archival union in one immediate transaction, including derived
    /// conflict witnesses and final materialized projection verification.
    pub fn append_confirmation_history(
        &self,
        input: &ConfirmationInventory,
        ctx: &GraphContext,
    ) -> Result<ConfirmationWriteSummary, LedgerError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let summary = append_in(&tx, self.registry_id(), input, ctx)?;
        tx.commit()?;
        Ok(summary)
    }
}
