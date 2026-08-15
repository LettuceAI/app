-- M8 normalized conversation persistence.  Durable identities, ownership,
-- ordering, lifecycle vocabulary, CAS state, and artifact references are
-- relational; closed value objects remain versioned JSON documents.

CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('direct', 'group')),
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('active', 'archived', 'tombstoned')),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    active_branch_id TEXT NOT NULL,
    kind_json TEXT NOT NULL CHECK (json_valid(kind_json) AND json_extract(kind_json, '$.format_version') = 1),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    next_timeline_ordinal INTEGER NOT NULL DEFAULT 1 CHECK (next_timeline_ordinal >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at),
    UNIQUE (id, active_branch_id),
    FOREIGN KEY (id, active_branch_id)
        REFERENCES conversation_branches(conversation_id, id)
        DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE INDEX conversations_updated_idx ON conversations(updated_at DESC, id ASC);

CREATE TABLE conversation_participants (
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE RESTRICT,
    id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'character', 'system')),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    source_kind TEXT NOT NULL CHECK (source_kind IN ('user', 'character', 'system')),
    source_id TEXT,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    muted INTEGER NOT NULL CHECK (muted IN (0, 1)),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) > 0),
    authored_description TEXT,
    model_selection_json TEXT NOT NULL CHECK (json_valid(model_selection_json) AND json_extract(model_selection_json, '$.format_version') = 1),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (conversation_id, id),
    UNIQUE (conversation_id, ordinal),
    CHECK (
        (role = 'user' AND source_kind = 'user' AND source_id IS NULL) OR
        (role = 'character' AND source_kind = 'character' AND source_id IS NOT NULL) OR
        (role = 'system' AND source_kind = 'system' AND source_id IS NULL)
    ),
    CHECK (created_at <= updated_at)
) STRICT;

CREATE TABLE conversation_settings (
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
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (author_note IS NULL OR (length(trim(author_note)) > 0 AND length(CAST(author_note AS BLOB)) <= 1048576)),
    CHECK ((author_note IS NOT NULL) = (author_note_provenance = 'current_override')),
    CHECK ((memory_json IS NOT NULL) = (memory_provenance = 'current_override')),
    CHECK ((model_override_json IS NOT NULL) = (model_provenance = 'current_override')),
    CHECK ((voice_json IS NOT NULL) = (voice_provenance = 'current_override')),
    CHECK (created_at <= updated_at)
) STRICT;

CREATE TABLE conversation_branches (
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE RESTRICT,
    id TEXT NOT NULL,
    parent_branch_id TEXT,
    fork_message_id TEXT,
    head_message_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('active', 'archived', 'tombstoned')),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (conversation_id, id),
    FOREIGN KEY (conversation_id, parent_branch_id)
        REFERENCES conversation_branches(conversation_id, id) ON DELETE RESTRICT
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (conversation_id, fork_message_id)
        REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, head_message_id)
        REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT,
    CHECK ((parent_branch_id IS NULL) = (fork_message_id IS NULL)),
    CHECK (parent_branch_id IS NULL OR parent_branch_id <> id),
    CHECK (created_at <= updated_at)
) STRICT;
CREATE UNIQUE INDEX conversation_branch_root_uq
    ON conversation_branches(conversation_id) WHERE parent_branch_id IS NULL;
CREATE INDEX conversation_branch_order_idx
    ON conversation_branches(conversation_id, created_at, id);

CREATE TABLE conversation_messages (
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE RESTRICT,
    id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    parent_message_id TEXT,
    author_participant_id TEXT,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system', 'scene')),
    timeline_ordinal INTEGER NOT NULL CHECK (timeline_ordinal >= 1),
    logical_time INTEGER NOT NULL,
    effective_time INTEGER NOT NULL,
    visibility TEXT NOT NULL CHECK (visibility IN ('visible', 'hidden', 'tombstoned')),
    pinned INTEGER NOT NULL CHECK (pinned IN (0, 1)),
    scene_edited INTEGER NOT NULL CHECK (scene_edited IN (0, 1)),
    active_revision_id TEXT,
    active_candidate_id TEXT,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (conversation_id, id),
    UNIQUE (conversation_id, timeline_ordinal),
    UNIQUE (conversation_id, id, active_revision_id),
    UNIQUE (conversation_id, id, active_candidate_id),
    UNIQUE (conversation_id, id, branch_id),
    FOREIGN KEY (conversation_id, branch_id)
        REFERENCES conversation_branches(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, parent_message_id)
        REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, author_participant_id)
        REFERENCES conversation_participants(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, id, active_revision_id)
        REFERENCES conversation_message_revisions(conversation_id, message_id, id)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (conversation_id, id, active_candidate_id)
        REFERENCES conversation_message_candidates(conversation_id, message_id, id)
        DEFERRABLE INITIALLY DEFERRED,
    CHECK ((active_revision_id IS NOT NULL) <> (active_candidate_id IS NOT NULL)),
    CHECK ((role IN ('system', 'scene')) = (author_participant_id IS NULL)),
    CHECK (created_at <= updated_at)
) STRICT;
CREATE INDEX conversation_messages_updated_idx ON conversation_messages(conversation_id, updated_at, id);
CREATE INDEX conversation_messages_parent_idx ON conversation_messages(conversation_id, parent_message_id, id);
CREATE INDEX conversation_messages_branch_timeline_idx
    ON conversation_messages(conversation_id, branch_id, timeline_ordinal, id);
