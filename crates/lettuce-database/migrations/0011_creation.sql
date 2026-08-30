CREATE TABLE creation_workflows (
    id TEXT PRIMARY KEY,
    target_json TEXT NOT NULL CHECK (json_valid(target_json)),
    stage TEXT NOT NULL CHECK (stage IN ('drafting', 'awaiting_review', 'awaiting_confirmation')),
    current_proposal_id TEXT,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (current_proposal_id) REFERENCES creation_proposals(id) ON DELETE RESTRICT,
    CHECK (updated_at >= created_at)
) STRICT;

CREATE TABLE creation_proposals (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    turn_id TEXT,
    parent_id TEXT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    stage TEXT NOT NULL CHECK (stage IN ('drafting', 'awaiting_review', 'awaiting_confirmation')),
    draft_json TEXT NOT NULL CHECK (json_valid(draft_json)),
    outcomes_json TEXT NOT NULL CHECK (json_valid(outcomes_json) AND json_type(outcomes_json) = 'array'),
    created_at INTEGER NOT NULL,
    UNIQUE (workflow_id, ordinal),
    FOREIGN KEY (workflow_id) REFERENCES creation_workflows(id) ON DELETE RESTRICT,
    FOREIGN KEY (turn_id) REFERENCES creation_turns(id) ON DELETE RESTRICT,
    FOREIGN KEY (parent_id) REFERENCES creation_proposals(id) ON DELETE RESTRICT,
    CHECK ((ordinal = 0) = (turn_id IS NULL)),
    CHECK ((ordinal = 0) = (parent_id IS NULL))
) STRICT;

CREATE TABLE creation_turns (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    base_proposal_id TEXT NOT NULL,
    user_message TEXT NOT NULL CHECK (length(trim(user_message)) > 0),
    created_at INTEGER NOT NULL,
    UNIQUE (workflow_id, ordinal),
    FOREIGN KEY (workflow_id) REFERENCES creation_workflows(id) ON DELETE RESTRICT,
    FOREIGN KEY (base_proposal_id) REFERENCES creation_proposals(id) ON DELETE RESTRICT
) STRICT;

CREATE INDEX creation_proposals_workflow_idx
    ON creation_proposals(workflow_id, ordinal, id);
CREATE INDEX creation_turns_workflow_idx
    ON creation_turns(workflow_id, ordinal, id);
CREATE UNIQUE INDEX creation_proposals_owner_id_uq
    ON creation_proposals(workflow_id, id);
CREATE UNIQUE INDEX creation_turns_owner_id_uq
    ON creation_turns(workflow_id, id);

