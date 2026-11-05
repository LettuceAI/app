-- Widen stored content limits without changing rows, identities or relationships.
-- The migration runner disables foreign keys only for this table rebuild and
-- checks all relationships before committing, then restores enforcement.

-- audio_providers

CREATE TABLE audio_providers_content_limits (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    secret_owner_id TEXT NOT NULL UNIQUE CHECK (length(secret_owner_id) = 36),
    provider_kind TEXT NOT NULL CHECK (provider_kind IN (
        'gemini_tts', 'elevenlabs', 'fish_tts', 'fish_speech', 'open_ai_tts', 'kokoro'
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

INSERT INTO audio_providers_content_limits ("id", "secret_owner_id", "provider_kind", "label", "api_key_secret_ref", "config_json", "revision", "created_at", "updated_at")
    SELECT "id", "secret_owner_id", "provider_kind", "label", "api_key_secret_ref", "config_json", "revision", "created_at", "updated_at" FROM audio_providers;

DROP TABLE audio_providers;

ALTER TABLE audio_providers_content_limits RENAME TO audio_providers;

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



-- companion_turn_effects

CREATE TABLE companion_turn_effects_content_limits (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    user_message_id TEXT,
    assistant_message_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('processing', 'ready', 'failed')),
    summary TEXT CHECK (summary IS NULL OR length(CAST(summary AS BLOB)) <= 134217728),
    enqueued_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE (conversation_id, turn_id),
    UNIQUE (conversation_id, assistant_message_id),
    FOREIGN KEY (conversation_id, turn_id, id)
        REFERENCES companion_turn_effect_drafts(conversation_id, turn_id, effect_id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, user_message_id)
        REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, assistant_message_id)
        REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT,
    CHECK (created_at <= updated_at),
    CHECK (
        (status = 'processing' AND enqueued_at IS NULL) OR
        (status = 'ready' AND enqueued_at IS NOT NULL) OR
        status = 'failed'
    )
) STRICT;

INSERT INTO companion_turn_effects_content_limits ("id", "conversation_id", "turn_id", "user_message_id", "assistant_message_id", "status", "summary", "enqueued_at", "created_at", "updated_at")
    SELECT "id", "conversation_id", "turn_id", "user_message_id", "assistant_message_id", "status", "summary", "enqueued_at", "created_at", "updated_at" FROM companion_turn_effects;

DROP TABLE companion_turn_effects;

ALTER TABLE companion_turn_effects_content_limits RENAME TO companion_turn_effects;

CREATE TRIGGER companion_turn_effect_no_delete
BEFORE DELETE ON companion_turn_effects
WHEN NOT EXISTS (SELECT 1 FROM purge_authorizations WHERE owner_id IN (OLD.conversation_id))
BEGIN
    SELECT RAISE(ABORT, 'companion turn effect is immutable');
END;

CREATE TRIGGER companion_turn_effect_terminal_update
BEFORE UPDATE ON companion_turn_effects
WHEN OLD.status = 'ready'
  OR (OLD.status = 'failed'
      AND NOT (NEW.status = 'processing' AND NEW.summary IS NULL AND NEW.enqueued_at IS NULL))
BEGIN
    SELECT RAISE(ABORT, 'terminal companion turn effect is immutable');
END;

CREATE TRIGGER companion_turn_effects_signal_settled
AFTER UPDATE OF status ON companion_turn_effects
WHEN OLD.status = 'processing' AND NEW.status IN ('ready', 'failed')
BEGIN
    DELETE FROM message_signals
    WHERE kind = 'companion_effect_settled'
      AND conversation_id = NEW.conversation_id
      AND message_id = NEW.assistant_message_id;
    INSERT INTO message_signals (kind, conversation_id, message_id)
    VALUES ('companion_effect_settled', NEW.conversation_id, NEW.assistant_message_id);
END;



-- conversation_settings

CREATE TABLE conversation_settings_content_limits (
    conversation_id TEXT PRIMARY KEY REFERENCES conversations(id) ON DELETE RESTRICT,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    author_note TEXT,
    author_note_provenance TEXT NOT NULL CHECK (author_note_provenance IN ('launch_inherited', 'current_override', 'disabled')),
    memory_json TEXT CHECK (memory_json IS NULL OR (json_valid(memory_json) AND json_extract(memory_json, '$.format_version') = 1)),
    memory_provenance TEXT NOT NULL CHECK (memory_provenance IN ('launch_inherited', 'current_override', 'disabled')),
    model_override_json TEXT CHECK (model_override_json IS NULL OR (json_valid(model_override_json) AND json_extract(model_override_json, '$.format_version') = 1)),
    model_provenance TEXT NOT NULL CHECK (model_provenance IN ('launch_inherited', 'current_override', 'disabled')),
    voice_json TEXT CHECK (voice_json IS NULL OR (json_valid(voice_json) AND json_extract(voice_json, '$.format_version') = 1)),
    voice_provenance TEXT NOT NULL CHECK (voice_provenance IN ('launch_inherited', 'current_override', 'disabled')),
    prompt_json TEXT CHECK (prompt_json IS NULL OR (json_valid(prompt_json) AND json_extract(prompt_json, '$.format_version') = 1)),
    prompt_provenance TEXT NOT NULL CHECK (prompt_provenance IN ('launch_inherited', 'current_override', 'disabled')),
    lorebooks_json TEXT CHECK (lorebooks_json IS NULL OR (json_valid(lorebooks_json) AND json_extract(lorebooks_json, '$.format_version') = 1 AND json_type(json_extract(lorebooks_json, '$.value')) = 'array' AND json_array_length(json_extract(lorebooks_json, '$.value')) > 0)),
    lorebooks_provenance TEXT NOT NULL CHECK (lorebooks_provenance IN ('launch_inherited', 'current_override', 'disabled')),
    persona_json TEXT CHECK (persona_json IS NULL OR (json_valid(persona_json) AND json_extract(persona_json, '$.format_version') = 1)),
    persona_provenance TEXT NOT NULL CHECK (persona_provenance IN ('launch_inherited', 'current_override', 'disabled')),
    scene_json TEXT CHECK (scene_json IS NULL OR (json_valid(scene_json) AND json_extract(scene_json, '$.format_version') = 1)),
    scene_provenance TEXT NOT NULL CHECK (scene_provenance IN ('launch_inherited', 'current_override', 'disabled')),
    speaker_selection TEXT CHECK (speaker_selection IS NULL OR speaker_selection IN ('llm', 'heuristic', 'round_robin', 'director', 'director_action')),
    speaker_selection_provenance TEXT NOT NULL DEFAULT 'launch_inherited' CHECK (speaker_selection_provenance IN ('launch_inherited', 'current_override')),
    companion_clock_json TEXT CHECK (companion_clock_json IS NULL OR (json_valid(companion_clock_json) AND json_extract(companion_clock_json, '$.format_version') = 1)),
    model_settings_json TEXT CHECK (model_settings_json IS NULL OR (json_valid(model_settings_json) AND json_extract(model_settings_json, '$.format_version') = 1)),
    background_asset_id TEXT,
    background_blob_kind TEXT NOT NULL DEFAULT 'image' CHECK (background_blob_kind = 'image'),
    background_hidden INTEGER NOT NULL DEFAULT 0 CHECK (background_hidden IN (0, 1)),
    chat_mode TEXT CHECK (chat_mode IS NULL OR chat_mode IN ('conversation', 'roleplay')),
    disable_character_lorebooks INTEGER CHECK (disable_character_lorebooks IS NULL OR disable_character_lorebooks IN (0, 1)),
    roleplay_prompt_json TEXT CHECK (roleplay_prompt_json IS NULL OR (json_valid(roleplay_prompt_json) AND json_extract(roleplay_prompt_json, '$.format_version') = 1)),
    roleplay_prompt_provenance TEXT NOT NULL DEFAULT 'launch_inherited' CHECK (roleplay_prompt_provenance IN ('launch_inherited', 'current_override', 'disabled')),
    members_overridden INTEGER NOT NULL DEFAULT 0 CHECK (members_overridden IN (0, 1)),
    muted_overridden INTEGER NOT NULL DEFAULT 0 CHECK (muted_overridden IN (0, 1)),
    member_models_overridden INTEGER NOT NULL DEFAULT 0 CHECK (member_models_overridden IN (0, 1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL, disable_character_notes INTEGER CHECK (disable_character_notes IS NULL OR disable_character_notes IN (0, 1)),
    CHECK (author_note IS NULL OR (length(trim(author_note)) > 0 AND length(CAST(author_note AS BLOB)) <= 134217728)),
    CHECK ((author_note IS NOT NULL) = (author_note_provenance = 'current_override')),
    CHECK ((memory_json IS NOT NULL) = (memory_provenance = 'current_override')),
    CHECK ((model_override_json IS NOT NULL) = (model_provenance = 'current_override')),
    CHECK ((voice_json IS NOT NULL) = (voice_provenance = 'current_override')),
    CHECK ((prompt_json IS NOT NULL) = (prompt_provenance = 'current_override')),
    CHECK ((lorebooks_json IS NOT NULL) = (lorebooks_provenance = 'current_override')),
    CHECK ((persona_json IS NOT NULL) = (persona_provenance = 'current_override')),
    CHECK ((scene_json IS NOT NULL) = (scene_provenance = 'current_override')),
    CHECK ((speaker_selection IS NOT NULL) = (speaker_selection_provenance = 'current_override')),
    CHECK ((roleplay_prompt_json IS NOT NULL) = (roleplay_prompt_provenance = 'current_override')),
    CHECK (created_at <= updated_at),
    CHECK (background_hidden = 0 OR background_asset_id IS NULL),
    FOREIGN KEY (background_asset_id, background_blob_kind)
        REFERENCES media_assets(id, blob_kind) ON DELETE RESTRICT
) STRICT;

INSERT INTO conversation_settings_content_limits ("conversation_id", "revision", "author_note", "author_note_provenance", "memory_json", "memory_provenance", "model_override_json", "model_provenance", "voice_json", "voice_provenance", "prompt_json", "prompt_provenance", "lorebooks_json", "lorebooks_provenance", "persona_json", "persona_provenance", "scene_json", "scene_provenance", "speaker_selection", "speaker_selection_provenance", "companion_clock_json", "model_settings_json", "background_asset_id", "background_blob_kind", "background_hidden", "chat_mode", "disable_character_lorebooks", "roleplay_prompt_json", "roleplay_prompt_provenance", "members_overridden", "muted_overridden", "member_models_overridden", "created_at", "updated_at", "disable_character_notes")
    SELECT "conversation_id", "revision", "author_note", "author_note_provenance", "memory_json", "memory_provenance", "model_override_json", "model_provenance", "voice_json", "voice_provenance", "prompt_json", "prompt_provenance", "lorebooks_json", "lorebooks_provenance", "persona_json", "persona_provenance", "scene_json", "scene_provenance", "speaker_selection", "speaker_selection_provenance", "companion_clock_json", "model_settings_json", "background_asset_id", "background_blob_kind", "background_hidden", "chat_mode", "disable_character_lorebooks", "roleplay_prompt_json", "roleplay_prompt_provenance", "members_overridden", "muted_overridden", "member_models_overridden", "created_at", "updated_at", "disable_character_notes" FROM conversation_settings;

DROP TABLE conversation_settings;

ALTER TABLE conversation_settings_content_limits RENAME TO conversation_settings;



-- conversation_turns

CREATE TABLE conversation_turns_content_limits (
    conversation_id TEXT NOT NULL,
    id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    operation TEXT NOT NULL CHECK (operation IN ('send', 'continue', 'regenerate')),
    input_kind TEXT NOT NULL CHECK (input_kind IN ('user_message', 'existing_head', 'existing_candidate')),
    user_message_id TEXT,
    head_message_id TEXT,
    candidate_message_id TEXT,
    candidate_id TEXT,
    idempotency_key TEXT NOT NULL CHECK (length(trim(idempotency_key)) > 0),
    correlation_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('created', 'preparing', 'selecting_speaker', 'context_prepared', 'running', 'cancellation_requested', 'finalizing', 'succeeded', 'failed', 'cancelled', 'interrupted', 'recovering')),
    selected_speaker_participant_id TEXT,
    -- This is an identity-free, versioned stored DTO.  It intentionally does
    -- not duplicate participant identity; selected_speaker_participant_id is
    -- the authoritative relational owner for that identity.
    selected_speaker_details_json TEXT CHECK (selected_speaker_details_json IS NULL OR (json_valid(selected_speaker_details_json) AND json_extract(selected_speaker_details_json, '$.format_version') = 1)),
    resolved_model_json TEXT CHECK (resolved_model_json IS NULL OR (json_valid(resolved_model_json) AND json_extract(resolved_model_json, '$.format_version') = 1)),
    prompt_document_id TEXT,
    prompt_name TEXT CHECK (prompt_name IS NULL OR length(trim(prompt_name)) > 0),
    prompt_revision INTEGER CHECK (prompt_revision IS NULL OR prompt_revision >= 1),
    prompt_entry_ids_json TEXT CHECK (prompt_entry_ids_json IS NULL OR (
        json_valid(prompt_entry_ids_json)
        AND json_extract(prompt_entry_ids_json, '$.format_version') = 1
        AND json_type(prompt_entry_ids_json, '$.value') = 'array'
    )),
    memory_revision_id TEXT,
    selected_candidate_id TEXT,
    failure TEXT CHECK (failure IS NULL OR failure IN ('invalid_conversation', 'missing_model', 'context_unavailable', 'speaker_unavailable', 'provider_unavailable', 'provider_rejected', 'empty_output', 'cancelled', 'timed_out', 'recovery_unavailable', 'embedding_unavailable', 'internal')),
    target_kind TEXT NOT NULL CHECK (target_kind IN ('new_assistant', 'existing_candidate')),
    target_message_id TEXT NOT NULL CHECK (length(trim(target_message_id)) > 0),
    target_parent_message_id TEXT,
    target_prior_candidate_id TEXT,
    retry_of_turn_id TEXT,
    guidance TEXT CHECK (guidance IS NULL OR (length(trim(guidance)) > 0 AND length(CAST(guidance AS BLOB)) <= 134217728)),
    requested_model_override_json TEXT CHECK (requested_model_override_json IS NULL OR (json_valid(requested_model_override_json) AND json_extract(requested_model_override_json, '$.format_version') = 1)),
    forced_speaker_participant_id TEXT,
    swap_roles INTEGER NOT NULL DEFAULT 0 CHECK (swap_roles IN (0, 1)),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK ((selected_speaker_participant_id IS NULL) = (selected_speaker_details_json IS NULL)),
    PRIMARY KEY (conversation_id, id),
    UNIQUE (conversation_id, idempotency_key),
    UNIQUE (conversation_id, id, branch_id),
    FOREIGN KEY (conversation_id, branch_id)
        REFERENCES conversation_branches(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, id, selected_candidate_id)
        REFERENCES conversation_message_candidates(conversation_id, turn_id, id)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (conversation_id, user_message_id) REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, user_message_id, branch_id)
        REFERENCES conversation_messages(conversation_id, id, branch_id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, head_message_id) REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, candidate_message_id) REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, candidate_message_id, candidate_id, branch_id)
        REFERENCES conversation_message_candidates(conversation_id, message_id, id, branch_id)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (conversation_id, selected_speaker_participant_id) REFERENCES conversation_participants(conversation_id, id) ON DELETE RESTRICT,
    CHECK ((input_kind = 'user_message' AND user_message_id IS NOT NULL AND head_message_id IS NULL AND candidate_message_id IS NULL AND candidate_id IS NULL) OR
           (input_kind = 'existing_head' AND user_message_id IS NULL AND head_message_id IS NOT NULL AND candidate_message_id IS NULL AND candidate_id IS NULL) OR
           (input_kind = 'existing_candidate' AND user_message_id IS NULL AND head_message_id IS NULL AND candidate_message_id IS NOT NULL AND candidate_id IS NOT NULL)),
    CHECK ((operation = 'send' AND input_kind = 'user_message') OR (operation = 'continue' AND input_kind = 'existing_head') OR (operation = 'regenerate' AND input_kind = 'existing_candidate')),
    CHECK ((prompt_document_id IS NULL) = (prompt_revision IS NULL) AND
           (prompt_document_id IS NULL) = (prompt_entry_ids_json IS NULL) AND
           (prompt_document_id IS NULL) = (prompt_name IS NULL)),
    CHECK ((status = 'failed') = (failure IS NOT NULL)),
    CHECK ((status = 'succeeded') = (selected_candidate_id IS NOT NULL)),
    CHECK ((target_kind = 'new_assistant' AND target_parent_message_id IS NOT NULL AND target_prior_candidate_id IS NULL) OR
          (target_kind = 'existing_candidate' AND target_parent_message_id IS NULL AND target_prior_candidate_id IS NOT NULL)),
    CHECK (created_at <= updated_at)
) STRICT;

