//! Test-owned damage/negative claims follow actual original producers.
use super::*;

fn edit(document: &mut OriginalJson, change: impl FnOnce(&mut Value)) {
    let mut value: Value = serde_json::from_slice(document.as_bytes()).unwrap();
    change(&mut value);
    *document = OriginalJson::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
}
fn rehash(document: &OriginalJson) -> String {
    let value: Value = serde_json::from_slice(document.as_bytes()).unwrap();
    sha256_hex(&crate::research::canonical_json(&value).unwrap())
}
struct NoOutput {
    prior: bool,
    before: usize,
}
impl NoOutput {
    fn new() -> Self {
        Self {
            prior: market::DENY_OUTPUT.with(|v| v.replace(true)),
            before: market::OUTPUT_ENTRIES.with(std::cell::Cell::get),
        }
    }
}
impl Drop for NoOutput {
    fn drop(&mut self) {
        market::DENY_OUTPUT.with(|v| v.set(self.prior));
        if !std::thread::panicking() {
            assert_eq!(
                market::OUTPUT_ENTRIES.with(std::cell::Cell::get),
                self.before
            );
        }
    }
}
fn semantic_refusal(f: &mut Actual, rows: &SourceRowsObservation) {
    let before = market::PAYLOAD_ENTRIES.with(std::cell::Cell::get);
    let _output = NoOutput::new();
    let _warmup = warmup_controls::DenyWork::new(false, true);
    preserves(f, rows, false);
    assert_eq!(
        market::PAYLOAD_ENTRIES.with(std::cell::Cell::get),
        before + 1
    );
}

