CREATE TABLE prompt_documents (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL CHECK (status IN ('active', 'archived')),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    purpose TEXT NOT NULL CHECK (purpose IN (
        'direct_chat', 'companion_chat', 'group_chat_roleplay',
        'group_chat_conversational', 'dynamic_memory_summarizer',
        'dynamic_memory_manager', 'reply_helper_roleplay',
        'reply_helper_conversational', 'lorebook_entry_writer',
        'lorebook_keyword_generator', 'lorebook_generator_planner',
        'lorebook_generator_writer', 'lorebook_generator_refine',
        'lorebook_generator_coherence', 'avatar_generation',
        'avatar_edit_request', 'scene_generation', 'scene_prompt_writer',
        'design_reference_writer', 'companion_soul_writer',
        'companion_growthcycle', 'companion_consolidation'
    )),
    condense INTEGER NOT NULL CHECK (condense IN (0, 1)),
    behavior_version TEXT NOT NULL CHECK (behavior_version IN ('legacy_v1', 'deterministic_v2')),
    provenance_kind TEXT NOT NULL CHECK (provenance_kind IN ('built_in', 'user', 'derived', 'imported')),
    built_in_key TEXT UNIQUE,
    derived_source_id TEXT REFERENCES prompt_documents(id) ON DELETE RESTRICT,
    provenance_json TEXT NOT NULL CHECK (length(trim(provenance_json)) > 0),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at),
    CHECK ((provenance_kind = 'built_in') = (built_in_key IS NOT NULL)),
    CHECK ((provenance_kind = 'derived') = (derived_source_id IS NOT NULL)),
    CHECK (provenance_kind <> 'built_in' OR length(trim(built_in_key)) > 0),
    CHECK (derived_source_id IS NULL OR derived_source_id <> id),
    UNIQUE (id, revision)
) STRICT;
CREATE INDEX prompt_documents_library_order_idx ON prompt_documents(updated_at DESC, id ASC);
CREATE INDEX prompt_documents_purpose_idx ON prompt_documents(purpose, updated_at DESC, id ASC);

CREATE TABLE prompt_entries (
    id TEXT PRIMARY KEY,
    prompt_id TEXT NOT NULL REFERENCES prompt_documents(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    role TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant')),
    content TEXT NOT NULL,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    injection_position TEXT NOT NULL CHECK (injection_position IN ('relative', 'in_chat', 'conditional', 'interval')),
    depth INTEGER NOT NULL CHECK (depth >= 0),
    conditional_min_messages INTEGER CHECK (conditional_min_messages IS NULL OR conditional_min_messages >= 0),
    interval_turns INTEGER CHECK (interval_turns IS NULL OR interval_turns >= 0),
    system_prompt INTEGER NOT NULL CHECK (system_prompt IN (0, 1)),
    conditions_json TEXT,
    payload_json TEXT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at),
    UNIQUE (prompt_id, ordinal)
) STRICT;
CREATE INDEX prompt_entries_prompt_idx ON prompt_entries(prompt_id, ordinal, id);

ALTER TABLE app_settings ADD COLUMN default_prompt_document_id TEXT REFERENCES prompt_documents(id) ON DELETE RESTRICT;

CREATE TABLE lorebooks (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL CHECK (status IN ('active', 'archived')),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    detection_policy TEXT NOT NULL CHECK (detection_policy IN ('recent_message_window', 'latest_user_message')),
    icon_asset_id TEXT,
    icon_blob_kind TEXT NOT NULL DEFAULT 'image' CHECK (icon_blob_kind = 'image'),
    behavior_version TEXT NOT NULL CHECK (behavior_version IN ('legacy_v1', 'deterministic_v2')),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at),
    FOREIGN KEY (icon_asset_id, icon_blob_kind) REFERENCES media_assets(id, blob_kind) ON DELETE RESTRICT
) STRICT;
CREATE INDEX lorebooks_library_order_idx ON lorebooks(updated_at DESC, id ASC);

CREATE TABLE lorebook_entries (
    id TEXT PRIMARY KEY,
    lorebook_id TEXT NOT NULL REFERENCES lorebooks(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    always_active INTEGER NOT NULL CHECK (always_active IN (0, 1)),
    keywords_json TEXT NOT NULL CHECK (length(trim(keywords_json)) > 0),
    case_sensitive INTEGER NOT NULL CHECK (case_sensitive IN (0, 1)),
    match_mode TEXT NOT NULL CHECK (match_mode IN ('literal', 'regex')),
    content TEXT NOT NULL,
    priority INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at),
    UNIQUE (lorebook_id, ordinal)
) STRICT;
CREATE INDEX lorebook_entries_book_idx ON lorebook_entries(lorebook_id, ordinal, id);

CREATE TABLE character_lorebook_bindings (
    character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    lorebook_id TEXT NOT NULL REFERENCES lorebooks(id) ON DELETE RESTRICT,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at),
    PRIMARY KEY (character_id, lorebook_id),
    UNIQUE (character_id, ordinal)
) STRICT;
CREATE INDEX character_lorebook_bindings_book_idx
    ON character_lorebook_bindings(lorebook_id, character_id);