CREATE TABLE creation_inference_attempts (
    workflow_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0 AND ordinal < 65536),
    retry_parent_id TEXT,
    base_proposal_id TEXT NOT NULL,
    planned_proposal_id TEXT NOT NULL,
    target TEXT NOT NULL CHECK (target IN ('character', 'persona', 'lorebook')),
    stage TEXT NOT NULL CHECK (stage IN ('drafting', 'awaiting_review')),
    tool_request_json TEXT NOT NULL CHECK (
        json_valid(tool_request_json)
        AND json_extract(tool_request_json, '$.format_version') = 1
        AND json_type(tool_request_json, '$.value.definitions') = 'array'
    ),
    status TEXT NOT NULL CHECK (
        status IN ('created', 'running', 'succeeded', 'failed', 'cancelled', 'interrupted')
    ),
    failure TEXT CHECK (
        failure IS NULL OR failure IN (
            'provider_unavailable', 'provider_rejected', 'empty_response', 'timed_out',
            'round_limit', 'internal'
        )
    ),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    finished_at INTEGER,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (workflow_id, turn_id, id),
    UNIQUE (id),
    UNIQUE (workflow_id, turn_id, ordinal),
    UNIQUE (planned_proposal_id),
    UNIQUE (workflow_id, turn_id, id, base_proposal_id),
    FOREIGN KEY (workflow_id, turn_id)
        REFERENCES creation_turns(workflow_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (workflow_id, base_proposal_id)
        REFERENCES creation_proposals(workflow_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (workflow_id, turn_id, retry_parent_id)
        REFERENCES creation_inference_attempts(workflow_id, turn_id, id) ON DELETE RESTRICT,
    CHECK ((ordinal = 0) = (retry_parent_id IS NULL)),
    CHECK (base_proposal_id != planned_proposal_id),
    CHECK ((status = 'created') = (started_at IS NULL AND finished_at IS NULL)),
    CHECK ((status = 'running') = (started_at IS NOT NULL AND finished_at IS NULL)),
    CHECK (status NOT IN ('succeeded', 'failed', 'interrupted') OR (started_at IS NOT NULL AND finished_at IS NOT NULL)),
    CHECK ((status = 'cancelled') = (finished_at IS NOT NULL) OR status != 'cancelled'),
    CHECK ((status = 'failed') = (failure IS NOT NULL)),
    CHECK (updated_at >= created_at),
    CHECK (started_at IS NULL OR started_at BETWEEN created_at AND updated_at),
    CHECK (finished_at IS NULL OR finished_at BETWEEN created_at AND updated_at),
    CHECK (started_at IS NULL OR finished_at IS NULL OR started_at <= finished_at)
) STRICT;

CREATE TABLE creation_inference_rounds (
    workflow_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0 AND ordinal < 8),
    first_call_ordinal INTEGER NOT NULL CHECK (first_call_ordinal >= 0 AND first_call_ordinal <= 64),
    call_count INTEGER NOT NULL CHECK (call_count >= 0 AND call_count <= 64),
    parts_json TEXT NOT NULL CHECK (
        json_valid(parts_json)
        AND json_extract(parts_json, '$.format_version') = 1
        AND json_type(parts_json, '$.value') = 'array'
        AND json_array_length(json_extract(parts_json, '$.value')) <= 64
    ),
    provider_replay_artifact_id TEXT,
    provider_replay_retention TEXT CHECK (
        provider_replay_retention IS NULL OR provider_replay_retention = 'conversation'
    ),
    input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
    finish_reason TEXT NOT NULL CHECK (finish_reason IN ('stop', 'length')),
    provider_request_id TEXT CHECK (
        provider_request_id IS NULL OR
        (length(trim(provider_request_id)) > 0 AND length(CAST(provider_request_id AS BLOB)) <= 256)
    ),
    admitted_at INTEGER NOT NULL,
    PRIMARY KEY (workflow_id, turn_id, attempt_id, ordinal),
    FOREIGN KEY (workflow_id, turn_id, attempt_id)
        REFERENCES creation_inference_attempts(workflow_id, turn_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (provider_replay_artifact_id, provider_replay_retention)
        REFERENCES conversation_replay_artifacts(artifact_id, retention) ON DELETE RESTRICT,
    CHECK (call_count > 0 OR json_array_length(json_extract(parts_json, '$.value')) > 0),
    CHECK (first_call_ordinal + call_count <= 64),
    CHECK ((input_tokens IS NULL) = (output_tokens IS NULL)),
    CHECK ((provider_replay_artifact_id IS NULL) = (provider_replay_retention IS NULL))
) STRICT;
CREATE INDEX creation_inference_rounds_attempt_idx
    ON creation_inference_rounds(workflow_id, turn_id, attempt_id, ordinal);

CREATE TABLE creation_admitted_tool_calls (
    workflow_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    round_ordinal INTEGER NOT NULL CHECK (round_ordinal >= 0 AND round_ordinal < 8),
    id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0 AND ordinal < 64),
    definition_name TEXT NOT NULL CHECK (
        length(definition_name) BETWEEN 1 AND 64
        AND definition_name NOT GLOB '*[^A-Za-z0-9_-]*'
    ),
    definition_version INTEGER NOT NULL CHECK (definition_version >= 1),
    provider_call_id TEXT CHECK (
        provider_call_id IS NULL OR
        (length(trim(provider_call_id)) > 0 AND length(CAST(provider_call_id AS BLOB)) <= 256)
    ),
    arguments_json TEXT NOT NULL CHECK (
        json_valid(arguments_json)
        AND json_extract(arguments_json, '$.format_version') = 1
        AND json_type(arguments_json, '$.value') = 'object'
        AND length(CAST(arguments_json AS BLOB)) <= 262208
    ),
    raw_arguments TEXT CHECK (
        raw_arguments IS NULL OR (
            length(CAST(raw_arguments AS BLOB)) <= 262144
            AND json_valid(raw_arguments)
            AND json(raw_arguments) = json(json_extract(arguments_json, '$.value'))
        )
    ),
    provider_replay_artifact_id TEXT,
    provider_replay_retention TEXT CHECK (
        provider_replay_retention IS NULL OR provider_replay_retention = 'conversation'
    ),
    admitted_at INTEGER NOT NULL,
    PRIMARY KEY (workflow_id, turn_id, attempt_id, id),
    UNIQUE (id),
    UNIQUE (workflow_id, turn_id, attempt_id, ordinal),
    FOREIGN KEY (workflow_id, turn_id, attempt_id)
        REFERENCES creation_inference_attempts(workflow_id, turn_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (workflow_id, turn_id, attempt_id, round_ordinal)
        REFERENCES creation_inference_rounds(workflow_id, turn_id, attempt_id, ordinal)
        ON DELETE RESTRICT,
    FOREIGN KEY (provider_replay_artifact_id, provider_replay_retention)
        REFERENCES conversation_replay_artifacts(artifact_id, retention) ON DELETE RESTRICT,
    CHECK ((provider_replay_artifact_id IS NULL) = (provider_replay_retention IS NULL))
) STRICT;
CREATE UNIQUE INDEX creation_admitted_tool_calls_provider_id_uq
    ON creation_admitted_tool_calls(workflow_id, turn_id, attempt_id, provider_call_id)
    WHERE provider_call_id IS NOT NULL;
CREATE INDEX creation_admitted_tool_calls_attempt_idx
    ON creation_admitted_tool_calls(workflow_id, turn_id, attempt_id, round_ordinal, ordinal);

CREATE TRIGGER creation_attempt_retry_guard
BEFORE INSERT ON creation_inference_attempts
WHEN NEW.retry_parent_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM creation_inference_attempts parent
    WHERE parent.workflow_id = NEW.workflow_id
      AND parent.turn_id = NEW.turn_id
      AND parent.id = NEW.retry_parent_id
      AND parent.ordinal + 1 = NEW.ordinal
      AND parent.status IN ('failed', 'cancelled', 'interrupted')
      AND parent.base_proposal_id = NEW.base_proposal_id
      AND parent.target = NEW.target
      AND parent.stage = NEW.stage
      AND parent.tool_request_json = NEW.tool_request_json
)
BEGIN SELECT RAISE(ABORT, 'invalid creation attempt retry'); END;

