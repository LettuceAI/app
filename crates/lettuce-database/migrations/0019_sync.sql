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
    operation_id TEXT UNIQUE CHECK (operation_id IS NULL OR length(operation_id) = 36),
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

CREATE TABLE sync_peer_frontiers (
    peer_device_id TEXT NOT NULL CHECK (length(peer_device_id) = 36),
    origin_device_id TEXT NOT NULL CHECK (length(origin_device_id) = 36),
    acknowledged_sequence INTEGER NOT NULL CHECK (acknowledged_sequence >= 1),
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (peer_device_id, origin_device_id)
) STRICT;

CREATE TABLE sync_incoming_batches (
    batch_id TEXT PRIMARY KEY CHECK (length(batch_id) = 36),
    peer_device_id TEXT NOT NULL CHECK (length(peer_device_id) = 36),
    batch_hash TEXT NOT NULL CHECK (length(batch_hash) = 64),
    change_count INTEGER NOT NULL CHECK (change_count BETWEEN 1 AND 256),
    payload_bytes INTEGER NOT NULL CHECK (payload_bytes BETWEEN 0 AND 16777216),
    state TEXT NOT NULL CHECK (state IN ('staged', 'pending', 'committed')),
    pending_reason TEXT,
    created_at INTEGER NOT NULL,
    committed_at INTEGER,
    CHECK (
        (state = 'committed' AND committed_at IS NOT NULL AND pending_reason IS NULL) OR
        (state <> 'committed' AND committed_at IS NULL)
    )
) STRICT;

CREATE TABLE sync_incoming_changes (
    batch_id TEXT NOT NULL REFERENCES sync_incoming_batches(batch_id) ON DELETE RESTRICT,
    change_id TEXT NOT NULL CHECK (length(change_id) = 36),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    fingerprint TEXT NOT NULL CHECK (length(fingerprint) = 64),
    document BLOB NOT NULL CHECK (length(document) BETWEEN 1 AND 131072),
    payload_bytes BLOB CHECK (payload_bytes IS NULL OR length(payload_bytes) BETWEEN 1 AND 8388608),
    PRIMARY KEY (batch_id, change_id),
    UNIQUE (batch_id, ordinal)
) STRICT;

CREATE TABLE sync_conflicts (
    conflict_id TEXT PRIMARY KEY CHECK (length(conflict_id) = 36),
    entity_kind TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    current_change_id TEXT REFERENCES sync_changes(change_id) ON DELETE RESTRICT,
    incoming_change_id TEXT NOT NULL UNIQUE REFERENCES sync_changes(change_id) ON DELETE RESTRICT,
    winning_side TEXT NOT NULL CHECK (winning_side IN ('current', 'incoming')),
    current_payload BLOB NOT NULL,
    incoming_payload BLOB NOT NULL,
    detected_at INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'unresolved' CHECK (status IN ('unresolved', 'resolved')),
    resolution_choice TEXT CHECK (resolution_choice IN ('current', 'other', 'superseded')),
    resolved_by_change_id TEXT REFERENCES sync_changes(change_id) ON DELETE RESTRICT,
    resolved_at INTEGER,
    CHECK (
        (status = 'unresolved' AND resolution_choice IS NULL AND
         resolved_by_change_id IS NULL AND resolved_at IS NULL) OR
        (status = 'resolved' AND resolution_choice IS NOT NULL AND
         resolved_by_change_id IS NOT NULL AND resolved_at IS NOT NULL)
    )
) STRICT;

CREATE INDEX sync_changes_origin_idx
ON sync_changes(origin_device_id, origin_sequence);

CREATE INDEX sync_changes_entity_idx
ON sync_changes(entity_kind, entity_id, origin_device_id, origin_sequence);

CREATE INDEX sync_incoming_batches_state_idx
ON sync_incoming_batches(peer_device_id, state, created_at);

