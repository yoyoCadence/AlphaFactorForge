//! P03b: the versioned command envelope driving a REAL discovery run — one
//! `discovery.start` per request id however often it is sent, and a ledger a
//! reconnecting reader can page from a cursor after taking a snapshot.
use super::*;
use crate::db::ownership::{acquire, HolderKind};
use crate::db::runtime_ledger;
use crate::runtime::commands::{Dispatcher, ErrorCode, COMMAND_PROTOCOL_VERSION, EVENT_PROTOCOL_VERSION};

fn owned_dispatcher(db: &SharedDb) -> (Dispatcher, String, Arc<RecordingSink>) {
    let (epoch, workspace) = {
        let mut conn = db.lock().unwrap();
        let epoch = acquire(&mut conn, HolderKind::DesktopEmbedded, 1).unwrap().epoch;
        (epoch, runtime_ledger::workspace_id(&conn).unwrap())
    };
    let sink = Arc::new(RecordingSink::new(db.clone()));
    let dispatcher = Dispatcher::new(
        db.clone(),
        DiscoveryRunner::with_epoch(epoch),
        epoch,
        workspace.clone(),
        sink.clone(),
    );
    (dispatcher, workspace, sink)
}

fn envelope(workspace: &str, request: &str, command: &str, payload: Value) -> Value {
    json!({
        "protocolVersion": COMMAND_PROTOCOL_VERSION,
        "workspaceId": workspace,
        "requestId": request,
        "command": command,
        "payload": payload,
    })
}

#[test]
fn one_request_id_starts_exactly_one_run_and_the_ledger_replays_it_from_a_cursor() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let config = runner_config(dataset_id, &dataset_hash, 1);
    let (dispatcher, workspace, sink) = owned_dispatcher(&db);

    // 重複命令: the same envelope three times starts ONE run and answers the
    // same run id each time; a reused id with another payload is refused.
    let first = dispatcher
        .dispatch(envelope(&workspace, "start-1", "discovery.start", config.clone()))
        .expect("first start");
    let run_id = first["runId"].as_i64().expect("run id");
    let second = dispatcher
        .dispatch(envelope(&workspace, "start-1", "discovery.start", config.clone()))
        .expect("replayed start");
    assert_eq!(second, first, "the replay is the stored first outcome");
    let conflict = dispatcher.dispatch(envelope(
        &workspace,
        "start-1",
        "discovery.start",
        runner_config(dataset_id, &dataset_hash, 2),
    ));
    assert_eq!(conflict.unwrap_err().code, ErrorCode::DuplicateRequest);
    {
        let conn = db.lock().unwrap();
        let runs: i64 = conn.query_row("SELECT COUNT(*) FROM discovery_runs", [], |r| r.get(0)).unwrap();
        assert_eq!(runs, 1, "one run for one request id");
    }

    // Let the run finish (through the envelope's own progress read), then
    // check the ledger holds exactly what the host saw.
    let deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let snapshot = dispatcher
            .dispatch(envelope(&workspace, "p", "discovery.progress", json!({ "runId": run_id })))
            .expect("progress read");
        if snapshot["status"] == "completed" {
            break;
        }
        assert!(Instant::now() < deadline, "run did not complete: {snapshot}");
        thread::sleep(Duration::from_millis(5));
    }
    sink.wait_for(|event| matches!(event, DiscoveryEvent::Done(_)));
    sink.assert_all_observed_after_commit();

    // 重連: snapshot first, then page the ledger from a cursor. Every event the
    // host received is in the ledger, in order, under this epoch, and the
    // per-run sequence inside the payload is strictly increasing.
    let active = dispatcher
        .dispatch(envelope(&workspace, "a", "discovery.active", json!({})))
        .expect("active read");
    assert_eq!(active, Value::Null, "the run is terminal, so no active run");

    let mut cursor = 0;
    let mut collected = Vec::new();
    loop {
        let page = dispatcher
            .dispatch(envelope(&workspace, "e", "events.read", json!({ "afterEventId": cursor, "limit": 2 })))
            .expect("events page");
        let events = page["events"].as_array().unwrap().clone();
        if events.is_empty() {
            assert_eq!(page["lastEventId"].as_i64().unwrap(), cursor, "cursor reached the end");
            break;
        }
        cursor = events.last().unwrap()["eventId"].as_i64().unwrap();
        collected.extend(events);
    }
    let host_seen = sink.snapshot().len();
    assert_eq!(collected.len(), host_seen, "every forwarded event was ledgered");
    assert!(collected.windows(2).all(|pair| pair[0]["eventId"].as_i64() < pair[1]["eventId"].as_i64()));
    assert!(collected
        .windows(2)
        .all(|pair| pair[0]["payload"]["sequence"].as_u64() < pair[1]["payload"]["sequence"].as_u64()));
    for event in &collected {
        assert_eq!(event["protocolVersion"], EVENT_PROTOCOL_VERSION);
        assert_eq!(event["eventVersion"], DISCOVERY_EVENT_VERSION);
        assert_eq!(event["entity"]["kind"], "discovery_run");
        assert_eq!(event["entity"]["id"], run_id.to_string());
        assert_eq!(event["epoch"], 1);
    }
    let last = collected.last().unwrap();
    assert_eq!(last["channel"], DISCOVERY_DONE_EVENT);
    assert_eq!(last["payload"]["status"], "completed");
    assert!(collected.iter().any(|event| event["channel"] == DISCOVERY_RESULT_EVENT));
}
