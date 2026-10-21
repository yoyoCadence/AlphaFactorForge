use super::*;
use alpha_factor_forge::discovery_core::confirmation_history::ConfirmationInventory;

fn raw(conn: &Connection) -> (String, Option<String>) {
    conn.query_row(
        "SELECT value_json,updated_at FROM app_settings WHERE key='trial_ledger_binding'",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap()
}

#[test]
fn actual_adoption_upgrades_all_legacy_versions_without_clearing_permanent_stops() {
    let scratch = Scratch::new();
    let mut conn = crate::db::open_at(&scratch.root.join(crate::db::DB_FILE_NAME)).unwrap();
    let id = crate::db::runtime_ledger::workspace_id(&conn).unwrap();
    let bound = adopt(&mut conn, &scratch.root, &id, None).unwrap();
    bound
        .ledger
        .append_witnessed_confirmation_history(&ConfirmationInventory {
            unknown_origins: vec!["e".repeat(32)],
            ..Default::default()
        })
        .unwrap();
    drop(bound);
    drop(adopt(&mut conn, &scratch.root, &id, None).unwrap());
    let actual: Value = serde_json::from_str(&raw(&conn).0).unwrap();
    for version in [
        None,
        Some("trial-ledger-binding-v2"),
        Some("trial-ledger-binding-v3"),
        Some("trial-ledger-binding-v4"),
    ] {
        let mut legacy = actual.clone();
        let object = legacy.as_object_mut().unwrap();
        object.remove("confirmation");
        if let Some(version) = version {
            object.insert("version".into(), Value::String(version.into()));
        } else {
            object.remove("version");
            object.remove("evidence");
        }
        if version.is_none() || version == Some("trial-ledger-binding-v2") {
            object.remove("alpha");
        }
        if version != Some("trial-ledger-binding-v4") {
            object.remove("alphaUnknownOrigins");
        }
        conn.execute(
            "UPDATE app_settings SET value_json=?1 WHERE key='trial_ledger_binding'",
            [legacy.to_string()],
        )
        .unwrap();
        assert!(read_workspace_binding(&conn)
            .unwrap()
            .unwrap()
            .confirmation
            .is_none());
        let adopted = adopt(&mut conn, &scratch.root, &id, None).unwrap();
        assert!(adopted
            .ledger
            .read_witnessed_confirmation_history()
            .unwrap()
            .is_restricted());
        assert_eq!(
            serde_json::from_str::<Value>(&raw(&conn).0).unwrap(),
            actual
        );
    }
}

#[test]
fn owner_epoch_and_late_workspace_failure_preserve_exact_last_good_binding() {
    use crate::db::ownership::{acquire, HolderKind};
    let scratch = Scratch::new();
    let mut conn = crate::db::open_at(&scratch.root.join(crate::db::DB_FILE_NAME)).unwrap();
    let id = crate::db::runtime_ledger::workspace_id(&conn).unwrap();
    let owner = acquire(&mut conn, HolderKind::Service, std::process::id()).unwrap();
    let bound = adopt(&mut conn, &scratch.root, &id, Some(owner.epoch)).unwrap();
    let before = raw(&conn);
    bound
        .ledger
        .append_witnessed_confirmation_history(&ConfirmationInventory {
            unknown_origins: vec!["e".repeat(32)],
            ..Default::default()
        })
        .unwrap();
    drop(bound);
    assert!(adopt(&mut conn, &scratch.root, &id, Some(owner.epoch + 1)).is_err());
    assert_eq!(raw(&conn), before);
    conn.execute_batch("CREATE TRIGGER injected_binding_failure AFTER UPDATE ON app_settings WHEN NEW.key='trial_ledger_binding' BEGIN SELECT RAISE(ABORT,'injected binding failure'); END").unwrap();
    assert!(adopt(&mut conn, &scratch.root, &id, Some(owner.epoch)).is_err());
    assert_eq!(raw(&conn), before);
    conn.execute_batch("DROP TRIGGER injected_binding_failure")
        .unwrap();
    let adopted = adopt(&mut conn, &scratch.root, &id, Some(owner.epoch)).unwrap();
    assert!(adopted
        .ledger
        .read_witnessed_confirmation_history()
        .unwrap()
        .is_restricted());
    assert_ne!(raw(&conn).0, before.0);
}
