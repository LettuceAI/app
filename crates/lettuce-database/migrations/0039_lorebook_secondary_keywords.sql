ALTER TABLE lorebook_entries ADD COLUMN secondary_keywords_json TEXT NOT NULL DEFAULT '{"format_version":1,"value":[]}';

ALTER TABLE lorebook_entries ADD COLUMN secondary_logic TEXT NOT NULL DEFAULT 'and_any'
    CHECK (secondary_logic IN ('and_any', 'not_all', 'not_any', 'and_all'));
