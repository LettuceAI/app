use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

pub const MAX_BLOB_CHUNK_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobReceiveState {
    Ready { offset: u64 },
    Complete { path: PathBuf },
}

#[derive(Debug, thiserror::Error)]
pub enum BlobError {
    #[error("sync v2 database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("sync v2 blob filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid BLAKE3 content hash")]
    InvalidContentHash,
    #[error("blob {content_hash} was announced with inconsistent sizes")]
    SizeMismatch { content_hash: String },
    #[error("blob {content_hash} is not registered")]
    UnknownBlob { content_hash: String },
    #[error("blob {content_hash} expected offset {expected}, received {received}")]
    OffsetMismatch {
        content_hash: String,
        expected: u64,
        received: u64,
    },
    #[error("blob chunk has {received} bytes; the limit is {limit}")]
    ChunkTooLarge { received: usize, limit: usize },
    #[error("blob {content_hash} exceeds its declared size")]
    ExceedsDeclaredSize { content_hash: String },
    #[error("blob {content_hash} is incomplete: expected {expected} bytes, found {received}")]
    Incomplete {
        content_hash: String,
        expected: u64,
        received: u64,
    },
    #[error("blob {content_hash} failed content verification")]
    HashMismatch { content_hash: String },
    #[error("blob {content_hash} is not verified")]
    NotVerified { content_hash: String },
}

pub fn blob_content_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub fn begin_blob_receive(
    conn: &Connection,
    root: &Path,
    content_hash: &str,
    size_bytes: u64,
    now_ms: i64,
) -> Result<BlobReceiveState, BlobError> {
    validate_hash(content_hash)?;
    let final_path = final_path(root, content_hash);
    if final_path.is_file()
        && final_path.metadata()?.len() == size_bytes
        && hash_file(&final_path)? == content_hash
    {
        register_verified_blob(conn, root, content_hash, size_bytes, &final_path, now_ms)?;
        return Ok(BlobReceiveState::Complete { path: final_path });
    }

    if let Some(existing_size) = conn
        .query_row(
            "SELECT size_bytes FROM sync_v2_blobs WHERE content_hash = ?1",
            params![content_hash],
            |row| row.get::<_, u64>(0),
        )
        .optional()?
    {
        if existing_size != size_bytes {
            return Err(BlobError::SizeMismatch {
                content_hash: content_hash.to_string(),
            });
        }
    }

    let partial_path = partial_path(root, content_hash);
    if let Some(parent) = partial_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let offset = if partial_path.is_file() {
        partial_path.metadata()?.len()
    } else {
        0
    };
    if offset > size_bytes {
        quarantine_partial(root, content_hash, &partial_path)?;
        upsert_receiving_blob(conn, content_hash, size_bytes, 0, now_ms)?;
        return Ok(BlobReceiveState::Ready { offset: 0 });
    }
    upsert_receiving_blob(conn, content_hash, size_bytes, offset, now_ms)?;
    Ok(BlobReceiveState::Ready { offset })
}

pub fn write_blob_chunk(
    conn: &Connection,
    root: &Path,
    content_hash: &str,
    offset: u64,
    bytes: &[u8],
) -> Result<u64, BlobError> {
    validate_hash(content_hash)?;
    if bytes.len() > MAX_BLOB_CHUNK_BYTES {
        return Err(BlobError::ChunkTooLarge {
            received: bytes.len(),
            limit: MAX_BLOB_CHUNK_BYTES,
        });
    }
    let (size_bytes, verified) = conn
        .query_row(
            "SELECT size_bytes, verified
             FROM sync_v2_blobs
             WHERE content_hash = ?1",
            params![content_hash],
            |row| Ok((row.get::<_, u64>(0)?, row.get::<_, bool>(1)?)),
        )
        .optional()?
        .ok_or_else(|| BlobError::UnknownBlob {
            content_hash: content_hash.to_string(),
        })?;
    if verified {
        return Ok(size_bytes);
    }

    let partial_path = partial_path(root, content_hash);
    let actual_offset = partial_path.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    if offset != actual_offset {
        return Err(BlobError::OffsetMismatch {
            content_hash: content_hash.to_string(),
            expected: actual_offset,
            received: offset,
        });
    }
    let next_offset = offset.saturating_add(bytes.len() as u64);
    if next_offset > size_bytes {
        return Err(BlobError::ExceedsDeclaredSize {
            content_hash: content_hash.to_string(),
        });
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&partial_path)?;
    file.write_all(bytes)?;
    file.sync_data()?;
    conn.execute(
        "UPDATE sync_v2_blobs
         SET received_bytes = ?2
         WHERE content_hash = ?1",
        params![content_hash, next_offset],
    )?;
    Ok(next_offset)
}