INSERT INTO conversation_turns_content_limits ("conversation_id", "id", "branch_id", "operation", "input_kind", "user_message_id", "head_message_id", "candidate_message_id", "candidate_id", "idempotency_key", "correlation_id", "status", "selected_speaker_participant_id", "selected_speaker_details_json", "resolved_model_json", "prompt_document_id", "prompt_name", "prompt_revision", "prompt_entry_ids_json", "memory_revision_id", "selected_candidate_id", "failure", "target_kind", "target_message_id", "target_parent_message_id", "target_prior_candidate_id", "retry_of_turn_id", "guidance", "requested_model_override_json", "forced_speaker_participant_id", "swap_roles", "revision", "created_at", "updated_at")
    SELECT "conversation_id", "id", "branch_id", "operation", "input_kind", "user_message_id", "head_message_id", "candidate_message_id", "candidate_id", "idempotency_key", "correlation_id", "status", "selected_speaker_participant_id", "selected_speaker_details_json", "resolved_model_json", "prompt_document_id", "prompt_name", "prompt_revision", "prompt_entry_ids_json", "memory_revision_id", "selected_candidate_id", "failure", "target_kind", "target_message_id", "target_parent_message_id", "target_prior_candidate_id", "retry_of_turn_id", "guidance", "requested_model_override_json", "forced_speaker_participant_id", "swap_roles", "revision", "created_at", "updated_at" FROM conversation_turns;

