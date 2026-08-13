CREATE TABLE provider_accounts (
    id TEXT PRIMARY KEY,
    provider_kind TEXT NOT NULL CHECK (length(trim(provider_kind)) > 0),
    protocol TEXT NOT NULL CHECK (protocol IN ('open_ai_compatible','anthropic','gemini','ollama','llama_cpp','stable_diffusion')),
    label TEXT NOT NULL CHECK (length(trim(label)) > 0),
    endpoint TEXT,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    api_key_secret_ref TEXT,
    secret_headers_json TEXT NOT NULL,
    config_json TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE model_profiles (
    id TEXT PRIMARY KEY,
    provider_account_id TEXT NOT NULL REFERENCES provider_accounts(id) ON DELETE RESTRICT,
    external_model_id TEXT NOT NULL CHECK (length(trim(external_model_id)) > 0),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) > 0),
    kind TEXT NOT NULL CHECK (kind IN ('chat','image','embedding','speech')),
    config_json TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE INDEX model_profiles_account_idx ON model_profiles(provider_account_id);

CREATE TABLE app_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    default_model_profile_id TEXT REFERENCES model_profiles(id) ON DELETE RESTRICT,
    format_version INTEGER NOT NULL CHECK (format_version >= 1),
    payload_json TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE media_blobs (
    id TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL UNIQUE CHECK (length(content_hash) = 64),
    kind TEXT NOT NULL CHECK (kind IN ('image','audio','video')),
    mime_type TEXT NOT NULL CHECK (length(trim(mime_type)) > 0),
    byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
    width INTEGER CHECK (width > 0),
    height INTEGER CHECK (height > 0),
    duration_ms INTEGER CHECK (duration_ms >= 0),
    validation_version INTEGER NOT NULL CHECK (validation_version >= 1),
    state TEXT NOT NULL CHECK (state IN ('staged','ready','quarantined','missing')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
