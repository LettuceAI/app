ALTER TABLE creation_turns ADD COLUMN inputs_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(inputs_json));

CREATE TABLE creation_generated_images (
    attempt_id TEXT NOT NULL REFERENCES creation_inference_attempts(id) ON DELETE RESTRICT,
    call_ordinal INTEGER NOT NULL CHECK (call_ordinal >= 0 AND call_ordinal < 65536),
    workflow_id TEXT NOT NULL REFERENCES creation_workflows(id) ON DELETE RESTRICT,
    asset_id TEXT,
    prompt TEXT NOT NULL,
    source_asset_id TEXT,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (attempt_id, call_ordinal)
) STRICT;

CREATE INDEX creation_generated_images_workflow ON creation_generated_images(workflow_id);