DROP TABLE conversation_turns;

ALTER TABLE conversation_turns_content_limits RENAME TO conversation_turns;

CREATE INDEX conversation_turns_id_idx ON conversation_turns(id);

CREATE INDEX conversation_turns_page_idx ON conversation_turns(conversation_id, created_at, id);

CREATE TRIGGER conversation_turn_final_insert_contract
BEFORE INSERT ON conversation_turns
WHEN (
    NEW.target_kind IS NULL OR NEW.target_message_id IS NULL OR length(trim(NEW.target_message_id)) = 0 OR
    (NEW.target_parent_message_id IS NOT NULL AND length(trim(NEW.target_parent_message_id)) = 0) OR
    (NEW.target_prior_candidate_id IS NOT NULL AND length(trim(NEW.target_prior_candidate_id)) = 0) OR
    (NEW.target_kind = 'new_assistant' AND EXISTS (
        SELECT 1 FROM conversation_messages
        WHERE conversation_id = NEW.conversation_id AND id = NEW.target_message_id
    )) OR
    (NEW.target_kind = 'new_assistant' AND (
        NEW.target_parent_message_id IS NULL OR NEW.target_prior_candidate_id IS NOT NULL OR
        (NEW.operation = 'send' AND (
            NEW.input_kind <> 'user_message' OR NEW.user_message_id IS NULL OR
            NEW.target_message_id = NEW.user_message_id OR
            NEW.target_parent_message_id <> NEW.user_message_id)) OR
        (NEW.operation = 'continue' AND (
            NEW.input_kind <> 'existing_head' OR NEW.head_message_id IS NULL OR
            NEW.target_message_id = NEW.head_message_id OR
            NEW.target_parent_message_id <> NEW.head_message_id)) OR
        NEW.operation NOT IN ('send', 'continue')
    )) OR
    (NEW.target_kind = 'existing_candidate' AND (
        NEW.target_parent_message_id IS NOT NULL OR NEW.target_prior_candidate_id IS NULL OR
        NEW.operation <> 'regenerate' OR NEW.input_kind <> 'existing_candidate' OR
        NEW.candidate_message_id IS NULL OR NEW.candidate_id IS NULL OR
        NEW.target_message_id <> NEW.candidate_message_id OR
        NEW.target_prior_candidate_id <> NEW.candidate_id
    )) OR
    (NEW.retry_of_turn_id IS NOT NULL AND (
        NEW.retry_of_turn_id = NEW.id OR NOT EXISTS (
            SELECT 1 FROM conversation_turns AS source
            WHERE source.conversation_id = NEW.conversation_id
              AND source.id = NEW.retry_of_turn_id
              AND source.branch_id = NEW.branch_id
              AND source.status IN ('failed', 'cancelled')
              AND source.operation = NEW.operation
              AND source.input_kind = NEW.input_kind
              AND coalesce(source.user_message_id, '') = coalesce(NEW.user_message_id, '')
              AND coalesce(source.head_message_id, '') = coalesce(NEW.head_message_id, '')
              AND coalesce(source.candidate_message_id, '') = coalesce(NEW.candidate_message_id, '')
              AND coalesce(source.candidate_id, '') = coalesce(NEW.candidate_id, '')
              AND source.target_kind = NEW.target_kind
              AND source.target_message_id = NEW.target_message_id
              AND coalesce(source.target_parent_message_id, '') = coalesce(NEW.target_parent_message_id, '')
              AND coalesce(source.target_prior_candidate_id, '') = coalesce(NEW.target_prior_candidate_id, '')
              AND coalesce(source.guidance, '') = coalesce(NEW.guidance, '')
              AND coalesce(source.requested_model_override_json, '') = coalesce(NEW.requested_model_override_json, '')
              AND coalesce(source.forced_speaker_participant_id, '') = coalesce(NEW.forced_speaker_participant_id, '')
              AND source.swap_roles = NEW.swap_roles
        )
    )) OR
    (NEW.forced_speaker_participant_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM conversations AS conversation
        JOIN conversation_participants AS participant
          ON participant.conversation_id = conversation.id
         AND participant.id = NEW.forced_speaker_participant_id
         AND participant.role = 'character'
        WHERE conversation.id = NEW.conversation_id
          AND conversation.kind = 'group'
    )) OR
    (NEW.swap_roles <> 0 AND EXISTS (
        SELECT 1 FROM conversations
        WHERE id = NEW.conversation_id AND kind = 'group'
    )) OR
    (NEW.operation = 'continue' AND (
        NEW.guidance IS NOT NULL OR NEW.requested_model_override_json IS NOT NULL
    )) OR
    (NEW.operation = 'regenerate' AND EXISTS (
        SELECT 1 FROM conversations WHERE id = NEW.conversation_id AND kind = 'group'
    ) AND NEW.selected_speaker_participant_id IS NOT NULL AND (
        (NEW.forced_speaker_participant_id IS NULL AND NOT EXISTS (
            SELECT 1 FROM conversation_messages AS target
            WHERE target.conversation_id = NEW.conversation_id
              AND target.id = NEW.target_message_id
              AND target.author_participant_id = NEW.selected_speaker_participant_id
        )) OR
        (NEW.forced_speaker_participant_id IS NOT NULL AND
            NEW.selected_speaker_participant_id <> NEW.forced_speaker_participant_id)
    ))
)
BEGIN SELECT RAISE(ABORT, 'final generation turn contract violation'); END;

CREATE TRIGGER conversation_turn_final_intent_immutable
BEFORE UPDATE OF conversation_id, id, branch_id, operation, input_kind,
    user_message_id, head_message_id, candidate_message_id, candidate_id,
    target_kind, target_message_id, target_parent_message_id,
    target_prior_candidate_id, retry_of_turn_id, guidance,
    requested_model_override_json, forced_speaker_participant_id, swap_roles
ON conversation_turns
WHEN (
    NEW.conversation_id <> OLD.conversation_id OR NEW.id <> OLD.id OR
    NEW.branch_id <> OLD.branch_id OR NEW.operation <> OLD.operation OR
    NEW.input_kind <> OLD.input_kind OR coalesce(NEW.user_message_id, '') <> coalesce(OLD.user_message_id, '') OR
    coalesce(NEW.head_message_id, '') <> coalesce(OLD.head_message_id, '') OR
    coalesce(NEW.candidate_message_id, '') <> coalesce(OLD.candidate_message_id, '') OR
    coalesce(NEW.candidate_id, '') <> coalesce(OLD.candidate_id, '') OR
    coalesce(NEW.target_kind, '') <> coalesce(OLD.target_kind, '') OR
    coalesce(NEW.target_message_id, '') <> coalesce(OLD.target_message_id, '') OR
    coalesce(NEW.target_parent_message_id, '') <> coalesce(OLD.target_parent_message_id, '') OR
    coalesce(NEW.target_prior_candidate_id, '') <> coalesce(OLD.target_prior_candidate_id, '') OR
    coalesce(NEW.retry_of_turn_id, '') <> coalesce(OLD.retry_of_turn_id, '') OR
    coalesce(NEW.guidance, '') <> coalesce(OLD.guidance, '') OR
    coalesce(NEW.requested_model_override_json, '') <> coalesce(OLD.requested_model_override_json, '') OR
    coalesce(NEW.forced_speaker_participant_id, '') <> coalesce(OLD.forced_speaker_participant_id, '') OR
    NEW.swap_roles <> OLD.swap_roles
)
BEGIN SELECT RAISE(ABORT, 'generation turn request intent is immutable'); END;

CREATE TRIGGER conversation_turn_final_update_contract
BEFORE UPDATE OF conversation_id, id, branch_id, operation, input_kind,
    user_message_id, head_message_id, candidate_message_id, candidate_id,
    target_kind, target_message_id, target_parent_message_id,
    target_prior_candidate_id, retry_of_turn_id, guidance,
    requested_model_override_json, forced_speaker_participant_id,
    swap_roles ON conversation_turns
