CREATE TABLE llama_runtime_reports (
    model_profile_id TEXT PRIMARY KEY REFERENCES model_profiles(id) ON DELETE CASCADE,
    model_path TEXT NOT NULL CHECK (length(trim(model_path)) > 0),
    report_json TEXT NOT NULL CHECK (json_valid(report_json) AND json_type(report_json) = 'object'),
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE llm_generation_metrics (
    id TEXT PRIMARY KEY CHECK (length(trim(id)) > 0),
    created_at INTEGER NOT NULL,
    model_path TEXT,
    summary_json TEXT NOT NULL CHECK (json_valid(summary_json) AND json_type(summary_json) = 'object'),
    samples_json TEXT NOT NULL CHECK (json_valid(samples_json) AND json_type(samples_json) = 'array')
) STRICT;

CREATE INDEX llm_generation_metrics_created_at_idx ON llm_generation_metrics(created_at);