CREATE INDEX conversation_messages_timeline_idx
    ON conversation_messages(conversation_id, effective_time, id);
CREATE TRIGGER conversation_message_author_role
BEFORE INSERT ON conversation_messages
WHEN NEW.author_participant_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_participants
    WHERE conversation_id = NEW.conversation_id
      AND id = NEW.author_participant_id
      AND ((NEW.role = 'user' AND role = 'user') OR
           (NEW.role = 'assistant' AND role = 'character'))
)
BEGIN SELECT RAISE(ABORT, 'message author role does not match participant'); END;
CREATE TRIGGER conversation_message_author_role_update
BEFORE UPDATE OF role, author_participant_id ON conversation_messages
WHEN NEW.author_participant_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_participants
    WHERE conversation_id = NEW.conversation_id
      AND id = NEW.author_participant_id
      AND ((NEW.role = 'user' AND role = 'user') OR
           (NEW.role = 'assistant' AND role = 'character'))
)
BEGIN SELECT RAISE(ABORT, 'message author role does not match participant'); END;
CREATE TRIGGER conversation_message_user_turn_role_update
BEFORE UPDATE OF role ON conversation_messages
WHEN NEW.role <> 'user' AND EXISTS (
    SELECT 1 FROM conversation_turns
    WHERE conversation_id = NEW.conversation_id
      AND input_kind = 'user_message'
      AND user_message_id = NEW.id
)
BEGIN SELECT RAISE(ABORT, 'send turn user message role is immutable'); END;

CREATE TABLE conversation_turns (
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
    prompt_revision INTEGER CHECK (prompt_revision IS NULL OR prompt_revision >= 1),
    memory_revision_id TEXT,
    selected_candidate_id TEXT,
    failure TEXT CHECK (failure IS NULL OR failure IN ('invalid_conversation', 'missing_model', 'context_unavailable', 'speaker_unavailable', 'provider_unavailable', 'provider_rejected', 'empty_output', 'cancelled', 'timed_out', 'recovery_unavailable', 'internal')),
    target_kind TEXT NOT NULL CHECK (target_kind IN ('new_assistant', 'existing_candidate')),
    target_message_id TEXT NOT NULL CHECK (length(trim(target_message_id)) > 0),
    target_parent_message_id TEXT,
    target_prior_candidate_id TEXT,
    retry_of_turn_id TEXT,
    guidance TEXT CHECK (guidance IS NULL OR (length(trim(guidance)) > 0 AND length(CAST(guidance AS BLOB)) <= 262144)),
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
    FOREIGN KEY (prompt_document_id) REFERENCES prompt_documents(id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, branch_id)
        REFERENCES conversation_branches(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, id, selected_candidate_id)
        REFERENCES conversation_message_candidates(conversation_id, turn_id, id)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (conversation_id, user_message_id) REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, user_message_id, branch_id)
        REFERENCES conversation_messages(conversation_id, id, branch_id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, head_message_id) REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, head_message_id, branch_id)
        REFERENCES conversation_messages(conversation_id, id, branch_id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, candidate_message_id) REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, candidate_message_id, candidate_id, branch_id)
        REFERENCES conversation_message_candidates(conversation_id, message_id, id, branch_id)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (conversation_id, selected_speaker_participant_id) REFERENCES conversation_participants(conversation_id, id) ON DELETE RESTRICT,
    CHECK ((input_kind = 'user_message' AND user_message_id IS NOT NULL AND head_message_id IS NULL AND candidate_message_id IS NULL AND candidate_id IS NULL) OR
           (input_kind = 'existing_head' AND user_message_id IS NULL AND head_message_id IS NOT NULL AND candidate_message_id IS NULL AND candidate_id IS NULL) OR
           (input_kind = 'existing_candidate' AND user_message_id IS NULL AND head_message_id IS NULL AND candidate_message_id IS NOT NULL AND candidate_id IS NOT NULL)),
    CHECK ((operation = 'send' AND input_kind = 'user_message') OR (operation = 'continue' AND input_kind = 'existing_head') OR (operation = 'regenerate' AND input_kind = 'existing_candidate')),
    CHECK ((prompt_document_id IS NULL) = (prompt_revision IS NULL)),
    CHECK ((status = 'failed') = (failure IS NOT NULL)),
    CHECK ((status = 'succeeded') = (selected_candidate_id IS NOT NULL)),
    CHECK ((target_kind = 'new_assistant' AND target_parent_message_id IS NOT NULL AND target_prior_candidate_id IS NULL) OR
          (target_kind = 'existing_candidate' AND target_parent_message_id IS NULL AND target_prior_candidate_id IS NOT NULL)),
    CHECK (created_at <= updated_at)
) STRICT;
CREATE INDEX conversation_turns_page_idx ON conversation_turns(conversation_id, created_at, id);
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
            AND head_message_id = NEW.head_message_id
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
            AND head_message_id = NEW.head_message_id
      ))
