CREATE TABLE memory_spaces (
    id TEXT PRIMARY KEY,
    revision INTEGER NOT NULL CHECK (revision >= 1)
) STRICT;

CREATE TABLE conversation_memory_spaces (
    conversation_id TEXT PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
    space_id TEXT NOT NULL UNIQUE REFERENCES memory_spaces(id) ON DELETE RESTRICT,
    UNIQUE (conversation_id, space_id)
) STRICT;

CREATE TABLE dynamic_memory_pending_approvals (
    conversation_id TEXT PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
    prompted_message_count INTEGER NOT NULL CHECK (prompted_message_count >= 1),
    pending INTEGER NOT NULL CHECK (pending IN (0, 1)),
    skipped INTEGER NOT NULL CHECK (skipped IN (0, 1)),
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE memory_summaries (
    space_id TEXT PRIMARY KEY REFERENCES memory_spaces(id) ON DELETE RESTRICT,
    conversation_id TEXT NOT NULL,
    text TEXT NOT NULL CHECK (
        length(trim(text)) > 0
        AND length(CAST(text AS BLOB)) <= 6000
    ),
    token_count INTEGER NOT NULL CHECK (token_count BETWEEN 0 AND 4294967295),
    window_start INTEGER NOT NULL CHECK (window_start >= 0),
    window_end INTEGER NOT NULL CHECK (window_end > window_start),
    updated_at INTEGER NOT NULL,
    UNIQUE (space_id, conversation_id),
    FOREIGN KEY (conversation_id, space_id)
        REFERENCES conversation_memory_spaces(conversation_id, space_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE memory_summary_source_messages (
    space_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 1023),
    PRIMARY KEY (space_id, ordinal),
    UNIQUE (space_id, message_id),
    FOREIGN KEY (space_id, conversation_id)
        REFERENCES memory_summaries(space_id, conversation_id) ON DELETE CASCADE,
    FOREIGN KEY (conversation_id, message_id)
        REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE memory_items (
    space_id TEXT NOT NULL REFERENCES memory_spaces(id) ON DELETE RESTRICT,
    id TEXT NOT NULL UNIQUE,
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 4095),
    text TEXT NOT NULL CHECK (
        length(trim(text)) > 0
        AND length(CAST(text AS BLOB)) <= 16384
    ),
    category TEXT NOT NULL CHECK (category IN (
        'character_trait', 'relationship', 'plot_event',
        'world_detail', 'preference', 'other'
    )),
    source_message_id TEXT,
    source_role TEXT CHECK (source_role IN ('user','assistant')),
    observed_at INTEGER,
    observed_time_precision TEXT CHECK (observed_time_precision = 'turn'),
    superseded_by TEXT,
    superseded_at INTEGER,
    supersedes_json TEXT NOT NULL CHECK (json_valid(supersedes_json) AND json_type(supersedes_json) = 'array'),
    token_count INTEGER NOT NULL CHECK (token_count BETWEEN 0 AND 4294967295),
    is_cold INTEGER NOT NULL CHECK (is_cold IN (0, 1)),
    is_pinned INTEGER NOT NULL CHECK (is_pinned IN (0, 1)),
    importance INTEGER NOT NULL CHECK (importance BETWEEN 0 AND 10000),
    persistence_importance INTEGER NOT NULL CHECK (persistence_importance BETWEEN 0 AND 10000),
    prompt_importance INTEGER NOT NULL CHECK (prompt_importance BETWEEN 0 AND 10000),
    volatility INTEGER NOT NULL CHECK (volatility BETWEEN 0 AND 10000),
    access_count INTEGER NOT NULL CHECK (access_count BETWEEN 0 AND 4294967295),
    created_at INTEGER NOT NULL,
    last_accessed_at INTEGER NOT NULL,
    PRIMARY KEY (space_id, id),
    UNIQUE (space_id, ordinal),
    CHECK (NOT (is_pinned = 1 AND is_cold = 1)),
    CHECK (
        (source_role IS NULL AND observed_at IS NULL AND observed_time_precision IS NULL)
        OR (source_message_id IS NOT NULL AND source_role IS NOT NULL AND observed_at IS NOT NULL AND observed_time_precision = 'turn')
    ),
    CHECK ((superseded_by IS NULL) = (superseded_at IS NULL)),
    CHECK (superseded_by IS NULL OR superseded_by <> id)
) STRICT;

CREATE INDEX memory_items_space_policy_idx
    ON memory_items(space_id, is_pinned DESC, is_cold ASC, last_accessed_at DESC, id ASC);

-- Rebuildable derived data. Rows deliberately reference only the space: the
-- memory CAS replaces item rows, and projections for unchanged text must not
-- be rewritten. Reads join live items and exact source text, so deleted or
-- changed memories cannot contribute stale similarity evidence.
CREATE TABLE memory_embedding_projections (
    space_id TEXT NOT NULL REFERENCES memory_spaces(id) ON DELETE CASCADE,
    memory_id TEXT NOT NULL,
    source_revision TEXT NOT NULL CHECK (
        length(trim(source_revision)) > 0
        AND length(CAST(source_revision AS BLOB)) <= 128
    ),
    dimensions INTEGER NOT NULL CHECK (dimensions IN (64, 128, 256, 512, 768)),
    source_text TEXT NOT NULL CHECK (
        length(trim(source_text)) > 0
        AND length(CAST(source_text AS BLOB)) <= 16384
    ),
    status TEXT NOT NULL CHECK (status IN ('ready', 'repair_needed')),
    vector BLOB,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (space_id, memory_id, source_revision, dimensions),
    CHECK (
        (status = 'ready' AND vector IS NOT NULL AND length(vector) = dimensions * 4)
        OR (status = 'repair_needed' AND vector IS NULL)
    )
) STRICT;

CREATE INDEX memory_embedding_projections_lookup_idx
    ON memory_embedding_projections(space_id, source_revision, dimensions, status, memory_id);

-- Authoritative restart input for a dynamic-memory tool round. The JSON owns
-- the full versioned plan; relational projections bind it to the exact
-- attempt, job, and memory revision that were used to prepare it.
CREATE TABLE dynamic_memory_preparation_plans (
    conversation_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    first_execution_ordinal INTEGER NOT NULL CHECK (
        first_execution_ordinal >= 0 AND first_execution_ordinal <= 65535
    ),
    job_id TEXT NOT NULL,
    space_id TEXT NOT NULL REFERENCES memory_spaces(id) ON DELETE RESTRICT,
    expected_memory_revision INTEGER NOT NULL CHECK (expected_memory_revision >= 1),
    plan_json TEXT NOT NULL CHECK (
        json_valid(plan_json)
        AND json_extract(plan_json, '$.format_version') = 1
        AND length(CAST(plan_json AS BLOB)) <= 2097152
    ),
    plan_digest TEXT NOT NULL CHECK (
        length(plan_digest) = 64
        AND lower(plan_digest) = plan_digest
        AND plan_digest NOT GLOB '*[^0-9a-f]*'
    ),
    PRIMARY KEY (conversation_id, turn_id, attempt_id, first_execution_ordinal),
    FOREIGN KEY (conversation_id, turn_id, attempt_id, job_id)
        REFERENCES generation_attempts(conversation_id, turn_id, id, job_id)
        ON DELETE RESTRICT
) STRICT;

CREATE TRIGGER dynamic_memory_preparation_plan_immutable
BEFORE UPDATE ON dynamic_memory_preparation_plans
BEGIN SELECT RAISE(ABORT, 'dynamic-memory preparation plan is immutable'); END;

CREATE TRIGGER dynamic_memory_preparation_plan_delete_restricted
BEFORE DELETE ON dynamic_memory_preparation_plans
BEGIN SELECT RAISE(ABORT, 'dynamic-memory preparation plan cannot be deleted'); END;

-- Background extraction owns its inference history independently from visible
-- conversation generation turns. The run is an immutable input snapshot;
-- attempts and rounds provide exact retry and provider replay evidence.
CREATE TABLE dynamic_memory_runs (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE RESTRICT,
    space_id TEXT NOT NULL REFERENCES memory_spaces(id) ON DELETE RESTRICT,
    time_awareness_enabled INTEGER NOT NULL CHECK (time_awareness_enabled IN (0,1)),
    supersession_enabled INTEGER NOT NULL CHECK (supersession_enabled IN (0,1)),
    structured_fallback_format TEXT NOT NULL CHECK (structured_fallback_format IN ('json','xml')),
    summary_message_interval INTEGER NOT NULL CHECK (summary_message_interval BETWEEN 1 AND 4294967295),
    summary_window_start INTEGER NOT NULL CHECK (summary_window_start >= 0),
    summary_window_end INTEGER NOT NULL CHECK (summary_window_end > summary_window_start),
    starting_memory_json TEXT NOT NULL CHECK (
        json_valid(starting_memory_json)
        AND json_extract(starting_memory_json, '$.format_version') = 1
    ),
    profile_json TEXT NOT NULL CHECK (
        json_valid(profile_json)
        AND json_extract(profile_json, '$.format_version') = 1
    ),
    tool_request_json TEXT NOT NULL CHECK (
        json_valid(tool_request_json)
        AND json_extract(tool_request_json, '$.format_version') = 1
        AND json_type(tool_request_json, '$.value.definitions') = 'array'
    ),
    created_at INTEGER NOT NULL,
    UNIQUE (id, conversation_id)
) STRICT;

CREATE TABLE dynamic_memory_run_source_messages (
    run_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user','assistant')),
    revision_id TEXT,
    candidate_id TEXT,
    effective_time INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 1023),
    PRIMARY KEY (run_id, ordinal),
    UNIQUE (run_id, message_id),
    FOREIGN KEY (run_id, conversation_id)
        REFERENCES dynamic_memory_runs(id, conversation_id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, message_id)
        REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, message_id, revision_id)
        REFERENCES conversation_message_revisions(conversation_id, message_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, message_id, candidate_id)
        REFERENCES conversation_message_candidates(conversation_id, message_id, id) ON DELETE RESTRICT,
    CHECK ((revision_id IS NOT NULL) <> (candidate_id IS NOT NULL))
) STRICT;