WHEN (
    NEW.target_kind IS NULL OR NEW.target_message_id IS NULL OR length(trim(NEW.target_message_id)) = 0 OR
    (NEW.target_parent_message_id IS NOT NULL AND length(trim(NEW.target_parent_message_id)) = 0) OR
    (NEW.target_prior_candidate_id IS NOT NULL AND length(trim(NEW.target_prior_candidate_id)) = 0) OR
    (NEW.target_kind = 'new_assistant' AND (
        NEW.target_parent_message_id IS NULL OR NEW.target_prior_candidate_id IS NOT NULL OR
        (NEW.operation = 'send' AND (
            NEW.input_kind <> 'user_message' OR NEW.user_message_id IS NULL OR
            NEW.target_message_id = NEW.user_message_id OR
            NEW.target_parent_message_id <> NEW.user_message_id)) OR
        (NEW.operation = 'continue' AND (
            NEW.input_kind <> 'existing_head' OR NEW.head_message_id IS NULL OR
            NEW.target_message_id = NEW.head_message_id OR
            NEW.target_parent_message_id <> NEW.head_message_id)) OR
        NEW.operation NOT IN ('send', 'continue')
    )) OR
    (NEW.target_kind = 'existing_candidate' AND (
        NEW.target_parent_message_id IS NOT NULL OR NEW.target_prior_candidate_id IS NULL OR
        NEW.operation <> 'regenerate' OR NEW.input_kind <> 'existing_candidate' OR
        NEW.candidate_message_id IS NULL OR NEW.candidate_id IS NULL OR
        NEW.target_message_id <> NEW.candidate_message_id OR
        NEW.target_prior_candidate_id <> NEW.candidate_id
    )) OR
    (NEW.retry_of_turn_id IS NOT NULL AND (
        NEW.retry_of_turn_id = NEW.id OR NOT EXISTS (
            SELECT 1 FROM conversation_turns AS source
            WHERE source.conversation_id = NEW.conversation_id
              AND source.id = NEW.retry_of_turn_id
              AND source.branch_id = NEW.branch_id
              AND source.status IN ('failed', 'cancelled')
              AND source.operation = NEW.operation
              AND source.input_kind = NEW.input_kind
              AND coalesce(source.user_message_id, '') = coalesce(NEW.user_message_id, '')
              AND coalesce(source.head_message_id, '') = coalesce(NEW.head_message_id, '')
              AND coalesce(source.candidate_message_id, '') = coalesce(NEW.candidate_message_id, '')
              AND coalesce(source.candidate_id, '') = coalesce(NEW.candidate_id, '')
              AND source.target_kind = NEW.target_kind
              AND source.target_message_id = NEW.target_message_id
              AND coalesce(source.target_parent_message_id, '') = coalesce(NEW.target_parent_message_id, '')
              AND coalesce(source.target_prior_candidate_id, '') = coalesce(NEW.target_prior_candidate_id, '')
              AND coalesce(source.guidance, '') = coalesce(NEW.guidance, '')
              AND coalesce(source.requested_model_override_json, '') = coalesce(NEW.requested_model_override_json, '')
              AND coalesce(source.forced_speaker_participant_id, '') = coalesce(NEW.forced_speaker_participant_id, '')
              AND source.swap_roles = NEW.swap_roles
        )
    )) OR
    (NEW.forced_speaker_participant_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM conversations AS conversation
        JOIN conversation_participants AS participant
          ON participant.conversation_id = conversation.id
         AND participant.id = NEW.forced_speaker_participant_id
         AND participant.role = 'character'
        WHERE conversation.id = NEW.conversation_id
          AND conversation.kind = 'group'
    )) OR
    (NEW.swap_roles <> 0 AND EXISTS (
        SELECT 1 FROM conversations
        WHERE id = NEW.conversation_id AND kind = 'group'
    )) OR
    (NEW.operation = 'continue' AND (
        NEW.guidance IS NOT NULL OR NEW.requested_model_override_json IS NOT NULL
    )) OR
    (NEW.operation = 'regenerate' AND EXISTS (
        SELECT 1 FROM conversations WHERE id = NEW.conversation_id AND kind = 'group'
    ) AND NEW.selected_speaker_participant_id IS NOT NULL AND (
        (NEW.forced_speaker_participant_id IS NULL AND NOT EXISTS (
            SELECT 1 FROM conversation_messages AS target
            WHERE target.conversation_id = NEW.conversation_id
              AND target.id = NEW.target_message_id
              AND target.author_participant_id = NEW.selected_speaker_participant_id
        )) OR
        (NEW.forced_speaker_participant_id IS NOT NULL AND
            NEW.selected_speaker_participant_id <> NEW.forced_speaker_participant_id)
    ))
)
BEGIN SELECT RAISE(ABORT, 'final generation turn contract violation'); END;

CREATE TRIGGER conversation_turn_input_insert
BEFORE INSERT ON conversation_turns
WHEN (NEW.input_kind = 'user_message' AND NOT EXISTS (
          SELECT 1 FROM conversation_messages
          WHERE conversation_id = NEW.conversation_id
            AND id = NEW.user_message_id
            AND branch_id = NEW.branch_id
            AND role = 'user'
      )) OR
     (NEW.input_kind = 'existing_head' AND NOT EXISTS (
          SELECT 1 FROM conversation_branches
          WHERE conversation_id = NEW.conversation_id
            AND id = NEW.branch_id
            AND COALESCE(head_message_id, fork_message_id) = NEW.head_message_id
      ))
BEGIN SELECT RAISE(ABORT, 'turn input does not match branch semantics'); END;

CREATE TRIGGER conversation_turn_input_update
BEFORE UPDATE OF branch_id, input_kind, user_message_id, head_message_id, candidate_message_id, candidate_id
ON conversation_turns
WHEN (NEW.input_kind = 'user_message' AND NOT EXISTS (
          SELECT 1 FROM conversation_messages
          WHERE conversation_id = NEW.conversation_id
            AND id = NEW.user_message_id
            AND branch_id = NEW.branch_id
            AND role = 'user'
      )) OR
     (NEW.input_kind = 'existing_head' AND NOT EXISTS (
          SELECT 1 FROM conversation_branches
          WHERE conversation_id = NEW.conversation_id
            AND id = NEW.branch_id
            AND COALESCE(head_message_id, fork_message_id) = NEW.head_message_id
      ))
BEGIN SELECT RAISE(ABORT, 'turn input does not match branch semantics'); END;

CREATE TRIGGER conversation_turn_retry_source_delete
BEFORE DELETE ON conversation_turns
WHEN EXISTS (
    SELECT 1 FROM conversation_turns AS child
    WHERE child.retry_of_turn_id = OLD.id
      AND child.conversation_id = OLD.conversation_id
) AND NOT EXISTS (SELECT 1 FROM purge_authorizations WHERE owner_id IN (OLD.conversation_id))
BEGIN SELECT RAISE(ABORT, 'retry source is protected'); END;

CREATE TRIGGER conversation_turn_selected_speaker_character_final_insert
BEFORE INSERT ON conversation_turns
WHEN NEW.selected_speaker_participant_id IS NOT NULL AND (
    NOT EXISTS (SELECT 1 FROM conversations WHERE id = NEW.conversation_id AND kind = 'group') OR
    NOT EXISTS (
    SELECT 1 FROM conversation_participants
    WHERE conversation_id = NEW.conversation_id
      AND id = NEW.selected_speaker_participant_id
      AND role = 'character'
    )
)
BEGIN SELECT RAISE(ABORT, 'selected speaker must be a character'); END;

CREATE TRIGGER conversation_turn_selected_speaker_character_final_update
BEFORE UPDATE OF conversation_id, selected_speaker_participant_id ON conversation_turns
WHEN NEW.selected_speaker_participant_id IS NOT NULL AND (
    NOT EXISTS (SELECT 1 FROM conversations WHERE id = NEW.conversation_id AND kind = 'group') OR
    NOT EXISTS (
    SELECT 1 FROM conversation_participants
    WHERE conversation_id = NEW.conversation_id
      AND id = NEW.selected_speaker_participant_id
      AND role = 'character'
    )
)
BEGIN SELECT RAISE(ABORT, 'selected speaker must be a character'); END;

CREATE TRIGGER conversation_turn_selected_speaker_immutable
BEFORE UPDATE OF selected_speaker_participant_id ON conversation_turns
WHEN OLD.selected_speaker_participant_id IS NOT NULL
 AND coalesce(NEW.selected_speaker_participant_id, '') <> OLD.selected_speaker_participant_id
BEGIN SELECT RAISE(ABORT, 'selected speaker is immutable once resolved'); END;

CREATE TRIGGER conversation_turn_status_transition
BEFORE UPDATE OF status ON conversation_turns
WHEN NEW.status <> OLD.status AND NOT (
 (OLD.status = 'created' AND NEW.status IN ('preparing','cancellation_requested','cancelled')) OR
 (OLD.status = 'preparing' AND NEW.status IN ('selecting_speaker','context_prepared','cancellation_requested','failed','interrupted')) OR
 (OLD.status = 'selecting_speaker' AND NEW.status IN ('context_prepared','cancellation_requested','failed','interrupted')) OR
 (OLD.status = 'context_prepared' AND NEW.status IN ('running','cancellation_requested','failed','interrupted')) OR
 (OLD.status = 'running' AND NEW.status IN ('finalizing','cancellation_requested','failed','interrupted')) OR
 (OLD.status = 'cancellation_requested' AND NEW.status IN ('finalizing','cancelled','failed','interrupted')) OR
 (OLD.status = 'finalizing' AND NEW.status IN ('succeeded','failed','cancelled','interrupted','recovering')) OR
 (OLD.status = 'interrupted' AND NEW.status IN ('recovering','failed','cancelled')) OR
 (OLD.status = 'recovering' AND NEW.status IN ('preparing','running','failed','cancelled'))
)
BEGIN SELECT RAISE(ABORT, 'invalid generation turn status transition'); END;

CREATE TRIGGER conversation_turn_terminal_attempts
BEFORE UPDATE ON conversation_turns
WHEN NEW.status IN ('succeeded','failed','cancelled','interrupted') AND (
    NOT EXISTS (SELECT 1 FROM generation_attempts WHERE conversation_id = NEW.conversation_id AND turn_id = NEW.id) OR
    EXISTS (SELECT 1 FROM generation_attempts WHERE conversation_id = NEW.conversation_id AND turn_id = NEW.id AND (status NOT IN ('succeeded','failed','cancelled','interrupted') OR usage_event_id IS NULL))
)
BEGIN SELECT RAISE(ABORT, 'terminal turn requires settled attempts'); END;

