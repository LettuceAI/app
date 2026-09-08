CREATE TABLE asr_vocabulary_terms (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    term TEXT NOT NULL CHECK (length(term) BETWEEN 1 AND 4096 AND instr(term, char(0)) = 0),
    normalized_term TEXT NOT NULL CHECK (
        length(normalized_term) BETWEEN 1 AND 4096
        AND trim(normalized_term) = normalized_term
    ),
    language TEXT CHECK (
        language IS NULL
        OR (length(language) BETWEEN 1 AND 32 AND trim(language) = language AND lower(language) = language)
    ),
    category TEXT CHECK (
        category IS NULL
        OR (length(category) BETWEEN 0 AND 512 AND instr(category, char(0)) = 0)
    ),
    scope TEXT NOT NULL CHECK (
        length(scope) BETWEEN 1 AND 64
        AND trim(scope) = scope
        AND lower(scope) = scope
    ),
    priority INTEGER NOT NULL,
    use_count INTEGER NOT NULL CHECK (use_count >= 0),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL CHECK (updated_at >= created_at)
) STRICT;

CREATE INDEX asr_vocabulary_scope_language_order
ON asr_vocabulary_terms(
    scope,
    language,
    priority DESC,
    use_count DESC,
    updated_at DESC,
    created_at DESC,
    id DESC
);

CREATE INDEX asr_vocabulary_normalized
ON asr_vocabulary_terms(normalized_term);