CREATE TABLE dynamic_memory_run_attempts (
    run_id TEXT NOT NULL REFERENCES dynamic_memory_runs(id) ON DELETE RESTRICT,
    id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 65535),
    retry_parent_id TEXT,
    job_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN ('created', 'processing', 'succeeded', 'failed', 'cancelled', 'interrupted')
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
    PRIMARY KEY (run_id, id),
    UNIQUE (id),
    UNIQUE (run_id, ordinal),
    UNIQUE (job_id),
    FOREIGN KEY (run_id, retry_parent_id)
        REFERENCES dynamic_memory_run_attempts(run_id, id) ON DELETE RESTRICT,
    CHECK ((ordinal = 0) = (retry_parent_id IS NULL)),
    CHECK ((status = 'created') = (started_at IS NULL AND finished_at IS NULL)),
    CHECK ((status = 'processing') = (started_at IS NOT NULL AND finished_at IS NULL)),
    CHECK (status NOT IN ('succeeded', 'failed', 'interrupted') OR (started_at IS NOT NULL AND finished_at IS NOT NULL)),
    CHECK ((status = 'cancelled') = (finished_at IS NOT NULL) OR status != 'cancelled'),
    CHECK ((status = 'failed') = (failure IS NOT NULL)),
    CHECK (updated_at >= created_at),
    CHECK (started_at IS NULL OR started_at BETWEEN created_at AND updated_at),
    CHECK (finished_at IS NULL OR finished_at BETWEEN created_at AND updated_at),
    CHECK (started_at IS NULL OR finished_at IS NULL OR started_at <= finished_at)
) STRICT;

