ALTER TABLE groups ADD COLUMN companion_personal_memories INTEGER NOT NULL DEFAULT 1 CHECK (companion_personal_memories IN (0, 1));
ALTER TABLE conversation_settings ADD COLUMN companion_personal_memories INTEGER CHECK (companion_personal_memories IS NULL OR companion_personal_memories IN (0, 1));
ALTER TABLE groups ADD COLUMN companion_memory_write_back INTEGER NOT NULL DEFAULT 1 CHECK (companion_memory_write_back IN (0, 1));
ALTER TABLE conversation_settings ADD COLUMN companion_memory_write_back INTEGER CHECK (companion_memory_write_back IS NULL OR companion_memory_write_back IN (0, 1));