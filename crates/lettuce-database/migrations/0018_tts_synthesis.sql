CREATE TABLE speech_syntheses (
    job_id TEXT PRIMARY KEY REFERENCES jobs(id) ON DELETE RESTRICT,
    request_id TEXT NOT NULL UNIQUE,
    provider_id TEXT NOT NULL,
    output_asset_id TEXT NOT NULL UNIQUE,
    output_retention TEXT NOT NULL CHECK (output_retention IN ('temporary', 'persistent')),
    output_expires_at INTEGER,
    admitted_at INTEGER NOT NULL,
    request_json TEXT NOT NULL CHECK (
        json_valid(request_json)
        AND json_extract(request_json, '$.format_version') = 1
    ),
    result_json TEXT CHECK (
        result_json IS NULL OR (
            json_valid(result_json)
            AND json_extract(result_json, '$.format_version') = 1
        )
    ),
    result_asset_id TEXT REFERENCES media_assets(id) ON DELETE RESTRICT,
    completed_at INTEGER,
    CHECK ((output_retention = 'temporary') = (output_expires_at IS NOT NULL)),
    CHECK (
        (result_json IS NULL AND result_asset_id IS NULL AND completed_at IS NULL)
        OR (result_json IS NOT NULL AND result_asset_id IS NOT NULL AND completed_at IS NOT NULL)
    )
) STRICT;

CREATE TRIGGER speech_syntheses_insert_guard
BEFORE INSERT ON speech_syntheses
WHEN NOT EXISTS (
    SELECT 1
      FROM jobs
     WHERE id = NEW.job_id
       AND kind = 'speech_synthesize'
       AND subject_kind = 'speech_request'
       AND subject_id = NEW.request_id
)
BEGIN
    SELECT RAISE(ABORT, 'invalid speech synthesis binding');
END;

CREATE TRIGGER speech_syntheses_binding_immutable
BEFORE UPDATE OF job_id, request_id, provider_id, output_asset_id,
    output_retention, output_expires_at, admitted_at, request_json ON speech_syntheses
BEGIN
    SELECT RAISE(ABORT, 'speech synthesis binding is immutable');
END;

CREATE TRIGGER speech_syntheses_settle_once
BEFORE UPDATE OF result_json, result_asset_id, completed_at ON speech_syntheses
WHEN OLD.result_json IS NOT NULL
    OR NEW.result_json IS NULL
    OR NEW.result_asset_id <> OLD.output_asset_id
    OR NEW.completed_at IS NULL
    OR NOT EXISTS (
        SELECT 1
          FROM media_assets AS asset
          JOIN media_blobs AS blob ON blob.id = asset.blob_id
         WHERE asset.id = NEW.result_asset_id
           AND asset.blob_kind = 'audio'
           AND asset.kind = 'synthesized_speech'
           AND asset.origin = 'synthesized'
           AND asset.retention = OLD.output_retention
           AND asset.expires_at IS OLD.output_expires_at
           AND json_extract(asset.provenance_json, '$.producing_job_id') = OLD.job_id
           AND json_extract(NEW.result_json, '$.value.request_id') = OLD.request_id
           AND json_extract(NEW.result_json, '$.value.audio_asset_id') = OLD.output_asset_id
           AND json_extract(NEW.result_json, '$.value.content_hash') = blob.content_hash
           AND json_extract(NEW.result_json, '$.value.byte_size') = blob.byte_size
           AND json_extract(NEW.result_json, '$.value.mime_type') = blob.mime_type
           AND json_extract(NEW.result_json, '$.value.completed_at') = NEW.completed_at
    )
BEGIN
    SELECT RAISE(ABORT, 'speech synthesis can settle once with its admitted audio');
END;

CREATE TRIGGER speech_syntheses_no_delete
BEFORE DELETE ON speech_syntheses
BEGIN
    SELECT RAISE(ABORT, 'speech synthesis evidence is immutable');
END;