BEGIN SELECT RAISE(ABORT, 'turn input does not match branch semantics'); END;
CREATE TRIGGER conversation_turn_terminal_insert
BEFORE INSERT ON conversation_turns
WHEN NEW.status IN ('succeeded','failed','cancelled','interrupted')
BEGIN SELECT RAISE(ABORT, 'terminal turn requires settled attempts'); END;
CREATE TRIGGER conversation_turn_status_transition
BEFORE UPDATE OF status ON conversation_turns
WHEN NEW.status <> OLD.status AND NOT (
 (OLD.status = 'created' AND NEW.status IN ('preparing','cancellation_requested','cancelled')) OR
 (OLD.status = 'preparing' AND NEW.status IN ('selecting_speaker','context_prepared','cancellation_requested','failed','interrupted')) OR
 (OLD.status = 'selecting_speaker' AND NEW.status IN ('context_prepared','cancellation_requested','failed','interrupted')) OR
 (OLD.status = 'context_prepared' AND NEW.status IN ('running','cancellation_requested','failed','interrupted')) OR
 (OLD.status = 'running' AND NEW.status IN ('finalizing','cancellation_requested','failed','interrupted')) OR
 (OLD.status = 'cancellation_requested' AND NEW.status IN ('cancelled','failed','interrupted')) OR
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

CREATE TABLE conversation_snapshot_artifacts (
    artifact_id TEXT PRIMARY KEY,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('character', 'persona', 'scene', 'starter', 'prompt', 'lorebook', 'model', 'voice', 'group', 'provider_account')),
    source_id TEXT NOT NULL,
    source_revision INTEGER NOT NULL CHECK (source_revision >= 1),
    digest TEXT NOT NULL CHECK (length(digest) = 64 AND lower(digest) = digest AND digest NOT GLOB '*[^0-9a-f]*'),
    schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
    byte_size INTEGER NOT NULL CHECK (byte_size > 0 AND byte_size <= 16777216),
    codec TEXT NOT NULL CHECK (codec IN ('json', 'cbor', 'binary')),
    retention TEXT NOT NULL CHECK (retention = 'conversation'),
    bytes BLOB NOT NULL CHECK (length(bytes) = byte_size),
    created_at INTEGER NOT NULL,
    UNIQUE (artifact_id, digest, schema_version, byte_size, codec, retention),
    UNIQUE (artifact_id, source_kind)
) STRICT;

CREATE TABLE conversation_replay_artifacts (
    artifact_id TEXT PRIMARY KEY,
    digest TEXT NOT NULL CHECK (length(digest) = 64 AND lower(digest) = digest AND digest NOT GLOB '*[^0-9a-f]*'),
    schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
    byte_size INTEGER NOT NULL CHECK (byte_size > 0 AND byte_size <= 16777216),
    codec TEXT NOT NULL CHECK (codec IN ('json', 'cbor', 'binary')),
    retention TEXT NOT NULL CHECK (retention IN ('conversation', 'ephemeral')),
    bytes BLOB NOT NULL CHECK (length(bytes) = byte_size),
    created_at INTEGER NOT NULL,
    UNIQUE (artifact_id, retention)
) STRICT;

CREATE TABLE conversation_snapshot_refs (
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE RESTRICT,
    artifact_id TEXT NOT NULL REFERENCES conversation_snapshot_artifacts(artifact_id) ON DELETE RESTRICT,
    PRIMARY KEY (conversation_id, artifact_id)
) STRICT;
CREATE INDEX conversation_snapshot_refs_artifact_idx ON conversation_snapshot_refs(artifact_id);