CREATE TRIGGER creation_attempt_owner_guard
BEFORE INSERT ON creation_inference_attempts
WHEN NOT EXISTS (
    SELECT 1
    FROM creation_workflows workflow
    JOIN creation_turns turn
      ON turn.workflow_id = workflow.id AND turn.id = NEW.turn_id
    JOIN creation_proposals proposal
      ON proposal.workflow_id = workflow.id AND proposal.id = NEW.base_proposal_id
    WHERE workflow.id = NEW.workflow_id
      AND workflow.current_proposal_id = NEW.base_proposal_id
      AND workflow.stage = NEW.stage
      AND turn.base_proposal_id = NEW.base_proposal_id
      AND proposal.stage = NEW.stage
      AND NOT EXISTS (
          SELECT 1 FROM creation_proposals existing WHERE existing.id = NEW.planned_proposal_id
      )
      AND (
          (NEW.target = 'character' AND json_extract(workflow.target_json, '$.kind') IN ('new_character', 'existing_character'))
          OR (NEW.target = 'persona' AND json_extract(workflow.target_json, '$.kind') IN ('new_persona', 'existing_persona'))
          OR (NEW.target = 'lorebook' AND json_extract(workflow.target_json, '$.kind') IN ('new_lorebook', 'existing_lorebook'))
      )
)
BEGIN SELECT RAISE(ABORT, 'creation attempt ownership mismatch'); END;