CREATE TABLE dynamic_memory_summary_checkpoints (
    cached_input_tokens INTEGER CHECK (cached_input_tokens IS NULL OR cached_input_tokens >= 0),
    reasoning_tokens INTEGER CHECK (reasoning_tokens IS NULL OR reasoning_tokens >= 0),
    cache_write_tokens INTEGER CHECK (cache_write_tokens IS NULL OR cache_write_tokens >= 0),
    web_search_requests INTEGER CHECK (web_search_requests IS NULL OR web_search_requests >= 0),
    run_id TEXT PRIMARY KEY REFERENCES dynamic_memory_runs(id) ON DELETE RESTRICT,
    attempt_id TEXT NOT NULL,
    space_id TEXT NOT NULL REFERENCES memory_spaces(id) ON DELETE RESTRICT,
    expected_memory_revision INTEGER NOT NULL CHECK (expected_memory_revision >= 1),
    resulting_memory_revision INTEGER NOT NULL CHECK (resulting_memory_revision = expected_memory_revision + 1),
    summary_text TEXT NOT NULL CHECK (
        length(trim(summary_text)) > 0
        AND length(CAST(summary_text AS BLOB)) <= 6000
    ),
    token_count INTEGER NOT NULL CHECK (token_count BETWEEN 0 AND 4294967295),
    request_context_json TEXT NOT NULL CHECK (
        json_valid(request_context_json)
        AND json_extract(request_context_json, '$.format_version') = 1
    ),
    input_tokens INTEGER CHECK (input_tokens >= 0),
    output_tokens INTEGER CHECK (output_tokens >= 0),
    provider_request_id TEXT CHECK (
        provider_request_id IS NULL OR (
            length(trim(provider_request_id)) > 0
            AND length(CAST(provider_request_id AS BLOB)) <= 256
        )
    ),
    settled_at INTEGER NOT NULL,
    FOREIGN KEY (run_id, attempt_id)
        REFERENCES dynamic_memory_run_attempts(run_id, id) ON DELETE RESTRICT,
    CHECK ((input_tokens IS NULL) = (output_tokens IS NULL))
) STRICT;