CREATE TABLE conversation_message_revisions (
    conversation_id TEXT NOT NULL,
    id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    sequence INTEGER NOT NULL CHECK (sequence >= 1),
    parts_json TEXT NOT NULL CHECK (json_valid(parts_json) AND json_extract(parts_json, '$.format_version') = 1),
    authored_at INTEGER NOT NULL,
    source_turn_id TEXT,
    provider_replay_artifact_id TEXT,
    provider_replay_retention TEXT CHECK (provider_replay_retention IS NULL OR provider_replay_retention = 'conversation'),
    PRIMARY KEY (conversation_id, id),
    UNIQUE (conversation_id, message_id, sequence),
    UNIQUE (conversation_id, message_id, id),
    UNIQUE (conversation_id, id, provider_replay_artifact_id),
    CHECK ((provider_replay_artifact_id IS NULL) = (provider_replay_retention IS NULL)),
    FOREIGN KEY (conversation_id, message_id)
        REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (conversation_id, message_id, branch_id)
        REFERENCES conversation_messages(conversation_id, id, branch_id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (conversation_id, source_turn_id)
        REFERENCES conversation_turns(conversation_id, id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (conversation_id, source_turn_id, branch_id)
        REFERENCES conversation_turns(conversation_id, id, branch_id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (provider_replay_artifact_id, provider_replay_retention)
        REFERENCES conversation_replay_artifacts(artifact_id, retention)
        ON DELETE RESTRICT
) STRICT;
CREATE INDEX conversation_message_revisions_page_idx
    ON conversation_message_revisions(conversation_id, message_id, sequence, id);
CREATE TRIGGER conversation_revision_owner_immutable
BEFORE UPDATE OF conversation_id, id, message_id, branch_id, source_turn_id ON conversation_message_revisions
WHEN NEW.conversation_id <> OLD.conversation_id OR NEW.id <> OLD.id OR NEW.message_id <> OLD.message_id OR NEW.branch_id <> OLD.branch_id OR coalesce(NEW.source_turn_id, '') <> coalesce(OLD.source_turn_id, '')
BEGIN SELECT RAISE(ABORT, 'revision ownership is immutable'); END;

CREATE TABLE conversation_message_candidates (
    conversation_id TEXT NOT NULL,
    id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    parts_json TEXT NOT NULL CHECK (json_valid(parts_json) AND json_extract(parts_json, '$.format_version') = 1),
    model_json TEXT NOT NULL CHECK (json_valid(model_json) AND json_extract(model_json, '$.format_version') = 1),
    created_at INTEGER NOT NULL,
    provider_replay_artifact_id TEXT,
    provider_replay_retention TEXT CHECK (provider_replay_retention IS NULL OR provider_replay_retention = 'conversation'),
    PRIMARY KEY (conversation_id, id),
    UNIQUE (conversation_id, message_id, ordinal),
    UNIQUE (conversation_id, message_id, id),
    UNIQUE (conversation_id, message_id, id, branch_id),
    UNIQUE (conversation_id, turn_id, id),
    CHECK ((provider_replay_artifact_id IS NULL) = (provider_replay_retention IS NULL)),
    UNIQUE (conversation_id, id, provider_replay_artifact_id),
    FOREIGN KEY (conversation_id, message_id)
        REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, message_id, branch_id)
        REFERENCES conversation_messages(conversation_id, id, branch_id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, turn_id)
        REFERENCES conversation_turns(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, turn_id, branch_id)
        REFERENCES conversation_turns(conversation_id, id, branch_id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, turn_id, attempt_id)
        REFERENCES generation_attempts(conversation_id, turn_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (provider_replay_artifact_id, provider_replay_retention)
        REFERENCES conversation_replay_artifacts(artifact_id, retention)
        ON DELETE RESTRICT
) STRICT;
CREATE TRIGGER conversation_candidate_assistant
BEFORE INSERT ON conversation_message_candidates
WHEN NOT EXISTS (SELECT 1 FROM conversation_messages WHERE conversation_id = NEW.conversation_id AND id = NEW.message_id AND branch_id = NEW.branch_id AND role = 'assistant')
BEGIN SELECT RAISE(ABORT, 'candidate target must be assistant on turn branch'); END;
CREATE TRIGGER conversation_candidate_assistant_update
BEFORE UPDATE OF conversation_id, message_id, turn_id, branch_id ON conversation_message_candidates
WHEN NOT EXISTS (SELECT 1 FROM conversation_messages WHERE conversation_id = NEW.conversation_id AND id = NEW.message_id AND branch_id = NEW.branch_id AND role = 'assistant')
BEGIN SELECT RAISE(ABORT, 'candidate target must be assistant on turn branch'); END;
CREATE INDEX conversation_candidates_page_idx
    ON conversation_message_candidates(conversation_id, message_id, ordinal, id);

CREATE TABLE conversation_initial_message_origins (
    conversation_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    snapshot_artifact_id TEXT NOT NULL,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('scene', 'starter')),
    starter_message_id TEXT,
    PRIMARY KEY (conversation_id, message_id),
    FOREIGN KEY (conversation_id, message_id) REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (snapshot_artifact_id, source_kind) REFERENCES conversation_snapshot_artifacts(artifact_id, source_kind) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, snapshot_artifact_id) REFERENCES conversation_snapshot_refs(conversation_id, artifact_id) ON DELETE RESTRICT,
    CHECK ((source_kind = 'scene' AND starter_message_id IS NULL) OR
           (source_kind = 'starter' AND starter_message_id IS NOT NULL)),
    UNIQUE (conversation_id, message_id, snapshot_artifact_id)
) STRICT;
CREATE INDEX conversation_initial_message_origins_source_idx
    ON conversation_initial_message_origins(conversation_id, source_kind, snapshot_artifact_id);

CREATE TABLE generation_attempts (
    conversation_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    parent_attempt_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('created', 'preparing', 'running', 'succeeded', 'failed', 'cancelled', 'interrupted')),
    job_idempotency_key TEXT NOT NULL CHECK (job_idempotency_key = 'generation.' || turn_id || '.' || id),
    job_id TEXT,
    started_at INTEGER,
    finished_at INTEGER,
    usage_event_id TEXT,
    usage_outcome TEXT CHECK (usage_outcome IS NULL OR usage_outcome IN ('succeeded', 'failed', 'cancelled', 'interrupted')),
    failure TEXT CHECK (failure IS NULL OR failure IN ('invalid_conversation', 'missing_model', 'context_unavailable', 'speaker_unavailable', 'provider_unavailable', 'provider_rejected', 'empty_output', 'cancelled', 'timed_out', 'recovery_unavailable', 'internal')),
    PRIMARY KEY (conversation_id, turn_id, id),
    UNIQUE (conversation_id, turn_id, ordinal),
    UNIQUE (job_idempotency_key),
    UNIQUE (conversation_id, turn_id, id, job_id),
    UNIQUE (conversation_id, turn_id, id, usage_event_id),
    FOREIGN KEY (conversation_id, turn_id)
        REFERENCES conversation_turns(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, turn_id, parent_attempt_id)
        REFERENCES generation_attempts(conversation_id, turn_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, turn_id, id, usage_event_id)
        REFERENCES conversation_usage_refs(conversation_id, turn_id, attempt_id, usage_event_id)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (conversation_id, turn_id, id, usage_event_id, usage_outcome)
        REFERENCES conversation_usage_refs(conversation_id, turn_id, attempt_id, usage_event_id, outcome)
        DEFERRABLE INITIALLY DEFERRED,
    CHECK ((ordinal = 0 AND parent_attempt_id IS NULL) OR (ordinal > 0 AND parent_attempt_id IS NOT NULL)),
    CHECK ((status IN ('succeeded', 'failed', 'cancelled', 'interrupted')) = (usage_event_id IS NOT NULL)),
    CHECK ((status IN ('succeeded', 'failed', 'cancelled', 'interrupted')) = (usage_outcome IS NOT NULL)),
    CHECK (status NOT IN ('succeeded', 'failed', 'cancelled', 'interrupted') OR ((status = 'succeeded' AND usage_outcome = 'succeeded') OR (status = 'failed' AND usage_outcome = 'failed') OR (status = 'cancelled' AND usage_outcome = 'cancelled') OR (status = 'interrupted' AND usage_outcome = 'interrupted'))),
    CHECK ((status = 'failed') = (failure IS NOT NULL)),
    CHECK ((status IN ('created', 'preparing')) = (started_at IS NULL AND finished_at IS NULL)),
    CHECK ((status = 'running') = (started_at IS NOT NULL AND finished_at IS NULL)),
    CHECK ((status IN ('succeeded', 'failed', 'cancelled', 'interrupted')) = (started_at IS NOT NULL AND finished_at IS NOT NULL)),
    CHECK (started_at IS NULL OR finished_at IS NULL OR started_at <= finished_at)
) STRICT;
CREATE UNIQUE INDEX generation_attempts_job_id_uq ON generation_attempts(job_id) WHERE job_id IS NOT NULL;
CREATE TRIGGER generation_attempt_parent_ordinal
BEFORE INSERT ON generation_attempts
WHEN NEW.parent_attempt_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM generation_attempts
    WHERE conversation_id = NEW.conversation_id AND turn_id = NEW.turn_id
      AND id = NEW.parent_attempt_id AND ordinal = NEW.ordinal - 1 AND status = 'interrupted'
)
BEGIN SELECT RAISE(ABORT, 'generation attempt parent must be immediate predecessor'); END;
CREATE TRIGGER generation_attempt_identity_immutable
BEFORE UPDATE OF turn_id, id, ordinal, parent_attempt_id ON generation_attempts
WHEN NEW.turn_id <> OLD.turn_id OR NEW.id <> OLD.id OR NEW.ordinal <> OLD.ordinal OR coalesce(NEW.parent_attempt_id, '') <> coalesce(OLD.parent_attempt_id, '')
BEGIN SELECT RAISE(ABORT, 'generation attempt identity is immutable'); END;
CREATE TRIGGER generation_attempt_terminal_immutable
BEFORE UPDATE OF status ON generation_attempts
WHEN OLD.status IN ('succeeded', 'failed', 'cancelled', 'interrupted') AND NEW.status <> OLD.status
BEGIN SELECT RAISE(ABORT, 'terminal generation attempt cannot regress'); END;