CREATE TRIGGER conversation_turn_terminal_insert
BEFORE INSERT ON conversation_turns
WHEN NEW.status IN ('succeeded','failed','cancelled','interrupted')
BEGIN SELECT RAISE(ABORT, 'terminal turn requires settled attempts'); END;

CREATE TRIGGER sync_mark_conversation_turns_delete
AFTER DELETE ON conversation_turns
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (OLD.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_turns_insert
AFTER INSERT ON conversation_turns
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_turns_update
AFTER UPDATE ON conversation_turns
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;



-- creation_admitted_tool_calls

CREATE TABLE creation_admitted_tool_calls_content_limits (
    workflow_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    round_ordinal INTEGER NOT NULL CHECK (round_ordinal >= 0 AND round_ordinal < 8),
    id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0 AND ordinal < 64),
    definition_name TEXT NOT NULL CHECK (
        length(definition_name) BETWEEN 1 AND 64
        AND definition_name NOT GLOB '*[^A-Za-z0-9_-]*'
    ),
    definition_version INTEGER NOT NULL CHECK (definition_version >= 1),
    provider_call_id TEXT CHECK (
        provider_call_id IS NULL OR
        (length(trim(provider_call_id)) > 0 AND length(CAST(provider_call_id AS BLOB)) <= 256)
    ),
    arguments_json TEXT NOT NULL CHECK (
        json_valid(arguments_json)
        AND json_extract(arguments_json, '$.format_version') = 1
        AND json_type(arguments_json, '$.value') = 'object'
        AND length(CAST(arguments_json AS BLOB)) <= 262208
    ),
    raw_arguments TEXT CHECK (
        raw_arguments IS NULL OR (
            length(CAST(raw_arguments AS BLOB)) <= 67108864
            AND json_valid(raw_arguments)
            AND json(raw_arguments) = json(json_extract(arguments_json, '$.value'))
        )
    ),
    provider_replay_artifact_id TEXT,
    provider_replay_retention TEXT CHECK (
        provider_replay_retention IS NULL OR provider_replay_retention = 'conversation'
    ),
    admitted_at INTEGER NOT NULL,
    PRIMARY KEY (workflow_id, turn_id, attempt_id, id),
    UNIQUE (id),
    UNIQUE (workflow_id, turn_id, attempt_id, ordinal),
    FOREIGN KEY (workflow_id, turn_id, attempt_id)
        REFERENCES creation_inference_attempts(workflow_id, turn_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (workflow_id, turn_id, attempt_id, round_ordinal)
        REFERENCES creation_inference_rounds(workflow_id, turn_id, attempt_id, ordinal)
        ON DELETE RESTRICT,
    FOREIGN KEY (provider_replay_artifact_id, provider_replay_retention)
        REFERENCES conversation_replay_artifacts(artifact_id, retention) ON DELETE RESTRICT,
    CHECK ((provider_replay_artifact_id IS NULL) = (provider_replay_retention IS NULL))
) STRICT;

INSERT INTO creation_admitted_tool_calls_content_limits ("workflow_id", "turn_id", "attempt_id", "round_ordinal", "id", "ordinal", "definition_name", "definition_version", "provider_call_id", "arguments_json", "raw_arguments", "provider_replay_artifact_id", "provider_replay_retention", "admitted_at")
    SELECT "workflow_id", "turn_id", "attempt_id", "round_ordinal", "id", "ordinal", "definition_name", "definition_version", "provider_call_id", "arguments_json", "raw_arguments", "provider_replay_artifact_id", "provider_replay_retention", "admitted_at" FROM creation_admitted_tool_calls;

DROP TABLE creation_admitted_tool_calls;

ALTER TABLE creation_admitted_tool_calls_content_limits RENAME TO creation_admitted_tool_calls;

CREATE INDEX creation_admitted_tool_calls_attempt_idx
    ON creation_admitted_tool_calls(workflow_id, turn_id, attempt_id, round_ordinal, ordinal);

CREATE UNIQUE INDEX creation_admitted_tool_calls_provider_id_uq
    ON creation_admitted_tool_calls(workflow_id, turn_id, attempt_id, provider_call_id)
    WHERE provider_call_id IS NOT NULL;

CREATE TRIGGER creation_tool_call_contract_guard
BEFORE INSERT ON creation_admitted_tool_calls
WHEN NOT EXISTS (
    SELECT 1
    FROM creation_inference_attempts attempt,
         json_each(json_extract(attempt.tool_request_json, '$.value.definitions')) definition
    WHERE attempt.workflow_id = NEW.workflow_id
      AND attempt.turn_id = NEW.turn_id
      AND attempt.id = NEW.attempt_id
      AND json_extract(definition.value, '$.name') = NEW.definition_name
      AND json_extract(definition.value, '$.version') = NEW.definition_version
) AND (
    NEW.definition_version != 1
    OR NOT EXISTS (
        SELECT 1 FROM creation_inference_attempts attempt
        WHERE attempt.workflow_id = NEW.workflow_id
          AND attempt.turn_id = NEW.turn_id
          AND attempt.id = NEW.attempt_id
    )
    OR EXISTS (
        SELECT 1
        FROM creation_inference_attempts attempt,
             json_each(json_extract(attempt.tool_request_json, '$.value.definitions')) definition
        WHERE attempt.workflow_id = NEW.workflow_id
          AND attempt.turn_id = NEW.turn_id
          AND attempt.id = NEW.attempt_id
          AND json_extract(definition.value, '$.name') = NEW.definition_name
    )
)
BEGIN SELECT RAISE(ABORT, 'creation tool call contract mismatch'); END;

CREATE TRIGGER creation_tool_call_immutable_delete
BEFORE DELETE ON creation_admitted_tool_calls
BEGIN SELECT RAISE(ABORT, 'creation tool call cannot be deleted'); END;

CREATE TRIGGER creation_tool_call_immutable_update
BEFORE UPDATE ON creation_admitted_tool_calls
BEGIN SELECT RAISE(ABORT, 'creation tool call is immutable'); END;

CREATE TRIGGER creation_tool_call_live_attempt_guard
BEFORE INSERT ON creation_admitted_tool_calls
WHEN NOT EXISTS (
    SELECT 1
    FROM creation_inference_attempts attempt
    JOIN creation_workflows workflow ON workflow.id = attempt.workflow_id
    JOIN creation_turns turn
      ON turn.workflow_id = attempt.workflow_id AND turn.id = attempt.turn_id
    WHERE attempt.workflow_id = NEW.workflow_id
      AND attempt.turn_id = NEW.turn_id
      AND attempt.id = NEW.attempt_id
      AND attempt.status = 'running'
      AND workflow.current_proposal_id = attempt.base_proposal_id
      AND workflow.stage = attempt.stage
      AND turn.base_proposal_id = attempt.base_proposal_id
)
BEGIN SELECT RAISE(ABORT, 'creation call requires a live non-stale attempt'); END;



-- device_ui_state

CREATE TABLE device_ui_state_content_limits (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    state_json TEXT NOT NULL CHECK (
        json_valid(state_json)
        AND json_type(state_json) = 'object'
        AND length(CAST(state_json AS BLOB)) <= 4194304
    ),
    updated_at INTEGER NOT NULL
) STRICT;

INSERT INTO device_ui_state_content_limits ("id", "state_json", "updated_at")
    SELECT "id", "state_json", "updated_at" FROM device_ui_state;

DROP TABLE device_ui_state;

ALTER TABLE device_ui_state_content_limits RENAME TO device_ui_state;



-- dynamic_memory_admitted_tool_calls

CREATE TABLE dynamic_memory_admitted_tool_calls_content_limits (
    run_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    round_ordinal INTEGER NOT NULL CHECK (round_ordinal BETWEEN 0 AND 99),
    id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    definition_name TEXT NOT NULL CHECK (
        length(definition_name) BETWEEN 1 AND 64
        AND definition_name NOT GLOB '*[^A-Za-z0-9_-]*'
    ),
    definition_version INTEGER NOT NULL CHECK (definition_version >= 1),
    provider_call_id TEXT CHECK (
        provider_call_id IS NULL OR
        (length(trim(provider_call_id)) > 0 AND length(CAST(provider_call_id AS BLOB)) <= 256)
    ),
    arguments_json TEXT NOT NULL CHECK (
        json_valid(arguments_json)
        AND json_extract(arguments_json, '$.format_version') = 1
        AND json_type(arguments_json, '$.value') = 'object'
        AND length(CAST(arguments_json AS BLOB)) <= 262208
    ),
    raw_arguments TEXT CHECK (
        raw_arguments IS NULL OR (
            length(CAST(raw_arguments AS BLOB)) <= 67108864
            AND json_valid(raw_arguments)
            AND json(raw_arguments) = json(json_extract(arguments_json, '$.value'))
        )
    ),
    provider_replay_artifact_id TEXT,
    provider_replay_retention TEXT CHECK (
        provider_replay_retention IS NULL OR provider_replay_retention = 'conversation'
    ),
    admitted_at INTEGER NOT NULL,
    PRIMARY KEY (run_id, attempt_id, id),
    UNIQUE (run_id, attempt_id, ordinal),
    FOREIGN KEY (run_id, attempt_id, round_ordinal)
        REFERENCES dynamic_memory_inference_rounds(run_id, attempt_id, ordinal)
        ON DELETE RESTRICT,
    FOREIGN KEY (provider_replay_artifact_id, provider_replay_retention)
        REFERENCES conversation_replay_artifacts(artifact_id, retention) ON DELETE RESTRICT,
    CHECK ((provider_replay_artifact_id IS NULL) = (provider_replay_retention IS NULL))
) STRICT;

INSERT INTO dynamic_memory_admitted_tool_calls_content_limits ("run_id", "attempt_id", "round_ordinal", "id", "ordinal", "definition_name", "definition_version", "provider_call_id", "arguments_json", "raw_arguments", "provider_replay_artifact_id", "provider_replay_retention", "admitted_at")
    SELECT "run_id", "attempt_id", "round_ordinal", "id", "ordinal", "definition_name", "definition_version", "provider_call_id", "arguments_json", "raw_arguments", "provider_replay_artifact_id", "provider_replay_retention", "admitted_at" FROM dynamic_memory_admitted_tool_calls;

DROP TABLE dynamic_memory_admitted_tool_calls;

ALTER TABLE dynamic_memory_admitted_tool_calls_content_limits RENAME TO dynamic_memory_admitted_tool_calls;

CREATE UNIQUE INDEX dynamic_memory_admitted_tool_calls_provider_id_uq
    ON dynamic_memory_admitted_tool_calls(run_id, attempt_id, provider_call_id)
    WHERE provider_call_id IS NOT NULL;

CREATE TRIGGER dynamic_memory_tool_call_contract_guard
BEFORE INSERT ON dynamic_memory_admitted_tool_calls
WHEN NOT EXISTS (
    SELECT 1
    FROM dynamic_memory_run_attempts attempt
    JOIN dynamic_memory_runs run ON run.id = attempt.run_id,
         json_each(json_extract(run.tool_request_json, '$.value.definitions')) definition
    WHERE attempt.run_id = NEW.run_id
      AND attempt.id = NEW.attempt_id
      AND attempt.status = 'processing'
      AND json_extract(definition.value, '$.name') = NEW.definition_name
      AND json_extract(definition.value, '$.version') = NEW.definition_version
)
BEGIN SELECT RAISE(ABORT, 'dynamic-memory tool call contract mismatch'); END;

CREATE TRIGGER dynamic_memory_tool_call_immutable_delete
BEFORE DELETE ON dynamic_memory_admitted_tool_calls
WHEN NOT EXISTS (SELECT 1 FROM purge_authorizations WHERE owner_id IN (SELECT conversation_id FROM dynamic_memory_runs WHERE id = OLD.run_id UNION ALL SELECT space_id FROM dynamic_memory_runs WHERE id = OLD.run_id))
BEGIN SELECT RAISE(ABORT, 'dynamic-memory tool call cannot be deleted'); END;

CREATE TRIGGER dynamic_memory_tool_call_immutable_update
BEFORE UPDATE ON dynamic_memory_admitted_tool_calls
BEGIN SELECT RAISE(ABORT, 'dynamic-memory tool call is immutable'); END;



-- memory_embedding_projections

CREATE TABLE memory_embedding_projections_content_limits (
    space_id TEXT NOT NULL REFERENCES memory_spaces(id) ON DELETE CASCADE,
    memory_id TEXT NOT NULL,
    source_revision TEXT NOT NULL CHECK (
        length(trim(source_revision)) > 0
        AND length(CAST(source_revision AS BLOB)) <= 128
    ),
    dimensions INTEGER NOT NULL CHECK (dimensions IN (64, 128, 256, 512, 768)),
    source_text TEXT NOT NULL CHECK (
        length(trim(source_text)) > 0
        AND length(CAST(source_text AS BLOB)) <= 134217728
    ),
    status TEXT NOT NULL CHECK (status IN ('ready', 'repair_needed')),
    vector BLOB,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (space_id, memory_id, source_revision, dimensions),
    CHECK (
        (status = 'ready' AND vector IS NOT NULL AND length(vector) = dimensions * 4)
        OR (status = 'repair_needed' AND vector IS NULL)
    )
) STRICT;

INSERT INTO memory_embedding_projections_content_limits ("space_id", "memory_id", "source_revision", "dimensions", "source_text", "status", "vector", "updated_at")
    SELECT "space_id", "memory_id", "source_revision", "dimensions", "source_text", "status", "vector", "updated_at" FROM memory_embedding_projections;

DROP TABLE memory_embedding_projections;

ALTER TABLE memory_embedding_projections_content_limits RENAME TO memory_embedding_projections;

CREATE INDEX memory_embedding_projections_lookup_idx
    ON memory_embedding_projections(space_id, source_revision, dimensions, status, memory_id);



-- memory_inherited_summaries

CREATE TABLE memory_inherited_summaries_content_limits (
    space_id TEXT PRIMARY KEY REFERENCES memory_spaces(id) ON DELETE RESTRICT,
    conversation_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    origin TEXT NOT NULL DEFAULT 'model' CHECK (origin IN ('user', 'model', 'import')),
    text TEXT NOT NULL CHECK (
        length(trim(text)) > 0
        AND length(CAST(text AS BLOB)) <= 134217728
    ),
    token_count INTEGER CHECK (token_count BETWEEN 0 AND 4294967295),
    window_start INTEGER NOT NULL CHECK (window_start >= 0),
    window_end INTEGER NOT NULL CHECK (
        (origin = 'user' AND window_start = 0 AND window_end = 0)
        OR (origin <> 'user' AND window_end > window_start)
    ),
    updated_at INTEGER NOT NULL,
    UNIQUE (space_id, conversation_id),
    FOREIGN KEY (conversation_id, branch_id) REFERENCES conversation_branches(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, space_id)
        REFERENCES conversation_memory_spaces(conversation_id, space_id) ON DELETE RESTRICT
) STRICT;

INSERT INTO memory_inherited_summaries_content_limits ("space_id", "conversation_id", "branch_id", "origin", "text", "token_count", "window_start", "window_end", "updated_at")
    SELECT "space_id", "conversation_id", "branch_id", "origin", "text", "token_count", "window_start", "window_end", "updated_at" FROM memory_inherited_summaries;

DROP TABLE memory_inherited_summaries;

ALTER TABLE memory_inherited_summaries_content_limits RENAME TO memory_inherited_summaries;

CREATE TRIGGER memory_inherited_summaries_record_delete AFTER DELETE ON memory_inherited_summaries
BEGIN
    DELETE FROM memory_changes WHERE conversation_id IN (SELECT conversation_id FROM conversation_memory_spaces WHERE space_id = OLD.space_id);
    INSERT INTO memory_changes (conversation_id) SELECT DISTINCT conversation_id FROM conversation_memory_spaces WHERE space_id = OLD.space_id;
END;

CREATE TRIGGER memory_inherited_summaries_record_insert AFTER INSERT ON memory_inherited_summaries
BEGIN
    DELETE FROM memory_changes WHERE conversation_id IN (SELECT conversation_id FROM conversation_memory_spaces WHERE space_id = NEW.space_id);
    INSERT INTO memory_changes (conversation_id) SELECT DISTINCT conversation_id FROM conversation_memory_spaces WHERE space_id = NEW.space_id;
END;

CREATE TRIGGER memory_inherited_summaries_record_update AFTER UPDATE ON memory_inherited_summaries
BEGIN
    DELETE FROM memory_changes WHERE conversation_id IN (SELECT conversation_id FROM conversation_memory_spaces WHERE space_id = NEW.space_id);
    INSERT INTO memory_changes (conversation_id) SELECT DISTINCT conversation_id FROM conversation_memory_spaces WHERE space_id = NEW.space_id;
END;



-- memory_items

CREATE TABLE memory_items_content_limits (
    space_id TEXT NOT NULL REFERENCES memory_spaces(id) ON DELETE RESTRICT,
    id TEXT NOT NULL UNIQUE,
    short_id INTEGER NOT NULL CHECK (short_id BETWEEN 0 AND 999999),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    origin TEXT NOT NULL DEFAULT 'model' CHECK (origin IN ('user', 'model', 'import')),
    text TEXT NOT NULL CHECK (
        length(trim(text)) > 0
        AND length(CAST(text AS BLOB)) <= 134217728
    ),
    category TEXT CHECK (category IN (
        'character_trait', 'relationship', 'plot_event',
        'world_detail', 'preference', 'other',
        'milestone', 'boundary', 'profile', 'routine', 'episodic', 'emotional_snapshot'
    )),
    source_message_id TEXT,
    source_role TEXT CHECK (source_role IN ('user','assistant')),
    observed_at INTEGER,
    observed_time_precision TEXT CHECK (observed_time_precision IN ('turn', 'user')),
    superseded_by TEXT,
    superseded_at INTEGER,
    supersedes_json TEXT NOT NULL CHECK (json_valid(supersedes_json) AND json_type(supersedes_json) = 'array'),
    token_count INTEGER CHECK (token_count BETWEEN 0 AND 4294967295),
    is_cold INTEGER NOT NULL CHECK (is_cold IN (0, 1)),
    is_pinned INTEGER NOT NULL CHECK (is_pinned IN (0, 1)),
    importance INTEGER NOT NULL CHECK (importance BETWEEN 0 AND 10000),
    persistence_importance INTEGER NOT NULL CHECK (persistence_importance BETWEEN 0 AND 10000),
    prompt_importance INTEGER NOT NULL CHECK (prompt_importance BETWEEN 0 AND 10000),
    volatility INTEGER NOT NULL CHECK (volatility BETWEEN 0 AND 10000),
    access_count INTEGER NOT NULL CHECK (access_count BETWEEN 0 AND 4294967295),
    created_at INTEGER NOT NULL,
    last_accessed_at INTEGER NOT NULL,
    PRIMARY KEY (space_id, id),
    UNIQUE (space_id, ordinal),
    UNIQUE (space_id, short_id),
    CHECK (NOT (is_pinned = 1 AND is_cold = 1)),
    CHECK (
        (observed_at IS NULL AND observed_time_precision IS NULL)
        OR (observed_at IS NOT NULL AND observed_time_precision IS NOT NULL AND observed_time_precision = 'user')
        OR (source_message_id IS NOT NULL AND source_role IS NOT NULL AND observed_at IS NOT NULL AND observed_time_precision IS NOT NULL AND observed_time_precision = 'turn')
    ),
    CHECK (source_role IS NULL OR source_message_id IS NOT NULL),
    CHECK ((superseded_by IS NULL) = (superseded_at IS NULL)),
    CHECK (superseded_by IS NULL OR superseded_by <> id)
) STRICT;

INSERT INTO memory_items_content_limits ("space_id", "id", "short_id", "ordinal", "origin", "text", "category", "source_message_id", "source_role", "observed_at", "observed_time_precision", "superseded_by", "superseded_at", "supersedes_json", "token_count", "is_cold", "is_pinned", "importance", "persistence_importance", "prompt_importance", "volatility", "access_count", "created_at", "last_accessed_at")
    SELECT "space_id", "id", "short_id", "ordinal", "origin", "text", "category", "source_message_id", "source_role", "observed_at", "observed_time_precision", "superseded_by", "superseded_at", "supersedes_json", "token_count", "is_cold", "is_pinned", "importance", "persistence_importance", "prompt_importance", "volatility", "access_count", "created_at", "last_accessed_at" FROM memory_items;

DROP TABLE memory_items;

ALTER TABLE memory_items_content_limits RENAME TO memory_items;

CREATE INDEX memory_items_space_policy_idx
    ON memory_items(space_id, is_pinned DESC, is_cold ASC, last_accessed_at DESC, id ASC);

CREATE TRIGGER memory_items_record_delete AFTER DELETE ON memory_items
BEGIN
    DELETE FROM memory_changes WHERE conversation_id IN (SELECT conversation_id FROM conversation_memory_spaces WHERE space_id = OLD.space_id);
    INSERT INTO memory_changes (conversation_id) SELECT DISTINCT conversation_id FROM conversation_memory_spaces WHERE space_id = OLD.space_id;
END;

CREATE TRIGGER memory_items_record_insert AFTER INSERT ON memory_items
BEGIN
    DELETE FROM memory_changes WHERE conversation_id IN (SELECT conversation_id FROM conversation_memory_spaces WHERE space_id = NEW.space_id);
    INSERT INTO memory_changes (conversation_id) SELECT DISTINCT conversation_id FROM conversation_memory_spaces WHERE space_id = NEW.space_id;
END;

CREATE TRIGGER memory_items_record_update AFTER UPDATE ON memory_items
BEGIN
    DELETE FROM memory_changes WHERE conversation_id IN (SELECT conversation_id FROM conversation_memory_spaces WHERE space_id = NEW.space_id);
    INSERT INTO memory_changes (conversation_id) SELECT DISTINCT conversation_id FROM conversation_memory_spaces WHERE space_id = NEW.space_id;
END;

CREATE TRIGGER sync_deleted_memory_items
AFTER DELETE ON memory_items
WHEN (SELECT 'pool:' || character_id FROM companion_memory_pools WHERE space_id = OLD.space_id
     UNION ALL
     SELECT 'conversation:' || conversation_id || ':branch:' || branch_id FROM conversation_memory_spaces
      WHERE space_id = OLD.space_id AND pooled = 0
     LIMIT 1) IS NOT NULL
BEGIN
    INSERT INTO sync_deleted_entities (entity_kind, entity_id, deleted_at)
    VALUES ('memory_item', (SELECT 'pool:' || character_id FROM companion_memory_pools WHERE space_id = OLD.space_id
     UNION ALL
     SELECT 'conversation:' || conversation_id || ':branch:' || branch_id FROM conversation_memory_spaces
      WHERE space_id = OLD.space_id AND pooled = 0
     LIMIT 1) || '/' || OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))
    ON CONFLICT(entity_kind, entity_id) DO NOTHING;
