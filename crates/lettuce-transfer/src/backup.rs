use std::{collections::HashSet, fmt, io::Read, path::Path};

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use lettuce_platform::{ConfinedInstallStore, InstallPreparation, ObjectKey, PlatformError};
use lettuce_types::{ContentHash, OperationId, TimestampMillis};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub const BACKUP_ENVELOPE_VERSION: u32 = 1;
pub const MAX_BACKUP_ENTRIES: usize = 256;
pub const MAX_BACKUP_ENTRY_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_BACKUP_TOTAL_BYTES: usize = 512 * 1024 * 1024;
pub const MAX_BACKUP_WRITE_CHUNK_BYTES: usize = 1024 * 1024;

const MAGIC: [u8; 16] = *b"LETTUCE-BACKUP3\0";
const MAX_MANIFEST_BYTES: usize = 256 * 1024;
const MAX_APP_VERSION_BYTES: usize = 64;
const MAX_ENTRY_NAME_BYTES: usize = 512;
const MAX_SCHEMA_BYTES: usize = 128;
const SALT_BYTES: usize = 16;
const NONCE_BYTES: usize = 24;
const TAG_BYTES: usize = 16;
const ARGON2_MEMORY_KIB: u32 = 19 * 1024;
const ARGON2_ITERATIONS: u32 = 2;
const ARGON2_LANES: u32 = 1;

#[derive(Clone, PartialEq, Eq)]
pub struct BackupSection {
    pub name: String,
    pub schema: String,
    pub bytes: Vec<u8>,
}