CREATE TRIGGER dynamic_memory_summary_checkpoints_no_update
BEFORE UPDATE ON dynamic_memory_summary_checkpoints
BEGIN
    SELECT RAISE(ABORT, 'dynamic memory summary checkpoints are immutable');
END;

CREATE TRIGGER dynamic_memory_summary_checkpoints_no_delete
BEFORE DELETE ON dynamic_memory_summary_checkpoints
BEGIN
    SELECT RAISE(ABORT, 'dynamic memory summary checkpoints are immutable');
END;

CREATE TABLE dynamic_memory_inference_rounds (
    cached_input_tokens INTEGER CHECK (cached_input_tokens IS NULL OR cached_input_tokens >= 0),
    reasoning_tokens INTEGER CHECK (reasoning_tokens IS NULL OR reasoning_tokens >= 0),
    cache_write_tokens INTEGER CHECK (cache_write_tokens IS NULL OR cache_write_tokens >= 0),
    web_search_requests INTEGER CHECK (web_search_requests IS NULL OR web_search_requests >= 0),
    run_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 7),
    first_call_ordinal INTEGER NOT NULL CHECK (first_call_ordinal BETWEEN 0 AND 64),
    call_count INTEGER NOT NULL CHECK (call_count BETWEEN 0 AND 64),
    request_context_json TEXT NOT NULL CHECK (
        json_valid(request_context_json)
        AND json_extract(request_context_json, '$.format_version') = 1
    ),
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
    PRIMARY KEY (run_id, attempt_id, ordinal),
    FOREIGN KEY (run_id, attempt_id)
        REFERENCES dynamic_memory_run_attempts(run_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (provider_replay_artifact_id, provider_replay_retention)
        REFERENCES conversation_replay_artifacts(artifact_id, retention) ON DELETE RESTRICT,
    CHECK (call_count > 0 OR json_array_length(json_extract(parts_json, '$.value')) > 0),
    CHECK (first_call_ordinal + call_count <= 64),
    CHECK ((input_tokens IS NULL) = (output_tokens IS NULL)),
    CHECK ((provider_replay_artifact_id IS NULL) = (provider_replay_retention IS NULL))
) STRICT;

