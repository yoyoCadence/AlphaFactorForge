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
