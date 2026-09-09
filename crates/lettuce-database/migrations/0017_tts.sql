CREATE TABLE audio_providers (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    secret_owner_id TEXT NOT NULL UNIQUE CHECK (length(secret_owner_id) = 36),
    provider_kind TEXT NOT NULL CHECK (provider_kind IN (
        'gemini_tts', 'elevenlabs', 'fish_tts', 'fish_speech', 'open_ai_tts', 'kokoro'
    )),
    label TEXT NOT NULL CHECK (
        length(label) BETWEEN 1 AND 256
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

CREATE INDEX audio_providers_display
ON audio_providers(created_at DESC, id DESC);

CREATE TABLE user_voices (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    provider_id TEXT NOT NULL,
    name TEXT NOT NULL CHECK (
        length(name) BETWEEN 1 AND 256
        AND trim(name) = name
        AND instr(name, char(0)) = 0
    ),
    model_id TEXT NOT NULL CHECK (
        length(model_id) BETWEEN 1 AND 4096
        AND trim(model_id) = model_id
        AND instr(model_id, char(0)) = 0
    ),
    voice_id TEXT NOT NULL CHECK (
        length(voice_id) BETWEEN 1 AND 4096
        AND trim(voice_id) = voice_id
        AND instr(voice_id, char(0)) = 0
    ),
    prompt TEXT CHECK (
        prompt IS NULL
        OR (length(CAST(prompt AS BLOB)) <= 16384 AND instr(prompt, char(0)) = 0)
    ),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL CHECK (updated_at >= created_at),
    FOREIGN KEY(provider_id) REFERENCES audio_providers(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX user_voices_display
ON user_voices(created_at DESC, id DESC);

CREATE INDEX user_voices_provider
ON user_voices(provider_id, created_at DESC, id DESC);

CREATE TABLE discovered_tts_voices (
    provider_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    voice_id TEXT NOT NULL CHECK (
        length(CAST(voice_id AS BLOB)) BETWEEN 1 AND 4096
        AND instr(voice_id, char(0)) = 0
    ),
    name TEXT NOT NULL CHECK (
        length(CAST(name AS BLOB)) BETWEEN 1 AND 256
        AND instr(name, char(0)) = 0
    ),
    preview_url TEXT CHECK (
        preview_url IS NULL
        OR (length(CAST(preview_url AS BLOB)) BETWEEN 1 AND 4096 AND instr(preview_url, char(0)) = 0)
    ),
    labels_json TEXT NOT NULL CHECK (
        json_valid(labels_json)
        AND json_extract(labels_json, '$.format_version') = 1
        AND json_type(labels_json, '$.value') = 'object'
    ),
    cached_at INTEGER NOT NULL,
    PRIMARY KEY(provider_id, voice_id),
    UNIQUE(provider_id, ordinal),
    FOREIGN KEY(provider_id) REFERENCES audio_providers(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX discovered_tts_voices_order
ON discovered_tts_voices(provider_id, ordinal);

CREATE TRIGGER audio_providers_stable_identity
BEFORE UPDATE ON audio_providers
WHEN NEW.id <> OLD.id
  OR NEW.secret_owner_id <> OLD.secret_owner_id
  OR NEW.created_at <> OLD.created_at
BEGIN
    SELECT RAISE(ABORT, 'audio provider identity is immutable');
END;

CREATE TRIGGER audio_providers_revision_step
BEFORE UPDATE ON audio_providers
WHEN NEW.revision <> OLD.revision + 1
  OR NEW.updated_at < OLD.updated_at
BEGIN
    SELECT RAISE(ABORT, 'audio provider revision is invalid');
END;

CREATE TRIGGER user_voices_stable_identity
BEFORE UPDATE ON user_voices
WHEN NEW.id <> OLD.id OR NEW.created_at <> OLD.created_at
BEGIN
    SELECT RAISE(ABORT, 'user voice identity is immutable');
END;

CREATE TRIGGER user_voices_revision_step
BEFORE UPDATE ON user_voices
WHEN NEW.revision <> OLD.revision + 1
  OR NEW.updated_at < OLD.updated_at
BEGIN
    SELECT RAISE(ABORT, 'user voice revision is invalid');
END;
