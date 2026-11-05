-- Match creation inference evidence to the 1024-operation application limit.
-- All identities, JSON checks, relationships and immutable-history guards remain.

CREATE TABLE creation_inference_rounds_generation_limits (
    cached_input_tokens INTEGER CHECK (cached_input_tokens IS NULL OR cached_input_tokens >= 0),
    reasoning_tokens INTEGER CHECK (reasoning_tokens IS NULL OR reasoning_tokens >= 0),
    cache_write_tokens INTEGER CHECK (cache_write_tokens IS NULL OR cache_write_tokens >= 0),
    web_search_requests INTEGER CHECK (web_search_requests IS NULL OR web_search_requests >= 0),
    image_tokens INTEGER CHECK (image_tokens IS NULL OR image_tokens >= 0),
    audio_tokens INTEGER CHECK (audio_tokens IS NULL OR audio_tokens >= 0),
    total_tokens INTEGER CHECK (total_tokens IS NULL OR total_tokens >= 0),
    provider_reported_cost REAL CHECK (provider_reported_cost IS NULL OR (provider_reported_cost >= 0 AND provider_reported_cost <= 1.7976931348623157e308)),
    workflow_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0 AND ordinal < 8),
    first_call_ordinal INTEGER NOT NULL CHECK (first_call_ordinal >= 0 AND first_call_ordinal <= 1024),
    call_count INTEGER NOT NULL CHECK (call_count >= 0 AND call_count <= 1024),
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
    CHECK (first_call_ordinal + call_count <= 1024),
    CHECK ((input_tokens IS NULL) = (output_tokens IS NULL)),
    CHECK ((provider_replay_artifact_id IS NULL) = (provider_replay_retention IS NULL))
) STRICT;

INSERT INTO creation_inference_rounds_generation_limits ("cached_input_tokens", "reasoning_tokens", "cache_write_tokens", "web_search_requests", "image_tokens", "audio_tokens", "total_tokens", "provider_reported_cost", "workflow_id", "turn_id", "attempt_id", "ordinal", "first_call_ordinal", "call_count", "parts_json", "provider_replay_artifact_id", "provider_replay_retention", "input_tokens", "output_tokens", "finish_reason", "provider_request_id", "admitted_at")
    SELECT "cached_input_tokens", "reasoning_tokens", "cache_write_tokens", "web_search_requests", "image_tokens", "audio_tokens", "total_tokens", "provider_reported_cost", "workflow_id", "turn_id", "attempt_id", "ordinal", "first_call_ordinal", "call_count", "parts_json", "provider_replay_artifact_id", "provider_replay_retention", "input_tokens", "output_tokens", "finish_reason", "provider_request_id", "admitted_at" FROM creation_inference_rounds;

DROP TABLE creation_inference_rounds;

ALTER TABLE creation_inference_rounds_generation_limits RENAME TO creation_inference_rounds;

CREATE INDEX creation_inference_rounds_attempt_idx
    ON creation_inference_rounds(workflow_id, turn_id, attempt_id, ordinal);

CREATE TRIGGER creation_round_immutable_delete
BEFORE DELETE ON creation_inference_rounds
BEGIN SELECT RAISE(ABORT, 'creation inference round cannot be deleted'); END;

CREATE TRIGGER creation_round_immutable_update
BEFORE UPDATE ON creation_inference_rounds
BEGIN SELECT RAISE(ABORT, 'creation inference round is immutable'); END;

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



CREATE TABLE creation_admitted_tool_calls_generation_limits (
    workflow_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    round_ordinal INTEGER NOT NULL CHECK (round_ordinal >= 0 AND round_ordinal < 8),
    id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0 AND ordinal < 1024),
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
            length(CAST(raw_arguments AS BLOB)) <= 67108864
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

INSERT INTO creation_admitted_tool_calls_generation_limits ("workflow_id", "turn_id", "attempt_id", "round_ordinal", "id", "ordinal", "definition_name", "definition_version", "provider_call_id", "arguments_json", "raw_arguments", "provider_replay_artifact_id", "provider_replay_retention", "admitted_at")
    SELECT "workflow_id", "turn_id", "attempt_id", "round_ordinal", "id", "ordinal", "definition_name", "definition_version", "provider_call_id", "arguments_json", "raw_arguments", "provider_replay_artifact_id", "provider_replay_retention", "admitted_at" FROM creation_admitted_tool_calls;

DROP TABLE creation_admitted_tool_calls;

ALTER TABLE creation_admitted_tool_calls_generation_limits RENAME TO creation_admitted_tool_calls;

CREATE INDEX creation_admitted_tool_calls_attempt_idx
    ON creation_admitted_tool_calls(workflow_id, turn_id, attempt_id, round_ordinal, ordinal);

CREATE UNIQUE INDEX creation_admitted_tool_calls_provider_id_uq
    ON creation_admitted_tool_calls(workflow_id, turn_id, attempt_id, provider_call_id)
    WHERE provider_call_id IS NOT NULL;

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
) AND (
    NEW.definition_version != 1
    OR NOT EXISTS (
        SELECT 1 FROM creation_inference_attempts attempt
        WHERE attempt.workflow_id = NEW.workflow_id
          AND attempt.turn_id = NEW.turn_id
          AND attempt.id = NEW.attempt_id
    )
    OR EXISTS (
        SELECT 1
        FROM creation_inference_attempts attempt,
             json_each(json_extract(attempt.tool_request_json, '$.value.definitions')) definition
        WHERE attempt.workflow_id = NEW.workflow_id
          AND attempt.turn_id = NEW.turn_id
          AND attempt.id = NEW.attempt_id
          AND json_extract(definition.value, '$.name') = NEW.definition_name
    )
)
BEGIN SELECT RAISE(ABORT, 'creation tool call contract mismatch'); END;

CREATE TRIGGER creation_tool_call_immutable_delete
BEFORE DELETE ON creation_admitted_tool_calls
BEGIN SELECT RAISE(ABORT, 'creation tool call cannot be deleted'); END;

CREATE TRIGGER creation_tool_call_immutable_update
BEFORE UPDATE ON creation_admitted_tool_calls
BEGIN SELECT RAISE(ABORT, 'creation tool call is immutable'); END;

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