CREATE TABLE asr_corrections (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    wrong TEXT NOT NULL CHECK (length(wrong) BETWEEN 1 AND 4096 AND instr(wrong, char(0)) = 0),
    normalized_wrong TEXT NOT NULL CHECK (
        length(normalized_wrong) BETWEEN 1 AND 4096
        AND trim(normalized_wrong) = normalized_wrong
    ),
    correct TEXT NOT NULL CHECK (length(correct) BETWEEN 1 AND 4096 AND instr(correct, char(0)) = 0),
    normalized_correct TEXT NOT NULL CHECK (
        length(normalized_correct) BETWEEN 1 AND 4096
        AND trim(normalized_correct) = normalized_correct
    ),
    language TEXT CHECK (
        language IS NULL
        OR (length(language) BETWEEN 1 AND 32 AND trim(language) = language AND lower(language) = language)
    ),
    scope TEXT NOT NULL CHECK (
        length(scope) BETWEEN 1 AND 64
        AND trim(scope) = scope
        AND lower(scope) = scope
    ),
    confidence REAL NOT NULL CHECK (confidence BETWEEN 0.0 AND 1.0),
    use_count INTEGER NOT NULL CHECK (use_count >= 1),
    accepted_count INTEGER NOT NULL CHECK (accepted_count >= 0),
    rejected_count INTEGER NOT NULL CHECK (rejected_count >= 0),
    seen_count INTEGER NOT NULL CHECK (seen_count >= 0),
    last_seen_at INTEGER,
    user_approved INTEGER NOT NULL CHECK (user_approved IN (0, 1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL CHECK (
        updated_at >= created_at
        AND (last_seen_at IS NULL OR last_seen_at <= updated_at)
    )
) STRICT;

CREATE INDEX asr_corrections_scope_language_display
ON asr_corrections(
    scope,
    language,
    user_approved DESC,
    accepted_count DESC,
    confidence DESC,
    use_count DESC,
    updated_at DESC,
    id DESC
);

CREATE INDEX asr_corrections_processing
ON asr_corrections(
    scope,
    language,
    length(normalized_wrong) DESC,
    confidence DESC,
    use_count DESC,
    created_at DESC,
    id DESC
);

CREATE TABLE asr_ignored_suggestions (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    wrong TEXT NOT NULL CHECK (length(wrong) BETWEEN 1 AND 4096 AND instr(wrong, char(0)) = 0),
    normalized_wrong TEXT NOT NULL CHECK (
        length(normalized_wrong) BETWEEN 1 AND 4096
        AND trim(normalized_wrong) = normalized_wrong
    ),
    correct TEXT NOT NULL CHECK (length(correct) BETWEEN 1 AND 4096 AND instr(correct, char(0)) = 0),
    normalized_correct TEXT NOT NULL CHECK (
        length(normalized_correct) BETWEEN 1 AND 4096
        AND trim(normalized_correct) = normalized_correct
    ),
    language TEXT CHECK (
        language IS NULL
        OR (length(language) BETWEEN 1 AND 32 AND trim(language) = language AND lower(language) = language)
    ),
    scope TEXT NOT NULL CHECK (
        length(scope) BETWEEN 1 AND 64
        AND trim(scope) = scope
        AND lower(scope) = scope
    ),
    ignored_count INTEGER NOT NULL CHECK (ignored_count >= 1),
    last_ignored_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL CHECK (
        updated_at >= created_at
        AND last_ignored_at <= updated_at
    )
) STRICT;

CREATE UNIQUE INDEX asr_ignored_suggestions_identity
ON asr_ignored_suggestions(
    normalized_wrong,
    normalized_correct,
    coalesce(language, ''),
    scope
);

CREATE INDEX asr_ignored_suggestions_lookup
ON asr_ignored_suggestions(
    normalized_wrong,
    normalized_correct,
    language,
    scope,
    ignored_count DESC,
    id DESC
);

CREATE TABLE asr_voice_examples (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    audio_asset_id TEXT NOT NULL,
    audio_blob_kind TEXT NOT NULL DEFAULT 'audio' CHECK (audio_blob_kind = 'audio'),
    expected_text TEXT NOT NULL CHECK (
        length(expected_text) BETWEEN 1 AND 4096 AND instr(expected_text, char(0)) = 0
    ),
    normalized_expected_text TEXT NOT NULL CHECK (
        length(normalized_expected_text) BETWEEN 1 AND 4096
        AND trim(normalized_expected_text) = normalized_expected_text
    ),
    whisper_output TEXT CHECK (
        whisper_output IS NULL
        OR (length(whisper_output) <= 4096 AND instr(whisper_output, char(0)) = 0)
    ),
    normalized_whisper_output TEXT CHECK (
        normalized_whisper_output IS NULL
        OR (
            length(normalized_whisper_output) BETWEEN 1 AND 4096
            AND trim(normalized_whisper_output) = normalized_whisper_output
        )
    ),
    language TEXT CHECK (
        language IS NULL
        OR (length(language) BETWEEN 1 AND 32 AND trim(language) = language AND lower(language) = language)
    ),
    scope TEXT NOT NULL CHECK (
        length(scope) BETWEEN 1 AND 64 AND trim(scope) = scope AND lower(scope) = scope
    ),
    vocabulary_term_id TEXT REFERENCES asr_vocabulary_terms(id) ON DELETE SET NULL,
    correction_id TEXT REFERENCES asr_corrections(id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL CHECK (updated_at >= created_at),
    FOREIGN KEY (audio_asset_id, audio_blob_kind)
        REFERENCES media_assets(id, blob_kind) ON DELETE RESTRICT
) STRICT;

CREATE INDEX asr_voice_examples_scope_language_order
ON asr_voice_examples(scope, language, created_at DESC, id DESC);

CREATE TABLE legacy_import_asr_results (
    run_id TEXT PRIMARY KEY REFERENCES legacy_import_runs(id) ON DELETE RESTRICT,
    plan_fingerprint TEXT NOT NULL CHECK (length(plan_fingerprint) = 64),
    vocabulary_count INTEGER NOT NULL CHECK (vocabulary_count >= 0),
    correction_count INTEGER NOT NULL CHECK (correction_count >= 0),
    ignored_suggestion_count INTEGER NOT NULL CHECK (ignored_suggestion_count >= 0),
    voice_example_count INTEGER NOT NULL CHECK (voice_example_count >= 0),
    completed_at INTEGER NOT NULL
) STRICT;

CREATE TRIGGER legacy_import_asr_results_insert_guard
BEFORE INSERT ON legacy_import_asr_results
WHEN NOT EXISTS (
    SELECT 1
    FROM legacy_import_runs AS run
    WHERE run.id = NEW.run_id
      AND run.plan_fingerprint = NEW.plan_fingerprint
      AND run.status IN ('admitted', 'importing', 'completed')
      AND NEW.vocabulary_count = (
          SELECT count(*) FROM legacy_import_assignments
          WHERE run_id = NEW.run_id AND source_kind = 'asr_vocabulary'
      )
      AND NEW.correction_count = (
          SELECT count(*) FROM legacy_import_assignments
          WHERE run_id = NEW.run_id AND source_kind = 'asr_correction'
      )
      AND NEW.ignored_suggestion_count = (
          SELECT count(*) FROM legacy_import_assignments
          WHERE run_id = NEW.run_id AND source_kind = 'asr_ignored_suggestion'
      )
      AND NEW.voice_example_count = (
          SELECT count(*) FROM legacy_import_assignments
          WHERE run_id = NEW.run_id AND source_kind = 'asr_voice_example'
      )
)
BEGIN
    SELECT RAISE(ABORT, 'legacy ASR import result is invalid');
END;

CREATE TRIGGER legacy_import_asr_results_update_forbidden
BEFORE UPDATE ON legacy_import_asr_results
BEGIN
    SELECT RAISE(ABORT, 'legacy ASR import results are immutable');
END;

CREATE TRIGGER legacy_import_asr_results_delete_forbidden
BEFORE DELETE ON legacy_import_asr_results
BEGIN
    SELECT RAISE(ABORT, 'legacy ASR import results cannot be deleted');
END;
