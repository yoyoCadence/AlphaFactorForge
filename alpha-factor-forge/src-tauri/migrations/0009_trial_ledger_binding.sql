-- P12b-2: a trial event is frozen with each research attempt. Existing P05
-- attempts receive their event during the first registry binding. A separate
-- registry commit must precede the workspace transaction that fills this ID.
ALTER TABLE research_attempts ADD COLUMN trial_event_id TEXT;

-- 0007 freezes terminal rows. The one-time NULL -> event link must also work
-- for completed historical attempts, without reopening any other column.
DROP TRIGGER research_attempts_are_frozen;
CREATE TRIGGER research_attempts_are_frozen
BEFORE UPDATE ON research_attempts
WHEN NEW.attempt_key IS NOT OLD.attempt_key
  OR NEW.hypothesis_id IS NOT OLD.hypothesis_id
  OR NEW.strategy_id IS NOT OLD.strategy_id
  OR NEW.dataset_id IS NOT OLD.dataset_id
  OR NEW.discovery_run_id IS NOT OLD.discovery_run_id
  OR NEW.candidate_index IS NOT OLD.candidate_index
  OR NEW.input_fingerprint_json IS NOT OLD.input_fingerprint_json
  OR NEW.engine_fingerprint_json IS NOT OLD.engine_fingerprint_json
  OR NEW.submitted_at IS NOT OLD.submitted_at
  OR (OLD.status IN ('completed','failed','skipped') AND NOT (
      OLD.trial_event_id IS NULL AND NEW.trial_event_id IS NOT NULL
      AND NEW.status IS OLD.status
      AND NEW.outcome_json IS OLD.outcome_json
      AND NEW.result_artifact_id IS OLD.result_artifact_id
      AND NEW.epoch IS OLD.epoch
      AND NEW.finished_at IS OLD.finished_at
  ))
BEGIN
    SELECT RAISE(ABORT, 'research attempt is frozen');
END;

CREATE INDEX idx_research_attempts_trial_event
    ON research_attempts(trial_event_id);

CREATE TRIGGER research_attempt_trial_event_frozen
BEFORE UPDATE OF trial_event_id ON research_attempts
WHEN OLD.trial_event_id IS NOT NULL
  OR NEW.trial_event_id IS NULL
  OR length(NEW.trial_event_id) = 0
BEGIN
    SELECT RAISE(ABORT, 'trial event id is frozen');
END;