impl fmt::Debug for BackupSection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupSection")
            .field("name", &self.name)
            .field("schema", &self.schema)
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupInfo {
    pub version: u32,
    pub created_at: TimestampMillis,
    pub app_version: String,
    pub entry_count: usize,
    pub plaintext_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BackupEnvelopeError {
    #[error("backup password is empty or exceeds its limit")]
    InvalidPassword,
    #[error("backup metadata is invalid")]
    InvalidMetadata,
    #[error("backup entry name is unsafe or duplicated")]
    InvalidEntryName,
    #[error("backup entry schema is invalid")]
    InvalidSchema,
    #[error("backup entry or aggregate size exceeds its limit")]
    LimitExceeded,
    #[error("backup encryption failed")]
    Encryption,
    #[error("backup authentication failed")]
    Authentication,
    #[error("backup payload is truncated or has trailing data")]
    InvalidEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupManifest {
    version: u32,
    created_at: TimestampMillis,
    app_version: String,
    kdf: BackupKdf,
    entries: Vec<BackupEntryDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupKdf {
    algorithm: String,
    memory_kib: u32,
    iterations: u32,
    lanes: u32,
    salt: [u8; SALT_BYTES],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupEntryDescriptor {
    name: String,
    schema: String,
    plaintext_bytes: u64,
    content_hash: ContentHash,
    nonce: [u8; NONCE_BYTES],
}

pub fn seal_backup(
    app_version: impl Into<String>,
    created_at: TimestampMillis,
    password: &str,
    sections: Vec<BackupSection>,
) -> Result<Vec<u8>, BackupEnvelopeError> {
    validate_password(password)?;
    let app_version = app_version.into();
    validate_app_version(&app_version)?;
    validate_sections(&sections)?;
    let mut salt = [0u8; SALT_BYTES];
    OsRng.fill_bytes(&mut salt);
    let mut nonces = HashSet::with_capacity(sections.len());
    let mut entries = Vec::with_capacity(sections.len());
    for section in &sections {
        let mut nonce = [0u8; NONCE_BYTES];
        loop {
            OsRng.fill_bytes(&mut nonce);
            if nonces.insert(nonce) {
                break;
            }
        }
        entries.push(BackupEntryDescriptor {
            name: section.name.clone(),
            schema: section.schema.clone(),
            plaintext_bytes: u64::try_from(section.bytes.len())
                .map_err(|_| BackupEnvelopeError::LimitExceeded)?,
            content_hash: hash_bytes(&section.bytes),
            nonce,
        });
    }
    let manifest = BackupManifest {
        version: BACKUP_ENVELOPE_VERSION,
        created_at,
        app_version,
        kdf: BackupKdf {
            algorithm: "argon2id-v19".into(),
            memory_kib: ARGON2_MEMORY_KIB,
            iterations: ARGON2_ITERATIONS,
            lanes: ARGON2_LANES,
            salt,
        },
        entries,
    };
    validate_manifest(&manifest)?;
    let manifest_bytes =
        serde_json::to_vec(&manifest).map_err(|_| BackupEnvelopeError::InvalidMetadata)?;
    if manifest_bytes.len() > MAX_MANIFEST_BYTES {
        return Err(BackupEnvelopeError::LimitExceeded);
    }
    let manifest_binding = blake3::hash(&manifest_bytes);
    let key = derive_key(password, &manifest.kdf)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref())
        .map_err(|_| BackupEnvelopeError::Encryption)?;
    let mut envelope = Vec::with_capacity(
        MAGIC.len()
            + 4
            + manifest_bytes.len()
            + sections
                .iter()
                .map(|value| value.bytes.len() + 8 + TAG_BYTES)
                .sum::<usize>(),
    );
    envelope.extend_from_slice(&MAGIC);
    envelope.extend_from_slice(
        &u32::try_from(manifest_bytes.len())
            .map_err(|_| BackupEnvelopeError::LimitExceeded)?
            .to_be_bytes(),
    );
    envelope.extend_from_slice(&manifest_bytes);
    for (index, (section, descriptor)) in sections.iter().zip(&manifest.entries).enumerate() {
        let aad = entry_aad(manifest_binding.as_bytes(), index)?;
        let encrypted = cipher
            .encrypt(
                XNonce::from_slice(&descriptor.nonce),
                Payload {
                    msg: &section.bytes,
                    aad: &aad,
                },
            )
            .map_err(|_| BackupEnvelopeError::Encryption)?;
        envelope.extend_from_slice(
            &u64::try_from(encrypted.len())
                .map_err(|_| BackupEnvelopeError::LimitExceeded)?
                .to_be_bytes(),
        );
        envelope.extend_from_slice(&encrypted);
    }
    Ok(envelope)
}

pub fn inspect_backup(bytes: &[u8]) -> Result<BackupInfo, BackupEnvelopeError> {
    let parsed = parse_envelope(bytes)?;
    Ok(BackupInfo {
        version: parsed.manifest.version,
        created_at: parsed.manifest.created_at,
        app_version: parsed.manifest.app_version,
        entry_count: parsed.manifest.entries.len(),
        plaintext_bytes: parsed
            .manifest
            .entries
            .iter()
            .map(|entry| entry.plaintext_bytes)
            .sum(),
    })
}

pub fn open_backup(
    bytes: &[u8],
    password: &str,
) -> Result<Vec<BackupSection>, BackupEnvelopeError> {
    validate_password(password)?;
    let parsed = parse_envelope(bytes)?;
    let manifest_binding = blake3::hash(parsed.manifest_bytes);
    let key = derive_key(password, &parsed.manifest.kdf)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref())
        .map_err(|_| BackupEnvelopeError::Authentication)?;
    let mut plaintext = Vec::with_capacity(parsed.manifest.entries.len());
    for (index, (descriptor, encrypted)) in parsed
        .manifest
        .entries
        .iter()
        .zip(parsed.encrypted_entries)
        .enumerate()
    {
        let aad = entry_aad(manifest_binding.as_bytes(), index)?;
        let value = cipher
            .decrypt(
                XNonce::from_slice(&descriptor.nonce),
                Payload {
                    msg: encrypted,
                    aad: &aad,
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| BackupEnvelopeError::Authentication)?;
        if value.len()
            != usize::try_from(descriptor.plaintext_bytes)
                .map_err(|_| BackupEnvelopeError::LimitExceeded)?
            || hash_bytes(&value) != descriptor.content_hash
        {
            return Err(BackupEnvelopeError::Authentication);
        }
        plaintext.push(value);
    }
    Ok(parsed
        .manifest
        .entries
        .into_iter()
        .zip(plaintext)
        .map(|(descriptor, mut bytes)| BackupSection {
            name: descriptor.name,
            schema: descriptor.schema,
            bytes: std::mem::take(&mut *bytes),
        })
        .collect())
}

struct ParsedEnvelope<'a> {
    manifest: BackupManifest,
    manifest_bytes: &'a [u8],
    encrypted_entries: Vec<&'a [u8]>,
}

fn parse_envelope(bytes: &[u8]) -> Result<ParsedEnvelope<'_>, BackupEnvelopeError> {
    if bytes.len() < MAGIC.len() + 4 || bytes[..MAGIC.len()] != MAGIC {
        return Err(BackupEnvelopeError::InvalidEnvelope);
    }
    let mut position = MAGIC.len();
    let manifest_len = read_u32(bytes, &mut position)? as usize;
    if manifest_len == 0 || manifest_len > MAX_MANIFEST_BYTES {
        return Err(BackupEnvelopeError::LimitExceeded);
    }
    let manifest_end = position
        .checked_add(manifest_len)
        .ok_or(BackupEnvelopeError::InvalidEnvelope)?;
    let manifest_bytes = bytes
        .get(position..manifest_end)
        .ok_or(BackupEnvelopeError::InvalidEnvelope)?;
    position = manifest_end;
    let manifest: BackupManifest =
        serde_json::from_slice(manifest_bytes).map_err(|_| BackupEnvelopeError::InvalidMetadata)?;
    validate_manifest(&manifest)?;
    let mut encrypted_entries = Vec::with_capacity(manifest.entries.len());
    for descriptor in &manifest.entries {
        let encrypted_len = usize::try_from(read_u64(bytes, &mut position)?)
            .map_err(|_| BackupEnvelopeError::LimitExceeded)?;
        let expected = usize::try_from(descriptor.plaintext_bytes)
            .map_err(|_| BackupEnvelopeError::LimitExceeded)?
            .checked_add(TAG_BYTES)
            .ok_or(BackupEnvelopeError::LimitExceeded)?;
        if encrypted_len != expected {
            return Err(BackupEnvelopeError::InvalidEnvelope);
        }
        let end = position
            .checked_add(encrypted_len)
            .ok_or(BackupEnvelopeError::InvalidEnvelope)?;
        encrypted_entries.push(
            bytes
                .get(position..end)
                .ok_or(BackupEnvelopeError::InvalidEnvelope)?,
        );
        position = end;
    }
    if position != bytes.len() {
        return Err(BackupEnvelopeError::InvalidEnvelope);
    }
    Ok(ParsedEnvelope {
        manifest,
        manifest_bytes,
        encrypted_entries,
    })
}

fn validate_manifest(manifest: &BackupManifest) -> Result<(), BackupEnvelopeError> {
    if manifest.version != BACKUP_ENVELOPE_VERSION
        || manifest.kdf.algorithm != "argon2id-v19"
        || manifest.kdf.memory_kib != ARGON2_MEMORY_KIB
        || manifest.kdf.iterations != ARGON2_ITERATIONS
        || manifest.kdf.lanes != ARGON2_LANES
    {
        return Err(BackupEnvelopeError::InvalidMetadata);
    }
    validate_app_version(&manifest.app_version)?;
    if manifest.entries.is_empty() || manifest.entries.len() > MAX_BACKUP_ENTRIES {
        return Err(BackupEnvelopeError::LimitExceeded);
    }
    let mut names = HashSet::with_capacity(manifest.entries.len());
    let mut nonces = HashSet::with_capacity(manifest.entries.len());
    let mut total = 0usize;
    for entry in &manifest.entries {
        validate_entry_name(&entry.name)?;
        validate_schema(&entry.schema)?;
        if !names.insert(entry.name.as_str()) {
            return Err(BackupEnvelopeError::InvalidEntryName);
        }
        if !nonces.insert(entry.nonce) {
            return Err(BackupEnvelopeError::InvalidMetadata);
        }
        let size = usize::try_from(entry.plaintext_bytes)
            .map_err(|_| BackupEnvelopeError::LimitExceeded)?;
        if size > MAX_BACKUP_ENTRY_BYTES
            || ContentHash::parse(entry.content_hash.as_str()).as_ref() != Ok(&entry.content_hash)
        {
            return Err(BackupEnvelopeError::LimitExceeded);
        }
        total = total
            .checked_add(size)
            .ok_or(BackupEnvelopeError::LimitExceeded)?;
        if total > MAX_BACKUP_TOTAL_BYTES {
            return Err(BackupEnvelopeError::LimitExceeded);
        }
    }
    Ok(())
}

fn validate_sections(sections: &[BackupSection]) -> Result<(), BackupEnvelopeError> {
    if sections.is_empty() || sections.len() > MAX_BACKUP_ENTRIES {
        return Err(BackupEnvelopeError::LimitExceeded);
    }
    let mut names = HashSet::with_capacity(sections.len());
    let mut total = 0usize;
    for section in sections {
        validate_entry_name(&section.name)?;
        validate_schema(&section.schema)?;
        if !names.insert(section.name.as_str()) {
            return Err(BackupEnvelopeError::InvalidEntryName);
        }
        if section.bytes.len() > MAX_BACKUP_ENTRY_BYTES {
            return Err(BackupEnvelopeError::LimitExceeded);
        }
        total = total
            .checked_add(section.bytes.len())
            .ok_or(BackupEnvelopeError::LimitExceeded)?;
        if total > MAX_BACKUP_TOTAL_BYTES {
            return Err(BackupEnvelopeError::LimitExceeded);
        }
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), BackupEnvelopeError> {
    if password.is_empty() || password.len() > 1024 || password.chars().any(char::is_control) {
        Err(BackupEnvelopeError::InvalidPassword)
    } else {
        Ok(())
    }
}

fn validate_app_version(value: &str) -> Result<(), BackupEnvelopeError> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > MAX_APP_VERSION_BYTES
        || value.chars().any(char::is_control)
    {
        Err(BackupEnvelopeError::InvalidMetadata)
    } else {
        Ok(())
    }
}

fn validate_entry_name(value: &str) -> Result<(), BackupEnvelopeError> {
    if value.is_empty()
        || value.len() > MAX_ENTRY_NAME_BYTES
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains('\\')
        || value.chars().any(char::is_control)
        || value.split('/').any(|part| {
            part.is_empty()
                || matches!(part, "." | "..")
                || part.len() > 128
                || !part.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'_' | b'-')
                })
        })
    {
        Err(BackupEnvelopeError::InvalidEntryName)
    } else {
        Ok(())
    }
}

