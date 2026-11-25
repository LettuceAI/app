-- Allow linked Google Gemini Audio providers in audio_providers.
-- The migration runner disables foreign keys only for this table rebuild and
-- checks all relationships before committing, then restores enforcement.

CREATE TABLE audio_providers_gemini_audio (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    secret_owner_id TEXT NOT NULL UNIQUE CHECK (length(secret_owner_id) = 36),
    provider_kind TEXT NOT NULL CHECK (provider_kind IN (
        'gemini_tts', 'elevenlabs', 'fish_tts', 'fish_speech', 'open_ai_tts',
        'openrouter_tts', 'nanogpt_tts', 'gemini_audio', 'kokoro'
    )),
    label TEXT NOT NULL CHECK (
        length(label) BETWEEN 1 AND 65536
        AND trim(label) = label
        AND instr(label, char(0)) = 0
    ),
    api_key_secret_ref TEXT UNIQUE CHECK (
        api_key_secret_ref IS NULL OR length(api_key_secret_ref) = 36
    ),
    config_json TEXT NOT NULL CHECK (
        json_valid(config_json)
        AND json_extract(config_json, '$.format_version') = 1
    ),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL CHECK (updated_at >= created_at)
) STRICT;

INSERT INTO audio_providers_gemini_audio ("id", "secret_owner_id", "provider_kind", "label", "api_key_secret_ref", "config_json", "revision", "created_at", "updated_at")
    SELECT "id", "secret_owner_id", "provider_kind", "label", "api_key_secret_ref", "config_json", "revision", "created_at", "updated_at" FROM audio_providers;

DROP TABLE audio_providers;

ALTER TABLE audio_providers_gemini_audio RENAME TO audio_providers;

CREATE INDEX audio_providers_display
ON audio_providers(created_at DESC, id DESC);

CREATE TRIGGER audio_providers_revision_step
BEFORE UPDATE ON audio_providers
WHEN NEW.revision <> OLD.revision + 1
  OR NEW.updated_at < OLD.updated_at
BEGIN
    SELECT RAISE(ABORT, 'audio provider revision is invalid');
END;

CREATE TRIGGER audio_providers_stable_identity
BEFORE UPDATE ON audio_providers
WHEN NEW.id <> OLD.id
  OR NEW.secret_owner_id <> OLD.secret_owner_id
  OR NEW.created_at <> OLD.created_at
BEGIN
    SELECT RAISE(ABORT, 'audio provider identity is immutable');
END;

CREATE TRIGGER sync_deleted_audio_providers
AFTER DELETE ON audio_providers
BEGIN
    INSERT INTO sync_deleted_entities (entity_kind, entity_id, deleted_at)
    VALUES ('audio_provider', OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))
    ON CONFLICT(entity_kind, entity_id) DO NOTHING;
END;