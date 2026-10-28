//! Actual registry writers checked against independent original-format literals.
//! In-memory storage only; this grants no market/source admission authority.

use super::*;
use crate::db;
use rusqlite::types::ValueRef;
use std::collections::BTreeMap;

const FIXTURE: &str =
    include_str!("../../../fixtures/research/original-market-registry-projections-v1.json");
const DECLARATION: &str =
    "#[cfg(test)]\n#[path = \"registry_projection_controls.rs\"]\nmod projection_controls;\n\n";

fn fixture() -> Value {
    assert_eq!(
        sha256_hex(FIXTURE.replace("\r\n", "\n").as_bytes()),
        "ea6ce3df35814450641d1e41e1997f039322a6c86464660a525fbd2532b90c92"
    );
    let v: Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(v["version"], "original-market-registry-projections-v1");
    assert_eq!(v["calendars"].as_array().unwrap().len(), 4);
    assert_eq!(v["instruments"].as_array().unwrap().len(), 5);
    assert_eq!(v["calendarRefusals"].as_array().unwrap().len(), 14);
    assert_eq!(v["instrumentRefusals"].as_array().unwrap().len(), 19);
    v
}

fn memory_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    db::apply_migrations(&conn).unwrap();
    conn
}

#[derive(Debug, Eq, PartialEq)]
enum Cell {
    Null,
    Integer(i64),
    Real(u64),
    Text(Vec<u8>),
    Blob(Vec<u8>),
}

fn cell(value: ValueRef<'_>) -> Cell {
    match value {
        ValueRef::Null => Cell::Null,
        ValueRef::Integer(n) => Cell::Integer(n),
        ValueRef::Real(n) => Cell::Real(n.to_bits()),
        ValueRef::Text(bytes) => Cell::Text(bytes.to_vec()),
        ValueRef::Blob(bytes) => Cell::Blob(bytes.to_vec()),
    }
}

fn rows(conn: &Connection, table: &str) -> Vec<BTreeMap<String, Cell>> {
    // Fixed test-owned tables only, including all columns and SQLite storage types.
    assert!(matches!(
        table,
        "market_calendars" | "market_instruments" | "sqlite_sequence"
    ));
    let order = if table == "sqlite_sequence" {
        "name"
    } else {
        "id"
    };
    let mut stmt = conn
        .prepare(&format!("SELECT * FROM {table} ORDER BY {order}"))
        .unwrap();
    let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    stmt.query_map([], |row| {
        Ok(names
            .iter()
            .enumerate()
            .map(|(i, name)| (name.clone(), cell(row.get_ref(i).unwrap())))
            .collect())
    })
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
}

fn snapshot(conn: &Connection) -> Vec<Vec<BTreeMap<String, Cell>>> {
    ["market_calendars", "market_instruments", "sqlite_sequence"]
        .iter()
        .map(|t| rows(conn, t))
        .collect()
}

fn scalar(value: &Value) -> Cell {
    match value {
        Value::Null => Cell::Null,
        Value::String(s) => Cell::Text(s.as_bytes().to_vec()),
        Value::Number(n) => Cell::Integer(n.as_i64().unwrap()),
        _ => panic!("unexpected frozen SQL scalar"),
    }
}

fn projection(row: &BTreeMap<String, Cell>, expected: &Value, id: i64) {
    assert_eq!(row["id"], Cell::Integer(id));
    for (key, value) in expected["scalars"].as_object().unwrap() {
        assert_eq!(row[key], scalar(value), "raw SQL {key}");
    }
    assert_eq!(row["content_hash"], scalar(&expected["contentHash"]));
    let (column, field) = if expected.get("definitionJson").is_some() {
        ("definition_json", "definitionJson")
    } else {
        ("content_json", "contentJson")
    };
    assert_eq!(row[column], scalar(&expected[field]), "exact stored JSON");
}

fn named<'a>(v: &'a Value, group: &str, id: &str) -> &'a Value {
    v[group]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == id)
        .unwrap()
}

fn calendars(conn: &Connection, v: &Value) {
    assert!(rows(conn, "market_calendars").is_empty());
    assert_eq!(v["calendarSequence"].as_array().unwrap().len(), 6);
    for step in v["calendarSequence"].as_array().unwrap() {
        let c = named(v, "calendars", step["input"].as_str().unwrap());
        let text = c["inputJson"].as_str().unwrap();
        let bytes = text.as_bytes().to_vec();
        let input: SessionCalendar = serde_json::from_str(text).unwrap();
        let saved = input.clone();
        let expected = &c["expected"];
        for _ in 0..2 {
            let actual = canonical_json(&calendar_content(&input).unwrap()).unwrap();
            assert_eq!(
                actual,
                expected["definitionJson"].as_str().unwrap().as_bytes()
            );
            assert_eq!(sha256_hex(&actual), expected["contentHash"]);
        }
        let before = snapshot(conn);
        let id = step["expectedRow"].as_i64().unwrap();
        let created = step["created"].as_bool().unwrap();
        assert_eq!(register_calendar(conn, &input).unwrap(), (id, created));
        let after = rows(conn, "market_calendars");
        assert_eq!(after.len(), before[0].len() + usize::from(created));
        assert_eq!(&after[..before[0].len()], before[0]);
        projection(&after[(id - 1) as usize], expected, id);
        if !created {
            assert_eq!(snapshot(conn), before, "calendar retry changes no row");
        }
        assert_eq!(input, saved);
        assert_eq!(text.as_bytes(), bytes);
    }
    assert_eq!(rows(conn, "market_calendars").len(), 4);
}

