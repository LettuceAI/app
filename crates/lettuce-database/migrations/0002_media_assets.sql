-- Logical assets are independent identities over immutable, deduplicated
-- media blobs. The blob kind is duplicated so SQLite can enforce the role
-- relationship with a composite foreign key.
CREATE UNIQUE INDEX media_blobs_id_kind_unique ON media_blobs(id, kind);

CREATE TABLE media_assets (
    id TEXT PRIMARY KEY,
    blob_id TEXT NOT NULL,
    blob_kind TEXT NOT NULL CHECK (blob_kind IN ('image', 'audio')),
    kind TEXT NOT NULL CHECK (kind IN (
        'avatar_original', 'background_image', 'illustration', 'lorebook_icon',
        'message_image', 'message_audio', 'generated_image',
        'synthesized_speech', 'other_image', 'other_audio'
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