pub fn finish_blob_receive(
    conn: &Connection,
    root: &Path,
    content_hash: &str,
) -> Result<PathBuf, BlobError> {
    validate_hash(content_hash)?;
    let size_bytes = conn
        .query_row(
            "SELECT size_bytes FROM sync_v2_blobs WHERE content_hash = ?1",
            params![content_hash],
            |row| row.get::<_, u64>(0),
        )
        .optional()?
        .ok_or_else(|| BlobError::UnknownBlob {
            content_hash: content_hash.to_string(),
        })?;
    let partial_path = partial_path(root, content_hash);
    let received = partial_path.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    if received != size_bytes {
        return Err(BlobError::Incomplete {
            content_hash: content_hash.to_string(),
            expected: size_bytes,
            received,
        });
    }
    if hash_file(&partial_path)? != content_hash {
        quarantine_partial(root, content_hash, &partial_path)?;
        conn.execute(
            "UPDATE sync_v2_blobs
             SET received_bytes = 0, verified = 0, relative_path = NULL
             WHERE content_hash = ?1",
            params![content_hash],
        )?;
        return Err(BlobError::HashMismatch {
            content_hash: content_hash.to_string(),
        });
    }

    let final_path = final_path(root, content_hash);
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent)?;
    }
    File::open(&partial_path)?.sync_all()?;
    fs::rename(&partial_path, &final_path)?;
    if let Some(parent) = final_path.parent() {
        File::open(parent)?.sync_all()?;
    }
    register_verified_blob(conn, root, content_hash, size_bytes, &final_path, 0)?;
    Ok(final_path)
}

pub fn read_blob_chunk(
    conn: &Connection,
    root: &Path,
    content_hash: &str,
    offset: u64,
    max_bytes: usize,
) -> Result<Vec<u8>, BlobError> {
    validate_hash(content_hash)?;
    let (relative_path, size_bytes, verified) = conn
        .query_row(
            "SELECT relative_path, size_bytes, verified
             FROM sync_v2_blobs
             WHERE content_hash = ?1",
            params![content_hash],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, bool>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| BlobError::UnknownBlob {
            content_hash: content_hash.to_string(),
        })?;
    if !verified {
        return Err(BlobError::NotVerified {
            content_hash: content_hash.to_string(),
        });
    }
    if offset > size_bytes {
        return Err(BlobError::OffsetMismatch {
            content_hash: content_hash.to_string(),
            expected: size_bytes,
            received: offset,
        });
    }
    let relative_path = relative_path.ok_or_else(|| BlobError::NotVerified {
        content_hash: content_hash.to_string(),
    })?;
    let path = root.join(relative_path);
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let chunk_size = max_bytes.min(MAX_BLOB_CHUNK_BYTES);
    let mut bytes = vec![0; chunk_size];
    let read = file.read(&mut bytes)?;
    bytes.truncate(read);
    Ok(bytes)
}

fn upsert_receiving_blob(
    conn: &Connection,
    content_hash: &str,
    size_bytes: u64,
    received_bytes: u64,
    now_ms: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO sync_v2_blobs (
           content_hash, size_bytes, verified, received_bytes, created_at
         ) VALUES (?1, ?2, 0, ?3, ?4)
         ON CONFLICT(content_hash) DO UPDATE SET
           received_bytes = excluded.received_bytes",
        params![content_hash, size_bytes, received_bytes, now_ms],
    )?;
    Ok(())
}