fn instruments(conn: &Connection, v: &Value) {
    assert!(rows(conn, "market_instruments").is_empty());
    assert_eq!(v["instrumentSequence"].as_array().unwrap().len(), 8);
    for step in v["instrumentSequence"].as_array().unwrap() {
        let c = named(v, "instruments", step["input"].as_str().unwrap());
        let text = c["inputJson"].as_str().unwrap();
        let bytes = text.as_bytes().to_vec();
        let input: InstrumentDraft = serde_json::from_str(text).unwrap();
        let saved = input.clone();
        let expected = &c["expected"];
        for _ in 0..2 {
            let actual = canonical_json(&input.content().unwrap()).unwrap();
            assert_eq!(actual, expected["contentJson"].as_str().unwrap().as_bytes());
            assert_eq!(input.hash().unwrap(), expected["contentHash"]);
        }
        let before = snapshot(conn);
        let id = step["expectedRow"].as_i64().unwrap();
        let created = step["created"].as_bool().unwrap();
        assert_eq!(register_instrument(conn, &input).unwrap(), (id, created));
        let after = rows(conn, "market_instruments");
        assert_eq!(after.len(), before[1].len() + usize::from(created));
        assert_eq!(&after[..before[1].len()], before[1]);
        let row = &after[(id - 1) as usize];
        projection(row, expected, id);
        assert_eq!(row["revision"], scalar(&step["revision"]));
        for (column, field) in [
            ("lot_size", "lotSize"),
            ("price_step", "priceStep"),
            ("min_notional", "minNotional"),
        ] {
            let bits = &expected["realBits"][field];
            let expected = bits.as_str().map_or(Cell::Null, |hex| {
                Cell::Real(u64::from_str_radix(hex, 16).unwrap())
            });
            assert_eq!(row[column], expected, "raw SQL REAL {column}");
        }
        if !created {
            assert_eq!(snapshot(conn), before, "old-content retry changes no row");
        }
        assert_eq!(input, saved);
        assert_eq!(text.as_bytes(), bytes);
    }
    assert_eq!(rows(conn, "market_instruments").len(), 5);
    assert_eq!(v["finalInstrumentRows"], 5);
}

fn populated(v: &Value) -> Connection {
    let conn = memory_db();
    calendars(&conn, v);
    instruments(&conn, v);
    conn
}

#[test]
fn frozen_registry_source_pins_and_builtin_calendar() {
    let v = fixture();
    assert_eq!(v["baseline"], "841e5dde467dcb1eae7454dbdb3c8da43ee15630");
    let registry = include_str!("registry.rs").replace("\r\n", "\n");
    assert_eq!(registry.matches(DECLARATION).count(), 1);
    let sources = [
        (
            "src/market/registry.rs",
            registry.replacen(DECLARATION, "", 1),
        ),
        (
            "src/discovery_core/market_foundation.rs",
            include_str!("../discovery_core/market_foundation.rs").replace("\r\n", "\n"),
        ),
        (
            "src/research/mod.rs",
            include_str!("../research/mod.rs").replace("\r\n", "\n"),
        ),
        (
            "migrations/0008_market_foundation.sql",
            include_str!("../../migrations/0008_market_foundation.sql").replace("\r\n", "\n"),
        ),
    ];
    assert_eq!(v["originalSourcePins"].as_object().unwrap().len(), 4);
    for (path, source) in sources {
        let full = format!("alpha-factor-forge/src-tauri/{path}");
        assert_eq!(sha256_hex(source.as_bytes()), v["originalSourcePins"][full]);
    }
    let original: SessionCalendar =
        serde_json::from_str(v["calendars"][0]["inputJson"].as_str().unwrap()).unwrap();
    assert_eq!(crypto_24x7_calendar(), original);
}

#[test]
fn original_calendar_projections_and_retry_sequence() {
    let v = fixture();
    calendars(&memory_db(), &v);
}

#[test]
fn original_instrument_projections_and_revision_sequence() {
    let v = fixture();
    let conn = memory_db();
    calendars(&conn, &v);
    instruments(&conn, &v);
}