CREATE TRIGGER sync_local_state_update_guard
BEFORE UPDATE ON sync_local_state
WHEN NEW.id <> OLD.id OR
     NEW.device_id <> OLD.device_id OR
     NOT (
       (NEW.origin_sequence = OLD.origin_sequence + 1 AND
        ((NEW.hlc_wall_time = OLD.hlc_wall_time AND NEW.hlc_counter = OLD.hlc_counter + 1) OR
         (NEW.hlc_wall_time > OLD.hlc_wall_time AND NEW.hlc_counter = 0))) OR
       (NEW.origin_sequence = OLD.origin_sequence AND
        (NEW.hlc_wall_time > OLD.hlc_wall_time OR
         (NEW.hlc_wall_time = OLD.hlc_wall_time AND NEW.hlc_counter > OLD.hlc_counter)))
     )
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

CREATE TRIGGER sync_peer_frontiers_update_guard
BEFORE UPDATE ON sync_peer_frontiers
WHEN NEW.peer_device_id <> OLD.peer_device_id OR
     NEW.origin_device_id <> OLD.origin_device_id OR
     NEW.acknowledged_sequence < OLD.acknowledged_sequence
BEGIN
    SELECT RAISE(ABORT, 'invalid peer acknowledgement transition');
END;

CREATE TRIGGER sync_peer_frontiers_no_delete
BEFORE DELETE ON sync_peer_frontiers
BEGIN
    SELECT RAISE(ABORT, 'peer acknowledgements are durable');
END;

CREATE TRIGGER sync_incoming_batches_update_guard
BEFORE UPDATE ON sync_incoming_batches
WHEN NEW.batch_id <> OLD.batch_id OR
     NEW.peer_device_id <> OLD.peer_device_id OR
     NEW.batch_hash <> OLD.batch_hash OR
     NEW.change_count <> OLD.change_count OR
     NEW.payload_bytes <> OLD.payload_bytes OR
     NEW.created_at <> OLD.created_at OR
     OLD.state = 'committed'
BEGIN
    SELECT RAISE(ABORT, 'invalid incoming batch transition');
END;

CREATE TRIGGER sync_incoming_batches_no_delete
BEFORE DELETE ON sync_incoming_batches
BEGIN
    SELECT RAISE(ABORT, 'incoming sync batches are durable');
END;

CREATE TRIGGER sync_incoming_changes_no_update
BEFORE UPDATE ON sync_incoming_changes
BEGIN
    SELECT RAISE(ABORT, 'incoming sync changes are immutable');
END;

CREATE TRIGGER sync_incoming_changes_no_delete
BEFORE DELETE ON sync_incoming_changes
BEGIN
    SELECT RAISE(ABORT, 'incoming sync changes are durable');
END;

CREATE TRIGGER sync_conflicts_update_guard
BEFORE UPDATE ON sync_conflicts
WHEN NEW.conflict_id <> OLD.conflict_id OR
     NEW.entity_kind <> OLD.entity_kind OR
     NEW.entity_id <> OLD.entity_id OR
     NEW.current_change_id IS NOT OLD.current_change_id OR
     NEW.incoming_change_id <> OLD.incoming_change_id OR
     NEW.winning_side <> OLD.winning_side OR
     NEW.current_payload IS NOT OLD.current_payload OR
     NEW.incoming_payload <> OLD.incoming_payload OR
     NEW.detected_at <> OLD.detected_at OR
     OLD.status <> 'unresolved' OR NEW.status <> 'resolved'
BEGIN
    SELECT RAISE(ABORT, 'invalid sync conflict transition');
END;

CREATE TRIGGER sync_conflicts_no_delete
BEFORE DELETE ON sync_conflicts
BEGIN
    SELECT RAISE(ABORT, 'sync conflicts are durable');
END;

CREATE TABLE sync_deferred_changes (
    change_id TEXT PRIMARY KEY REFERENCES sync_changes(change_id) ON DELETE RESTRICT,
    entity_kind TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    deferred_at INTEGER NOT NULL
) STRICT;

CREATE INDEX sync_deferred_changes_entity_idx
ON sync_deferred_changes(entity_kind, entity_id);

