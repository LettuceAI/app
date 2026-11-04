ALTER TABLE groups ADD COLUMN time_awareness INTEGER NOT NULL DEFAULT 0 CHECK (time_awareness IN (0, 1));
ALTER TABLE groups ADD COLUMN disable_character_notes INTEGER NOT NULL DEFAULT 0 CHECK (disable_character_notes IN (0, 1));
ALTER TABLE conversation_settings ADD COLUMN disable_character_notes INTEGER CHECK (disable_character_notes IS NULL OR disable_character_notes IN (0, 1));

CREATE TABLE group_scheduled_notes (
    id TEXT PRIMARY KEY,
    group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    visible_to_json TEXT NOT NULL CHECK (json_valid(visible_to_json) AND json_type(visible_to_json) = 'array'),
    label TEXT NOT NULL,
    content TEXT NOT NULL CHECK (length(trim(content)) > 0),
    available_at INTEGER NOT NULL,
    expires_at INTEGER CHECK (expires_at IS NULL OR expires_at > available_at),
    recurrence TEXT NOT NULL CHECK (recurrence IN ('none', 'daily', 'weekly', 'monthly', 'yearly')),
    recurrence_window_ms INTEGER CHECK (recurrence_window_ms IS NULL OR recurrence_window_ms >= 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    created_at INTEGER NOT NULL CHECK (created_at >= 0),
    updated_at INTEGER NOT NULL CHECK (updated_at >= 0)
) STRICT;

CREATE INDEX group_scheduled_notes_group_idx
    ON group_scheduled_notes(group_id, available_at, id);