CREATE TRIGGER creation_attempt_identity_immutable
BEFORE UPDATE OF workflow_id, turn_id, id, ordinal, retry_parent_id, base_proposal_id,
                 planned_proposal_id, target, stage, tool_request_json
ON creation_inference_attempts
BEGIN SELECT RAISE(ABORT, 'creation attempt identity is immutable'); END;

CREATE TRIGGER creation_attempt_terminal_immutable
BEFORE UPDATE ON creation_inference_attempts
WHEN OLD.status IN ('succeeded', 'failed', 'cancelled', 'interrupted')
BEGIN SELECT RAISE(ABORT, 'terminal creation attempt is immutable'); END;

CREATE TRIGGER creation_attempt_transition_guard
BEFORE UPDATE OF status ON creation_inference_attempts
WHEN NEW.status != OLD.status AND NOT (
    (OLD.status = 'created' AND NEW.status IN ('running', 'cancelled')) OR
    (OLD.status = 'running' AND NEW.status IN ('succeeded', 'failed', 'cancelled', 'interrupted'))
)
BEGIN SELECT RAISE(ABORT, 'invalid creation attempt transition'); END;

CREATE TRIGGER creation_attempt_revision_guard
BEFORE UPDATE ON creation_inference_attempts
WHEN NEW.revision != OLD.revision + 1 OR NEW.updated_at < OLD.updated_at
BEGIN SELECT RAISE(ABORT, 'invalid creation attempt revision'); END;

CREATE TRIGGER creation_tool_call_live_attempt_guard
BEFORE INSERT ON creation_admitted_tool_calls
WHEN NOT EXISTS (
    SELECT 1
    FROM creation_inference_attempts attempt
    JOIN creation_workflows workflow ON workflow.id = attempt.workflow_id
    JOIN creation_turns turn
      ON turn.workflow_id = attempt.workflow_id AND turn.id = attempt.turn_id
    WHERE attempt.workflow_id = NEW.workflow_id
      AND attempt.turn_id = NEW.turn_id
      AND attempt.id = NEW.attempt_id
      AND attempt.status = 'running'
      AND workflow.current_proposal_id = attempt.base_proposal_id
      AND workflow.stage = attempt.stage
      AND turn.base_proposal_id = attempt.base_proposal_id
)
BEGIN SELECT RAISE(ABORT, 'creation call requires a live non-stale attempt'); END;

CREATE TRIGGER creation_round_live_attempt_guard
BEFORE INSERT ON creation_inference_rounds
WHEN NOT EXISTS (
    SELECT 1
    FROM creation_inference_attempts attempt
    JOIN creation_workflows workflow ON workflow.id = attempt.workflow_id
    JOIN creation_turns turn
      ON turn.workflow_id = attempt.workflow_id AND turn.id = attempt.turn_id
    WHERE attempt.workflow_id = NEW.workflow_id
      AND attempt.turn_id = NEW.turn_id
      AND attempt.id = NEW.attempt_id
      AND attempt.status = 'running'
      AND workflow.current_proposal_id = attempt.base_proposal_id
      AND workflow.stage = attempt.stage
      AND turn.base_proposal_id = attempt.base_proposal_id
)
BEGIN SELECT RAISE(ABORT, 'creation round requires a live non-stale attempt'); END;

CREATE TRIGGER creation_round_ordinal_guard
BEFORE INSERT ON creation_inference_rounds
WHEN NEW.ordinal != (
    SELECT coalesce(max(ordinal) + 1, 0)
    FROM creation_inference_rounds
    WHERE workflow_id = NEW.workflow_id
      AND turn_id = NEW.turn_id
      AND attempt_id = NEW.attempt_id
)
OR NEW.first_call_ordinal != (
    SELECT coalesce(sum(call_count), 0)
    FROM creation_inference_rounds
    WHERE workflow_id = NEW.workflow_id
      AND turn_id = NEW.turn_id
      AND attempt_id = NEW.attempt_id
)
BEGIN SELECT RAISE(ABORT, 'creation round ordinal mismatch'); END;

