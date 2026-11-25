CREATE TABLE dynamic_memory_summary_checkpoints_text_limit (
    cached_input_tokens INTEGER CHECK (cached_input_tokens IS NULL OR cached_input_tokens >= 0),
    reasoning_tokens INTEGER CHECK (reasoning_tokens IS NULL OR reasoning_tokens >= 0),
    cache_write_tokens INTEGER CHECK (cache_write_tokens IS NULL OR cache_write_tokens >= 0),
    web_search_requests INTEGER CHECK (web_search_requests IS NULL OR web_search_requests >= 0),
    image_tokens INTEGER CHECK (image_tokens IS NULL OR image_tokens >= 0),
    audio_tokens INTEGER CHECK (audio_tokens IS NULL OR audio_tokens >= 0),
    total_tokens INTEGER CHECK (total_tokens IS NULL OR total_tokens >= 0),
    provider_reported_cost REAL CHECK (provider_reported_cost IS NULL OR (provider_reported_cost >= 0 AND provider_reported_cost <= 1.7976931348623157e308)),
    run_id TEXT PRIMARY KEY REFERENCES dynamic_memory_runs(id) ON DELETE RESTRICT,
    attempt_id TEXT NOT NULL,
    space_id TEXT NOT NULL REFERENCES memory_spaces(id) ON DELETE RESTRICT,
    expected_memory_revision INTEGER NOT NULL CHECK (expected_memory_revision >= 1),
    resulting_memory_revision INTEGER NOT NULL CHECK (resulting_memory_revision = expected_memory_revision + 1),
    summary_text TEXT NOT NULL CHECK (
        length(trim(summary_text)) > 0
        AND length(CAST(summary_text AS BLOB)) <= 8388608
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

INSERT INTO dynamic_memory_summary_checkpoints_text_limit (
    "cached_input_tokens",
    "reasoning_tokens",
    "cache_write_tokens",
    "web_search_requests",
    "image_tokens",
    "audio_tokens",
    "total_tokens",
    "provider_reported_cost",
    "run_id",
    "attempt_id",
    "space_id",
    "expected_memory_revision",
    "resulting_memory_revision",
    "summary_text",
    "token_count",
    "request_context_json",
    "input_tokens",
    "output_tokens",
    "provider_request_id",
    "settled_at"
)
SELECT
    "cached_input_tokens",
    "reasoning_tokens",
    "cache_write_tokens",
    "web_search_requests",
    "image_tokens",
    "audio_tokens",
    "total_tokens",
    "provider_reported_cost",
    "run_id",
    "attempt_id",
    "space_id",
    "expected_memory_revision",
    "resulting_memory_revision",
    "summary_text",
    "token_count",
    "request_context_json",
    "input_tokens",
    "output_tokens",
    "provider_request_id",
    "settled_at"
FROM dynamic_memory_summary_checkpoints;

DROP TABLE dynamic_memory_summary_checkpoints;

ALTER TABLE dynamic_memory_summary_checkpoints_text_limit RENAME TO dynamic_memory_summary_checkpoints;

CREATE TRIGGER dynamic_memory_summary_checkpoints_no_update
BEFORE UPDATE ON dynamic_memory_summary_checkpoints
BEGIN
    SELECT RAISE(ABORT, 'dynamic memory summary checkpoints are immutable');
END;

CREATE TRIGGER dynamic_memory_summary_checkpoints_no_delete
BEFORE DELETE ON dynamic_memory_summary_checkpoints
WHEN NOT EXISTS (SELECT 1 FROM purge_authorizations WHERE owner_id IN (SELECT conversation_id FROM dynamic_memory_runs WHERE id = OLD.run_id UNION ALL SELECT space_id FROM dynamic_memory_runs WHERE id = OLD.run_id))
BEGIN
    SELECT RAISE(ABORT, 'dynamic memory summary checkpoints are immutable');
END;