fn validate_schema(value: &str) -> Result<(), BackupEnvelopeError> {
    if value.is_empty()
        || value.len() > MAX_SCHEMA_BYTES
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
    {
        Err(BackupEnvelopeError::InvalidSchema)
    } else {
        Ok(())
    }
}

fn derive_key(password: &str, kdf: &BackupKdf) -> Result<Zeroizing<[u8; 32]>, BackupEnvelopeError> {
    let params = Params::new(kdf.memory_kib, kdf.iterations, kdf.lanes, Some(32))
        .map_err(|_| BackupEnvelopeError::InvalidMetadata)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(password.as_bytes(), &kdf.salt, key.as_mut())
        .map_err(|_| BackupEnvelopeError::Authentication)?;
    Ok(key)
}

fn entry_aad(binding: &[u8; 32], index: usize) -> Result<[u8; 40], BackupEnvelopeError> {
    let mut aad = [0u8; 40];
    aad[..32].copy_from_slice(binding);
    aad[32..].copy_from_slice(
        &u64::try_from(index)
            .map_err(|_| BackupEnvelopeError::LimitExceeded)?
            .to_be_bytes(),
    );
    Ok(aad)
}

fn hash_bytes(bytes: &[u8]) -> ContentHash {
    ContentHash::parse(blake3::hash(bytes).to_hex().to_string())
        .expect("BLAKE3 produces a valid content hash")
}

