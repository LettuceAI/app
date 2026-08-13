-- Reusable authored group profiles.  Group sessions/conversations deliberately
-- live in a later vertical and do not share these tables.
CREATE TABLE groups (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL CHECK (status IN ('active', 'archived')),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    normalized_name TEXT NOT NULL CHECK (length(normalized_name) > 0),
    chat_mode TEXT NOT NULL CHECK (chat_mode IN ('conversation', 'roleplay')),
    persona_selection_kind TEXT NOT NULL CHECK (persona_selection_kind IN ('inherit', 'explicit', 'disabled')),
    persona_id TEXT REFERENCES personas(id) ON DELETE RESTRICT,
    speaker_selection TEXT NOT NULL CHECK (speaker_selection IN ('llm', 'heuristic', 'round_robin', 'director', 'director_action')),
    memory_policy TEXT NOT NULL CHECK (memory_policy IN ('manual', 'dynamic')),
    disable_character_lorebooks INTEGER NOT NULL CHECK (disable_character_lorebooks IN (0, 1)),
    group_conversation_prompt_id TEXT,
    group_roleplay_prompt_id TEXT,
    presentation_json TEXT NOT NULL CHECK (length(trim(presentation_json)) > 0),
    background_asset_id TEXT,
    background_blob_kind TEXT NOT NULL DEFAULT 'image' CHECK (background_blob_kind = 'image'),
    starting_scene_id TEXT,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at),
    CHECK ((persona_selection_kind = 'explicit') = (persona_id IS NOT NULL)),
    FOREIGN KEY (background_asset_id, background_blob_kind)
        REFERENCES media_assets(id, blob_kind) ON DELETE RESTRICT,
    FOREIGN KEY (id, starting_scene_id)
        REFERENCES group_starting_scenes(group_id, id)
        DEFERRABLE INITIALLY DEFERRED,
    UNIQUE (id, starting_scene_id)
) STRICT;

CREATE INDEX groups_library_order_idx ON groups(updated_at DESC, id ASC);
CREATE INDEX groups_name_search_idx ON groups(normalized_name, id);

CREATE TABLE group_members (
    group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    muted INTEGER NOT NULL CHECK (muted IN (0, 1)),
    model_profile_override_id TEXT REFERENCES model_profiles(id) ON DELETE RESTRICT,
    PRIMARY KEY (group_id, character_id),
    UNIQUE (group_id, ordinal)
) STRICT;

CREATE INDEX group_members_character_idx ON group_members(character_id, group_id);
CREATE INDEX group_members_model_profile_idx ON group_members(model_profile_override_id, group_id, character_id);

CREATE TABLE group_presentation_asset_refs (
    group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    asset_id TEXT NOT NULL,
    blob_kind TEXT NOT NULL CHECK (blob_kind = 'image'),
    PRIMARY KEY (group_id, asset_id),
    FOREIGN KEY (asset_id, blob_kind)
        REFERENCES media_assets(id, blob_kind) ON DELETE RESTRICT
) STRICT;

CREATE TABLE group_starting_scenes (
    group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'archived')),
    ordinal INTEGER NOT NULL CHECK (ordinal = 0),
    content_json TEXT NOT NULL CHECK (length(trim(content_json)) > 0),
    direction TEXT,
    selected_variant_id TEXT,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at),
    PRIMARY KEY (group_id, id),
    UNIQUE (group_id, ordinal),
    FOREIGN KEY (group_id, id, selected_variant_id)
        REFERENCES group_scene_variants(group_id, scene_id, id)
        DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE TABLE group_scene_variants (
    group_id TEXT NOT NULL,
    id TEXT NOT NULL,
    scene_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    content_json TEXT NOT NULL CHECK (length(trim(content_json)) > 0),
    direction TEXT,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (created_at <= updated_at),
    PRIMARY KEY (group_id, id),
    UNIQUE (group_id, scene_id, ordinal),
    UNIQUE (group_id, scene_id, id),
    FOREIGN KEY (group_id, scene_id)
        REFERENCES group_starting_scenes(group_id, id) ON DELETE CASCADE
) STRICT;

CREATE INDEX group_scene_variants_scene_idx
    ON group_scene_variants(group_id, scene_id, ordinal, id);

CREATE TABLE group_scene_assets (
    group_id TEXT NOT NULL,
    scene_id TEXT NOT NULL,
    id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    blob_kind TEXT NOT NULL CHECK (blob_kind = 'image'),
    slot TEXT NOT NULL CHECK (slot IN ('background', 'inline')),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (group_id, id),
    UNIQUE (group_id, scene_id, asset_id),
    UNIQUE (group_id, scene_id, slot, ordinal),
    FOREIGN KEY (group_id, scene_id)
        REFERENCES group_starting_scenes(group_id, id) ON DELETE CASCADE,
    FOREIGN KEY (asset_id, blob_kind)
        REFERENCES media_assets(id, blob_kind) ON DELETE RESTRICT
) STRICT;

CREATE UNIQUE INDEX group_scene_background_uq
    ON group_scene_assets(group_id, scene_id) WHERE slot = 'background';
CREATE INDEX group_scene_assets_scene_idx
    ON group_scene_assets(group_id, scene_id, slot, ordinal, id);
