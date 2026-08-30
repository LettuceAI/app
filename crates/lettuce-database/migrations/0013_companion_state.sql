CREATE TABLE companion_relationship_states (
    character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    persona_key TEXT NOT NULL,
    persona_id TEXT,
    closeness REAL NOT NULL CHECK (closeness BETWEEN -1.0 AND 1.0),
    trust REAL NOT NULL CHECK (trust BETWEEN -1.0 AND 1.0),
    affection REAL NOT NULL CHECK (affection BETWEEN -1.0 AND 1.0),
    tension REAL NOT NULL CHECK (tension BETWEEN 0.0 AND 1.0),
    stability REAL NOT NULL CHECK (stability BETWEEN 0.0 AND 1.0),
    interaction_count INTEGER NOT NULL CHECK (interaction_count >= 0),
    last_interaction_at INTEGER NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (character_id, persona_key),
    CHECK (
        (persona_id IS NULL AND persona_key = '__default__') OR
        (persona_id IS NOT NULL AND persona_key = persona_id)
    ),
    CHECK (created_at <= updated_at)
) STRICT;

CREATE TABLE companion_session_states (
    conversation_id TEXT PRIMARY KEY REFERENCES conversations(id) ON DELETE RESTRICT,
    character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE RESTRICT,
    persona_key TEXT NOT NULL,
    persona_id TEXT,
    initial_hash BLOB NOT NULL CHECK (length(initial_hash) = 32),
    confidence REAL NOT NULL CHECK (confidence BETWEEN 0.0 AND 1.0),
    emotional_updated_at INTEGER NOT NULL,
    state_updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (character_id, persona_key)
        REFERENCES companion_relationship_states(character_id, persona_key) ON DELETE RESTRICT,
    CHECK (
        (persona_id IS NULL AND persona_key = '__default__') OR
        (persona_id IS NOT NULL AND persona_key = persona_id)
    ),
    CHECK (emotional_updated_at <= state_updated_at),
    CHECK (created_at <= updated_at)
) STRICT;

CREATE TABLE companion_emotion_vectors (
    conversation_id TEXT NOT NULL REFERENCES companion_session_states(conversation_id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('felt', 'expressed', 'blocked', 'momentum')),
    warmth REAL NOT NULL,
    trust REAL NOT NULL,
    calm REAL NOT NULL,
    vulnerability REAL NOT NULL,
    longing REAL NOT NULL,
    hurt REAL NOT NULL,
    tension REAL NOT NULL,
    irritation REAL NOT NULL,
    affection_intensity REAL NOT NULL,
    reassurance_need REAL NOT NULL,
    PRIMARY KEY (conversation_id, kind),
    CHECK (
        (kind = 'momentum' AND
            warmth BETWEEN -1.0 AND 1.0 AND trust BETWEEN -1.0 AND 1.0 AND
            calm BETWEEN -1.0 AND 1.0 AND vulnerability BETWEEN -1.0 AND 1.0 AND
            longing BETWEEN -1.0 AND 1.0 AND hurt BETWEEN -1.0 AND 1.0 AND
            tension BETWEEN -1.0 AND 1.0 AND irritation BETWEEN -1.0 AND 1.0 AND
            affection_intensity BETWEEN -1.0 AND 1.0 AND reassurance_need BETWEEN -1.0 AND 1.0) OR
        (kind != 'momentum' AND
            warmth BETWEEN 0.0 AND 1.0 AND trust BETWEEN 0.0 AND 1.0 AND
            calm BETWEEN 0.0 AND 1.0 AND vulnerability BETWEEN 0.0 AND 1.0 AND
            longing BETWEEN 0.0 AND 1.0 AND hurt BETWEEN 0.0 AND 1.0 AND
            tension BETWEEN 0.0 AND 1.0 AND irritation BETWEEN 0.0 AND 1.0 AND
            affection_intensity BETWEEN 0.0 AND 1.0 AND reassurance_need BETWEEN 0.0 AND 1.0)
    )
) STRICT;