CREATE TRIGGER creation_round_immutable_update
BEFORE UPDATE ON creation_inference_rounds
BEGIN SELECT RAISE(ABORT, 'creation inference round is immutable'); END;

CREATE TRIGGER creation_round_immutable_delete
BEFORE DELETE ON creation_inference_rounds
BEGIN SELECT RAISE(ABORT, 'creation inference round cannot be deleted'); END;

CREATE TRIGGER creation_tool_call_contract_guard
BEFORE INSERT ON creation_admitted_tool_calls
WHEN NOT EXISTS (
    SELECT 1
    FROM creation_inference_attempts attempt,
         json_each(json_extract(attempt.tool_request_json, '$.value.definitions')) definition
    WHERE attempt.workflow_id = NEW.workflow_id
      AND attempt.turn_id = NEW.turn_id
      AND attempt.id = NEW.attempt_id
      AND json_extract(definition.value, '$.name') = NEW.definition_name
      AND json_extract(definition.value, '$.version') = NEW.definition_version
)
BEGIN SELECT RAISE(ABORT, 'creation tool call contract mismatch'); END;

CREATE TRIGGER creation_tool_call_immutable_update
BEFORE UPDATE ON creation_admitted_tool_calls
BEGIN SELECT RAISE(ABORT, 'creation tool call is immutable'); END;

CREATE TRIGGER creation_tool_call_immutable_delete
BEFORE DELETE ON creation_admitted_tool_calls
BEGIN SELECT RAISE(ABORT, 'creation tool call cannot be deleted'); END;

CREATE TRIGGER creation_proposals_immutable_update
BEFORE UPDATE ON creation_proposals
BEGIN SELECT RAISE(ABORT, 'creation proposal is immutable'); END;

CREATE TRIGGER creation_proposals_immutable_delete
BEFORE DELETE ON creation_proposals
BEGIN SELECT RAISE(ABORT, 'creation proposal cannot be deleted'); END;

CREATE TRIGGER creation_turns_immutable_update
BEFORE UPDATE ON creation_turns
BEGIN SELECT RAISE(ABORT, 'creation turn is immutable'); END;

CREATE TRIGGER creation_turns_immutable_delete
BEFORE DELETE ON creation_turns
BEGIN SELECT RAISE(ABORT, 'creation turn cannot be deleted'); END;

CREATE TRIGGER creation_workflow_transition_guard
BEFORE UPDATE OF stage, current_proposal_id, revision ON creation_workflows
WHEN OLD.current_proposal_id IS NOT NULL AND (
  NEW.revision != OLD.revision + 1
  OR NEW.current_proposal_id = OLD.current_proposal_id
  OR (OLD.stage = 'awaiting_confirmation' AND NEW.stage != 'awaiting_confirmation')
)
BEGIN SELECT RAISE(ABORT, 'invalid creation workflow transition'); END;

CREATE TRIGGER creation_workflow_proposal_ownership
BEFORE UPDATE OF stage, current_proposal_id ON creation_workflows
WHEN NEW.current_proposal_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM creation_proposals
    WHERE id = NEW.current_proposal_id
      AND workflow_id = NEW.id
      AND stage = NEW.stage
)
BEGIN SELECT RAISE(ABORT, 'creation proposal ownership mismatch'); END;

CREATE TRIGGER creation_proposal_lineage_guard
BEFORE INSERT ON creation_proposals
WHEN (NEW.ordinal = 0 AND (NEW.stage != 'drafting' OR NEW.turn_id IS NOT NULL OR NEW.parent_id IS NOT NULL))
  OR (NEW.ordinal > 0 AND NOT EXISTS (
      SELECT 1
      FROM creation_proposals parent
      JOIN creation_turns turn ON turn.id = NEW.turn_id
      WHERE parent.id = NEW.parent_id
        AND parent.workflow_id = NEW.workflow_id
        AND parent.ordinal + 1 = NEW.ordinal
        AND turn.workflow_id = NEW.workflow_id
        AND turn.base_proposal_id = parent.id
  ))
BEGIN SELECT RAISE(ABORT, 'invalid creation proposal lineage'); END;
