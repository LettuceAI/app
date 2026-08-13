CREATE TABLE personas (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL CHECK (status IN ('active', 'archived')),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    normalized_title TEXT NOT NULL CHECK (length(normalized_title) > 0),
    nickname TEXT,
    normalized_nickname TEXT,
    description TEXT NOT NULL CHECK (length(trim(description)) > 0),
    design_description TEXT,
    avatar_crop_json TEXT,
    image_recommendation_json TEXT,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at)
) STRICT;

CREATE INDEX personas_library_order_idx ON personas(updated_at DESC, id ASC);
CREATE INDEX personas_title_search_idx ON personas(normalized_title, id);
CREATE INDEX personas_nickname_search_idx ON personas(normalized_nickname, id);

CREATE UNIQUE INDEX media_assets_id_blob_kind_unique ON media_assets(id, blob_kind);

CREATE TABLE persona_media (
    persona_id TEXT NOT NULL REFERENCES personas(id) ON DELETE CASCADE,
    asset_id TEXT NOT NULL REFERENCES media_assets(id) ON DELETE RESTRICT,
    blob_kind TEXT NOT NULL CHECK (blob_kind = 'image'),
    slot TEXT NOT NULL CHECK (slot IN ('avatar', 'design_reference')),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (persona_id, asset_id),
    UNIQUE (persona_id, slot, ordinal),
    FOREIGN KEY (asset_id, blob_kind)
        REFERENCES media_assets(id, blob_kind)
        ON DELETE RESTRICT
) STRICT;

CREATE UNIQUE INDEX persona_media_avatar_uq
    ON persona_media(persona_id) WHERE slot = 'avatar';

CREATE TABLE persona_defaults (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    default_persona_id TEXT REFERENCES personas(id) ON DELETE RESTRICT,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at)
) STRICT;

INSERT INTO persona_defaults (id, default_persona_id, revision, created_at, updated_at)
VALUES (1, NULL, 1, 0, 0);