CREATE TABLE companion_state_signals (
    conversation_id TEXT NOT NULL REFERENCES companion_session_states(conversation_id) ON DELETE CASCADE,
    scope TEXT NOT NULL CHECK (scope IN ('driver', 'active')),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    value TEXT NOT NULL CHECK (length(trim(value)) > 0),
    PRIMARY KEY (conversation_id, scope, ordinal)
) STRICT;

CREATE TABLE companion_state_apply_receipts (
    operation_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES companion_session_states(conversation_id) ON DELETE RESTRICT,
    character_id TEXT NOT NULL,
    persona_key TEXT NOT NULL,
    expected_session_revision INTEGER NOT NULL CHECK (expected_session_revision >= 1),
    resulting_session_revision INTEGER NOT NULL CHECK (resulting_session_revision = expected_session_revision + 1),
    expected_relationship_revision INTEGER NOT NULL CHECK (expected_relationship_revision >= 1),
    resulting_relationship_revision INTEGER NOT NULL CHECK (resulting_relationship_revision = expected_relationship_revision + 1),
    applied_at INTEGER NOT NULL,
    change_hash BLOB NOT NULL CHECK (length(change_hash) = 32),
    FOREIGN KEY (character_id, persona_key)
        REFERENCES companion_relationship_states(character_id, persona_key) ON DELETE RESTRICT
) STRICT;

CREATE INDEX companion_state_receipts_owner_idx
    ON companion_state_apply_receipts(conversation_id, applied_at, operation_id);

CREATE TRIGGER companion_state_receipts_immutable_update
BEFORE UPDATE ON companion_state_apply_receipts
BEGIN
    SELECT RAISE(ABORT, 'companion state apply receipts are immutable');
END;

CREATE TRIGGER companion_state_receipts_immutable_delete
BEFORE DELETE ON companion_state_apply_receipts
BEGIN
    SELECT RAISE(ABORT, 'companion state apply receipts are immutable');
END;

