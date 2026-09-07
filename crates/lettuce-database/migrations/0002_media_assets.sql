-- Logical assets are independent identities over immutable, deduplicated
-- media blobs. The blob kind is duplicated so SQLite can enforce the role
-- relationship with a composite foreign key.
CREATE UNIQUE INDEX media_blobs_id_kind_unique ON media_blobs(id, kind);

CREATE TABLE media_assets (
    id TEXT PRIMARY KEY,
    blob_id TEXT NOT NULL,
    blob_kind TEXT NOT NULL CHECK (blob_kind IN ('image', 'audio', 'document')),
    kind TEXT NOT NULL CHECK (kind IN (
        'avatar_original', 'background_image', 'illustration', 'lorebook_icon',
        'message_image', 'message_audio', 'generated_image',
        'synthesized_speech', 'other_image', 'other_audio', 'source_document'
    )),
    origin TEXT NOT NULL CHECK (origin IN (
        'upload', 'import', 'remote_fetch', 'generated', 'synthesized', 'legacy'
    )),
    retention TEXT NOT NULL CHECK (retention IN ('persistent', 'library', 'temporary')),
    expires_at INTEGER,
    provenance_json TEXT NOT NULL CHECK (length(trim(provenance_json)) > 0),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK ((retention = 'temporary') = (expires_at IS NOT NULL)),
    CHECK (
        (kind IN (
            'avatar_original', 'background_image', 'illustration', 'lorebook_icon',
            'message_image', 'generated_image', 'other_image'
        ) AND blob_kind = 'image')
        OR
        (kind IN ('message_audio', 'synthesized_speech', 'other_audio') AND blob_kind = 'audio')
        OR (kind = 'source_document' AND blob_kind = 'document')
    ),
    FOREIGN KEY (blob_id, blob_kind)
        REFERENCES media_blobs(id, kind)
        ON DELETE RESTRICT
) STRICT;

CREATE INDEX media_assets_blob_idx ON media_assets(blob_id, blob_kind);
CREATE INDEX media_assets_library_order_idx
    ON media_assets(updated_at DESC, id ASC)
    WHERE retention = 'library';
CREATE INDEX media_assets_temporary_expiry_idx
    ON media_assets(expires_at, id ASC)
    WHERE retention = 'temporary';

CREATE TRIGGER media_assets_require_ready_blob
BEFORE INSERT ON media_assets
WHEN COALESCE(
    (SELECT state FROM media_blobs WHERE id = NEW.blob_id AND kind = NEW.blob_kind),
    ''
) <> 'ready'
BEGIN
    SELECT RAISE(ABORT, 'media asset requires a ready blob');
END;

CREATE TABLE legacy_import_media_completions (
    run_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    destination_asset_id TEXT NOT NULL REFERENCES media_assets(id) ON DELETE RESTRICT,
    blob_id TEXT NOT NULL REFERENCES media_blobs(id) ON DELETE RESTRICT,
    byte_len INTEGER NOT NULL CHECK (byte_len >= 0),
    content_hash TEXT NOT NULL CHECK (length(content_hash) = 64),
    completed_at INTEGER NOT NULL,
    PRIMARY KEY (run_id, relative_path),
    UNIQUE (run_id, destination_asset_id),
    FOREIGN KEY (run_id, destination_asset_id)
        REFERENCES legacy_import_assignments(run_id, destination_id)
        ON DELETE RESTRICT
) STRICT;

CREATE TRIGGER legacy_import_media_completion_guard
BEFORE INSERT ON legacy_import_media_completions
WHEN NOT EXISTS (
    SELECT 1
    FROM legacy_import_assignments AS assignment
    JOIN legacy_import_runs AS run ON run.id = assignment.run_id
    JOIN media_assets AS asset ON asset.id = NEW.destination_asset_id
    JOIN media_blobs AS blob ON blob.id = NEW.blob_id
    WHERE assignment.run_id = NEW.run_id
      AND assignment.source_kind = 'media'
      AND assignment.source_key = NEW.relative_path
      AND assignment.destination_id = NEW.destination_asset_id
      AND assignment.expected_byte_len = NEW.byte_len
      AND assignment.expected_content_hash = NEW.content_hash
      AND run.status IN ('admitted','importing')
      AND asset.blob_id = NEW.blob_id
      AND blob.content_hash = NEW.content_hash
      AND blob.byte_size = NEW.byte_len
      AND blob.state = 'ready'
)
BEGIN
    SELECT RAISE(ABORT, 'legacy import media completion is invalid');
END;

CREATE TRIGGER legacy_import_media_completions_update_forbidden
BEFORE UPDATE ON legacy_import_media_completions
BEGIN
    SELECT RAISE(ABORT, 'legacy import media completion is immutable');
END;

CREATE TRIGGER legacy_import_media_completions_delete_forbidden
BEFORE DELETE ON legacy_import_media_completions
BEGIN
    SELECT RAISE(ABORT, 'legacy import media completion is immutable');
END;
