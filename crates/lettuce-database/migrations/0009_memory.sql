CREATE TABLE memory_spaces (
    id TEXT PRIMARY KEY,
    revision INTEGER NOT NULL CHECK (revision >= 1)
) STRICT;

CREATE TABLE conversation_memory_spaces (
    conversation_id TEXT PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
    space_id TEXT NOT NULL UNIQUE REFERENCES memory_spaces(id) ON DELETE RESTRICT
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
    CHECK (NOT (is_pinned = 1 AND is_cold = 1))
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