CREATE TABLE companion_turn_effect_drafts (
    conversation_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    effect_id TEXT NOT NULL UNIQUE,
    user_message_id TEXT,
    closeness_delta REAL NOT NULL CHECK (closeness_delta BETWEEN -2.0 AND 2.0),
    trust_delta REAL NOT NULL CHECK (trust_delta BETWEEN -2.0 AND 2.0),
    affection_delta REAL NOT NULL CHECK (affection_delta BETWEEN -2.0 AND 2.0),
    tension_delta REAL NOT NULL CHECK (tension_delta BETWEEN -2.0 AND 2.0),
    stability_delta REAL NOT NULL CHECK (stability_delta BETWEEN -2.0 AND 2.0),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (conversation_id, turn_id),
    UNIQUE (conversation_id, turn_id, effect_id),
    FOREIGN KEY (conversation_id, turn_id)
        REFERENCES conversation_turns(conversation_id, id) ON DELETE CASCADE,
    FOREIGN KEY (conversation_id, user_message_id)
        REFERENCES conversation_messages(conversation_id, id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE companion_turn_effect_emotion_deltas (
    conversation_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('felt', 'expressed', 'blocked')),
    warmth REAL NOT NULL CHECK (warmth BETWEEN -1.0 AND 1.0),
    trust REAL NOT NULL CHECK (trust BETWEEN -1.0 AND 1.0),
    calm REAL NOT NULL CHECK (calm BETWEEN -1.0 AND 1.0),
    vulnerability REAL NOT NULL CHECK (vulnerability BETWEEN -1.0 AND 1.0),
    longing REAL NOT NULL CHECK (longing BETWEEN -1.0 AND 1.0),
    hurt REAL NOT NULL CHECK (hurt BETWEEN -1.0 AND 1.0),
    tension REAL NOT NULL CHECK (tension BETWEEN -1.0 AND 1.0),
    irritation REAL NOT NULL CHECK (irritation BETWEEN -1.0 AND 1.0),
    affection_intensity REAL NOT NULL CHECK (affection_intensity BETWEEN -1.0 AND 1.0),
    reassurance_need REAL NOT NULL CHECK (reassurance_need BETWEEN -1.0 AND 1.0),
    PRIMARY KEY (conversation_id, turn_id, kind),
    FOREIGN KEY (conversation_id, turn_id)
        REFERENCES companion_turn_effect_drafts(conversation_id, turn_id) ON DELETE CASCADE
) STRICT;

CREATE TABLE companion_turn_effect_signal_changes (
    conversation_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    change_kind TEXT NOT NULL CHECK (change_kind IN ('added', 'removed')),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    value TEXT NOT NULL CHECK (length(trim(value)) > 0 AND length(CAST(value AS BLOB)) <= 256),
    PRIMARY KEY (conversation_id, turn_id, change_kind, ordinal),
    FOREIGN KEY (conversation_id, turn_id)
        REFERENCES companion_turn_effect_drafts(conversation_id, turn_id) ON DELETE CASCADE
) STRICT;

CREATE TABLE companion_turn_effects (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    user_message_id TEXT,
    assistant_message_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('processing', 'ready', 'failed')),
    summary TEXT CHECK (summary IS NULL OR length(CAST(summary AS BLOB)) <= 8192),
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

CREATE TABLE companion_turn_effect_memory_changes (
    effect_id TEXT NOT NULL REFERENCES companion_turn_effects(id) ON DELETE CASCADE,
    change_kind TEXT NOT NULL CHECK (change_kind IN ('added', 'updated', 'superseded')),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    memory_id TEXT NOT NULL,
    PRIMARY KEY (effect_id, change_kind, ordinal)
) STRICT;

CREATE TABLE companion_turn_effect_source_messages (
    effect_id TEXT NOT NULL REFERENCES companion_turn_effects(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    message_id TEXT NOT NULL,
    PRIMARY KEY (effect_id, ordinal)
) STRICT;

CREATE TRIGGER companion_turn_effect_terminal_update
BEFORE UPDATE ON companion_turn_effects
WHEN OLD.status IN ('ready', 'failed')
BEGIN
    SELECT RAISE(ABORT, 'terminal companion turn effect is immutable');
END;

CREATE TRIGGER companion_turn_effect_no_delete
BEFORE DELETE ON companion_turn_effects
BEGIN
    SELECT RAISE(ABORT, 'companion turn effect is immutable');
END;

CREATE TRIGGER companion_turn_effect_memory_insert_open
BEFORE INSERT ON companion_turn_effect_memory_changes
WHEN (SELECT status FROM companion_turn_effects WHERE id = NEW.effect_id) != 'processing'
BEGIN
    SELECT RAISE(ABORT, 'terminal companion turn effect children are immutable');
END;

CREATE TRIGGER companion_turn_effect_memory_update_open
BEFORE UPDATE ON companion_turn_effect_memory_changes
WHEN (SELECT status FROM companion_turn_effects WHERE id = OLD.effect_id) != 'processing'
BEGIN
    SELECT RAISE(ABORT, 'terminal companion turn effect children are immutable');
END;

CREATE TRIGGER companion_turn_effect_memory_delete_open
BEFORE DELETE ON companion_turn_effect_memory_changes
WHEN (SELECT status FROM companion_turn_effects WHERE id = OLD.effect_id) != 'processing'
BEGIN
    SELECT RAISE(ABORT, 'terminal companion turn effect children are immutable');
END;

CREATE TRIGGER companion_turn_effect_source_insert_open
BEFORE INSERT ON companion_turn_effect_source_messages
WHEN (SELECT status FROM companion_turn_effects WHERE id = NEW.effect_id) != 'processing'
BEGIN
    SELECT RAISE(ABORT, 'terminal companion turn effect children are immutable');
END;

CREATE TRIGGER companion_turn_effect_source_update_open
BEFORE UPDATE ON companion_turn_effect_source_messages
WHEN (SELECT status FROM companion_turn_effects WHERE id = OLD.effect_id) != 'processing'
BEGIN
    SELECT RAISE(ABORT, 'terminal companion turn effect children are immutable');
END;

CREATE TRIGGER companion_turn_effect_source_delete_open
BEFORE DELETE ON companion_turn_effect_source_messages
WHEN (SELECT status FROM companion_turn_effects WHERE id = OLD.effect_id) != 'processing'
BEGIN
    SELECT RAISE(ABORT, 'terminal companion turn effect children are immutable');
END;
