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
    samples_json TEXT NOT NULL CHECK (json_valid(samples_json) AND json_type(samples_json) = 'array'),
    message_stats_only INTEGER NOT NULL DEFAULT 0 CHECK (message_stats_only IN (0, 1))
) STRICT;

CREATE INDEX llm_generation_metrics_created_at_idx ON llm_generation_metrics(created_at);

CREATE TRIGGER llm_generation_metrics_drop_stats_of_deleted_candidates
AFTER DELETE ON conversation_message_candidates
BEGIN
    DELETE FROM llm_generation_metrics
    WHERE id = OLD.attempt_id AND message_stats_only = 1;
END;

CREATE TRIGGER llm_generation_metrics_drop_stats_of_tombstoned_messages
AFTER UPDATE OF visibility ON conversation_messages
WHEN NEW.visibility = 'tombstoned' AND OLD.visibility <> 'tombstoned'
BEGIN
    DELETE FROM llm_generation_metrics
    WHERE message_stats_only = 1
      AND id IN (
          SELECT attempt_id FROM conversation_message_candidates
          WHERE conversation_id = NEW.conversation_id AND message_id = NEW.id
      );
END;

CREATE TABLE local_model_jobs (
    job_id TEXT PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE,
    detail_json TEXT NOT NULL CHECK (json_valid(detail_json) AND json_type(detail_json) = 'object'),
    result_json TEXT CHECK (result_json IS NULL OR (json_valid(result_json) AND json_type(result_json) = 'object')),
    failure_json TEXT CHECK (failure_json IS NULL OR (json_valid(failure_json) AND json_type(failure_json) = 'object'))
) STRICT;

CREATE TABLE local_model_operations (
    operation_key TEXT PRIMARY KEY CHECK (length(trim(operation_key)) > 0),
    request_digest TEXT NOT NULL CHECK (length(request_digest) > 0),
    job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX local_model_operations_job_idx ON local_model_operations(job_id);

CREATE TABLE hugging_face_job_refusals (
    job_id TEXT PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE,
    repository TEXT NOT NULL CHECK (length(trim(repository)) > 0)
) STRICT;