fn register_verified_blob(
    conn: &Connection,
    root: &Path,
    content_hash: &str,
    size_bytes: u64,
    path: &Path,
    now_ms: i64,
) -> Result<(), BlobError> {
    let relative_path = path
        .strip_prefix(root)
        .map_err(|_| BlobError::InvalidContentHash)?
        .to_string_lossy()
        .to_string();
    conn.execute(
        "INSERT INTO sync_v2_blobs (
           content_hash, size_bytes, relative_path, verified, received_bytes, created_at
         ) VALUES (?1, ?2, ?3, 1, ?2, ?4)
         ON CONFLICT(content_hash) DO UPDATE SET
           size_bytes = excluded.size_bytes,
           relative_path = excluded.relative_path,
           verified = 1,
           received_bytes = excluded.received_bytes",
        params![content_hash, size_bytes, relative_path, now_ms],
    )?;
    Ok(())
}

fn validate_hash(content_hash: &str) -> Result<(), BlobError> {
    if content_hash.len() == 64
        && content_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(BlobError::InvalidContentHash)
    }
}

fn partial_path(root: &Path, content_hash: &str) -> PathBuf {
    root.join(".incoming")
        .join(format!("{content_hash}.partial"))
}

fn final_path(root: &Path, content_hash: &str) -> PathBuf {
    root.join(&content_hash[..2]).join(content_hash)
}

fn quarantine_partial(
    root: &Path,
    content_hash: &str,
    partial_path: &Path,
) -> Result<(), std::io::Error> {
    if partial_path.exists() {
        let quarantine = root.join(".incoming").join(format!(
            "{content_hash}.corrupt-{}",
            Uuid::new_v4()
        ));
        fs::rename(partial_path, quarantine)?;
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use rusqlite::Connection;
    use uuid::Uuid;

    use super::{
        begin_blob_receive, blob_content_hash, finish_blob_receive, read_blob_chunk,
        write_blob_chunk, BlobError, BlobReceiveState,
    };
    use crate::sync::v2::create_schema;

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("lettuce-sync-v2-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn blob_download_resumes_verifies_and_streams() {
        let conn = connection();
        let root = TestRoot::new();
        let content = b"content-addressed asset";
        let hash = blob_content_hash(content);

        assert_eq!(
            begin_blob_receive(&conn, root.path(), &hash, content.len() as u64, 100)
                .unwrap(),
            BlobReceiveState::Ready { offset: 0 }
        );
        assert_eq!(
            write_blob_chunk(&conn, root.path(), &hash, 0, &content[..8]).unwrap(),
            8
        );
        assert_eq!(
            begin_blob_receive(&conn, root.path(), &hash, content.len() as u64, 110)
                .unwrap(),
            BlobReceiveState::Ready { offset: 8 }
        );
        write_blob_chunk(&conn, root.path(), &hash, 8, &content[8..]).unwrap();
        let final_path = finish_blob_receive(&conn, root.path(), &hash).unwrap();

        assert_eq!(fs::read(final_path).unwrap(), content);
        assert_eq!(
            read_blob_chunk(&conn, root.path(), &hash, 8, 7).unwrap(),
            content[8..15]
        );
    }

    #[test]
    fn wrong_offsets_and_hashes_never_become_verified() {
        let conn = connection();
        let root = TestRoot::new();
        let announced = blob_content_hash(b"expected");
        begin_blob_receive(&conn, root.path(), &announced, 8, 100).unwrap();

        assert!(matches!(
            write_blob_chunk(&conn, root.path(), &announced, 2, b"wrong"),
            Err(BlobError::OffsetMismatch { .. })
        ));
        write_blob_chunk(&conn, root.path(), &announced, 0, b"bad data").unwrap();
        assert!(matches!(
            finish_blob_receive(&conn, root.path(), &announced),
            Err(BlobError::HashMismatch { .. })
        ));
        assert!(matches!(
            read_blob_chunk(&conn, root.path(), &announced, 0, 8),
            Err(BlobError::NotVerified { .. })
        ));
        assert!(matches!(
            begin_blob_receive(&conn, root.path(), &announced, 8, 110).unwrap(),
            BlobReceiveState::Ready { offset: 0 }
        ));
    }
}