CREATE TABLE turn_lorebooks (
    conversation_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    lorebook_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (conversation_id, turn_id, lorebook_id),
    UNIQUE (conversation_id, turn_id, ordinal),
    FOREIGN KEY (conversation_id, turn_id)
        REFERENCES conversation_turns(conversation_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (lorebook_id) REFERENCES lorebooks(id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE generation_checkpoints (
    conversation_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    sequence INTEGER NOT NULL CHECK (sequence >= 1),
    job_id TEXT,
    correlation_id TEXT,
    event_json TEXT NOT NULL CHECK (json_valid(event_json) AND json_extract(event_json, '$.format_version') = 1),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (conversation_id, turn_id, attempt_id, sequence),
    FOREIGN KEY (conversation_id, turn_id, attempt_id, job_id)
        REFERENCES generation_attempts(conversation_id, turn_id, id, job_id)
        ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, turn_id, attempt_id)
        REFERENCES generation_attempts(conversation_id, turn_id, id) ON DELETE RESTRICT
) STRICT;
CREATE TRIGGER generation_checkpoint_contiguous
BEFORE INSERT ON generation_checkpoints
WHEN NEW.sequence <> COALESCE((SELECT max(sequence) + 1 FROM generation_checkpoints WHERE conversation_id = NEW.conversation_id AND turn_id = NEW.turn_id AND attempt_id = NEW.attempt_id), 1)
BEGIN SELECT RAISE(ABORT, 'generation checkpoint sequence must be contiguous'); END;

CREATE TABLE revision_media_refs (
    conversation_id TEXT NOT NULL,
    message_revision_id TEXT NOT NULL,
    part_ordinal INTEGER NOT NULL CHECK (part_ordinal >= 0),
    asset_id TEXT NOT NULL REFERENCES media_assets(id) ON DELETE RESTRICT,
    media_role TEXT NOT NULL CHECK (media_role IN ('inline', 'attachment', 'avatar', 'scene', 'reference')),
    state TEXT NOT NULL CHECK (state IN ('active', 'historical')),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (conversation_id, message_revision_id, part_ordinal),
    FOREIGN KEY (conversation_id, message_revision_id)
        REFERENCES conversation_message_revisions(conversation_id, id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE candidate_media_refs (
    conversation_id TEXT NOT NULL,
    candidate_id TEXT NOT NULL,
    part_ordinal INTEGER NOT NULL CHECK (part_ordinal >= 0),
    asset_id TEXT NOT NULL REFERENCES media_assets(id) ON DELETE RESTRICT,
    media_role TEXT NOT NULL CHECK (media_role IN ('inline', 'attachment', 'avatar', 'scene', 'reference')),
    state TEXT NOT NULL CHECK (state IN ('active', 'historical')),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (conversation_id, candidate_id, part_ordinal),
    FOREIGN KEY (conversation_id, candidate_id)
        REFERENCES conversation_message_candidates(conversation_id, id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE conversation_usage_refs (
    conversation_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    usage_event_id TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('succeeded', 'failed', 'cancelled', 'interrupted')),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (conversation_id, usage_event_id),
    UNIQUE (conversation_id, turn_id, attempt_id, usage_event_id),
    UNIQUE (conversation_id, turn_id, attempt_id, usage_event_id, outcome),
    FOREIGN KEY (conversation_id, turn_id, attempt_id)
        REFERENCES generation_attempts(conversation_id, turn_id, id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE conversation_operations (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK (kind IN ('create', 'send', 'continue', 'regenerate', 'retry', 'checkpoint', 'cancel', 'finalize', 'fail', 'recover', 'choose_candidate', 'edit', 'fork', 'select_branch', 'tombstone', 'archive', 'restore', 'participant_policy', 'settings', 'attach_job')),
    operation_key TEXT NOT NULL CHECK (length(trim(operation_key)) > 0),
    request_digest TEXT NOT NULL CHECK (length(request_digest) = 64 AND lower(request_digest) = request_digest AND request_digest NOT GLOB '*[^0-9a-f]*'),
    result_kind TEXT NOT NULL CHECK (result_kind IN ('conversation', 'turn', 'message', 'candidate', 'branch')),
    result_id TEXT NOT NULL,
    result_json TEXT NOT NULL CHECK (json_valid(result_json) AND json_extract(result_json, '$.format_version') = 1),
    created_at INTEGER NOT NULL,
    UNIQUE (conversation_id, kind, operation_key),
    UNIQUE (conversation_id, id)
) STRICT;

CREATE TABLE conversation_outbox (
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE RESTRICT,
    id TEXT NOT NULL,
    sequence INTEGER NOT NULL CHECK (sequence >= 1),
    conversation_revision INTEGER NOT NULL CHECK (conversation_revision >= 1),
    operation_record_id TEXT NOT NULL,
    at INTEGER NOT NULL,
    event_json TEXT NOT NULL CHECK (json_valid(event_json) AND json_extract(event_json, '$.format_version') = 1),
    PRIMARY KEY (conversation_id, id),
    UNIQUE (conversation_id, sequence),
    FOREIGN KEY (conversation_id, operation_record_id)
        REFERENCES conversation_operations(conversation_id, id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX conversation_outbox_page_idx ON conversation_outbox(conversation_id, sequence, id);

CREATE INDEX conversation_turns_id_idx ON conversation_turns(id);
CREATE INDEX conversation_message_revisions_id_idx ON conversation_message_revisions(id);
CREATE INDEX conversation_message_candidates_id_idx ON conversation_message_candidates(id);

CREATE TRIGGER conversation_candidate_final_target_insert
BEFORE INSERT ON conversation_message_candidates
WHEN EXISTS (
    SELECT 1 FROM conversation_turns AS turn
    WHERE turn.conversation_id = NEW.conversation_id
      AND turn.id = NEW.turn_id
      AND (
          NEW.message_id <> turn.target_message_id OR
          (turn.target_kind = 'new_assistant' AND NOT EXISTS (
              SELECT 1 FROM conversation_messages AS message
              WHERE message.conversation_id = NEW.conversation_id
                AND message.id = turn.target_message_id
                AND message.branch_id = NEW.branch_id
                AND message.role = 'assistant'
                AND message.parent_message_id = turn.target_parent_message_id
          ))
      )
)
BEGIN SELECT RAISE(ABORT, 'candidate does not match turn target'); END;

CREATE TRIGGER conversation_candidate_final_target_update
BEFORE UPDATE OF conversation_id, message_id, branch_id, turn_id ON conversation_message_candidates
WHEN EXISTS (
    SELECT 1 FROM conversation_turns AS turn
    WHERE turn.conversation_id = NEW.conversation_id
      AND turn.id = NEW.turn_id
      AND (
          NEW.message_id <> turn.target_message_id OR
          (turn.target_kind = 'new_assistant' AND NOT EXISTS (
              SELECT 1 FROM conversation_messages AS message
              WHERE message.conversation_id = NEW.conversation_id
                AND message.id = turn.target_message_id
                AND message.branch_id = NEW.branch_id
                AND message.role = 'assistant'
                AND message.parent_message_id = turn.target_parent_message_id
          ))
      )
)
BEGIN SELECT RAISE(ABORT, 'candidate does not match turn target'); END;





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

CREATE TRIGGER conversation_turn_retry_source_delete
BEFORE DELETE ON conversation_turns
WHEN EXISTS (
    SELECT 1 FROM conversation_turns AS child
    WHERE child.retry_of_turn_id = OLD.id
      AND child.conversation_id = OLD.conversation_id
)
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

CREATE TRIGGER conversation_message_identity_immutable
BEFORE UPDATE OF conversation_id, id, branch_id, parent_message_id ON conversation_messages
WHEN NEW.conversation_id <> OLD.conversation_id
   OR NEW.id <> OLD.id
   OR NEW.branch_id <> OLD.branch_id
   OR coalesce(NEW.parent_message_id, '') <> coalesce(OLD.parent_message_id, '')
BEGIN SELECT RAISE(ABORT, 'message topology is immutable'); END;

CREATE TRIGGER conversation_branch_head_same_branch_insert
BEFORE INSERT ON conversation_branches
WHEN NEW.head_message_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_messages
    WHERE conversation_id = NEW.conversation_id
      AND id = NEW.head_message_id
      AND branch_id = NEW.id
)
BEGIN SELECT RAISE(ABORT, 'branch head must belong to branch'); END;

CREATE TRIGGER conversation_branch_head_same_branch_update
BEFORE UPDATE OF conversation_id, id, head_message_id ON conversation_branches
WHEN NEW.head_message_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_messages
    WHERE conversation_id = NEW.conversation_id
      AND id = NEW.head_message_id
      AND branch_id = NEW.id
)
BEGIN SELECT RAISE(ABORT, 'branch head must belong to branch'); END;

CREATE TRIGGER conversation_branch_fork_message_parent_insert
BEFORE INSERT ON conversation_branches
WHEN NEW.fork_message_id IS NOT NULL AND NEW.parent_branch_id <> NEW.id AND NOT EXISTS (
    SELECT 1 FROM conversation_messages
    WHERE conversation_id = NEW.conversation_id
      AND id = NEW.fork_message_id
      AND branch_id = NEW.parent_branch_id
)
BEGIN SELECT RAISE(ABORT, 'fork message must belong to parent branch'); END;

CREATE TRIGGER conversation_branch_fork_message_parent_update
BEFORE UPDATE OF conversation_id, id, parent_branch_id, fork_message_id ON conversation_branches
WHEN NEW.fork_message_id IS NOT NULL AND NEW.parent_branch_id <> NEW.id AND NOT EXISTS (
    SELECT 1 FROM conversation_messages
    WHERE conversation_id = NEW.conversation_id
      AND id = NEW.fork_message_id
      AND branch_id = NEW.parent_branch_id
)
BEGIN SELECT RAISE(ABORT, 'fork message must belong to parent branch'); END;

CREATE TRIGGER conversation_branch_parent_fork_immutable
BEFORE UPDATE OF parent_branch_id, fork_message_id ON conversation_branches
WHEN coalesce(NEW.parent_branch_id, '') <> coalesce(OLD.parent_branch_id, '')
   OR coalesce(NEW.fork_message_id, '') <> coalesce(OLD.fork_message_id, '')
BEGIN SELECT RAISE(ABORT, 'branch topology is immutable'); END;

CREATE TRIGGER conversation_message_parent_topology_insert
BEFORE INSERT ON conversation_messages
WHEN NEW.parent_message_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_messages AS parent
    WHERE parent.conversation_id = NEW.conversation_id
      AND parent.id = NEW.parent_message_id
      AND (
          parent.branch_id = NEW.branch_id OR
          EXISTS (
              SELECT 1 FROM conversation_branches AS branch
              WHERE branch.conversation_id = NEW.conversation_id
                AND branch.id = NEW.branch_id
                AND branch.parent_branch_id = parent.branch_id
                AND branch.fork_message_id = parent.id
          )
      )
)
BEGIN SELECT RAISE(ABORT, 'message parent violates branch topology'); END;

CREATE TRIGGER conversation_message_parent_topology_update
BEFORE UPDATE OF conversation_id, id, branch_id, parent_message_id ON conversation_messages
WHEN NEW.parent_message_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_messages AS parent
    WHERE parent.conversation_id = NEW.conversation_id
      AND parent.id = NEW.parent_message_id
      AND (
          parent.branch_id = NEW.branch_id OR
          EXISTS (
              SELECT 1 FROM conversation_branches AS branch
              WHERE branch.conversation_id = NEW.conversation_id
                AND branch.id = NEW.branch_id
                AND branch.parent_branch_id = parent.branch_id
                AND branch.fork_message_id = parent.id
          )
      )
)
BEGIN SELECT RAISE(ABORT, 'message parent violates branch topology'); END;

-- Conversation launch artifacts, origin indexes, idempotency records, and
-- outbox events are append-only facts.  Mutations create a new domain record
-- (or a new operation) rather than rewriting history in place.
CREATE TRIGGER conversation_snapshot_artifact_immutable_update
BEFORE UPDATE ON conversation_snapshot_artifacts
BEGIN SELECT RAISE(ABORT, 'snapshot artifact is immutable'); END;

CREATE TRIGGER conversation_snapshot_ref_immutable_update
BEFORE UPDATE ON conversation_snapshot_refs
BEGIN SELECT RAISE(ABORT, 'snapshot reference is immutable'); END;

CREATE TRIGGER conversation_snapshot_ref_immutable_delete
BEFORE DELETE ON conversation_snapshot_refs
BEGIN SELECT RAISE(ABORT, 'snapshot reference is immutable'); END;

CREATE TRIGGER conversation_initial_origin_immutable_update
BEFORE UPDATE ON conversation_initial_message_origins
BEGIN SELECT RAISE(ABORT, 'initial message origin is immutable'); END;

CREATE TRIGGER conversation_initial_origin_immutable_delete
BEFORE DELETE ON conversation_initial_message_origins
BEGIN SELECT RAISE(ABORT, 'initial message origin is immutable'); END;

CREATE TRIGGER conversation_initial_origin_limit
BEFORE INSERT ON conversation_initial_message_origins
WHEN (SELECT count(*) FROM conversation_initial_message_origins
      WHERE conversation_id = NEW.conversation_id) >= 512
BEGIN SELECT RAISE(ABORT, 'conversation initial timeline exceeds 512 origins'); END;

CREATE TRIGGER conversation_initial_origin_after_create_forbidden
BEFORE INSERT ON conversation_initial_message_origins
WHEN EXISTS (
    SELECT 1 FROM conversation_operations
    WHERE conversation_id = NEW.conversation_id AND kind = 'create'
)
BEGIN SELECT RAISE(ABORT, 'initial message origin cannot be added after create'); END;

CREATE TRIGGER conversation_operation_immutable_update
BEFORE UPDATE ON conversation_operations
BEGIN SELECT RAISE(ABORT, 'conversation operation is immutable'); END;

CREATE TRIGGER conversation_operation_immutable_delete
BEFORE DELETE ON conversation_operations
BEGIN SELECT RAISE(ABORT, 'conversation operation is immutable'); END;

CREATE TRIGGER conversation_create_operation_shape
BEFORE INSERT ON conversation_operations
WHEN NEW.kind = 'create' AND (
    NEW.result_kind IS NOT 'conversation' OR
    NEW.result_id IS NOT NEW.conversation_id OR
    json_extract(NEW.result_json, '$.value.kind') IS NOT 'conversation' OR
    json_extract(NEW.result_json, '$.value.id') IS NOT NEW.conversation_id
)
BEGIN SELECT RAISE(ABORT, 'create operation result must be its conversation'); END;

CREATE TRIGGER conversation_outbox_immutable_update
BEFORE UPDATE ON conversation_outbox
BEGIN SELECT RAISE(ABORT, 'conversation outbox event is immutable'); END;

CREATE TRIGGER conversation_outbox_immutable_delete
BEFORE DELETE ON conversation_outbox
BEGIN SELECT RAISE(ABORT, 'conversation outbox event is immutable'); END;

CREATE TRIGGER conversation_create_outbox_shape
BEFORE INSERT ON conversation_outbox
WHEN EXISTS (
    SELECT 1 FROM conversation_operations AS operation
    WHERE operation.conversation_id = NEW.conversation_id
      AND operation.id = NEW.operation_record_id
      AND operation.kind = 'create'
) AND (
    NEW.sequence IS NOT 1 OR
    NEW.conversation_revision IS NOT 1 OR
    json_extract(NEW.event_json, '$.value.kind') IS NOT 'conversation_created' OR
    json_extract(NEW.event_json, '$.value.value.conversation_id') IS NOT NEW.conversation_id OR
    json_extract(NEW.event_json, '$.value.value.at') IS NOT NEW.at
)
BEGIN SELECT RAISE(ABORT, 'create outbox event has invalid shape'); END;

CREATE TRIGGER conversation_created_event_requires_create_operation
BEFORE INSERT ON conversation_outbox
WHEN json_extract(NEW.event_json, '$.value.kind') IS 'conversation_created'
 AND NOT EXISTS (
    SELECT 1 FROM conversation_operations
    WHERE conversation_id = NEW.conversation_id
      AND id = NEW.operation_record_id
      AND kind = 'create'
 )
BEGIN SELECT RAISE(ABORT, 'conversation_created event requires create operation'); END;

CREATE TRIGGER conversation_create_outbox_one_per_operation
BEFORE INSERT ON conversation_outbox
WHEN EXISTS (
    SELECT 1 FROM conversation_operations AS operation
    WHERE operation.conversation_id = NEW.conversation_id
      AND operation.id = NEW.operation_record_id
      AND operation.kind = 'create'
) AND EXISTS (
    SELECT 1 FROM conversation_outbox AS existing
    WHERE existing.conversation_id = NEW.conversation_id
      AND existing.operation_record_id = NEW.operation_record_id
)
BEGIN SELECT RAISE(ABORT, 'create operation already has an outbox event'); END;
