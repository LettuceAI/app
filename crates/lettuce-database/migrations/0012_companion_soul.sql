CREATE TABLE companion_soul_states (
    character_id TEXT PRIMARY KEY REFERENCES characters(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at)
) STRICT;

CREATE TABLE companion_soul_facts (
    character_id TEXT NOT NULL REFERENCES companion_soul_states(character_id) ON DELETE CASCADE,
    id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    category TEXT NOT NULL CHECK (category IN (
        'essence', 'traits', 'backstory', 'appearance', 'goals', 'likes', 'voice',
        'relationalStyle', 'vulnerabilities', 'fears', 'habits', 'boundaries'
    )),
    value TEXT NOT NULL CHECK (length(trim(value)) > 0),
    kind TEXT NOT NULL CHECK (kind IN ('add', 'adjust', 'authored', 'consolidated')),
    policy TEXT NOT NULL CHECK (policy IN ('current', 'adaptive', 'historical')),
    slot TEXT NOT NULL CHECK (length(trim(slot)) > 0),
    confidence REAL NOT NULL CHECK (confidence BETWEEN 0.0 AND 1.0),
    evidence_count INTEGER NOT NULL CHECK (evidence_count >= 0),
    weight REAL NOT NULL CHECK (weight BETWEEN 0.0 AND 1.0),
    valid_from INTEGER NOT NULL,
    valid_until INTEGER,
    locked INTEGER NOT NULL CHECK (locked IN (0, 1)),
    created_at INTEGER NOT NULL,
    superseded_by TEXT,
    superseded_at INTEGER,
    PRIMARY KEY (character_id, id),
    UNIQUE (character_id, ordinal),
    CHECK (valid_until IS NULL OR valid_until > valid_from),
    CHECK ((superseded_by IS NULL) = (superseded_at IS NULL))
) STRICT;

CREATE TABLE companion_soul_fact_sources (
    character_id TEXT NOT NULL,
    fact_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    memory_id TEXT NOT NULL CHECK (length(trim(memory_id)) > 0),
    PRIMARY KEY (character_id, fact_id, ordinal),
    FOREIGN KEY (character_id, fact_id)
        REFERENCES companion_soul_facts(character_id, id) ON DELETE CASCADE
) STRICT;

CREATE TABLE companion_soul_fact_supersedes (
    character_id TEXT NOT NULL,
    fact_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    superseded_fact_id TEXT NOT NULL CHECK (length(trim(superseded_fact_id)) > 0),
    PRIMARY KEY (character_id, fact_id, ordinal),
    FOREIGN KEY (character_id, fact_id)
        REFERENCES companion_soul_facts(character_id, id) ON DELETE CASCADE
) STRICT;

CREATE TABLE companion_soul_apply_receipts (
    operation_id TEXT PRIMARY KEY,
    character_id TEXT NOT NULL REFERENCES companion_soul_states(character_id) ON DELETE RESTRICT,
    expected_revision INTEGER NOT NULL CHECK (expected_revision >= 1),
    resulting_revision INTEGER NOT NULL CHECK (resulting_revision = expected_revision + 1),
    applied_at INTEGER NOT NULL,
    change_hash BLOB NOT NULL CHECK (length(change_hash) = 32)
) STRICT;

CREATE INDEX companion_soul_receipts_owner_idx
    ON companion_soul_apply_receipts(character_id, applied_at, operation_id);

CREATE TRIGGER companion_soul_receipts_immutable_update
BEFORE UPDATE ON companion_soul_apply_receipts
BEGIN
    SELECT RAISE(ABORT, 'companion soul apply receipts are immutable');
END;

CREATE TRIGGER companion_soul_receipts_immutable_delete
BEFORE DELETE ON companion_soul_apply_receipts
BEGIN
    SELECT RAISE(ABORT, 'companion soul apply receipts are immutable');
END;

CREATE TABLE companion_growth_runs (
    job_id TEXT PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    memory_run_id TEXT NOT NULL UNIQUE REFERENCES dynamic_memory_runs(id) ON DELETE RESTRICT,
    memory_attempt_id TEXT NOT NULL UNIQUE REFERENCES dynamic_memory_run_attempts(id) ON DELETE RESTRICT,
    operation_id TEXT NOT NULL UNIQUE,
    expected_soul_revision INTEGER NOT NULL CHECK (expected_soul_revision >= 1),
    created_at INTEGER NOT NULL,
    run_json TEXT NOT NULL CHECK (json_valid(run_json)),
    proposal_checkpoint_json TEXT CHECK (
        proposal_checkpoint_json IS NULL OR json_valid(proposal_checkpoint_json)
    ),
    reduced_at INTEGER,
    CHECK ((proposal_checkpoint_json IS NULL) = (reduced_at IS NULL))
) STRICT;

CREATE INDEX companion_growth_runs_character_idx
    ON companion_growth_runs(character_id, created_at, job_id);

CREATE TABLE companion_consolidation_runs (
    job_id TEXT PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE,
    growth_job_id TEXT NOT NULL UNIQUE,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    operation_id TEXT NOT NULL UNIQUE,
    expected_soul_revision INTEGER NOT NULL CHECK (expected_soul_revision >= 1),
    created_at INTEGER NOT NULL,
    run_json TEXT NOT NULL CHECK (json_valid(run_json)),
    proposal_checkpoint_json TEXT CHECK (
        proposal_checkpoint_json IS NULL OR json_valid(proposal_checkpoint_json)
    ),
    reduced_at INTEGER,
    CHECK ((proposal_checkpoint_json IS NULL) = (reduced_at IS NULL))
) STRICT;

CREATE INDEX companion_consolidation_runs_character_idx
    ON companion_consolidation_runs(character_id, created_at, job_id);

CREATE TABLE companion_soul_writer_runs (
    request_id TEXT PRIMARY KEY,
    prompt_id TEXT NOT NULL REFERENCES prompt_documents(id) ON DELETE RESTRICT,
    prompt_revision INTEGER NOT NULL CHECK (prompt_revision >= 1),
    created_at INTEGER NOT NULL,
    run_json TEXT NOT NULL CHECK (json_valid(run_json)),
    rounds_json TEXT NOT NULL CHECK (json_valid(rounds_json))
) STRICT;
