CREATE TABLE dynamic_memory_duplicate_consolidations (
    run_id TEXT PRIMARY KEY REFERENCES dynamic_memory_runs(id) ON DELETE CASCADE,
    phase_json TEXT NOT NULL
);
