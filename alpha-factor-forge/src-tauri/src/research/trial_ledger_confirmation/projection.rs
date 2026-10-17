//! Exact non-audit scalar/interval projections of already checked documents.
//! Value routing here is never an authorization or duplicate-key parser.
use super::*;
#[derive(Clone, Copy)]
pub(super) struct DocSpec {
    pub table: &'static str,
    pub columns: &'static [(&'static str, usize)],
}
pub(super) const DOCS: [DocSpec; 7] = [
    DocSpec {
        table: "confirmation_batches",
        columns: &[
            ("batch_id", 64),
            ("family_id", 80),
            ("stage", 16),
            ("parent_batch_id", 64),
            ("selection_id", 64),
        ],
    },
    DocSpec {
        table: "confirmation_selections",
        columns: &[
            ("selection_id", 64),
            ("validation_batch_id", 64),
            ("sealed_receipt_id", 64),
            ("alpha_reservation_id", 64),
            ("report_sha256", 64),
        ],
    },
    DocSpec {
        table: "confirmation_operations",
        columns: &[
            ("operation_id", 64),
            ("origin_registry_id", 32),
            ("reservation_key", 32),
            ("workspace_id", 32),
            ("batch_id", 64),
            ("family_id", 80),
        ],
    },
    DocSpec {
        table: "confirmation_exposures",
        columns: &[
            ("exposure_id", 64),
            ("instrument_id", 256),
            ("kind", 32),
            ("operation_id", 64),
            ("alpha_reservation_id", 64),
        ],
    },
    DocSpec {
        table: "confirmation_admissions",
        columns: &[
            ("admission_id", 64),
            ("operation_id", 64),
            ("alpha_reservation_id", 64),
            ("exposure_id", 64),
            ("binding_sha256", 64),
        ],
    },
    DocSpec {
        table: "confirmation_events",
        columns: &[
            ("event_id", 64),
            ("operation_id", 64),
            ("admission_id", 64),
            ("kind", 32),
        ],
    },
    DocSpec {
        table: "confirmation_conflicts",
        columns: &[("conflict_id", 64)],
    },
];
fn text(v: &Value, name: &str) -> Result<SqlValue, LedgerError> {
    Ok(SqlValue::Text(string(v, name)?.into()))
}
fn number(v: &Value, name: &str) -> Result<SqlValue, LedgerError> {
    let n = v
        .get(name)
        .and_then(Value::as_u64)
        .and_then(|n| i64::try_from(n).ok())
        .ok_or_else(|| bad("invalid interval projection"))?;
    Ok(SqlValue::Integer(n))
}
pub(super) fn values(raw: &str, spec: &DocSpec) -> Result<Vec<SqlValue>, LedgerError> {
    let v = json(raw)?;
    let b = v.get("body").ok_or_else(|| bad("invalid projection"))?;
    let mut values = vec![text(&v, "id")?];
    match spec.table {
        "confirmation_batches" => {
            let stage = &b["stage"];
            values.push(text(&b["source"], "familyId")?);
            values.push(text(stage, "kind")?);
            if string(stage, "kind")? == "validation" {
                values.extend([SqlValue::Null, SqlValue::Null]);
            } else {
                values.push(text(stage, "validationBatchId")?);
                values.push(text(stage, "selectionId")?);
            }
        }
        "confirmation_selections" => {
            for name in [
                "validationBatchId",
                "sealedReceiptId",
                "alphaReservationId",
                "reportSha256",
            ] {
                values.push(text(b, name)?);
            }
        }
        "confirmation_operations" => {
            for name in [
                "originRegistryId",
                "reservationKey",
                "workspaceId",
                "confirmationBatchId",
                "familyId",
            ] {
                values.push(text(b, name)?);
            }
        }
        "confirmation_exposures" => {
            let claim = &b["claim"];
            values.push(text(b, "instrumentId")?);
            values.push(text(claim, "kind")?);
            if string(claim, "kind")? == "confirmation" {
                values.push(text(claim, "operationId")?);
                values.push(text(claim, "alphaReservationId")?);
            } else {
                values.extend([SqlValue::Null, SqlValue::Null]);
            }
        }
        "confirmation_admissions" => {
            for name in ["operationId", "alphaReservationId", "exposureId"] {
                values.push(text(b, name)?);
            }
            values.push(text(&b["registryObservation"], "bindingSha256")?);
        }
        "confirmation_events" => {
            values.push(text(b, "operationId")?);
            values.push(text(b, "admissionId")?);
            values.push(text(&b["event"], "kind")?);
        }
        _ => {}
    }
    Ok(values)
}
type IntervalKey = (String, i64);
pub(super) fn intervals(
    i: &ConfirmationInventory,
) -> Result<BTreeMap<IntervalKey, Vec<SqlValue>>, LedgerError> {
    let mut values = BTreeMap::new();
    for raw in &i.exposures {
        let v = json(raw)?;
        let id = document_id(&v)?;
        let body = &v["body"];
        for (index, interval) in body["intervals"]
            .as_array()
            .ok_or_else(|| bad("invalid intervals"))?
            .iter()
            .enumerate()
        {
            values.insert(
                (id.into(), index as i64),
                vec![
                    text(body, "instrumentId")?,
                    number(interval, "fromMs")?,
                    number(interval, "toExclusiveMs")?,
                ],
            );
        }
    }
    Ok(values)
}
pub(super) fn verify(conn: &Connection, i: &ConfirmationInventory) -> Result<(), LedgerError> {
    for spec in DOCS {
        let mut expected = BTreeMap::new();
        for raw in section(i, &spec) {
            let v = json(raw)?;
            expected.insert(document_id(&v)?.to_string(), values(raw, &spec)?);
        }
        let columns = spec
            .columns
            .iter()
            .map(|c| c.0)
            .collect::<Vec<_>>()
            .join(",");
        let mut stmt = conn.prepare(&format!("SELECT {columns} FROM {}", spec.table))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let actual = (0..spec.columns.len())
                .map(|n| row.get::<_, SqlValue>(n))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| bad("invalid stored projection"))?;
            let Some(SqlValue::Text(id)) = actual.first() else {
                return Err(bad("invalid stored projection"));
            };
            if expected.remove(id).as_ref() != Some(&actual) {
                return Err(bad("document column projection differs"));
            }
        }
        if !expected.is_empty() {
            return Err(bad("missing document projection"));
        }
    }
    let mut expected = intervals(i)?;
    let mut stmt=conn.prepare("SELECT exposure_id,interval_index,instrument_id,from_ms,to_exclusive_ms FROM confirmation_intervals")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0).map_err(|_| bad("invalid interval key"))?;
        let index: i64 = row.get(1).map_err(|_| bad("invalid interval key"))?;
        let actual = (2..5)
            .map(|n| row.get::<_, SqlValue>(n))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| bad("invalid interval projection"))?;
        if expected.remove(&(id, index)).as_ref() != Some(&actual) {
            return Err(bad("interval projection differs"));
        }
    }
    if !expected.is_empty() {
        return Err(bad("missing interval projection"));
    }
    Ok(())
}
