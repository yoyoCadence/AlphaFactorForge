// Explicit field coverage, authored from the independently frozen cell manifest.
use super::*;

trait Perturb {
    fn perturb(&mut self);
    fn sample() -> Self;
}
impl Perturb for i64 {
    fn perturb(&mut self) {
        *self ^= 1;
    }
    fn sample() -> Self {
        1
    }
}
impl Perturb for u64 {
    fn perturb(&mut self) {
        *self ^= 1;
    }
    fn sample() -> Self {
        0
    }
}
impl Perturb for String {
    fn perturb(&mut self) {
        self.push('!');
    }
    fn sample() -> Self {
        "changed".into()
    }
}
impl Perturb for OriginalJson {
    fn perturb(&mut self) {
        *self = OriginalJson::parse(if self.as_bytes() == b"{}" {
            b"[]"
        } else {
            b"{}"
        })
        .unwrap();
    }
    fn sample() -> Self {
        OriginalJson::parse(b"null").unwrap()
    }
}
impl<T: Perturb> Perturb for Option<T> {
    fn perturb(&mut self) {
        *self = if self.is_some() {
            None
        } else {
            Some(T::sample())
        };
    }
    fn sample() -> Self {
        None
    }
}

#[test]
fn recheck_compares_each_of_133_fields_membership_and_final_usage_without_adoption() {
    let mut f = fixture();
    let saved = capture(&mut f);
    let expected = f.expected.clone();
    let tx = f.connection().transaction().unwrap();
    macro_rules! changed_field { ($($path:tt)+) => {{
        let mut changed=saved.clone();
        changed.$($path)+.perturb();
        refused(recheck_source_rows_in(&tx,&expected,&changed),"source_rows_changed");
    }}; }

    changed_field!(roots.campaign.campaign_id);
    changed_field!(roots.campaign.version);
    changed_field!(roots.campaign.document_json);
    changed_field!(roots.admission.id);
    changed_field!(roots.admission.discovery_run_id);
    changed_field!(roots.admission.campaign_id);
    changed_field!(roots.admission.instrument_id);
    changed_field!(roots.admission.batch_id);
    changed_field!(roots.admission.status);
    changed_field!(roots.admission.report_version);
    changed_field!(roots.admission.report_json);
    changed_field!(roots.admission.fee_pct_bits);
    changed_field!(roots.admission.slip_pct_bits);
    changed_field!(roots.run.id);
    changed_field!(roots.run.status);
    changed_field!(roots.run.config_json);
    changed_field!(jobs[0].id);
    changed_field!(jobs[0].discovery_run_id);
    changed_field!(jobs[0].candidate_index);
    changed_field!(jobs[0].strategy_id);
    changed_field!(jobs[0].dataset_id);
    changed_field!(jobs[0].segment);
    changed_field!(jobs[0].status);
    changed_field!(jobs[0].result_id);
    changed_field!(attempts[0].id);
    changed_field!(attempts[0].attempt_key);
    changed_field!(attempts[0].hypothesis_id);
    changed_field!(attempts[0].strategy_id);
    changed_field!(attempts[0].dataset_id);
    changed_field!(attempts[0].discovery_run_id);
    changed_field!(attempts[0].candidate_index);
    changed_field!(attempts[0].status);
    changed_field!(attempts[0].input_fingerprint_json);
    changed_field!(attempts[0].engine_fingerprint_json);
    changed_field!(attempts[0].outcome_json);
    changed_field!(attempts[0].result_artifact_id);
    changed_field!(attempts[0].trial_event_id);
    changed_field!(strategies[0].id);
    changed_field!(strategies[0].r#type);
    changed_field!(strategies[0].original_definition_json);
    changed_field!(strategies[0].dsl_json);
    changed_field!(strategies[0].strategy_hash);
    changed_field!(strategies[0].parent_strategy_id);
    changed_field!(hypotheses[0].id);
    changed_field!(hypotheses[0].hypothesis_hash);
    changed_field!(hypotheses[0].version);
    changed_field!(hypotheses[0].source);
    changed_field!(hypotheses[0].mechanism);
    changed_field!(hypotheses[0].applicability_json);
    changed_field!(hypotheses[0].failure_modes);
    changed_field!(hypotheses[0].strategy_hash);
    changed_field!(hypotheses[0].strategy_id);
    changed_field!(hypotheses[0].parent_strategy_id);
    changed_field!(hypotheses[0].variation_kind);
    changed_field!(hypotheses[0].content_json);
    changed_field!(artifacts[0].id);
    changed_field!(artifacts[0].sha256);
    changed_field!(artifacts[0].kind);
    changed_field!(artifacts[0].byte_len);
    changed_field!(artifacts[0].relative_path);
    changed_field!(snapshot.id);
    changed_field!(snapshot.snapshot_id);
    changed_field!(snapshot.version);
    changed_field!(snapshot.instrument_row_id);
    changed_field!(snapshot.instrument_id);
    changed_field!(snapshot.interval);
    changed_field!(snapshot.dataset_id);
    changed_field!(snapshot.dataset_hash);
    changed_field!(snapshot.price_basis);
    changed_field!(snapshot.calendar_id);
    changed_field!(snapshot.corporate_action_version);
    changed_field!(snapshot.cost_profile_version);
    changed_field!(snapshot.kind);
    changed_field!(snapshot.status);
    changed_field!(snapshot.as_of);
    changed_field!(snapshot.coverage_json);
    changed_field!(snapshot.content_json);
    changed_field!(instrument.id);
    changed_field!(instrument.instrument_id);
    changed_field!(instrument.revision);
    changed_field!(instrument.content_hash);
    changed_field!(instrument.version);
    changed_field!(instrument.market);
    changed_field!(instrument.venue);
    changed_field!(instrument.symbol);
    changed_field!(instrument.base);
    changed_field!(instrument.quote);
    changed_field!(instrument.asset_type);
    changed_field!(instrument.session_calendar_id);
    changed_field!(instrument.timezone);
    changed_field!(instrument.lot_size_bits);
    changed_field!(instrument.price_step_bits);
    changed_field!(instrument.min_notional_bits);
    changed_field!(instrument.listed_from);
    changed_field!(instrument.delisted_at);
    changed_field!(instrument.suspensions_json);
    changed_field!(instrument.source_capabilities_json);
    changed_field!(instrument.content_json);
    changed_field!(calendar.id);
    changed_field!(calendar.calendar_id);
    changed_field!(calendar.version);
    changed_field!(calendar.kind);
    changed_field!(calendar.timezone);
    changed_field!(calendar.content_hash);
    changed_field!(calendar.definition_json);
    changed_field!(dataset.id);
    changed_field!(dataset.exchange);
    changed_field!(dataset.symbol);
    changed_field!(dataset.interval);
    changed_field!(dataset.start_time);
    changed_field!(dataset.end_time);
    changed_field!(dataset.candle_count);
    changed_field!(dataset.source);
    changed_field!(dataset.dataset_hash);
    changed_field!(sources[0].snapshot_row_id);
    changed_field!(sources[0].provenance_id);
    changed_field!(provenance[0].id);
    changed_field!(provenance[0].record_hash);
    changed_field!(provenance[0].version);
    changed_field!(provenance[0].instrument_id);
    changed_field!(provenance[0].interval);
    changed_field!(provenance[0].source);
    changed_field!(provenance[0].role);
    changed_field!(provenance[0].request_scope_json);
    changed_field!(provenance[0].raw_sha256);
    changed_field!(provenance[0].raw_byte_len);
    changed_field!(provenance[0].raw_artifact_path);
    changed_field!(provenance[0].raw_media_type);
    changed_field!(provenance[0].retrieved_at);
    changed_field!(provenance[0].available_at);
    changed_field!(provenance[0].accepted);
    changed_field!(provenance[0].rejection_reason);
    changed_field!(provenance[0].revision_of);

    let mut changed = saved.clone();
    changed.usage.text_bytes += 1;
    refused(
        recheck_source_rows_in(&tx, &expected, &changed),
        "source_rows_changed",
    );
    let mut changed = saved.clone();
    changed.usage.json_nodes += 1;
    refused(
        recheck_source_rows_in(&tx, &expected, &changed),
        "source_rows_changed",
    );
    for table in 0..7 {
        let mut changed = saved.clone();
        match table {
            0 => {
                changed.jobs.pop();
            }
            1 => {
                changed.attempts.pop();
            }
            2 => {
                changed.strategies.pop();
            }
            3 => {
                changed.hypotheses.pop();
            }
            4 => {
                changed.artifacts.pop();
            }
            5 => {
                changed.sources.pop();
            }
            _ => {
                changed.provenance.pop();
            }
        }
        refused(
            recheck_source_rows_in(&tx, &expected, &changed),
            "source_rows_changed",
        );
    }
    unchanged(&tx, || recheck_source_rows_in(&tx, &expected, &saved)).unwrap();
    tx.rollback().unwrap();
}
