use super::*;
use std::{
    future::Future,
    sync::{mpsc, Arc, Mutex},
    task::{Context, Poll, Waker},
    time::Duration,
};

fn database() -> crate::runtime::SharedDb {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::apply_migrations(&conn).unwrap();
    Arc::new(Mutex::new(conn))
}

#[test]
fn a_busy_database_does_not_block_the_command_polling_thread() {
    let db = database();
    // Deliberately keep SQLite unavailable until after the first command poll.
    let held = db.lock().unwrap();
    let (sent, received) = mpsc::channel();
    let worker_db = db.clone();
    let polling_thread = std::thread::spawn(move || {
        let mut command = Box::pin(database_task(worker_db, |_| Ok(7)));
        let mut context = Context::from_waker(Waker::noop());
        match command.as_mut().poll(&mut context) {
            Poll::Pending => {
                sent.send(true).unwrap();
                tauri::async_runtime::block_on(command)
            }
            Poll::Ready(result) => {
                sent.send(false).unwrap();
                result
            }
        }
    });
    // A finite test-only deadline ensures an accidental inline lock fails
    // without leaving the test deadlocked. Release the lock before asserting.
    let yielded = received.recv_timeout(Duration::from_secs(5));
    drop(held);
    assert_eq!(polling_thread.join().unwrap().unwrap(), 7);
    assert!(
        yielded.unwrap(),
        "the first poll must yield while DB is busy"
    );
}

#[test]
fn repository_results_and_errors_cross_the_worker_unchanged() {
    let db = database();
    let rows = tauri::async_runtime::block_on(database_task(db.clone(), |conn| {
        repositories::list_datasets(conn)
    }))
    .unwrap();
    assert!(rows.is_empty());
    let error = tauri::async_runtime::block_on(database_task(db, |conn| {
        conn.execute("INSERT INTO missing_table VALUES (1)", [])?;
        Ok(())
    }))
    .unwrap_err();
    assert!(
        matches!(error, AppError::Db(_)),
        "repository errors must not become worker-join errors: {error}"
    );
}

#[test]
fn a_panicking_worker_returns_an_explicit_command_error() {
    let error = tauri::async_runtime::block_on(database_task::<()>(database(), |_| {
        panic!("controlled test-only worker failure")
    }))
    .unwrap_err();
    assert!(error.to_string().contains("database command task failed"));
}

#[test]
fn prepared_saved_strategy_read_returns_a_verified_string_without_changing_the_row() {
    let db = database();
    let definition = r#"{"definitionVersion":"manual-strategy-definition-v1","numericPolicy":"json-f64-roundtrip-v1","mode":"params","feePct":0.0036944444444444438,"slipPct":0}"#;
    let hash = crate::identity::strategy_hash_from_definition_json(definition).unwrap();
    let id = {
        let conn = db.lock().unwrap();
        repositories::insert_verified_strategy(
            &conn,
            &StrategyDef {
                id: None,
                name: "prepared fixture".into(),
                kind: "params".into(),
                dsl_json: None,
                original_definition_json: definition.into(),
                param_schema_json: None,
                source: "manual".into(),
                ai_prompt_hash: None,
                strategy_hash: hash.clone(),
                lifecycle: "candidate".into(),
                parent_strategy_id: None,
            },
        )
        .unwrap()
    };
    let prepared = tauri::async_runtime::block_on(database_task(db.clone(), move |conn| {
        repositories::prepare_saved_strategy(conn, id)
    }))
    .unwrap();
    let value = serde_json::to_value(prepared).unwrap();
    assert_eq!(value["sourceStrategyId"], id);
    assert_eq!(value["sourceStrategyHash"], hash);
    assert_eq!(value["numericPolicy"], "json-f64-roundtrip-v1");
    assert!(value["interpretedDefinitionJson"].is_string());
    let conn = db.lock().unwrap();
    let stored = repositories::get_strategy_by_id(&conn, id).unwrap();
    assert_eq!(stored.original_definition_json, definition);
    assert_eq!(stored.strategy_hash, hash);
}