CREATE TABLE persona_lorebook_bindings (
    persona_id TEXT NOT NULL REFERENCES personas(id) ON DELETE CASCADE,
    lorebook_id TEXT NOT NULL REFERENCES lorebooks(id) ON DELETE RESTRICT,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at),
    PRIMARY KEY (persona_id, lorebook_id),
    UNIQUE (persona_id, ordinal)
) STRICT;
CREATE INDEX persona_lorebook_bindings_book_idx
    ON persona_lorebook_bindings(lorebook_id, persona_id);

CREATE TABLE legacy_import_results (
    run_id TEXT PRIMARY KEY REFERENCES legacy_import_runs(id) ON DELETE RESTRICT,
    plan_fingerprint TEXT NOT NULL CHECK (length(plan_fingerprint) = 64),
    persona_count INTEGER NOT NULL CHECK (persona_count >= 0),
    lorebook_count INTEGER NOT NULL CHECK (lorebook_count >= 0),
    lorebook_entry_count INTEGER NOT NULL CHECK (lorebook_entry_count >= 0),
    completed_at INTEGER NOT NULL
) STRICT;

CREATE TRIGGER legacy_import_results_insert_guard
BEFORE INSERT ON legacy_import_results
WHEN NOT EXISTS (
    SELECT 1 FROM legacy_import_runs
    WHERE id = NEW.run_id
      AND plan_fingerprint = NEW.plan_fingerprint
      AND status = 'importing'
)
BEGIN
    SELECT RAISE(ABORT, 'legacy import result is invalid');
END;

CREATE TRIGGER legacy_import_results_update_forbidden
BEFORE UPDATE ON legacy_import_results
BEGIN
    SELECT RAISE(ABORT, 'legacy import result is immutable');
END;

CREATE TRIGGER legacy_import_results_delete_forbidden
BEFORE DELETE ON legacy_import_results
BEGIN
    SELECT RAISE(ABORT, 'legacy import result is immutable');
END;

CREATE TABLE legacy_import_provider_model_results (
    run_id TEXT PRIMARY KEY REFERENCES legacy_import_runs(id) ON DELETE RESTRICT,
    plan_fingerprint TEXT NOT NULL CHECK (length(plan_fingerprint) = 64),
    provider_account_count INTEGER NOT NULL CHECK (provider_account_count >= 0),
    model_profile_count INTEGER NOT NULL CHECK (model_profile_count >= 0),
    prompt_count INTEGER NOT NULL CHECK (prompt_count >= 0),
    completed_at INTEGER NOT NULL
) STRICT;

CREATE TRIGGER legacy_import_provider_model_results_insert_guard
BEFORE INSERT ON legacy_import_provider_model_results
WHEN NOT EXISTS (
    SELECT 1 FROM legacy_import_runs AS run
    JOIN legacy_import_results AS graph ON graph.run_id = run.id
    WHERE run.id = NEW.run_id
      AND run.plan_fingerprint = NEW.plan_fingerprint
      AND graph.plan_fingerprint = NEW.plan_fingerprint
      AND run.status = 'importing'
      AND NEW.provider_account_count = (
          SELECT count(*) FROM legacy_import_assignments
          WHERE run_id = NEW.run_id AND source_kind = 'provider_account'
      )
      AND NEW.model_profile_count = (
          SELECT count(*) FROM legacy_import_assignments
          WHERE run_id = NEW.run_id AND source_kind = 'model_profile'
      )
      AND NEW.prompt_count = (
          SELECT count(*) FROM legacy_import_assignments
          WHERE run_id = NEW.run_id AND source_kind = 'prompt'
      )
      AND NOT EXISTS (
          SELECT 1 FROM legacy_import_assignments AS assignment
          LEFT JOIN legacy_import_secret_completions AS completion
            ON completion.run_id = assignment.run_id
           AND completion.source_kind = assignment.source_kind
           AND completion.source_key = assignment.source_key
           AND completion.source_detail = assignment.source_detail
           AND completion.destination_ref = assignment.destination_id
          WHERE assignment.run_id = NEW.run_id
            AND assignment.source_kind IN ('provider_api_key','provider_secret_header')
            AND completion.run_id IS NULL
      )
)
BEGIN
    SELECT RAISE(ABORT, 'legacy provider model result is invalid');
END;

CREATE TRIGGER legacy_import_provider_model_results_update_forbidden
BEFORE UPDATE ON legacy_import_provider_model_results
BEGIN
    SELECT RAISE(ABORT, 'legacy provider model result is immutable');
END;

CREATE TRIGGER legacy_import_provider_model_results_delete_forbidden
BEFORE DELETE ON legacy_import_provider_model_results
BEGIN
    SELECT RAISE(ABORT, 'legacy provider model result is immutable');
END;
CREATE TABLE group_lorebook_bindings (
    group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    lorebook_id TEXT NOT NULL REFERENCES lorebooks(id) ON DELETE RESTRICT,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at),
    PRIMARY KEY (group_id, lorebook_id),
    UNIQUE (group_id, ordinal)
) STRICT;
CREATE INDEX group_lorebook_bindings_book_idx
    ON group_lorebook_bindings(lorebook_id, group_id);
