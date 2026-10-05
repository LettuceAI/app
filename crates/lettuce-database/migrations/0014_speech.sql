CREATE TABLE speech_transcriptions (
    job_id TEXT PRIMARY KEY REFERENCES jobs(id) ON DELETE RESTRICT,
    request_id TEXT NOT NULL UNIQUE,
    audio_asset_id TEXT REFERENCES media_assets(id) ON DELETE RESTRICT,
    model_id TEXT NOT NULL CHECK (
        length(model_id) BETWEEN 1 AND 128
        AND trim(model_id) = model_id
    ),
    model_artifact_hash TEXT NOT NULL CHECK (length(model_artifact_hash) = 64),
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
    completed_at INTEGER,
    CHECK ((result_json IS NULL) = (completed_at IS NULL))
) STRICT;

CREATE TRIGGER speech_transcriptions_insert_guard
BEFORE INSERT ON speech_transcriptions
WHEN NOT EXISTS (
    SELECT 1
      FROM jobs
     WHERE id = NEW.job_id
       AND kind = 'speech_transcribe'
       AND subject_kind = 'speech_request'
       AND subject_id = NEW.request_id
)
OR (NEW.audio_asset_id IS NOT NULL AND NOT EXISTS (
    SELECT 1
      FROM media_assets
     WHERE id = NEW.audio_asset_id
       AND blob_kind = 'audio'
       AND kind IN ('message_audio', 'other_audio')
))
OR (NEW.audio_asset_id IS NULL AND EXISTS (SELECT 1 FROM jobs WHERE id=NEW.job_id AND state NOT IN ('succeeded','failed','cancelled','interrupted')))
BEGIN
    SELECT RAISE(ABORT, 'invalid speech transcription binding');
END;

CREATE TRIGGER speech_transcriptions_binding_immutable
BEFORE UPDATE OF job_id, request_id, audio_asset_id, model_id,
    model_artifact_hash, admitted_at, request_json ON speech_transcriptions
WHEN NOT (
    OLD.audio_asset_id IS NOT NULL AND NEW.audio_asset_id IS NULL
    AND NEW.job_id = OLD.job_id AND NEW.request_id = OLD.request_id
    AND NEW.model_id = OLD.model_id AND NEW.model_artifact_hash = OLD.model_artifact_hash
    AND NEW.admitted_at = OLD.admitted_at
    AND NEW.request_json = json_set(OLD.request_json, '$.value.audio_asset_id', NULL)
    AND EXISTS (SELECT 1 FROM jobs WHERE id=NEW.job_id AND state IN ('succeeded','failed','cancelled','interrupted'))
)
BEGIN
    SELECT RAISE(ABORT, 'speech transcription binding is immutable');
END;

CREATE TRIGGER speech_transcriptions_settle_once
BEFORE UPDATE OF result_json, completed_at ON speech_transcriptions
WHEN (OLD.result_json IS NOT NULL OR NEW.result_json IS NULL OR NEW.completed_at IS NULL)
    AND NOT (
        OLD.result_json IS NOT NULL AND NEW.completed_at = OLD.completed_at
        AND NEW.result_json = json_set(OLD.result_json, '$.value.audio_asset_id', NULL)
        AND NEW.audio_asset_id IS NULL
        AND EXISTS (SELECT 1 FROM jobs WHERE id=NEW.job_id AND state IN ('succeeded','failed','cancelled','interrupted'))
    )
BEGIN
    SELECT RAISE(ABORT, 'speech transcription can settle once');
END;

CREATE TRIGGER speech_transcriptions_no_delete
BEFORE DELETE ON speech_transcriptions
BEGIN
    SELECT RAISE(ABORT, 'speech transcription evidence is immutable');
END;