END;



-- memory_summaries

CREATE TABLE memory_summaries_content_limits (
    space_id TEXT PRIMARY KEY REFERENCES memory_spaces(id) ON DELETE RESTRICT,
    conversation_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    origin TEXT NOT NULL DEFAULT 'model' CHECK (origin IN ('user', 'model', 'import')),
    text TEXT NOT NULL CHECK (
        length(trim(text)) > 0
        AND length(CAST(text AS BLOB)) <= 134217728
    ),
    token_count INTEGER CHECK (token_count BETWEEN 0 AND 4294967295),
    window_start INTEGER NOT NULL CHECK (window_start >= 0),
    window_end INTEGER NOT NULL CHECK (
        (origin = 'user' AND window_start = 0 AND window_end = 0)
        OR (origin <> 'user' AND window_end > window_start)
    ),
    updated_at INTEGER NOT NULL,
    UNIQUE (space_id, conversation_id),
    FOREIGN KEY (conversation_id, branch_id) REFERENCES conversation_branches(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, space_id)
        REFERENCES conversation_memory_spaces(conversation_id, space_id) ON DELETE RESTRICT
) STRICT;

INSERT INTO memory_summaries_content_limits ("space_id", "conversation_id", "branch_id", "origin", "text", "token_count", "window_start", "window_end", "updated_at")
    SELECT "space_id", "conversation_id", "branch_id", "origin", "text", "token_count", "window_start", "window_end", "updated_at" FROM memory_summaries;

