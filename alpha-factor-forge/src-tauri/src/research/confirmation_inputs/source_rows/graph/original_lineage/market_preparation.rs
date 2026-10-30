//! P/P+E limits are tested independently of later domain/resource refusals.
use super::*;

fn shape_from(rows: &mut SourceRowsObservation, target: usize, nodes: bool, original: usize) {
    let mut remaining = target - original;
    let mut document = |doc: &mut OriginalJson| {
        if remaining == 0 {
            return;
        }
        if nodes {
            let old = doc.node_count();
            let extra = remaining.min(65536 - old);
            *doc = OriginalJson::parse(
                &serde_json::to_vec(&vec![Value::Null; old + extra - 1]).unwrap(),
            )
            .unwrap();
            remaining -= extra;
        } else {
            let mut raw = doc.as_bytes().to_vec();
            let extra = remaining.min(4 * 1024 * 1024 - raw.len());
            raw.extend(std::iter::repeat_n(b' ', extra));
            *doc = OriginalJson::parse(&raw).unwrap();
            remaining -= extra;
        }
    };
    // Test-owned syntax/resource shaping after actual capture. Market originals,
    // config and its E stay fixed; no semantic success follows from node poison.
    for row in &mut rows.strategies {
        document(&mut row.original_definition_json);
    }
    for row in &mut rows.attempts {
        document(&mut row.input_fingerprint_json);
        document(&mut row.engine_fingerprint_json);
    }
    for row in &mut rows.hypotheses {
        document(&mut row.content_json);
        document(&mut row.applicability_json);
    }
    assert_eq!(remaining, 0);
    let old = parse_work(rows).unwrap();
    let c = case("captured-old-revision");
    let market = c["marketWork"][if nodes {
        "originalNodes"
    } else {
        "originalBytes"
    }]
    .as_u64()
    .unwrap() as usize;
    assert_eq!(if nodes { old.nodes } else { old.bytes } + market, target);
}
fn captured_shape(target: usize, nodes: bool) -> (Actual, SourceRowsObservation) {
    let c = case("captured-old-revision");
    let mut f = if nodes {
        let mut f = Actual::market();
        let mut raw: Value =
            serde_json::from_str(c["family"]["configJson"].as_str().unwrap()).unwrap();
        raw["bases"][0]["strategy"]["entryRules"] = json!(vec![Value::Null; 4096]);
        f.install(
            &serde_json::to_string(&raw).unwrap(),
            c["family"]["campaignDocumentJson"].as_str().unwrap(),
            c["family"]["campaignId"].as_str().unwrap(),
            "crypto:binance:BTCUSDT",
            "1h",
        );
        f
    } else {
        Actual::from_case(&c)
    };
    let mut rows = f.capture();
    let mut config = String::from_utf8(rows.roots.run.config_json.as_bytes().to_vec()).unwrap();
    let initial = if nodes {
        20961
    } else {
        assert_eq!(config.len(), 1887);
        let extra = 1024 * 1024 - config.len();
        config.extend(std::iter::repeat_n(' ', extra));
        10835 + 2 * extra
    };
    rows.roots.run.config_json = OriginalJson::parse(config.as_bytes()).unwrap();
    let old_hypotheses = rows.hypotheses.clone();
    shape_from(&mut rows, target, nodes, initial);
    assert!(
        rows.hypotheses == old_hypotheses,
        "fixed hypotheses are never rewritten"
    );
    // Keep the original immutable attempt rows. Append a test-owned raw resource
    // claim for run2, using real schema constraints and unchanged market evidence.
    // No trigger/schema is removed and this is never a successful domain claim.
    f.conn().execute("INSERT INTO discovery_runs(id,name,status,config_json) VALUES(2,'raw resource claim','completed',?)",[config]).unwrap();
    f.conn().execute("INSERT INTO campaign_run_admissions(id,discovery_run_id,campaign_id,instrument_id,batch_id,status,report_version,report_json,fee_pct,slip_pct) SELECT 2,2,campaign_id,instrument_id,batch_id,status,report_version,report_json,fee_pct,slip_pct FROM campaign_run_admissions WHERE id=1",[]).unwrap();
    for row in &rows.strategies {
        f.conn()
            .execute(
                "UPDATE strategy_def SET original_definition_json=? WHERE id=?",
                rusqlite::params![
                    std::str::from_utf8(row.original_definition_json.as_bytes()).unwrap(),
                    row.id
                ],
            )
            .unwrap();
    }
    for row in &rows.attempts {
        f.conn().execute("INSERT INTO research_attempts(attempt_key,hypothesis_id,strategy_id,dataset_id,discovery_run_id,candidate_index,status,input_fingerprint_json,engine_fingerprint_json,epoch,trial_event_id) SELECT ?,hypothesis_id,strategy_id,dataset_id,2,candidate_index,status,?,?,epoch,? FROM research_attempts WHERE id=?",
            rusqlite::params![format!("run:2:candidate:{}",row.candidate_index.unwrap()),std::str::from_utf8(row.input_fingerprint_json.as_bytes()).unwrap(),std::str::from_utf8(row.engine_fingerprint_json.as_bytes()).unwrap(),format!("trial-event-v1:{:064x}",4096+row.id),row.id]).unwrap();
    }
    f.conn().execute("INSERT INTO discovery_jobs(discovery_run_id,candidate_index,strategy_id,dataset_id,segment,status) SELECT 2,candidate_index,strategy_id,dataset_id,segment,status FROM discovery_jobs WHERE discovery_run_id=1",[]).unwrap();
    let locator = SourceLocator::new(&f.campaign_id, 2).unwrap();
    let expected = f.expected.clone();
    let tx = f.conn_mut().transaction().unwrap();
    let captured = unchanged(&tx, || capture_source_rows_in(&tx, &expected, &locator)).unwrap();
    tx.rollback().unwrap();
    assert_eq!(captured.roots.run.id, 2);
    assert!(captured.usage.text_bytes <= 33554432);
    assert!(captured.usage.json_nodes <= 262144);
    assert!(
        captured.snapshot == rows.snapshot
            && captured.instrument == rows.instrument
            && captured.calendar == rows.calendar
            && captured.provenance == rows.provenance
    );
    (f, captured)
}
fn boundary(nodes: bool, combined: bool) {
    let limit = if nodes { 262144 } else { 33554432 };
    // Node recipe replaces a 5-node/44-byte rule list with4097 nodes/20481
    // bytes in config and3 originals: E=(1662+3*20437,114+3*4092).
    let expansion = if nodes { 12390 } else { 1662 };
    for extra in [0, 1] {
        let target = limit - if combined { expansion } else { 0 } + extra;
        let (mut f, rows) = captured_shape(target, nodes);
        let saved = rows.clone();
        let before = entries();
        let warm = warmup_controls::warm_entries();
        let _old = warmup_controls::DenyWork::new(true, true);
        let _market = boundaries::DenyMarket::new(true, true);
        if combined && extra == 0 {
            let work = market::preparation_work(&rows).unwrap();
            assert_eq!(if nodes { work.0 .1 } else { work.0 .0 } + expansion, limit);
            assert_eq!(work.1, if nodes { (62973, 12390) } else { (1662, 114) });
            assert_eq!(entries(), (before.0 + 20, before.1 + 1));
        } else {
            let _deny = Deny::new(!combined && extra == 1, true);
            preserves(&mut f, &rows, false);
            assert_eq!(
                entries(),
                (before.0 + usize::from(combined || extra == 0), before.1)
            );
        }
        assert_eq!(warmup_controls::warm_entries(), warm);
        assert!(rows == saved);
    }
}
#[test]
fn market_joint_parse_bytes_exact_and_plus_one_gate_before_materialization() {
    boundary(false, false);
}
#[test]
fn market_joint_parse_nodes_exact_and_plus_one_gate_before_materialization() {
    boundary(true, false);
}
#[test]
fn market_joint_expansion_bytes_exact_and_plus_one_gate_before_enumeration() {
    boundary(false, true);
}
#[test]
fn market_joint_expansion_nodes_exact_and_plus_one_gate_before_enumeration() {
    boundary(true, true);
}