#[test]
fn market_five_coherent_negative_claims_reach_market_then_refuse_all_output() {
    let c = case("captured-old-revision");
    for r in oracle()["coherentRefusals"].as_array().unwrap() {
        let mut f = Actual::market();
        // Explicit controlled negative claim, not a successful snapshot writer.
        // All underlying rows/files/lineage still come from original producers.
        f.conn().execute(
            "INSERT INTO market_snapshots(snapshot_id,version,instrument_row_id,instrument_id,interval,dataset_id,dataset_hash,price_basis,calendar_id,corporate_action_version,cost_profile_version,kind,status,as_of,coverage_json,content_json) SELECT ?,version,instrument_row_id,instrument_id,interval,dataset_id,dataset_hash,price_basis,calendar_id,corporate_action_version,cost_profile_version,?,'ok',?,coverage_json,? FROM market_snapshots WHERE id=1",
            rusqlite::params![r["snapshotId"].as_str().unwrap(),r["kind"].as_str().unwrap(),r["asOfMs"].as_i64().unwrap(),r["contentJson"].as_str().unwrap()],
        ).unwrap();
        let id = f.conn().last_insert_rowid();
        for source in r["sourceRows"].as_array().unwrap() {
            f.conn().execute("INSERT INTO market_snapshot_sources(snapshot_row_id,provenance_id) VALUES(?,?)",rusqlite::params![id,source.as_i64().unwrap()]).unwrap();
        }
        f.install(
            c["family"]["configJson"].as_str().unwrap(),
            r["campaignDocumentJson"].as_str().unwrap(),
            r["campaignId"].as_str().unwrap(),
            "crypto:binance:BTCUSDT",
            "1h",
        );
        let rows = f.capture();
        assert_eq!(rows.snapshot.id, id);
        assert_eq!(rows.snapshot.snapshot_id, r["snapshotId"]);
        assert_eq!(
            rows.provenance.iter().map(|p| p.id).collect::<Vec<_>>(),
            r["sourceRows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect::<Vec<_>>()
        );
        semantic_refusal(&mut f, &rows);
    }
}

#[test]
fn market_actual_snapshot_writer_accepts_single_and_multiple_identical_unprefixed_sources() {
    for count in [1, 2] {
        let mut f = Actual::market();
        let source = data(SOURCE);
        let template = source["provenance"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["expectedRow"] == 7)
            .unwrap();
        let mut input: provenance::RawObservation =
            serde_json::from_str(template["inputJson"].as_str().unwrap()).unwrap();
        input.source = "-custom".into();
        input.revision_of = None;
        let raw = hex::decode(template["rawHex"].as_str().unwrap()).unwrap();
        let mut ids = Vec::new();
        for part in 0..count {
            input.request_scope = json!({"controlledPart":part});
            let (id, created) = provenance::record_raw(f.conn(), &f.store, &input, &raw).unwrap();
            assert!(created);
            ids.push(id);
        }
        let original = named(&source, "snapshots", "historical-created");
        let mut request: SnapshotRequest =
            serde_json::from_str(original["requestJson"].as_str().unwrap()).unwrap();
        request.provenance_ids = ids.clone();
        let created = match snapshots::build_snapshot(f.conn_mut(), &request).unwrap() {
            SnapshotOutcome::Created(s) => s,
            other => panic!("original writer: {other:?}"),
        };
        assert!(created.qualification_eligible());
        let c = case("captured-old-revision");
        let mut campaign: Value =
            serde_json::from_str(c["family"]["campaignDocumentJson"].as_str().unwrap()).unwrap();
        campaign["instruments"][0]["snapshotId"] = json!(created.snapshot_id);
        let frozen = freeze_campaign(&campaign).unwrap();
        f.install(
            c["family"]["configJson"].as_str().unwrap(),
            &serde_json::to_string(&campaign).unwrap(),
            frozen.campaign_id(),
            &request.instrument_id,
            &request.interval,
        );
        let rows = f.capture();
        assert_eq!(
            rows.provenance.iter().map(|p| p.id).collect::<Vec<_>>(),
            ids
        );
        let before = market::OUTPUT_ENTRIES.with(std::cell::Cell::get);
        let result = preserves(&mut f, &rows, true).unwrap();
        assert_eq!(result.snapshot_id(), created.snapshot_id);
        let original_instrument = registry::get_instrument(f.conn(), created.instrument_row_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            result.instrument_revision_hash(),
            original_instrument.content_hash
        );
        assert_eq!(result.sources().len(), count);
        for (binding, id) in result.sources().iter().zip(ids) {
            let original = provenance::get_provenance(f.conn(), id).unwrap().unwrap();
            assert_eq!(binding.provenance_id(), id);
            assert_eq!(binding.record_hash(), original.record_hash);
        }
        assert_eq!(
            market::OUTPUT_ENTRIES.with(std::cell::Cell::get),
            before + 1
        );
    }
}

type Damage = (&'static str, fn(&mut SourceRowsObservation));
fn damaged_controls(id: &str, controls: &[Damage]) {
    let mut f = Actual::from_case(&case(id));
    let original = f.capture();
    for (name, damage) in controls {
        eprintln!("controlled damage: {name}");
        let mut rows = original.clone();
        damage(&mut rows);
        assert!(rows != original, "control must change {name}");
        semantic_refusal(&mut f, &rows);
    }
}

#[test]
fn market_calendar_and_captured_instrument_scalar_schema_hash_and_real_bits_are_verified() {
    damaged_controls(
        "captured-old-revision",
        &[
            ("calendar version", |r| {
                r.calendar.version = "unknown".into()
            }),
            ("calendar scalar", |r| {
                r.calendar.timezone = "Elsewhere".into()
            }),
            ("calendar hash", |r| {
                r.calendar.content_hash = "0".repeat(64)
            }),
            ("calendar unknown field", |r| {
                edit(&mut r.calendar.definition_json, |v| v["extra"] = json!(0))
            }),
            ("coherent invalid calendar", |r| {
                edit(&mut r.calendar.definition_json, |v| {
                    v["holidays"] = json!(["not-a-date"])
                });
                r.calendar.content_hash = rehash(&r.calendar.definition_json);
            }),
            ("instrument version", |r| {
                r.instrument.version = "unknown".into()
            }),
            ("instrument ID", |r| {
                r.instrument.instrument_id = "crypto:binance:ETHUSDT".into()
            }),
            ("instrument market", |r| {
                r.instrument.market = "us-etf".into()
            }),
            ("instrument venue", |r| {
                r.instrument.venue = "elsewhere".into()
            }),
            ("instrument symbol", |r| {
                r.instrument.symbol = "ETHUSDT".into()
            }),
            ("instrument base", |r| r.instrument.base = "ETH".into()),
            ("instrument quote", |r| r.instrument.quote = "USD".into()),
            ("instrument calendar", |r| {
                r.instrument.session_calendar_id = "different".into()
            }),
            ("instrument timezone", |r| {
                r.instrument.timezone = "different".into()
            }),
            ("instrument asset type", |r| {
                r.instrument.asset_type = "etf".into()
            }),
            ("lot size bits", |r| {
                r.instrument.lot_size_bits = Some(1.0f64.to_bits())
            }),
            ("price step bits", |r| {
                r.instrument.price_step_bits = Some(1.0f64.to_bits())
            }),
            ("min notional bits", |r| {
                r.instrument.min_notional_bits = Some(1.0f64.to_bits())
            }),
            ("listing", |r| r.instrument.listed_from = Some(123)),
            ("delisting", |r| r.instrument.delisted_at = Some(123)),
            ("capabilities", |r| {
                edit(&mut r.instrument.source_capabilities_json, |v| {
                    v["extra"] = json!(false)
                })
            }),
            ("suspensions", |r| {
                edit(&mut r.instrument.suspensions_json, |v| {
                    *v = json!([{"fromMs":1,"toMsExclusive":2}])
                })
            }),
            ("instrument unknown field", |r| {
                edit(&mut r.instrument.content_json, |v| v["extra"] = json!(0))
            }),
            ("captured row substitution", |r| r.instrument.id = 2),
            ("instrument hash", |r| {
                r.instrument.content_hash = "0".repeat(64)
            }),
        ],
    );
}

#[test]
fn market_snapshot_dataset_coverage_and_declarations_have_no_usable_subset() {
    damaged_controls(
        "captured-old-revision",
        &[
            ("snapshot version", |r| {
                r.snapshot.version = "unknown".into()
            }),
            ("snapshot status", |r| r.snapshot.status = "degraded".into()),
            ("snapshot kind", |r| r.snapshot.kind = "demo".into()),
            ("price basis", |r| {
                r.snapshot.price_basis = "adjusted-split".into()
            }),
            ("cost absent", |r| r.snapshot.cost_profile_version = None),
            ("snapshot ID", |r| r.snapshot.snapshot_id = "0".repeat(64)),
            ("dataset ID", |r| r.dataset.id += 1),
            ("dataset symbol", |r| r.dataset.symbol = "ETHUSDT".into()),
            ("dataset exchange", |r| {
                r.dataset.exchange = "elsewhere".into()
            }),
            ("dataset start", |r| r.dataset.start_time += 1),
            ("dataset end", |r| r.dataset.end_time -= 1),
            ("selected start", |r| r.campaign_from_ms += 1),
            ("selected end", |r| r.campaign_to_ms -= 1),
            ("dataset count", |r| r.dataset.candle_count += 1),
            ("coverage version", |r| {
                edit(&mut r.snapshot.coverage_json, |v| {
                    v["version"] = json!("unknown")
                })
            }),
            ("coverage unknown field", |r| {
                edit(&mut r.snapshot.coverage_json, |v| v["extra"] = json!(0))
            }),
            ("coverage blocking", |r| {
                edit(&mut r.snapshot.coverage_json, |v| {
                    v["blocking"] = json!(true)
                })
            }),
            ("coverage events", |r| {
                edit(&mut r.snapshot.coverage_json, |v| {
                    v["events"] = json!([null])
                })
            }),
            ("coverage issue", |r| {
                edit(&mut r.snapshot.coverage_json, |v| {
                    v["issue"] = json!("unknown")
                })
            }),
            ("coverage not due", |r| {
                edit(&mut r.snapshot.coverage_json, |v| {
                    v["notDueCount"] = json!(1)
                })
            }),
            ("coverage expected", |r| {
                edit(&mut r.snapshot.coverage_json, |v| {
                    v["expectedCount"] = json!(5)
                })
            }),
            ("coverage observed", |r| {
                edit(&mut r.snapshot.coverage_json, |v| {
                    v["observedCount"] = json!(5)
                })
            }),
            ("coverage matched", |r| {
                edit(&mut r.snapshot.coverage_json, |v| {
                    v["matchedCount"] = json!(5)
                })
            }),
            ("snapshot unknown field", |r| {
                edit(&mut r.snapshot.content_json, |v| v["extra"] = json!(0))
            }),
            ("source hash order", |r| {
                edit(&mut r.snapshot.content_json, |v| {
                    v["sourceRecordHashes"].as_array_mut().unwrap().reverse()
                })
            }),
            ("source relation duplicate", |r| {
                r.sources[1].provenance_id = r.sources[0].provenance_id
            }),
            ("source relation missing", |r| {
                r.sources.pop();
            }),
            ("source relation parent", |r| {
                r.sources[1].snapshot_row_id += 1
            }),
        ],
    );
    damaged_controls(
        "etf-declared-references",
        &[("ETF action absent", |r| {
            r.snapshot.corporate_action_version = None
        })],
    );
}

#[test]
fn market_every_provenance_member_including_the_late_source_is_checked() {
    damaged_controls(
        "captured-old-revision",
        &[
            ("late member rejected", |r| {
                r.provenance[1].accepted = 0;
                r.provenance[1].rejection_reason = Some("late rejected".into());
            }),
            ("late comparison", |r| {
                r.provenance[1].role = "comparison".into()
            }),
            ("late different publisher", |r| {
                r.provenance[1].source = "coinbase-rest".into()
            }),
            ("source blank", |r| r.provenance[1].source = " ".into()),
            ("version", |r| r.provenance[1].version = "unknown".into()),
            ("instrument", |r| {
                r.provenance[1].instrument_id = "crypto:binance:ETHUSDT".into()
            }),
            ("interval", |r| r.provenance[1].interval = "2h".into()),
            ("scope", |r| {
                edit(&mut r.provenance[1].request_scope_json, |v| {
                    *v = json!({"changed":v.clone()})
                })
            }),
            ("record hash", |r| {
                r.provenance[1].record_hash = "0".repeat(64)
            }),
            ("raw hash", |r| r.provenance[1].raw_sha256 = "0".repeat(64)),
            ("raw length", |r| r.provenance[1].raw_byte_len += 1),
            ("raw path", |r| {
                r.provenance[1].raw_artifact_path = "different.json".into()
            }),
            ("media type", |r| {
                r.provenance[1].raw_media_type = "image/png".into()
            }),
            ("retrieved syntax", |r| {
                r.provenance[1].retrieved_at = "not-a-time".into()
            }),
            ("available syntax", |r| {
                r.provenance[1].available_at = Some("not-a-time".into())
            }),
            ("available after retrieval", |r| {
                r.provenance[1].available_at = Some("2099-01-01T00:00:00Z".into())
            }),
            ("revision identity", |r| {
                r.provenance[1].revision_of = Some(1)
            }),
            ("accepted reason", |r| {
                r.provenance[1].rejection_reason = Some("contradiction".into())
            }),
        ],
    );
    damaged_controls(
        "forward-offset-availability",
        &[
            ("unknown availability", |r| {
                r.provenance[1].available_at = None
            }),
            ("late availability", |r| r.snapshot.as_of = 0),
        ],
    );
}

#[test]
fn market_preserves_earlier_original_preparation_and_lineage_refusals() {
    let mut f = Actual::from_case(&case("captured-old-revision"));
    let original = f.capture();
    let controls: [Damage; 3] = [
        ("dataset hash mismatch at original preparation", |r| {
            r.dataset.dataset_hash = "dataset-content-v2:".to_owned() + &"0".repeat(64)
        }),
        ("dataset interval mismatch at original lineage", |r| {
            r.dataset.interval = "2h".into()
        }),
        ("late original lineage identity failure", |r| {
            r.strategies.last_mut().unwrap().strategy_hash =
                "strategy-v2:".to_owned() + &"0".repeat(64)
        }),
    ];
    for (name, damage) in controls {
        eprintln!("controlled earlier refusal: {name}");
        let mut rows = original.clone();
        damage(&mut rows);
        let before = market::PAYLOAD_ENTRIES.with(std::cell::Cell::get);
        let _output = NoOutput::new();
        let _warmup = warmup_controls::DenyWork::new(false, true);
        preserves(&mut f, &rows, false);
        assert_eq!(market::PAYLOAD_ENTRIES.with(std::cell::Cell::get), before);
    }
}

#[test]
fn market_actual_campaign_range_only_contradiction_and_nonfirst_selected_binding() {
    let c = case("captured-old-revision");
    for mismatch in [false, true] {
        let mut f = Actual::market();
        let mut campaign: Value =
            serde_json::from_str(c["family"]["campaignDocumentJson"].as_str().unwrap()).unwrap();
        if mismatch {
            campaign["instruments"][0]["fromMs"] =
                json!(campaign["instruments"][0]["fromMs"].as_i64().unwrap() + 1);
        } else {
            let mut other = campaign["instruments"][0].clone();
            other["instrumentId"] = json!("crypto:binance:ETHUSDT");
            other["snapshotId"] = json!("f".repeat(64));
            other["fromMs"] = json!(17);
            other["toMs"] = json!(1120);
            campaign["instruments"]
                .as_array_mut()
                .unwrap()
                .insert(0, other);
        }
        let frozen = freeze_campaign(&campaign).unwrap();
        f.install(
            c["family"]["configJson"].as_str().unwrap(),
            &serde_json::to_string_pretty(&campaign).unwrap(),
            frozen.campaign_id(),
            "crypto:binance:BTCUSDT",
            "1h",
        );
        let rows = f.capture();
        assert_eq!(
            rows.campaign_from_ms,
            rows.dataset.start_time + i64::from(mismatch)
        );
        assert_eq!(rows.campaign_to_ms, rows.dataset.end_time);
        // Capturing/storing the range adds no new refusal to the old entry.
        assert!(observe_original_warmup(&rows).is_ok());
        if mismatch {
            semantic_refusal(&mut f, &rows);
        } else {
            let result = preserves(&mut f, &rows, true).unwrap();
            assert_eq!(observed(&result), c["expectedMarket"]);
        }
    }
}

#[test]
fn market_late_failed_original_warmup_discards_successful_prefix_and_all_market_output() {
    let warm = data(WARM);
    let refusal = &warm["derivationRefusalCases"][0];
    let c = case("captured-old-revision");
    let mut raw: Value = serde_json::from_str(refusal["configJson"].as_str().unwrap()).unwrap();
    raw["dataset"] = json!({"id":1,"contentHash":c["family"]["datasetHash"]});
    let mut f = Actual::market();
    f.install(
        &serde_json::to_string(&raw).unwrap(),
        c["family"]["campaignDocumentJson"].as_str().unwrap(),
        c["family"]["campaignId"].as_str().unwrap(),
        "crypto:binance:BTCUSDT",
        "1h",
    );
    f.conn()
        .execute(
            "UPDATE research_attempts SET status='failed' WHERE candidate_index=2",
            [],
        )
        .unwrap();
    f.conn()
        .execute(
            "UPDATE discovery_jobs SET status='failed' WHERE candidate_index=2",
            [],
        )
        .unwrap();
    let rows = f.capture();
    assert_eq!(rows.attempts[2].status, "failed");
    assert!(observe_original_lineage(&rows).is_ok());
    let _output = NoOutput::new();
    let before = market::PAYLOAD_ENTRIES.with(std::cell::Cell::get);
    let calls = warmup_controls::warm_entries();
    preserves(&mut f, &rows, false);
    assert_eq!(
        market::PAYLOAD_ENTRIES.with(std::cell::Cell::get),
        before + 1
    );
    assert_eq!(warmup_controls::warm_entries(), (calls.0 + 3, calls.1));
}

fn rebind_instrument(rows: &mut SourceRowsObservation) {
    rows.instrument.content_hash = rehash(&rows.instrument.content_json);
    let hash = rows.instrument.content_hash.clone();
    edit(&mut rows.snapshot.content_json, |v| {
        v["instrumentContentHash"] = json!(hash)
    });
    rows.snapshot.snapshot_id = rehash(&rows.snapshot.content_json);
}
#[test]
fn market_coherent_schema_and_typed_refusals_do_not_rely_on_stale_hashes() {
    let mut f = Actual::from_case(&case("captured-old-revision"));
    let original = f.capture();
    let controls: &[Damage] = &[
        ("unknown nested suspension", |r| {
            let suspensions = json!([{"fromMs":1,"toMsExclusive":2,"unknown":0}]);
            r.instrument.suspensions_json =
                OriginalJson::parse(&serde_json::to_vec(&suspensions).unwrap()).unwrap();
            edit(&mut r.instrument.content_json, |v| {
                v["suspensions"] = suspensions
            });
            rebind_instrument(r);
        }),
        ("coherent trim-invalid base", |r| {
            r.instrument.base = " BTC".into();
            edit(&mut r.instrument.content_json, |v| {
                v["base"] = json!(" BTC")
            });
            rebind_instrument(r);
        }),
        ("coherent zero specification", |r| {
            r.instrument.price_step_bits = Some(0.0f64.to_bits());
            edit(&mut r.instrument.content_json, |v| {
                v["priceStep"] = json!(0.0)
            });
            rebind_instrument(r);
        }),
        ("coherent capability shape", |r| {
            r.instrument.source_capabilities_json = OriginalJson::parse(b"[]").unwrap();
            edit(&mut r.instrument.content_json, |v| {
                v["sourceCapabilities"] = json!([])
            });
            rebind_instrument(r);
        }),
        ("coherent listing chronology", |r| {
            r.instrument.listed_from = Some(2);
            r.instrument.delisted_at = Some(1);
            edit(&mut r.instrument.content_json, |v| {
                v["listedFrom"] = json!(2);
                v["delistedAt"] = json!(1);
            });
            rebind_instrument(r);
        }),
        ("coherent calendar schema", |r| {
            r.calendar.version = "unknown".into();
            edit(&mut r.calendar.definition_json, |v| {
                v["version"] = json!("unknown")
            });
            r.calendar.content_hash = rehash(&r.calendar.definition_json);
        }),
        ("coherent demo snapshot", |r| {
            r.snapshot.kind = "demo".into();
            edit(&mut r.snapshot.content_json, |v| v["kind"] = json!("demo"));
            r.snapshot.snapshot_id = rehash(&r.snapshot.content_json);
        }),
        ("coherent unsupported price basis", |r| {
            r.snapshot.price_basis = "adjusted-split".into();
            edit(&mut r.snapshot.content_json, |v| {
                v["priceBasis"] = json!("adjusted-split")
            });
            r.snapshot.snapshot_id = rehash(&r.snapshot.content_json);
        }),
        ("coverage outside snapshot identity", |r| {
            edit(&mut r.snapshot.coverage_json, |v| {
                v["blocking"] = json!(true)
            })
        }),
        ("status outside snapshot identity", |r| {
            r.snapshot.status = "degraded".into()
        }),
    ];
    for (name, damage) in controls {
        eprintln!("coherent control: {name}");
        let mut rows = original.clone();
        damage(&mut rows);
        assert_eq!(
            rehash(&rows.calendar.definition_json),
            rows.calendar.content_hash
        );
        assert_eq!(
            rehash(&rows.instrument.content_json),
            rows.instrument.content_hash
        );
        assert_eq!(
            rehash(&rows.snapshot.content_json),
            rows.snapshot.snapshot_id
        );
        semantic_refusal(&mut f, &rows);
    }
}

#[test]
fn market_complete_success_keeps_failed_members_and_executes_every_original_warmup() {
    let c = case("new-revision-mixed");
    let mut f = Actual::from_case(&c);
    f.conn()
        .execute(
            "UPDATE research_attempts SET status='failed' WHERE candidate_index IN (2,3)",
            [],
        )
        .unwrap();
    f.conn()
        .execute(
            "UPDATE discovery_jobs SET status='failed' WHERE candidate_index IN (2,3)",
            [],
        )
        .unwrap();
    let rows = f.capture();
    assert_eq!(
        rows.attempts
            .iter()
            .filter(|a| a.status == "failed")
            .count(),
        2
    );
    let before = warmup_controls::warm_entries();
    let result = preserves(&mut f, &rows, true).unwrap();
    assert_eq!(
        warmup_controls::warm_entries(),
        (before.0 + 4, before.1 + 1)
    );
    assert_eq!(observed(&result), c["expectedMarket"]);
    assert_eq!(
        semantic_controls::observation(result.warmup().lineage()),
        c["family"]["expectedLineage"]
    );
    assert_eq!(
        serde_json::to_value(result.warmup().derivations()).unwrap(),
        c["family"]["expectedWarmup"]
    );
}