CREATE TABLE dynamic_memory_admitted_tool_calls (
    run_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    round_ordinal INTEGER NOT NULL CHECK (round_ordinal BETWEEN 0 AND 7),
    id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 63),
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
    PRIMARY KEY (run_id, attempt_id, id),
    UNIQUE (run_id, attempt_id, ordinal),
    FOREIGN KEY (run_id, attempt_id, round_ordinal)
        REFERENCES dynamic_memory_inference_rounds(run_id, attempt_id, ordinal)
        ON DELETE RESTRICT,
    FOREIGN KEY (provider_replay_artifact_id, provider_replay_retention)
        REFERENCES conversation_replay_artifacts(artifact_id, retention) ON DELETE RESTRICT,
    CHECK ((provider_replay_artifact_id IS NULL) = (provider_replay_retention IS NULL))
) STRICT;

CREATE UNIQUE INDEX dynamic_memory_admitted_tool_calls_provider_id_uq
    ON dynamic_memory_admitted_tool_calls(run_id, attempt_id, provider_call_id)
    WHERE provider_call_id IS NOT NULL;

CREATE TABLE dynamic_memory_background_round_settlements (
    run_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    round_ordinal INTEGER NOT NULL CHECK (round_ordinal BETWEEN 0 AND 7),
    space_id TEXT NOT NULL REFERENCES memory_spaces(id) ON DELETE RESTRICT,
    expected_memory_revision INTEGER NOT NULL CHECK (expected_memory_revision >= 1),
    resulting_memory_revision INTEGER NOT NULL CHECK (resulting_memory_revision >= expected_memory_revision),
    change_digest TEXT NOT NULL CHECK (
        length(change_digest) = 64
        AND lower(change_digest) = change_digest
        AND change_digest NOT GLOB '*[^0-9a-f]*'
    ),
    settled_at INTEGER NOT NULL,
    PRIMARY KEY (run_id, attempt_id, round_ordinal),
    FOREIGN KEY (run_id, attempt_id, round_ordinal)
        REFERENCES dynamic_memory_inference_rounds(run_id, attempt_id, ordinal)
        ON DELETE RESTRICT
) STRICT;

CREATE TABLE dynamic_memory_background_tool_results (
    run_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    round_ordinal INTEGER NOT NULL CHECK (round_ordinal BETWEEN 0 AND 7),
    call_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 63),
    outcome_json TEXT NOT NULL CHECK (
        json_valid(outcome_json)
        AND json_extract(outcome_json, '$.format_version') = 1
    ),
    settled_at INTEGER NOT NULL,
    PRIMARY KEY (run_id, attempt_id, call_id),
    UNIQUE (run_id, attempt_id, ordinal),
    FOREIGN KEY (run_id, attempt_id, round_ordinal)
        REFERENCES dynamic_memory_background_round_settlements(run_id, attempt_id, round_ordinal)
        ON DELETE RESTRICT,
    FOREIGN KEY (run_id, attempt_id, call_id)
        REFERENCES dynamic_memory_admitted_tool_calls(run_id, attempt_id, id)
        ON DELETE RESTRICT
) STRICT;

CREATE TRIGGER dynamic_memory_background_settlement_immutable
BEFORE UPDATE ON dynamic_memory_background_round_settlements
BEGIN SELECT RAISE(ABORT, 'dynamic-memory background settlement is immutable'); END;

