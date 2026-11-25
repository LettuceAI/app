CREATE TABLE memory_summary_units (
    id TEXT PRIMARY KEY,
    space_id TEXT NOT NULL REFERENCES memory_spaces(id) ON DELETE RESTRICT,
    conversation_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    created_by_run_id TEXT NOT NULL REFERENCES dynamic_memory_runs(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK (kind IN ('cycle', 'chapter', 'seed', 'reset')),
    level INTEGER NOT NULL CHECK (level >= 0),
    origin TEXT NOT NULL CHECK (origin IN ('user', 'model', 'import')),
    window_start INTEGER NOT NULL CHECK (window_start >= 0),
    window_end INTEGER NOT NULL CHECK (window_end >= window_start),
    text TEXT NOT NULL CHECK (length(CAST(text AS BLOB)) <= 8388608),
    token_count INTEGER NOT NULL CHECK (token_count BETWEEN 0 AND 4294967295),
    created_at INTEGER NOT NULL,
    CHECK ((kind = 'reset') = (length(trim(text)) = 0)),
    CHECK (kind = 'chapter' OR level = 0),
    CHECK (kind <> 'chapter' OR level >= 1),
    FOREIGN KEY (conversation_id, branch_id) REFERENCES conversation_branches(conversation_id, id) ON DELETE RESTRICT,
    UNIQUE (created_by_run_id, kind, level, window_start, window_end)
) STRICT;

CREATE INDEX memory_summary_units_scope
    ON memory_summary_units(space_id, conversation_id, branch_id, window_start);

CREATE TABLE memory_summary_unit_inputs (
    parent_id TEXT NOT NULL REFERENCES memory_summary_units(id) ON DELETE CASCADE,
    child_id TEXT NOT NULL REFERENCES memory_summary_units(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (parent_id, child_id),
    UNIQUE (parent_id, ordinal)
) STRICT;

CREATE INDEX memory_summary_unit_inputs_child ON memory_summary_unit_inputs(child_id);
