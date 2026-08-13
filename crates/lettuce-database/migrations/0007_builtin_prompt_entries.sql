ALTER TABLE prompt_entries
    ADD COLUMN built_in_entry_key TEXT
    CHECK (
        built_in_entry_key IS NULL OR (
            built_in_entry_key = trim(built_in_entry_key)
            AND length(trim(built_in_entry_key)) > 0
            AND length(built_in_entry_key) <= 1024
        )
    );

CREATE UNIQUE INDEX prompt_entries_built_in_key_idx
    ON prompt_entries(prompt_id, built_in_entry_key)
    WHERE built_in_entry_key IS NOT NULL;