-- The version each locally held synced secret was last set at, for the
-- secret-store generation it was recorded against. Secret values never live
-- here.
CREATE TABLE sync_secret_versions (
    reference TEXT PRIMARY KEY CHECK (length(reference) = 36),
    purpose_json TEXT NOT NULL CHECK (json_valid(purpose_json)),
    generation INTEGER NOT NULL CHECK (generation >= 1),
    set_at INTEGER NOT NULL,
    device_id TEXT NOT NULL CHECK (length(device_id) = 36)
) STRICT;

CREATE TABLE sync_conversation_forks (
    conversation_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    holds_local INTEGER NOT NULL CHECK (holds_local IN (0, 1)),
    detected_at INTEGER NOT NULL,
    resolved_at INTEGER,
    PRIMARY KEY (conversation_id, branch_id),
    FOREIGN KEY (conversation_id, branch_id)
        REFERENCES conversation_branches(conversation_id, id) ON DELETE RESTRICT
) STRICT;

-- Conversations whose messages changed since the last journal scan, so the
-- scan reads only those. Every write to a table a synced message snapshot is
-- built from counts here, whatever path made it; device-local and not carried
-- across a restore (a restored database is scanned whole once).
CREATE TABLE sync_conversation_marks (
    conversation_id TEXT PRIMARY KEY,
    changed INTEGER NOT NULL CHECK (changed >= 0),
    scanned INTEGER NOT NULL CHECK (scanned >= 0)
) STRICT;

CREATE TRIGGER sync_mark_conversation_messages_insert
AFTER INSERT ON conversation_messages
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_messages_update
AFTER UPDATE ON conversation_messages
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_messages_delete
AFTER DELETE ON conversation_messages
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (OLD.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_message_revisions_insert
AFTER INSERT ON conversation_message_revisions
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_message_revisions_update
AFTER UPDATE ON conversation_message_revisions
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_message_revisions_delete
AFTER DELETE ON conversation_message_revisions
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (OLD.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_message_candidates_insert
AFTER INSERT ON conversation_message_candidates
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_message_candidates_update
AFTER UPDATE ON conversation_message_candidates
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_message_candidates_delete
AFTER DELETE ON conversation_message_candidates
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (OLD.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_initial_message_origins_insert
AFTER INSERT ON conversation_initial_message_origins
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_initial_message_origins_update
AFTER UPDATE ON conversation_initial_message_origins
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_initial_message_origins_delete
AFTER DELETE ON conversation_initial_message_origins
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (OLD.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_turns_insert
AFTER INSERT ON conversation_turns
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_turns_update
AFTER UPDATE ON conversation_turns
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_conversation_turns_delete
AFTER DELETE ON conversation_turns
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (OLD.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_generation_attempts_insert
AFTER INSERT ON generation_attempts
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_generation_attempts_update
AFTER UPDATE ON generation_attempts
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_generation_attempts_delete
AFTER DELETE ON generation_attempts
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (OLD.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_revision_media_refs_insert
AFTER INSERT ON revision_media_refs
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_revision_media_refs_update
AFTER UPDATE ON revision_media_refs
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_revision_media_refs_delete
AFTER DELETE ON revision_media_refs
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (OLD.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_candidate_media_refs_insert
AFTER INSERT ON candidate_media_refs
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_candidate_media_refs_update
AFTER UPDATE ON candidate_media_refs
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_candidate_media_refs_delete
AFTER DELETE ON candidate_media_refs
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (OLD.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_usage_events_insert
AFTER INSERT ON usage_events
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_usage_events_update
AFTER UPDATE ON usage_events
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (NEW.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;

CREATE TRIGGER sync_mark_usage_events_delete
AFTER DELETE ON usage_events
BEGIN
    INSERT INTO sync_conversation_marks (conversation_id, changed, scanned)
    VALUES (OLD.conversation_id, 1, 0)
    ON CONFLICT(conversation_id) DO UPDATE SET changed = changed + 1;
END;