DROP TABLE memory_summaries;

ALTER TABLE memory_summaries_content_limits RENAME TO memory_summaries;

CREATE TRIGGER memory_summaries_record_delete AFTER DELETE ON memory_summaries
BEGIN
    DELETE FROM memory_changes WHERE conversation_id IN (SELECT conversation_id FROM conversation_memory_spaces WHERE space_id = OLD.space_id);
    INSERT INTO memory_changes (conversation_id) SELECT DISTINCT conversation_id FROM conversation_memory_spaces WHERE space_id = OLD.space_id;
END;

CREATE TRIGGER memory_summaries_record_insert AFTER INSERT ON memory_summaries
BEGIN
    DELETE FROM memory_changes WHERE conversation_id IN (SELECT conversation_id FROM conversation_memory_spaces WHERE space_id = NEW.space_id);
    INSERT INTO memory_changes (conversation_id) SELECT DISTINCT conversation_id FROM conversation_memory_spaces WHERE space_id = NEW.space_id;
END;

CREATE TRIGGER memory_summaries_record_update AFTER UPDATE ON memory_summaries
BEGIN
    DELETE FROM memory_changes WHERE conversation_id IN (SELECT conversation_id FROM conversation_memory_spaces WHERE space_id = NEW.space_id);
    INSERT INTO memory_changes (conversation_id) SELECT DISTINCT conversation_id FROM conversation_memory_spaces WHERE space_id = NEW.space_id;
END;



-- tool_executions

