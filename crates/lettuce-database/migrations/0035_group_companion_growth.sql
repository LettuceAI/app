CREATE TABLE companion_growth_runs_group_companion_growth (
    job_id TEXT PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    memory_run_id TEXT NOT NULL REFERENCES dynamic_memory_runs(id) ON DELETE RESTRICT,
    memory_attempt_id TEXT NOT NULL REFERENCES dynamic_memory_run_attempts(id) ON DELETE RESTRICT,
    operation_id TEXT NOT NULL UNIQUE,
    expected_soul_revision INTEGER NOT NULL CHECK (expected_soul_revision >= 1),
    created_at INTEGER NOT NULL,
    run_json TEXT NOT NULL CHECK (json_valid(run_json)),
    proposal_checkpoint_json TEXT CHECK (
        proposal_checkpoint_json IS NULL OR json_valid(proposal_checkpoint_json)
    ),
    reduced_at INTEGER,
    UNIQUE (memory_run_id, character_id),
    UNIQUE (memory_attempt_id, character_id),
    CHECK ((proposal_checkpoint_json IS NULL) = (reduced_at IS NULL))
) STRICT;

INSERT INTO companion_growth_runs_group_companion_growth (
    job_id, conversation_id, character_id, memory_run_id, memory_attempt_id,
    operation_id, expected_soul_revision, created_at, run_json,
    proposal_checkpoint_json, reduced_at
)
SELECT job_id, conversation_id, character_id, memory_run_id, memory_attempt_id,
       operation_id, expected_soul_revision, created_at, run_json,
       proposal_checkpoint_json, reduced_at
  FROM companion_growth_runs;

DROP TABLE companion_growth_runs;

ALTER TABLE companion_growth_runs_group_companion_growth RENAME TO companion_growth_runs;

CREATE INDEX companion_growth_runs_character_idx
    ON companion_growth_runs(character_id, created_at, job_id);
