CREATE TABLE group_companion_checkpoints (
    conversation_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    source_revision_id TEXT,
    source_candidate_id TEXT,
    emotional_state_json TEXT NOT NULL CHECK (json_valid(emotional_state_json)),
    member_relationships_json TEXT NOT NULL CHECK (
        json_valid(member_relationships_json) AND json_type(member_relationships_json) = 'array'
    ),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (conversation_id, message_id, character_id),
    FOREIGN KEY (conversation_id, message_id)
        REFERENCES conversation_messages(conversation_id, id) ON DELETE CASCADE
) STRICT;

CREATE INDEX group_companion_checkpoints_character_idx
    ON group_companion_checkpoints(conversation_id, character_id);

CREATE TABLE group_companion_effects (
    conversation_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    character_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    effect_json TEXT NOT NULL CHECK (json_valid(effect_json)),
    PRIMARY KEY (conversation_id, message_id, character_id, ordinal),
    FOREIGN KEY (conversation_id, message_id, character_id)
        REFERENCES group_companion_checkpoints(conversation_id, message_id, character_id) ON DELETE CASCADE
) STRICT;

CREATE TABLE group_companion_persona_applications (
    conversation_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    persona_key TEXT NOT NULL CHECK (length(trim(persona_key)) > 0),
    applied_at INTEGER NOT NULL,
    PRIMARY KEY (conversation_id, message_id, character_id),
    FOREIGN KEY (conversation_id, message_id)
        REFERENCES conversation_messages(conversation_id, id) ON DELETE CASCADE
) STRICT;