fn read_u32(bytes: &[u8], position: &mut usize) -> Result<u32, BackupEnvelopeError> {
    let end = position
        .checked_add(4)
        .ok_or(BackupEnvelopeError::InvalidEnvelope)?;
    let value = bytes
        .get(*position..end)
        .ok_or(BackupEnvelopeError::InvalidEnvelope)?;
    *position = end;
    Ok(u32::from_be_bytes(
        value
            .try_into()
            .map_err(|_| BackupEnvelopeError::InvalidEnvelope)?,
    ))
}

fn read_u64(bytes: &[u8], position: &mut usize) -> Result<u64, BackupEnvelopeError> {
    let end = position
        .checked_add(8)
        .ok_or(BackupEnvelopeError::InvalidEnvelope)?;
    let value = bytes
        .get(*position..end)
        .ok_or(BackupEnvelopeError::InvalidEnvelope)?;
    *position = end;
    Ok(u64::from_be_bytes(
        value
            .try_into()
            .map_err(|_| BackupEnvelopeError::InvalidEnvelope)?,
    ))
}

#[derive(Debug)]
pub struct BackupArchiveStore {
    files: ConfinedInstallStore,
}

impl BackupArchiveStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, BackupArchiveStoreError> {
        Ok(Self {
            files: ConfinedInstallStore::open(root).map_err(BackupArchiveStoreError::Platform)?,
        })
    }

    pub fn receive_offset(
        &self,
        backup_id: OperationId,
        total_bytes: u64,
    ) -> Result<u64, BackupArchiveStoreError> {
        match self.prepare(backup_id, total_bytes)? {
            InstallPreparation::Installed(file) => {
                if file.len() == total_bytes {
                    Ok(total_bytes)
                } else {
                    Err(BackupArchiveStoreError::Conflict)
                }
            }
            InstallPreparation::Resume(file) => Ok(file.offset()),
        }
    }

    pub fn append(
        &self,
        backup_id: OperationId,
        total_bytes: u64,
        offset: u64,
        bytes: &[u8],
    ) -> Result<u64, BackupArchiveStoreError> {
        if bytes.is_empty() || bytes.len() > MAX_BACKUP_WRITE_CHUNK_BYTES {
            return Err(BackupArchiveStoreError::LimitExceeded);
        }
        match self.prepare(backup_id, total_bytes)? {
            InstallPreparation::Installed(_) => Err(BackupArchiveStoreError::Conflict),
            InstallPreparation::Resume(mut file) => {
                if file.offset() != offset {
                    return Err(BackupArchiveStoreError::Conflict);
                }
                file.append(bytes)
                    .map_err(BackupArchiveStoreError::Platform)
            }
        }
    }

    pub fn finish(
        &self,
        backup_id: OperationId,
        total_bytes: u64,
        expected_hash: &ContentHash,
    ) -> Result<std::path::PathBuf, BackupArchiveStoreError> {
        match self.prepare(backup_id, total_bytes)? {
            InstallPreparation::Installed(mut file) => {
                if file.len() != total_bytes || hash_reader(&mut file)? != *expected_hash {
                    return Err(BackupArchiveStoreError::Conflict);
                }
                Ok(file.native_path().to_owned())
            }
            InstallPreparation::Resume(mut file) => {
                if file.offset() != total_bytes {
                    return Err(BackupArchiveStoreError::Incomplete);
                }
                file.rewind().map_err(BackupArchiveStoreError::Platform)?;
                let mut bytes = Vec::with_capacity(
                    usize::try_from(total_bytes)
                        .map_err(|_| BackupArchiveStoreError::LimitExceeded)?,
                );
                file.read_to_end(&mut bytes)
                    .map_err(|_| BackupArchiveStoreError::Platform(PlatformError::Io))?;
                if hash_bytes(&bytes) != *expected_hash {
                    file.restart().map_err(BackupArchiveStoreError::Platform)?;
                    return Err(BackupArchiveStoreError::HashMismatch);
                }
                inspect_backup(&bytes).map_err(BackupArchiveStoreError::Envelope)?;
                file.commit_new().map_err(|error| {
                    if error == PlatformError::Conflict {
                        BackupArchiveStoreError::Conflict
                    } else {
                        BackupArchiveStoreError::Platform(error)
                    }
                })
            }
        }
    }

    fn prepare(
        &self,
        backup_id: OperationId,
        total_bytes: u64,
    ) -> Result<InstallPreparation, BackupArchiveStoreError> {
        let max = u64::try_from(MAX_BACKUP_TOTAL_BYTES + MAX_MANIFEST_BYTES + 1024 * 1024)
            .map_err(|_| BackupArchiveStoreError::LimitExceeded)?;
        if total_bytes == 0 || total_bytes > max {
            return Err(BackupArchiveStoreError::LimitExceeded);
        }
        let id = backup_id.to_string();
        let partial = ObjectKey::from_segments(["backups", &format!("{id}.partial")])
            .map_err(BackupArchiveStoreError::Platform)?;
        let target = ObjectKey::from_segments(["backups", &format!("{id}.lettuce")])
            .map_err(BackupArchiveStoreError::Platform)?;
        self.files
            .prepare(partial, target, total_bytes)
            .map_err(BackupArchiveStoreError::Platform)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BackupArchiveStoreError {
    #[error("backup staging exceeds its size or chunk limit")]
    LimitExceeded,
    #[error("backup staging offset or installed target conflicts")]
    Conflict,
    #[error("backup staging is incomplete")]
    Incomplete,
    #[error("backup staging content hash does not match")]
    HashMismatch,
    #[error("backup staging envelope failed verification: {0}")]
    Envelope(BackupEnvelopeError),
    #[error("backup staging filesystem failed: {0}")]
    Platform(PlatformError),
}

fn hash_reader(reader: &mut impl Read) -> Result<ContentHash, BackupArchiveStoreError> {
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| BackupArchiveStoreError::Platform(PlatformError::Io))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(ContentHash::parse(hasher.finalize().to_hex().to_string())
        .expect("BLAKE3 produces a valid content hash"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sections() -> Vec<BackupSection> {
        vec![
            BackupSection {
                name: "data/personas.json".into(),
                schema: "persona.snapshot.v1".into(),
                bytes: br#"[{"title":"Private persona"}]"#.to_vec(),
            },
            BackupSection {
                name: "media/blake3/asset.bin".into(),
                schema: "media.blob.v1".into(),
                bytes: b"exact media bytes".to_vec(),
            },
        ]
    }

    #[test]
    fn encrypted_envelope_round_trips_and_info_is_stable() {
        let source = sections();
        let envelope = seal_backup(
            "1.2.3",
            TimestampMillis::new(1234),
            "correct horse battery staple",
            source.clone(),
        )
        .expect("seal backup");
        let first = inspect_backup(&envelope).expect("inspect backup");
        let second = inspect_backup(&envelope).expect("inspect backup again");

        assert_eq!(first, second);
        assert_eq!(first.entry_count, 2);
        assert_eq!(
            first.plaintext_bytes,
            source
                .iter()
                .map(|section| u64::try_from(section.bytes.len()).expect("section length"))
                .sum::<u64>()
        );
        assert_eq!(
            open_backup(&envelope, "correct horse battery staple").expect("open backup"),
            source
        );
        assert!(!format!("{:?}", sections()[0]).contains("Private persona"));
        let parsed = parse_envelope(&envelope).expect("parse envelope");
        assert_ne!(
            parsed.manifest.entries[0].nonce,
            parsed.manifest.entries[1].nonce
        );
    }

    #[test]
    fn envelope_rejects_wrong_password_tampering_paths_duplicates_and_trailing_data() {
        let envelope = seal_backup(
            "1.2.3",
            TimestampMillis::new(1234),
            "correct password",
            sections(),
        )
        .expect("seal backup");
        assert_eq!(
            open_backup(&envelope, "wrong password"),
            Err(BackupEnvelopeError::Authentication)
        );

        let mut tampered = envelope.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert_eq!(
            open_backup(&tampered, "correct password"),
            Err(BackupEnvelopeError::Authentication)
        );

        let parsed = parse_envelope(&envelope).expect("parse envelope");
        let mut changed_manifest = parsed.manifest.clone();
        changed_manifest.created_at = TimestampMillis::new(1235);
        let changed_manifest_bytes =
            serde_json::to_vec(&changed_manifest).expect("serialize changed manifest");
        assert_eq!(changed_manifest_bytes.len(), parsed.manifest_bytes.len());
        let mut manifest_tampered = envelope.clone();
        let start = MAGIC.len() + 4;
        manifest_tampered[start..start + changed_manifest_bytes.len()]
            .copy_from_slice(&changed_manifest_bytes);
        assert_eq!(
            open_backup(&manifest_tampered, "correct password"),
            Err(BackupEnvelopeError::Authentication)
        );

        let mut trailing = envelope;
        trailing.push(0);
        assert_eq!(
            inspect_backup(&trailing),
            Err(BackupEnvelopeError::InvalidEnvelope)
        );

        let mut unsafe_sections = sections();
        unsafe_sections[0].name = "../personas.json".into();
        assert_eq!(
            seal_backup(
                "1.2.3",
                TimestampMillis::new(1),
                "password",
                unsafe_sections
            ),
            Err(BackupEnvelopeError::InvalidEntryName)
        );
        let duplicate = vec![sections()[0].clone(), sections()[0].clone()];
        assert_eq!(
            seal_backup("1.2.3", TimestampMillis::new(1), "password", duplicate),
            Err(BackupEnvelopeError::InvalidEntryName)
        );
    }

    #[test]
    fn envelope_rejects_oversized_declared_entries_before_decryption() {
        let envelope = seal_backup("1.2.3", TimestampMillis::new(1234), "password", sections())
            .expect("seal backup");
        let parsed = parse_envelope(&envelope).expect("parse envelope");
        let mut oversized_manifest = parsed.manifest;
        oversized_manifest.entries[0].plaintext_bytes =
            u64::try_from(MAX_BACKUP_ENTRY_BYTES + 1).expect("oversized length");
        let manifest_bytes =
            serde_json::to_vec(&oversized_manifest).expect("serialize oversized manifest");
        let mut oversized = Vec::new();
        oversized.extend_from_slice(&MAGIC);
        oversized.extend_from_slice(
            &u32::try_from(manifest_bytes.len())
                .expect("manifest length")
                .to_be_bytes(),
        );
        oversized.extend_from_slice(&manifest_bytes);
        oversized.extend_from_slice(&envelope[MAGIC.len() + 4 + parsed.manifest_bytes.len()..]);

        assert_eq!(
            inspect_backup(&oversized),
            Err(BackupEnvelopeError::LimitExceeded)
        );
    }

    #[test]
    fn confined_staging_resumes_and_never_replaces_an_existing_backup() {
        let root = std::env::temp_dir().join(format!("backup-store-{}", OperationId::new()));
        let envelope = seal_backup("1.2.3", TimestampMillis::new(1234), "password", sections())
            .expect("seal backup");
        let total = u64::try_from(envelope.len()).expect("envelope length");
        let hash = hash_bytes(&envelope);
        let id = OperationId::new();
        let split = envelope.len() / 2;
        let store = BackupArchiveStore::open(&root).expect("backup store");
        assert_eq!(store.receive_offset(id, total).expect("initial offset"), 0);
        assert_eq!(
            store
                .append(id, total, 0, &envelope[..split])
                .expect("first half"),
            u64::try_from(split).expect("split")
        );
        drop(store);

        let reopened = BackupArchiveStore::open(&root).expect("reopen backup store");
        assert_eq!(
            reopened.receive_offset(id, total).expect("resumed offset"),
            u64::try_from(split).expect("split")
        );
        reopened
            .append(
                id,
                total,
                u64::try_from(split).expect("split"),
                &envelope[split..],
            )
            .expect("second half");
        let installed = reopened.finish(id, total, &hash).expect("finish backup");
        assert_eq!(
            std::fs::read(&installed).expect("installed bytes"),
            envelope
        );
        assert_eq!(
            reopened
                .receive_offset(id, total)
                .expect("installed offset"),
            total
        );
        assert_eq!(
            reopened.finish(id, total, &hash).expect("finish replay"),
            installed
        );

        let changed = seal_backup("1.2.3", TimestampMillis::new(1235), "password", sections())
            .expect("changed backup");
        assert_eq!(
            reopened.append(id, total, total, &changed[..1]),
            Err(BackupArchiveStoreError::Conflict)
        );
        assert_eq!(std::fs::read(installed).expect("preserved bytes"), envelope);
    }
}