CREATE TRIGGER dynamic_memory_background_settlement_delete_restricted
BEFORE DELETE ON dynamic_memory_background_round_settlements
BEGIN SELECT RAISE(ABORT, 'dynamic-memory background settlement cannot be deleted'); END;

CREATE TRIGGER dynamic_memory_background_result_immutable
BEFORE UPDATE ON dynamic_memory_background_tool_results
BEGIN SELECT RAISE(ABORT, 'dynamic-memory background result is immutable'); END;

CREATE TRIGGER dynamic_memory_background_result_delete_restricted
BEFORE DELETE ON dynamic_memory_background_tool_results
BEGIN SELECT RAISE(ABORT, 'dynamic-memory background result cannot be deleted'); END;

CREATE TRIGGER dynamic_memory_run_immutable_update
BEFORE UPDATE ON dynamic_memory_runs
BEGIN SELECT RAISE(ABORT, 'dynamic-memory run is immutable'); END;

CREATE TRIGGER dynamic_memory_run_space_guard
BEFORE INSERT ON dynamic_memory_runs
WHEN NOT EXISTS (
    SELECT 1 FROM conversation_memory_spaces binding
    WHERE binding.conversation_id = NEW.conversation_id AND binding.space_id = NEW.space_id
)
BEGIN SELECT RAISE(ABORT, 'dynamic-memory run memory-space ownership mismatch'); END;

CREATE TRIGGER dynamic_memory_run_immutable_delete
BEFORE DELETE ON dynamic_memory_runs
BEGIN SELECT RAISE(ABORT, 'dynamic-memory run cannot be deleted'); END;

CREATE TRIGGER dynamic_memory_run_source_immutable_update
BEFORE UPDATE ON dynamic_memory_run_source_messages
BEGIN SELECT RAISE(ABORT, 'dynamic-memory source window is immutable'); END;

CREATE TRIGGER dynamic_memory_run_source_active_render_guard
BEFORE INSERT ON dynamic_memory_run_source_messages
WHEN NOT EXISTS (
    SELECT 1 FROM conversation_messages message
    WHERE message.conversation_id = NEW.conversation_id
      AND message.id = NEW.message_id
      AND message.role = NEW.role
      AND message.active_revision_id IS NEW.revision_id
      AND message.active_candidate_id IS NEW.candidate_id
      AND message.visibility = 'visible'
)
BEGIN SELECT RAISE(ABORT, 'dynamic-memory source requires the visible active render source'); END;

CREATE TRIGGER dynamic_memory_run_source_immutable_delete
BEFORE DELETE ON dynamic_memory_run_source_messages
BEGIN SELECT RAISE(ABORT, 'dynamic-memory source window cannot be deleted'); END;

CREATE TRIGGER dynamic_memory_attempt_retry_guard
BEFORE INSERT ON dynamic_memory_run_attempts
WHEN NEW.retry_parent_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM dynamic_memory_run_attempts parent
    WHERE parent.run_id = NEW.run_id
      AND parent.id = NEW.retry_parent_id
      AND parent.ordinal + 1 = NEW.ordinal
      AND parent.status IN ('failed', 'cancelled', 'interrupted')
      AND parent.job_id != NEW.job_id
)
BEGIN SELECT RAISE(ABORT, 'invalid dynamic-memory attempt retry'); END;

CREATE TRIGGER dynamic_memory_attempt_identity_immutable
BEFORE UPDATE OF run_id, id, ordinal, retry_parent_id, job_id
ON dynamic_memory_run_attempts
BEGIN SELECT RAISE(ABORT, 'dynamic-memory attempt identity is immutable'); END;

CREATE TRIGGER dynamic_memory_attempt_terminal_immutable
BEFORE UPDATE ON dynamic_memory_run_attempts
WHEN OLD.status IN ('succeeded', 'failed', 'cancelled', 'interrupted')
BEGIN SELECT RAISE(ABORT, 'terminal dynamic-memory attempt is immutable'); END;

