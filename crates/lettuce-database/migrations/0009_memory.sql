CREATE TABLE memory_spaces (
    id TEXT PRIMARY KEY,
    revision INTEGER NOT NULL CHECK (revision >= 1)
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
