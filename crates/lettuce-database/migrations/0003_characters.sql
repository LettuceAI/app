CREATE TABLE characters (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL CHECK (status IN ('active', 'archived')),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    nickname TEXT,
    normalized_name TEXT NOT NULL CHECK (length(normalized_name) > 0),
    normalized_nickname TEXT,
    profile_json TEXT NOT NULL CHECK (length(trim(profile_json)) > 0),
    provenance_json TEXT NOT NULL CHECK (length(trim(provenance_json)) > 0),
    defaults_json TEXT NOT NULL CHECK (length(trim(defaults_json)) > 0),
    interaction_mode TEXT NOT NULL CHECK (interaction_mode IN ('roleplay', 'companion')),
    memory_policy TEXT NOT NULL CHECK (memory_policy IN ('manual', 'dynamic')),
    model_profile_id TEXT REFERENCES model_profiles(id) ON DELETE RESTRICT,
    default_scene_id TEXT,
    default_starter_id TEXT,
    direct_prompt_id TEXT,
    group_conversation_prompt_id TEXT,
    group_roleplay_prompt_id TEXT,
    voice_profile_id TEXT,
    voice_legacy_locator TEXT,
    voice_autoplay INTEGER NOT NULL CHECK (voice_autoplay IN (0, 1)),
    presentation_json TEXT NOT NULL CHECK (length(trim(presentation_json)) > 0),
    image_recommendation_json TEXT,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at),
    CHECK ((voice_profile_id IS NULL) OR (voice_legacy_locator IS NULL)),
    FOREIGN KEY (id, default_scene_id)
        REFERENCES scenes(character_id, id) DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (id, default_starter_id)
        REFERENCES conversation_starters(character_id, id) DEFERRABLE INITIALLY DEFERRED,
    UNIQUE (normalized_name, id)
) STRICT;

CREATE INDEX characters_library_order_idx ON characters(updated_at DESC, id ASC);
CREATE INDEX characters_name_search_idx ON characters(normalized_name, id);
CREATE INDEX characters_nickname_search_idx ON characters(normalized_nickname, id);

CREATE TABLE character_media (
    character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    asset_id TEXT NOT NULL REFERENCES media_assets(id) ON DELETE RESTRICT,
    slot TEXT NOT NULL CHECK (slot IN ('avatar_original', 'background', 'design_reference')),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (character_id, asset_id),
    UNIQUE (character_id, slot, ordinal)
) STRICT;
CREATE UNIQUE INDEX character_media_avatar_uq ON character_media(character_id) WHERE slot = 'avatar_original';
CREATE UNIQUE INDEX character_media_background_uq ON character_media(character_id) WHERE slot = 'background';

CREATE TABLE character_presentation_asset_refs (
    character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    asset_id TEXT NOT NULL REFERENCES media_assets(id) ON DELETE RESTRICT,
    PRIMARY KEY (character_id, asset_id)
) STRICT;

CREATE TABLE scenes (
    character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'archived')),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    content_json TEXT NOT NULL CHECK (length(trim(content_json)) > 0),
    direction TEXT,
    selected_variant_id TEXT,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at),
    PRIMARY KEY (character_id, id),
    UNIQUE (character_id, ordinal),
    FOREIGN KEY (character_id, id, selected_variant_id)
        REFERENCES scene_variants(character_id, scene_id, id) DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE TABLE scene_variants (
    character_id TEXT NOT NULL,
    id TEXT NOT NULL,
    scene_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    content_json TEXT NOT NULL CHECK (length(trim(content_json)) > 0),
    direction TEXT,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at),
    PRIMARY KEY (character_id, id),
    UNIQUE (character_id, scene_id, ordinal),
    UNIQUE (character_id, scene_id, id),
    FOREIGN KEY (character_id, scene_id) REFERENCES scenes(character_id, id) ON DELETE CASCADE
) STRICT;

CREATE TABLE scene_assets (
    character_id TEXT NOT NULL,
    scene_id TEXT NOT NULL,
    id TEXT NOT NULL,
    asset_id TEXT NOT NULL REFERENCES media_assets(id) ON DELETE RESTRICT,
    slot TEXT NOT NULL CHECK (slot IN ('background', 'inline')),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (character_id, id),
    UNIQUE (character_id, scene_id, asset_id),
    UNIQUE (character_id, scene_id, slot, ordinal),
    FOREIGN KEY (character_id, scene_id) REFERENCES scenes(character_id, id) ON DELETE CASCADE
) STRICT;
CREATE UNIQUE INDEX scene_background_uq ON scene_assets(character_id, scene_id) WHERE slot = 'background';

CREATE TABLE conversation_starters (
    character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    id TEXT NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    scene_id TEXT,
    prompt_id TEXT,
    lorebooks_json TEXT NOT NULL CHECK (length(trim(lorebooks_json)) > 0),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at),
    PRIMARY KEY (character_id, id),
    UNIQUE (character_id, ordinal),
    FOREIGN KEY (character_id, scene_id) REFERENCES scenes(character_id, id)
        ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE TABLE starter_messages (
    character_id TEXT NOT NULL,
    starter_id TEXT NOT NULL,
    id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    content TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (character_id, id),
    UNIQUE (character_id, starter_id, ordinal),
    FOREIGN KEY (character_id, starter_id) REFERENCES conversation_starters(character_id, id) ON DELETE CASCADE
) STRICT;

CREATE UNIQUE INDEX starter_message_id_uq ON starter_messages(character_id, starter_id, id);

CREATE UNIQUE INDEX character_default_scene_fk ON characters(id, default_scene_id);
CREATE UNIQUE INDEX character_default_starter_fk ON characters(id, default_starter_id);
CREATE INDEX scene_assets_scene_idx ON scene_assets(character_id, scene_id, slot, ordinal);
CREATE INDEX scene_variants_scene_idx ON scene_variants(character_id, scene_id, ordinal);
CREATE INDEX starter_messages_starter_idx ON starter_messages(character_id, starter_id, ordinal);
