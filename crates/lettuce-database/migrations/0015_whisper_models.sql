CREATE TABLE installed_whisper_models (
    model_id TEXT PRIMARY KEY CHECK (
        length(model_id) BETWEEN 1 AND 128
        AND trim(model_id) = model_id
    ),
    source_revision TEXT NOT NULL CHECK (
        length(source_revision) BETWEEN 1 AND 128
        AND trim(source_revision) = source_revision
    ),
    model_path TEXT NOT NULL UNIQUE CHECK (
        length(model_path) BETWEEN 1 AND 4096
        AND trim(model_path) = model_path
    ),
    byte_size INTEGER NOT NULL CHECK (byte_size BETWEEN 1 AND 8589934592),
    blake3 TEXT NOT NULL CHECK (length(blake3) = 64),
    english_only INTEGER NOT NULL CHECK (english_only IN (0, 1)),
    quantized INTEGER NOT NULL CHECK (quantized IN (0, 1)),
    admitted_at INTEGER NOT NULL,
    manifest_json TEXT NOT NULL CHECK (
        json_valid(manifest_json)
        AND json_extract(manifest_json, '$.format_version') = 1
    )
) STRICT;

CREATE TRIGGER installed_whisper_models_immutable
BEFORE UPDATE ON installed_whisper_models
BEGIN
    SELECT RAISE(ABORT, 'installed Whisper manifest is immutable');
END;
