CREATE TABLE provider_accounts (
    id TEXT PRIMARY KEY,
    provider_kind TEXT NOT NULL CHECK (length(trim(provider_kind)) > 0 AND length(CAST(provider_kind AS BLOB)) <= 128),
    protocol TEXT NOT NULL CHECK (protocol IN ('open_ai_compatible','anthropic','gemini','ollama','llama_cpp','stable_diffusion')),
    label TEXT NOT NULL CHECK (length(trim(label)) > 0),
    endpoint TEXT,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    streaming_enabled INTEGER NOT NULL CHECK (streaming_enabled IN (0, 1)),
    allow_invalid_tls INTEGER NOT NULL CHECK (allow_invalid_tls IN (0, 1)),
    api_key_secret_ref TEXT,
    secret_owner_id TEXT NOT NULL UNIQUE,
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
    dynamic_memory_model_profile_id TEXT REFERENCES model_profiles(id) ON DELETE RESTRICT,
    group_speaker_model_profile_id TEXT REFERENCES model_profiles(id) ON DELETE RESTRICT,
    format_version INTEGER NOT NULL CHECK (format_version >= 1),
    payload_json TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE jobs (
    id TEXT PRIMARY KEY,
    idempotency_key TEXT UNIQUE,
    kind TEXT NOT NULL,
    subject_kind TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    state TEXT NOT NULL,
    priority TEXT NOT NULL,
    parent_id TEXT REFERENCES jobs(id) DEFERRABLE INITIALLY DEFERRED,
    lease_expires_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    spec_json TEXT NOT NULL,
    snapshot_json TEXT NOT NULL
) STRICT;

CREATE INDEX jobs_claim_idx ON jobs(state, priority, created_at, id);
CREATE INDEX jobs_subject_idx ON jobs(subject_kind, subject_id, created_at, id);
CREATE INDEX jobs_parent_idx ON jobs(parent_id);
CREATE INDEX jobs_lease_idx ON jobs(lease_expires_at) WHERE lease_expires_at IS NOT NULL;

CREATE TABLE job_events (
    job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    seq INTEGER NOT NULL CHECK (seq >= 1),
    at INTEGER NOT NULL,
    correlation_id TEXT NOT NULL,
    event_json TEXT NOT NULL,
    PRIMARY KEY (job_id, seq)
) STRICT;

CREATE TABLE media_blobs (
    id TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL UNIQUE CHECK (length(content_hash) = 64),
    kind TEXT NOT NULL CHECK (kind IN ('image','audio','video','document')),
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

CREATE TABLE legacy_import_runs (
    id TEXT PRIMARY KEY,
    source_schema_version INTEGER NOT NULL CHECK (source_schema_version > 0),
    inventory_fingerprint TEXT NOT NULL CHECK (length(inventory_fingerprint) = 64),
    plan_fingerprint TEXT NOT NULL CHECK (length(plan_fingerprint) = 64),
    status TEXT NOT NULL CHECK (status IN ('admitting','admitted','importing','completed','failed')),
    admitted_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE legacy_import_assignments (
    run_id TEXT NOT NULL REFERENCES legacy_import_runs(id) ON DELETE RESTRICT,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('persona','lorebook','lorebook_entry','media')),
    source_key TEXT NOT NULL CHECK (length(trim(source_key)) > 0),
    destination_id TEXT NOT NULL,
    expected_byte_len INTEGER,
    expected_content_hash TEXT,
    PRIMARY KEY (run_id, source_kind, source_key),
    UNIQUE (run_id, destination_id),
    CHECK (
        (source_kind = 'media' AND expected_byte_len >= 0 AND length(expected_content_hash) = 64) OR
        (source_kind <> 'media' AND expected_byte_len IS NULL AND expected_content_hash IS NULL)
    )
) STRICT;

CREATE TRIGGER legacy_import_runs_binding_immutable
BEFORE UPDATE OF id, source_schema_version, inventory_fingerprint, plan_fingerprint, admitted_at ON legacy_import_runs
BEGIN
    SELECT RAISE(ABORT, 'legacy import binding is immutable');
END;

CREATE TRIGGER legacy_import_runs_delete_forbidden
BEFORE DELETE ON legacy_import_runs
BEGIN
    SELECT RAISE(ABORT, 'legacy import evidence is immutable');
END;

CREATE TRIGGER legacy_import_runs_status_guard
BEFORE UPDATE OF status ON legacy_import_runs
WHEN NOT (
    (OLD.status = 'admitting' AND NEW.status = 'admitted') OR
    (OLD.status = 'admitted' AND NEW.status IN ('importing','failed')) OR
    (OLD.status = 'importing' AND NEW.status IN ('completed','failed'))
)
BEGIN
    SELECT RAISE(ABORT, 'invalid legacy import status transition');
END;

CREATE TRIGGER legacy_import_assignments_insert_guard
BEFORE INSERT ON legacy_import_assignments
WHEN (SELECT status FROM legacy_import_runs WHERE id = NEW.run_id) <> 'admitting'
BEGIN
    SELECT RAISE(ABORT, 'legacy import assignments are sealed');
END;

CREATE TRIGGER legacy_import_assignments_update_forbidden
BEFORE UPDATE ON legacy_import_assignments
BEGIN
    SELECT RAISE(ABORT, 'legacy import assignment is immutable');
END;

CREATE TRIGGER legacy_import_assignments_delete_forbidden
BEFORE DELETE ON legacy_import_assignments
BEGIN
    SELECT RAISE(ABORT, 'legacy import assignment is immutable');
END;
