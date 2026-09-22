CREATE TABLE image_generations (
    job_id TEXT PRIMARY KEY REFERENCES jobs(id) ON DELETE RESTRICT,
    request_id TEXT NOT NULL UNIQUE,
    model_profile_id TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('direct', 'scene', 'playground', 'creation_helper')),
    admitted_at INTEGER NOT NULL,
    request_json TEXT NOT NULL CHECK (
        json_valid(request_json)
        AND json_extract(request_json, '$.format_version') = 1
    ),
    state TEXT NOT NULL CHECK (state IN ('pending', 'succeeded', 'failed', 'cancelled')),
    state_json TEXT NOT NULL CHECK (
        json_valid(state_json)
        AND json_extract(state_json, '$.format_version') = 1
        AND json_extract(state_json, '$.value.state') = state
    ),
    completed_at INTEGER,
    CHECK ((state = 'pending') = (completed_at IS NULL))
) STRICT;

CREATE INDEX image_generations_source_idx
    ON image_generations(source, admitted_at, job_id);

CREATE TABLE image_generation_outputs (
    job_id TEXT NOT NULL REFERENCES image_generations(job_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    asset_id TEXT NOT NULL REFERENCES media_assets(id) ON DELETE RESTRICT,
    PRIMARY KEY (job_id, ordinal)
) STRICT;

CREATE INDEX image_generation_outputs_asset_idx ON image_generation_outputs(asset_id);

CREATE TRIGGER image_generation_outputs_insert_guard
BEFORE INSERT ON image_generation_outputs
WHEN NOT EXISTS (
    SELECT 1
      FROM image_generations
     WHERE job_id = NEW.job_id
       AND state = 'succeeded'
       AND json_extract(state_json, '$.value.result.images[' || NEW.ordinal || '].asset_id')
           = NEW.asset_id
)
BEGIN
    SELECT RAISE(ABORT, 'image generation outputs must match its settled result');
END;

CREATE TRIGGER image_generation_outputs_immutable
BEFORE UPDATE ON image_generation_outputs
BEGIN
    SELECT RAISE(ABORT, 'image generation outputs are immutable');
END;

CREATE TRIGGER image_generations_insert_guard
BEFORE INSERT ON image_generations
WHEN NEW.state != 'pending' OR NOT EXISTS (
    SELECT 1
      FROM jobs
     WHERE id = NEW.job_id
       AND kind = 'image_generate'
       AND subject_kind = 'image_request'
       AND subject_id = NEW.request_id
)
BEGIN
    SELECT RAISE(ABORT, 'invalid image generation binding');
END;

CREATE TRIGGER image_generations_binding_immutable
BEFORE UPDATE OF job_id, request_id, model_profile_id, source, admitted_at, request_json
    ON image_generations
BEGIN
    SELECT RAISE(ABORT, 'image generation binding is immutable');
END;

CREATE TRIGGER image_generations_settle_once
BEFORE UPDATE OF state, state_json, completed_at ON image_generations
WHEN OLD.state != 'pending'
    OR NEW.state = 'pending'
    OR (NEW.state = 'succeeded' AND (
        json_array_length(NEW.state_json, '$.value.result.images') = 0
        OR json_extract(NEW.state_json, '$.value.result.request_id') != OLD.request_id
        OR EXISTS (
            SELECT 1
              FROM json_each(NEW.state_json, '$.value.result.images') AS image
             WHERE NOT EXISTS (
                SELECT 1
                  FROM media_assets AS asset
                 WHERE asset.id = json_extract(image.value, '$.asset_id')
                   AND asset.kind = 'generated_image'
                   AND asset.origin = 'generated'
                   AND json_extract(asset.provenance_json, '$.producing_job_id') = OLD.job_id
             )
        )
    ))
BEGIN
    SELECT RAISE(ABORT, 'image generation can settle once with its own generated images');
END;

CREATE TRIGGER image_generations_pending_no_delete
BEFORE DELETE ON image_generations
WHEN OLD.state = 'pending'
BEGIN
    SELECT RAISE(ABORT, 'a pending image generation cannot be deleted');
END;

CREATE TABLE image_loras (
    path TEXT PRIMARY KEY CHECK (length(path) > 0),
    filename TEXT NOT NULL,
    bytes_on_disk INTEGER NOT NULL CHECK (bytes_on_disk >= 0),
    modified_at INTEGER NOT NULL CHECK (modified_at >= 0),
    sha256 TEXT CHECK (sha256 IS NULL OR length(sha256) = 64),
    keywords TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(keywords) AND json_type(keywords) = 'array'),
    keyword_source TEXT NOT NULL DEFAULT 'none'
        CHECK (keyword_source IN ('none', 'metadata', 'civitai', 'manual')),
    architecture TEXT,
    architecture_source TEXT NOT NULL DEFAULT 'none'
        CHECK (architecture_source IN ('none', 'metadata', 'civitai')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE INDEX image_loras_sha256_idx ON image_loras(sha256) WHERE sha256 IS NOT NULL;

CREATE TABLE playground_history (
    id TEXT PRIMARY KEY,
    origin TEXT NOT NULL CHECK (origin IN ('generated', 'imported')),
    job_id TEXT,
    import_run_id TEXT,
    source_id TEXT,
    created_at INTEGER NOT NULL,
    provider_kind TEXT NOT NULL,
    source_model_id TEXT,
    model_profile_id TEXT,
    model_name TEXT NOT NULL,
    prompt TEXT NOT NULL,
    negative_prompt TEXT,
    seed INTEGER,
    params_json TEXT NOT NULL,
    status TEXT NOT NULL,
    error TEXT,
    CHECK ((origin = 'imported') = (import_run_id IS NOT NULL AND source_id IS NOT NULL)),
    UNIQUE (import_run_id, source_id)
) STRICT;

CREATE INDEX playground_history_created_idx ON playground_history(created_at, id);

CREATE TABLE playground_history_images (
    history_id TEXT NOT NULL REFERENCES playground_history(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    asset_id TEXT REFERENCES media_assets(id) ON DELETE RESTRICT,
    source_asset_id TEXT,
    mime_type TEXT,
    url TEXT,
    width INTEGER CHECK (width IS NULL OR width >= 0),
    height INTEGER CHECK (height IS NULL OR height >= 0),
    PRIMARY KEY (history_id, ordinal)
) STRICT;

CREATE INDEX playground_history_images_asset_idx ON playground_history_images(asset_id);
CREATE INDEX playground_history_images_source_idx ON playground_history_images(source_asset_id);