CREATE TABLE tool_executions_content_limits (
    conversation_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0 AND ordinal < 64),
    definition_name TEXT NOT NULL CHECK (
        length(definition_name) BETWEEN 1 AND 64
        AND definition_name NOT GLOB '*[^A-Za-z0-9_-]*'
    ),
    definition_version INTEGER NOT NULL CHECK (definition_version >= 1),
    provider_call_id TEXT CHECK (
        provider_call_id IS NULL OR
        (length(trim(provider_call_id)) > 0 AND length(CAST(provider_call_id AS BLOB)) <= 256)
    ),
    arguments_json TEXT NOT NULL CHECK (
        json_valid(arguments_json)
        AND json_extract(arguments_json, '$.format_version') = 1
        AND json_type(arguments_json, '$.value') = 'object'
        AND length(CAST(arguments_json AS BLOB)) <= 262208
    ),
    raw_arguments TEXT CHECK (
        raw_arguments IS NULL OR length(CAST(raw_arguments AS BLOB)) <= 67108864
    ),
    provider_replay_artifact_id TEXT,
    provider_replay_retention TEXT CHECK (
        provider_replay_retention IS NULL OR provider_replay_retention = 'conversation'
    ),
    status TEXT NOT NULL CHECK (
        status IN ('requested', 'validated', 'running', 'succeeded', 'rejected', 'failed', 'cancelled', 'interrupted')
    ),
    output_json TEXT CHECK (
        output_json IS NULL OR
        (json_valid(output_json)
         AND json_extract(output_json, '$.format_version') = 1
         AND length(CAST(output_json AS BLOB)) <= 1048640)
    ),
    failure_code TEXT CHECK (
        failure_code IS NULL OR failure_code IN (
            'invalid_arguments', 'undeclared_tool', 'permission_denied',
            'handler_failed', 'result_invalid', 'internal'
        )
    ),
    failure_message TEXT CHECK (
        failure_message IS NULL OR
        (length(trim(failure_message)) > 0 AND length(CAST(failure_message AS BLOB)) <= 4096)
    ),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    requested_at INTEGER NOT NULL,
    started_at INTEGER,
    finished_at INTEGER,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (conversation_id, turn_id, attempt_id, id),
    UNIQUE (conversation_id, turn_id, attempt_id, ordinal),
    UNIQUE (conversation_id, id),
    FOREIGN KEY (conversation_id, turn_id, attempt_id)
        REFERENCES generation_attempts(conversation_id, turn_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (provider_replay_artifact_id, provider_replay_retention)
        REFERENCES conversation_replay_artifacts(artifact_id, retention)
        ON DELETE RESTRICT,
    CHECK ((provider_replay_artifact_id IS NULL) = (provider_replay_retention IS NULL)),
    CHECK (requested_at <= updated_at),
    CHECK (started_at IS NULL OR started_at >= requested_at),
    CHECK (finished_at IS NULL OR finished_at >= requested_at),
    CHECK (started_at IS NULL OR started_at <= updated_at),
    CHECK (finished_at IS NULL OR finished_at <= updated_at),
    CHECK (started_at IS NULL OR finished_at IS NULL OR started_at <= finished_at),
    CHECK ((status IN ('requested', 'validated')) = (started_at IS NULL AND finished_at IS NULL)),
    CHECK ((status = 'running') = (started_at IS NOT NULL AND finished_at IS NULL)),
    CHECK ((status IN ('succeeded', 'rejected', 'failed', 'cancelled', 'interrupted')) = (finished_at IS NOT NULL)),
    CHECK (status NOT IN ('succeeded', 'failed', 'interrupted') OR started_at IS NOT NULL),
    CHECK ((status = 'succeeded') = (output_json IS NOT NULL)),
    CHECK ((status IN ('rejected', 'failed')) = (failure_code IS NOT NULL)),
    CHECK (failure_message IS NULL OR failure_code IS NOT NULL)
) STRICT;

INSERT INTO tool_executions_content_limits ("conversation_id", "turn_id", "attempt_id", "id", "ordinal", "definition_name", "definition_version", "provider_call_id", "arguments_json", "raw_arguments", "provider_replay_artifact_id", "provider_replay_retention", "status", "output_json", "failure_code", "failure_message", "revision", "requested_at", "started_at", "finished_at", "updated_at")
    SELECT "conversation_id", "turn_id", "attempt_id", "id", "ordinal", "definition_name", "definition_version", "provider_call_id", "arguments_json", "raw_arguments", "provider_replay_artifact_id", "provider_replay_retention", "status", "output_json", "failure_code", "failure_message", "revision", "requested_at", "started_at", "finished_at", "updated_at" FROM tool_executions;

DROP TABLE tool_executions;

ALTER TABLE tool_executions_content_limits RENAME TO tool_executions;

CREATE INDEX tool_executions_attempt_idx
    ON tool_executions(conversation_id, turn_id, attempt_id, ordinal);

CREATE UNIQUE INDEX tool_executions_id_uq ON tool_executions(id);

CREATE UNIQUE INDEX tool_executions_provider_call_uq
    ON tool_executions(conversation_id, turn_id, attempt_id, provider_call_id)
    WHERE provider_call_id IS NOT NULL;

CREATE TRIGGER tool_execution_identity_immutable
BEFORE UPDATE OF conversation_id, turn_id, attempt_id, id, ordinal, definition_name, definition_version, provider_call_id, arguments_json, raw_arguments, provider_replay_artifact_id, provider_replay_retention ON tool_executions
WHEN NEW.conversation_id <> OLD.conversation_id
  OR NEW.turn_id <> OLD.turn_id
  OR NEW.attempt_id <> OLD.attempt_id
  OR NEW.id <> OLD.id
  OR NEW.ordinal <> OLD.ordinal
  OR NEW.definition_name <> OLD.definition_name
  OR NEW.definition_version <> OLD.definition_version
  OR coalesce(NEW.provider_call_id, '') <> coalesce(OLD.provider_call_id, '')
  OR NEW.arguments_json <> OLD.arguments_json
  OR coalesce(NEW.raw_arguments, '') <> coalesce(OLD.raw_arguments, '')
  OR coalesce(NEW.provider_replay_artifact_id, '') <> coalesce(OLD.provider_replay_artifact_id, '')
  OR coalesce(NEW.provider_replay_retention, '') <> coalesce(OLD.provider_replay_retention, '')
BEGIN SELECT RAISE(ABORT, 'tool execution request is immutable'); END;

CREATE TRIGGER tool_execution_live_attempt_insert
BEFORE INSERT ON tool_executions
WHEN NOT EXISTS (
    SELECT 1 FROM generation_attempts
     WHERE conversation_id = NEW.conversation_id
       AND turn_id = NEW.turn_id
       AND id = NEW.attempt_id
       AND status = 'running'
)
BEGIN SELECT RAISE(ABORT, 'tool execution requires a running generation attempt'); END;

CREATE TRIGGER tool_execution_revision_valid
BEFORE UPDATE ON tool_executions
WHEN NEW.revision <> OLD.revision + 1 OR NEW.updated_at < OLD.updated_at
BEGIN SELECT RAISE(ABORT, 'invalid tool execution revision'); END;

CREATE TRIGGER tool_execution_started_at_immutable
BEFORE UPDATE OF started_at ON tool_executions
WHEN OLD.started_at IS NOT NULL AND NEW.started_at <> OLD.started_at
BEGIN SELECT RAISE(ABORT, 'tool execution start is immutable'); END;

CREATE TRIGGER tool_execution_terminal_immutable
BEFORE UPDATE ON tool_executions
WHEN OLD.status IN ('succeeded', 'rejected', 'failed', 'cancelled', 'interrupted')
BEGIN SELECT RAISE(ABORT, 'terminal tool execution cannot regress'); END;

CREATE TRIGGER tool_execution_transition_valid
BEFORE UPDATE OF status ON tool_executions
WHEN NEW.status <> OLD.status AND NOT (
    (OLD.status = 'requested' AND NEW.status IN ('validated', 'rejected', 'cancelled')) OR
    (OLD.status = 'validated' AND NEW.status IN ('running', 'rejected', 'cancelled')) OR
    (OLD.status = 'running' AND NEW.status IN ('succeeded', 'failed', 'cancelled', 'interrupted'))
)
BEGIN SELECT RAISE(ABORT, 'invalid tool execution transition'); END;



-- user_voices

CREATE TABLE user_voices_content_limits (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    provider_id TEXT NOT NULL,
    name TEXT NOT NULL CHECK (
        length(name) BETWEEN 1 AND 65536
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
        OR (length(CAST(prompt AS BLOB)) <= 1048576 AND instr(prompt, char(0)) = 0)
    ),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL CHECK (updated_at >= created_at),
    FOREIGN KEY(provider_id) REFERENCES audio_providers(id) ON DELETE CASCADE
) STRICT;

INSERT INTO user_voices_content_limits ("id", "provider_id", "name", "model_id", "voice_id", "prompt", "revision", "created_at", "updated_at")
    SELECT "id", "provider_id", "name", "model_id", "voice_id", "prompt", "revision", "created_at", "updated_at" FROM user_voices;

DROP TABLE user_voices;

ALTER TABLE user_voices_content_limits RENAME TO user_voices;

CREATE INDEX user_voices_display
ON user_voices(created_at DESC, id DESC);

CREATE INDEX user_voices_provider
ON user_voices(provider_id, created_at DESC, id DESC);

CREATE TRIGGER sync_deleted_user_voices
AFTER DELETE ON user_voices
BEGIN
    INSERT INTO sync_deleted_entities (entity_kind, entity_id, deleted_at)
    VALUES ('user_voice', OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))
    ON CONFLICT(entity_kind, entity_id) DO NOTHING;
END;

CREATE TRIGGER user_voices_revision_step
BEFORE UPDATE ON user_voices
WHEN NEW.revision <> OLD.revision + 1
  OR NEW.updated_at < OLD.updated_at
BEGIN
    SELECT RAISE(ABORT, 'user voice revision is invalid');
END;

CREATE TRIGGER user_voices_stable_identity
BEFORE UPDATE ON user_voices
WHEN NEW.id <> OLD.id OR NEW.created_at <> OLD.created_at
BEGIN
    SELECT RAISE(ABORT, 'user voice identity is immutable');
END;


