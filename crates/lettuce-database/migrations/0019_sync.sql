CREATE TABLE sync_local_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    device_id TEXT NOT NULL UNIQUE CHECK (length(device_id) = 36),
    origin_sequence INTEGER NOT NULL CHECK (origin_sequence >= 0),
    hlc_wall_time INTEGER NOT NULL,
    hlc_counter INTEGER NOT NULL CHECK (hlc_counter >= 0)
) STRICT;

CREATE TABLE sync_frontiers (
    origin_device_id TEXT PRIMARY KEY CHECK (length(origin_device_id) = 36),
    contiguous_sequence INTEGER NOT NULL CHECK (contiguous_sequence >= 1)
) STRICT;

CREATE TABLE sync_changes (
    change_id TEXT PRIMARY KEY CHECK (length(change_id) = 36),
    operation_id TEXT NOT NULL UNIQUE CHECK (length(operation_id) = 36),
    format_version INTEGER NOT NULL CHECK (format_version >= 1),
    fingerprint TEXT NOT NULL UNIQUE CHECK (length(fingerprint) = 64),
    origin_device_id TEXT NOT NULL CHECK (length(origin_device_id) = 36),
    origin_sequence INTEGER NOT NULL CHECK (origin_sequence >= 1),
    hlc_wall_time INTEGER NOT NULL,
    hlc_counter INTEGER NOT NULL CHECK (hlc_counter >= 0),
    entity_kind TEXT NOT NULL CHECK (
        length(entity_kind) BETWEEN 1 AND 64 AND
        entity_kind = lower(entity_kind)
    ),
    entity_id TEXT NOT NULL CHECK (
        length(trim(entity_id)) > 0 AND
        length(CAST(entity_id AS BLOB)) <= 512
    ),
    operation TEXT NOT NULL CHECK (operation IN ('insert', 'update', 'delete')),
    base_revision TEXT CHECK (base_revision IS NULL OR length(base_revision) = 64),
    payload_schema TEXT,
    payload_version INTEGER,
    payload_bytes BLOB,
    payload_hash TEXT,
    created_at INTEGER NOT NULL,
    UNIQUE(origin_device_id, origin_sequence),
    CHECK (
        (operation = 'insert' AND base_revision IS NULL AND payload_bytes IS NOT NULL) OR
        (operation = 'update' AND base_revision IS NOT NULL AND payload_bytes IS NOT NULL) OR
        (operation = 'delete' AND base_revision IS NOT NULL AND payload_bytes IS NULL)
    ),
    CHECK (
        (payload_bytes IS NULL AND payload_schema IS NULL AND payload_version IS NULL AND payload_hash IS NULL) OR
        (payload_bytes IS NOT NULL AND
         length(payload_bytes) BETWEEN 1 AND 8388608 AND
         length(payload_schema) BETWEEN 1 AND 128 AND
         payload_schema = lower(payload_schema) AND
         payload_version >= 1 AND
         length(payload_hash) = 64)
    )
) STRICT;

CREATE TABLE sync_change_frontiers (
    change_id TEXT NOT NULL REFERENCES sync_changes(change_id) ON DELETE RESTRICT,
    origin_device_id TEXT NOT NULL CHECK (length(origin_device_id) = 36),
    seen_sequence INTEGER NOT NULL CHECK (seen_sequence >= 1),
    PRIMARY KEY (change_id, origin_device_id)
) STRICT;

CREATE INDEX sync_changes_origin_idx
ON sync_changes(origin_device_id, origin_sequence);

CREATE INDEX sync_changes_entity_idx
ON sync_changes(entity_kind, entity_id, origin_device_id, origin_sequence);

CREATE TRIGGER sync_local_state_update_guard
BEFORE UPDATE ON sync_local_state
WHEN NEW.id <> OLD.id OR
     NEW.device_id <> OLD.device_id OR
     NEW.origin_sequence <> OLD.origin_sequence + 1 OR
     NEW.hlc_wall_time < OLD.hlc_wall_time OR
     (NEW.hlc_wall_time = OLD.hlc_wall_time AND NEW.hlc_counter <> OLD.hlc_counter + 1) OR
     (NEW.hlc_wall_time > OLD.hlc_wall_time AND NEW.hlc_counter <> 0)
BEGIN
    SELECT RAISE(ABORT, 'invalid local sync clock transition');
END;

CREATE TRIGGER sync_local_state_no_delete
BEFORE DELETE ON sync_local_state
BEGIN
    SELECT RAISE(ABORT, 'local sync identity is durable');
END;

CREATE TRIGGER sync_changes_no_update
BEFORE UPDATE ON sync_changes
BEGIN
    SELECT RAISE(ABORT, 'sync changes are immutable');
END;

CREATE TRIGGER sync_changes_no_delete
BEFORE DELETE ON sync_changes
BEGIN
    SELECT RAISE(ABORT, 'sync changes are durable');
END;

CREATE TRIGGER sync_change_frontiers_no_update
BEFORE UPDATE ON sync_change_frontiers
BEGIN
    SELECT RAISE(ABORT, 'sync change frontiers are immutable');
END;

CREATE TRIGGER sync_change_frontiers_no_delete
BEFORE DELETE ON sync_change_frontiers
BEGIN
    SELECT RAISE(ABORT, 'sync change frontiers are durable');
END;
