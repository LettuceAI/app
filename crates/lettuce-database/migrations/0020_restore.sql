CREATE TABLE backup_restore_admissions (
    id TEXT PRIMARY KEY NOT NULL,
    source_version INTEGER NOT NULL CHECK (source_version IN (1, 2)),
    source_hash TEXT NOT NULL CHECK (length(source_hash) = 64 AND source_hash = lower(source_hash) AND source_hash NOT GLOB '*[^0-9a-f]*'),
    plan_fingerprint TEXT NOT NULL CHECK (length(plan_fingerprint) = 64 AND plan_fingerprint = lower(plan_fingerprint) AND plan_fingerprint NOT GLOB '*[^0-9a-f]*'),
    staging_receipt_fingerprint TEXT NOT NULL CHECK (length(staging_receipt_fingerprint) = 64 AND staging_receipt_fingerprint = lower(staging_receipt_fingerprint) AND staging_receipt_fingerprint NOT GLOB '*[^0-9a-f]*'),
    document_count INTEGER NOT NULL CHECK (document_count >= 0),
    media_count INTEGER NOT NULL CHECK (media_count >= 0),
    secret_count INTEGER NOT NULL CHECK (secret_count >= 0),
    artifact_count INTEGER NOT NULL CHECK (artifact_count >= 0),
    status TEXT NOT NULL CHECK (status = 'admitted'),
    admitted_at INTEGER NOT NULL CHECK (admitted_at >= 0)
);

CREATE TRIGGER backup_restore_admissions_update_forbidden
BEFORE UPDATE ON backup_restore_admissions
BEGIN
    SELECT RAISE(ABORT, 'backup restore admission is immutable');
END;

CREATE TRIGGER backup_restore_admissions_delete_forbidden
BEFORE DELETE ON backup_restore_admissions
BEGIN
    SELECT RAISE(ABORT, 'backup restore admission is immutable');
END;