#[test]
fn original_calendar_staged_refusals_and_conflict_preserve_storage() {
    let v = fixture();
    let conn = populated(&v);
    for c in v["calendarRefusals"].as_array().unwrap() {
        let before = snapshot(&conn);
        let text = c["inputJson"].as_str().unwrap();
        let bytes = text.as_bytes().to_vec();
        let parsed = serde_json::from_str::<SessionCalendar>(text);
        let message = if c["phase"] == "decode" {
            parsed.unwrap_err().to_string()
        } else {
            assert_eq!(c["phase"], "validate");
            let input = parsed.expect("typed input before validation refusal");
            let saved = input.clone();
            assert_eq!(
                validate_calendar(&input).unwrap().as_str(),
                c["errorContains"]
            );
            let err = register_calendar(&conn, &input).unwrap_err().to_string();
            assert_eq!(input, saved);
            err
        };
        assert!(
            message.contains(c["errorContains"].as_str().unwrap()),
            "{}: {message}",
            c["id"]
        );
        assert_eq!(snapshot(&conn), before, "{}", c["id"]);
        assert_eq!(text.as_bytes(), bytes);
    }
    let c = &v["calendarConflict"];
    let input: SessionCalendar = serde_json::from_str(c["inputJson"].as_str().unwrap()).unwrap();
    let saved = input.clone();
    assert_eq!(validate_calendar(&input), None);
    let before = snapshot(&conn);
    for _ in 0..2 {
        assert!(register_calendar(&conn, &input)
            .unwrap_err()
            .to_string()
            .contains(c["errorContains"].as_str().unwrap()));
        assert_eq!(snapshot(&conn), before);
        assert_eq!(input, saved);
    }
    assert_eq!(
        rows(&conn, "market_calendars").len() as i64,
        c["expectedRows"].as_i64().unwrap()
    );
}

#[test]
fn original_instrument_staged_refusals_preserve_storage() {
    let v = fixture();
    let conn = populated(&v);
    for c in v["instrumentRefusals"].as_array().unwrap() {
        let before = snapshot(&conn);
        let text = c["inputJson"].as_str().unwrap();
        let bytes = text.as_bytes().to_vec();
        let parsed = serde_json::from_str::<InstrumentDraft>(text);
        let message = if c["phase"] == "decode" {
            parsed.unwrap_err().to_string()
        } else {
            assert_eq!(c["phase"], "register");
            let input = parsed.expect("typed input before registration refusal");
            let saved = input.clone();
            let err = register_instrument(&conn, &input).unwrap_err().to_string();
            assert_eq!(input, saved);
            err
        };
        assert!(
            message.contains(c["errorContains"].as_str().unwrap()),
            "{}: {message}",
            c["id"]
        );
        assert_eq!(snapshot(&conn), before, "{}", c["id"]);
        assert_eq!(text.as_bytes(), bytes);
    }
}

#[test]
fn original_nonfinite_optional_specs_refuse_before_any_write() {
    let v = fixture();
    let conn = populated(&v);
    let original: InstrumentDraft =
        serde_json::from_str(v["instruments"][0]["inputJson"].as_str().unwrap()).unwrap();
    for field in ["lotSize", "priceStep", "minNotional"] {
        for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut input = original.clone();
            match field {
                "lotSize" => input.lot_size = Some(n),
                "priceStep" => input.price_step = Some(n),
                "minNotional" => input.min_notional = Some(n),
                _ => unreachable!(),
            }
            let before = snapshot(&conn);
            let bits = || {
                [input.lot_size, input.price_step, input.min_notional].map(|x| x.map(f64::to_bits))
            };
            let saved_bits = bits();
            let saved_value = serde_json::to_value(&input).unwrap();
            for _ in 0..2 {
                let error = register_instrument(&conn, &input).unwrap_err().to_string();
                assert!(error.contains(&format!("instrument {field} must be a finite positive")));
                assert_eq!(snapshot(&conn), before);
                assert_eq!(bits(), saved_bits);
                assert_eq!(serde_json::to_value(&input).unwrap(), saved_value);
            }
        }
    }
}

#[test]
fn original_registry_append_only_triggers_preserve_complete_rows() {
    let conn = populated(&fixture());
    let before = snapshot(&conn);
    for (sql, message) in [
        (
            "UPDATE market_calendars SET timezone='Etc/UTC' WHERE id=1",
            "immutable",
        ),
        ("DELETE FROM market_calendars WHERE id=1", "never deleted"),
        (
            "UPDATE market_instruments SET quote='USD' WHERE id=1",
            "immutable",
        ),
        ("DELETE FROM market_instruments WHERE id=1", "never deleted"),
    ] {
        assert!(conn
            .execute(sql, [])
            .unwrap_err()
            .to_string()
            .contains(message));
        assert_eq!(snapshot(&conn), before);
    }
}