CREATE TRIGGER dynamic_memory_attempt_transition_guard
BEFORE UPDATE OF status ON dynamic_memory_run_attempts
WHEN NEW.status != OLD.status AND NOT (
    (OLD.status = 'created' AND NEW.status IN ('processing', 'cancelled')) OR
    (OLD.status = 'processing' AND NEW.status IN ('succeeded', 'failed', 'cancelled', 'interrupted'))
)
BEGIN SELECT RAISE(ABORT, 'invalid dynamic-memory attempt transition'); END;

CREATE TRIGGER dynamic_memory_attempt_revision_guard
BEFORE UPDATE ON dynamic_memory_run_attempts
WHEN NEW.revision != OLD.revision + 1 OR NEW.updated_at < OLD.updated_at
BEGIN SELECT RAISE(ABORT, 'invalid dynamic-memory attempt revision'); END;

CREATE TRIGGER dynamic_memory_round_live_attempt_guard
BEFORE INSERT ON dynamic_memory_inference_rounds
WHEN NOT EXISTS (
    SELECT 1 FROM dynamic_memory_run_attempts attempt
    WHERE attempt.run_id = NEW.run_id
      AND attempt.id = NEW.attempt_id
      AND attempt.status = 'processing'
)
BEGIN SELECT RAISE(ABORT, 'dynamic-memory round requires a processing attempt'); END;

CREATE TRIGGER dynamic_memory_round_ordinal_guard
BEFORE INSERT ON dynamic_memory_inference_rounds
WHEN NEW.ordinal != (
    SELECT coalesce(max(ordinal) + 1, 0)
    FROM dynamic_memory_inference_rounds
    WHERE run_id = NEW.run_id AND attempt_id = NEW.attempt_id
) OR NEW.first_call_ordinal != (
    SELECT coalesce(sum(call_count), 0)
    FROM dynamic_memory_inference_rounds
    WHERE run_id = NEW.run_id AND attempt_id = NEW.attempt_id
)
BEGIN SELECT RAISE(ABORT, 'dynamic-memory round ordinal mismatch'); END;

CREATE TRIGGER dynamic_memory_round_immutable_update
BEFORE UPDATE ON dynamic_memory_inference_rounds
BEGIN SELECT RAISE(ABORT, 'dynamic-memory inference round is immutable'); END;

CREATE TRIGGER dynamic_memory_round_immutable_delete
BEFORE DELETE ON dynamic_memory_inference_rounds
BEGIN SELECT RAISE(ABORT, 'dynamic-memory inference round cannot be deleted'); END;

CREATE TRIGGER dynamic_memory_tool_call_contract_guard
BEFORE INSERT ON dynamic_memory_admitted_tool_calls
WHEN NOT EXISTS (
    SELECT 1
    FROM dynamic_memory_run_attempts attempt
    JOIN dynamic_memory_runs run ON run.id = attempt.run_id,
         json_each(json_extract(run.tool_request_json, '$.value.definitions')) definition
    WHERE attempt.run_id = NEW.run_id
      AND attempt.id = NEW.attempt_id
      AND attempt.status = 'processing'
      AND json_extract(definition.value, '$.name') = NEW.definition_name
      AND json_extract(definition.value, '$.version') = NEW.definition_version
)
BEGIN SELECT RAISE(ABORT, 'dynamic-memory tool call contract mismatch'); END;

CREATE TRIGGER dynamic_memory_tool_call_immutable_update
BEFORE UPDATE ON dynamic_memory_admitted_tool_calls
BEGIN SELECT RAISE(ABORT, 'dynamic-memory tool call is immutable'); END;

CREATE TRIGGER dynamic_memory_tool_call_immutable_delete
BEFORE DELETE ON dynamic_memory_admitted_tool_calls
BEGIN SELECT RAISE(ABORT, 'dynamic-memory tool call cannot be deleted'); END;
