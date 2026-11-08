CREATE TABLE group_companion_memory_spaces (
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    branch_id TEXT NOT NULL,
    character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    space_id TEXT NOT NULL UNIQUE REFERENCES memory_spaces(id) ON DELETE RESTRICT,
    PRIMARY KEY (conversation_id, branch_id, character_id),
    FOREIGN KEY (conversation_id, branch_id)
        REFERENCES conversation_branches(conversation_id, id) ON DELETE RESTRICT
        DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE INDEX group_companion_memory_spaces_space_idx ON group_companion_memory_spaces(space_id);