use std::{
    collections::HashSet,
    fmt,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

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

pub const BACKUP_ENVELOPE_VERSION: u32 = 2;
pub const MAX_BACKUP_ENTRIES: usize = 2_097_152;
pub const MAX_BACKUP_ENTRY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_BACKUP_TOTAL_BYTES: u64 = 1024 * 1024 * 1024 * 1024;
pub const MAX_BACKUP_WRITE_CHUNK_BYTES: usize = 1024 * 1024;

const MAGIC: [u8; 16] = *b"LETTUCE-BACKUP2\0";
const FOOTER_MAGIC: [u8; 8] = *b"LTBKEND2";
const LEGACY_ZIP_PREFIX: [u8; 4] = *b"PK\x03\x04";
const MAX_MANIFEST_BYTES: usize = 1024 * 1024 * 1024;
const MAX_APP_VERSION_BYTES: usize = 64;
const MAX_ENTRY_NAME_BYTES: usize = 512;
const MAX_SCHEMA_BYTES: usize = 128;
const SALT_BYTES: usize = 16;
const NONCE_BYTES: usize = 24;
const ENTRY_NONCE_PREFIX_BYTES: usize = 19;
const TAG_BYTES: usize = 16;
const CHUNK_BYTES: usize = 1024 * 1024;
const HEADER_BYTES: usize = MAGIC.len() + 12 + SALT_BYTES;
const FOOTER_BYTES: usize = NONCE_BYTES + 4 + FOOTER_MAGIC.len();
const ARGON2_MEMORY_KIB: u32 = 64 * 1024;
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_LANES: u32 = 4;
const MIN_ARGON2_MEMORY_KIB: u32 = 19 * 1024;
const MAX_ARGON2_MEMORY_KIB: u32 = 256 * 1024;
const MAX_ARGON2_ITERATIONS: u32 = 8;
const MAX_ARGON2_LANES: u32 = 8;

#[derive(Clone, PartialEq, Eq)]
pub struct BackupSection {
    pub name: String,
    pub schema: String,
    pub bytes: Zeroizing<Vec<u8>>,
}

impl BackupSection {
    #[must_use]
    pub fn new(name: impl Into<String>, schema: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            schema: schema.into(),
            bytes: Zeroizing::new(bytes),
        }
    }
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

/// One section of an opened backup, readable through [`BackupReader`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupSectionInfo {
    pub name: String,
    pub schema: String,
    pub plaintext_bytes: u64,
    pub content_hash: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupFormatVersion {
    LegacyV1,
    CurrentV2,
}

/// Classifies a backup from its first bytes: the v2 magic, else a legacy
/// v1 ZIP. The full v2 structure is checked when the backup is opened.
pub fn detect_backup_format(prefix: &[u8]) -> Result<BackupFormatVersion, BackupEnvelopeError> {
    if prefix.starts_with(&MAGIC) {
        Ok(BackupFormatVersion::CurrentV2)
    } else if prefix.starts_with(&LEGACY_ZIP_PREFIX) {
        Ok(BackupFormatVersion::LegacyV1)
    } else {
        Err(BackupEnvelopeError::InvalidEnvelope)
    }
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
    #[error("backup file could not be read or written")]
    Io,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupManifest {
    version: u32,
    created_at: TimestampMillis,
    app_version: String,
    entries: Vec<BackupEntryDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BackupKdf {
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
    #[serde(with = "hex_prefix")]
    nonce_prefix: [u8; ENTRY_NONCE_PREFIX_BYTES],
}

mod hex_prefix {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    use super::ENTRY_NONCE_PREFIX_BYTES;

    pub(super) fn serialize<S: Serializer>(
        value: &[u8; ENTRY_NONCE_PREFIX_BYTES],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(
            &value
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
        )
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<[u8; ENTRY_NONCE_PREFIX_BYTES], D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.len() != ENTRY_NONCE_PREFIX_BYTES * 2
            || !text
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(D::Error::custom("invalid nonce prefix"));
        }
        let mut value = [0u8; ENTRY_NONCE_PREFIX_BYTES];
        for (index, byte) in value.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
                .map_err(D::Error::custom)?;
        }
        Ok(value)
    }
}

/// Writes a v2 backup to `out` section by section. The plaintext header
/// holds only the KDF parameters; every entry is encrypted in chunks and the
/// manifest (names, schemas, sizes, hashes) is an encrypted trailer.
pub struct BackupWriter<W: Write> {
    out: W,
    cipher: XChaCha20Poly1305,
    binding: [u8; 32],
    created_at: TimestampMillis,
    app_version: String,
    entries: Vec<BackupEntryDescriptor>,
    names: HashSet<String>,
    prefixes: HashSet<[u8; ENTRY_NONCE_PREFIX_BYTES]>,
    total: u64,
    failed: bool,
}

impl<W: Write> fmt::Debug for BackupWriter<W> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupWriter")
            .field("entries", &self.entries.len())
            .finish_non_exhaustive()
    }
}

impl<W: Write> BackupWriter<W> {
    pub fn new(
        mut out: W,
        app_version: impl Into<String>,
        created_at: TimestampMillis,
        password: &str,
    ) -> Result<Self, BackupEnvelopeError> {
        validate_password(password)?;
        let app_version = app_version.into();
        validate_app_version(&app_version)?;
        let mut salt = [0u8; SALT_BYTES];
        OsRng.fill_bytes(&mut salt);
        let kdf = BackupKdf {
            memory_kib: ARGON2_MEMORY_KIB,
            iterations: ARGON2_ITERATIONS,
            lanes: ARGON2_LANES,
            salt,
        };
        let header = encode_header(&kdf);
        let key = derive_key(password, &kdf)?;
        let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref())
            .map_err(|_| BackupEnvelopeError::Encryption)?;
        out.write_all(&header)
            .map_err(|_| BackupEnvelopeError::Io)?;
        Ok(Self {
            out,
            cipher,
            binding: *blake3::hash(&header).as_bytes(),
            created_at,
            app_version,
            entries: Vec::new(),
            names: HashSet::new(),
            prefixes: HashSet::new(),
            total: 0,
            failed: false,
        })
    }

    pub fn append_bytes(
        &mut self,
        name: &str,
        schema: &str,
        bytes: &[u8],
    ) -> Result<(), BackupEnvelopeError> {
        self.append_section(name, schema, &mut &*bytes)
    }

    /// Encrypts everything `source` yields as one section. Any error leaves
    /// the writer unusable: the output is incomplete and must be discarded.
    pub fn append_section(
        &mut self,
        name: &str,
        schema: &str,
        source: &mut dyn Read,
    ) -> Result<(), BackupEnvelopeError> {
        if self.failed {
            return Err(BackupEnvelopeError::InvalidEnvelope);
        }
        let result = self.write_section(name, schema, source);
        self.failed = result.is_err();
        result
    }

    fn write_section(
        &mut self,
        name: &str,
        schema: &str,
        source: &mut dyn Read,
    ) -> Result<(), BackupEnvelopeError> {
        validate_entry_name(name)?;
        validate_schema(schema)?;
        if self.entries.len() >= MAX_BACKUP_ENTRIES {
            return Err(BackupEnvelopeError::LimitExceeded);
        }
        if self.names.contains(name) {
            return Err(BackupEnvelopeError::InvalidEntryName);
        }
        let mut nonce_prefix = [0u8; ENTRY_NONCE_PREFIX_BYTES];
        loop {
            OsRng.fill_bytes(&mut nonce_prefix);
            if self.prefixes.insert(nonce_prefix) {
                break;
            }
        }
        let aad = entry_aad(&self.binding, self.entries.len())?;
        let mut hasher = blake3::Hasher::new();
        let mut current = Zeroizing::new(vec![0u8; CHUNK_BYTES]);
        let mut next = Zeroizing::new(vec![0u8; CHUNK_BYTES]);
        let mut current_len = fill(source, &mut current)?;
        let mut size = 0u64;
        let mut counter = 0u32;
        loop {
            let next_len = if current_len == CHUNK_BYTES {
                fill(source, &mut next)?
            } else {
                0
            };
            let last = next_len == 0;
            let plaintext = &current[..current_len];
            hasher.update(plaintext);
            size = size
                .checked_add(current_len as u64)
                .ok_or(BackupEnvelopeError::LimitExceeded)?;
            if size > MAX_BACKUP_ENTRY_BYTES
                || self.total.saturating_add(size) > MAX_BACKUP_TOTAL_BYTES
            {
                return Err(BackupEnvelopeError::LimitExceeded);
            }
            let encrypted = self
                .cipher
                .encrypt(
                    &chunk_nonce(&nonce_prefix, counter, last),
                    Payload {
                        msg: plaintext,
                        aad: &aad,
                    },
                )
                .map_err(|_| BackupEnvelopeError::Encryption)?;
            self.out
                .write_all(&encrypted)
                .map_err(|_| BackupEnvelopeError::Io)?;
            if last {
                break;
            }
            std::mem::swap(&mut current, &mut next);
            current_len = next_len;
            counter = counter
                .checked_add(1)
                .ok_or(BackupEnvelopeError::LimitExceeded)?;
        }
        self.total = self
            .total
            .checked_add(size)
            .filter(|total| *total <= MAX_BACKUP_TOTAL_BYTES)
            .ok_or(BackupEnvelopeError::LimitExceeded)?;
        self.names.insert(name.to_owned());
        self.entries.push(BackupEntryDescriptor {
            name: name.to_owned(),
            schema: schema.to_owned(),
            plaintext_bytes: size,
            content_hash: ContentHash::parse(hasher.finalize().to_hex().to_string())
                .expect("BLAKE3 produces a valid content hash"),
            nonce_prefix,
        });
        Ok(())
    }

    /// Writes the encrypted manifest trailer and returns the sink.
    pub fn finish(mut self) -> Result<W, BackupEnvelopeError> {
        if self.failed {
            return Err(BackupEnvelopeError::InvalidEnvelope);
        }
        if self.entries.is_empty() {
            return Err(BackupEnvelopeError::LimitExceeded);
        }
        let manifest = BackupManifest {
            version: BACKUP_ENVELOPE_VERSION,
            created_at: self.created_at,
            app_version: std::mem::take(&mut self.app_version),
            entries: std::mem::take(&mut self.entries),
        };
        let manifest_bytes = Zeroizing::new(
            serde_json::to_vec(&manifest).map_err(|_| BackupEnvelopeError::InvalidMetadata)?,
        );
        if manifest_bytes.len() > MAX_MANIFEST_BYTES {
            return Err(BackupEnvelopeError::LimitExceeded);
        }
        let mut nonce = [0u8; NONCE_BYTES];
        OsRng.fill_bytes(&mut nonce);
        let encrypted = self
            .cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &manifest_bytes,
                    aad: &self.binding,
                },
            )
            .map_err(|_| BackupEnvelopeError::Encryption)?;
        let length =
            u32::try_from(encrypted.len()).map_err(|_| BackupEnvelopeError::LimitExceeded)?;
        let mut footer = Vec::with_capacity(FOOTER_BYTES);
        footer.extend_from_slice(&nonce);
        footer.extend_from_slice(&length.to_be_bytes());
        footer.extend_from_slice(&FOOTER_MAGIC);
        self.out
            .write_all(&encrypted)
            .and_then(|()| self.out.write_all(&footer))
            .and_then(|()| self.out.flush())
            .map_err(|_| BackupEnvelopeError::Io)?;
        Ok(self.out)
    }
}

/// An opened, authenticated v2 backup whose sections are decrypted on
/// demand from the seekable source.
pub struct BackupReader<R: Read + Seek> {
    input: R,
    cipher: XChaCha20Poly1305,
    binding: [u8; 32],
    manifest: BackupManifest,
    offsets: Vec<u64>,
}

impl<R: Read + Seek> fmt::Debug for BackupReader<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupReader")
            .field("entries", &self.manifest.entries.len())
            .finish_non_exhaustive()
    }
}

impl<R: Read + Seek> BackupReader<R> {
    pub fn open(mut input: R, password: &str) -> Result<Self, BackupEnvelopeError> {
        validate_password(password)?;
        let layout = read_layout(&mut input)?;
        let key = derive_key(password, &layout.kdf)?;
        let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref())
            .map_err(|_| BackupEnvelopeError::Authentication)?;
        input
            .seek(SeekFrom::Start(layout.manifest_offset))
            .map_err(|_| BackupEnvelopeError::Io)?;
        let mut encrypted = vec![0u8; layout.manifest_len];
        input
            .read_exact(&mut encrypted)
            .map_err(|_| BackupEnvelopeError::InvalidEnvelope)?;
        let manifest_bytes = Zeroizing::new(
            cipher
                .decrypt(
                    XNonce::from_slice(&layout.manifest_nonce),
                    Payload {
                        msg: &encrypted,
                        aad: &layout.binding,
                    },
                )
                .map_err(|_| BackupEnvelopeError::Authentication)?,
        );
        let manifest: BackupManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|_| BackupEnvelopeError::InvalidMetadata)?;
        validate_manifest(&manifest)?;
        let mut offsets = Vec::with_capacity(manifest.entries.len());
        let mut position = HEADER_BYTES as u64;
        for entry in &manifest.entries {
            offsets.push(position);
            position = position
                .checked_add(encrypted_len(entry.plaintext_bytes)?)
                .ok_or(BackupEnvelopeError::InvalidEnvelope)?;
        }
        if position != layout.manifest_offset {
            return Err(BackupEnvelopeError::InvalidEnvelope);
        }
        Ok(Self {
            input,
            cipher,
            binding: layout.binding,
            manifest,
            offsets,
        })
    }

    #[must_use]
    pub fn info(&self) -> BackupInfo {
        BackupInfo {
            version: self.manifest.version,
            created_at: self.manifest.created_at,
            app_version: self.manifest.app_version.clone(),
            entry_count: self.manifest.entries.len(),
            plaintext_bytes: self
                .manifest
                .entries
                .iter()
                .map(|entry| entry.plaintext_bytes)
                .sum(),
        }
    }

    #[must_use]
    pub fn sections(&self) -> Vec<BackupSectionInfo> {
        self.manifest
            .entries
            .iter()
            .map(|entry| BackupSectionInfo {
                name: entry.name.clone(),
                schema: entry.schema.clone(),
                plaintext_bytes: entry.plaintext_bytes,
                content_hash: entry.content_hash.clone(),
            })
            .collect()
    }

    /// Decrypts one section into memory.
    pub fn read_section(
        &mut self,
        index: usize,
    ) -> Result<Zeroizing<Vec<u8>>, BackupEnvelopeError> {
        let size = self
            .manifest
            .entries
            .get(index)
            .ok_or(BackupEnvelopeError::InvalidMetadata)?
            .plaintext_bytes;
        let mut bytes = Zeroizing::new(Vec::with_capacity(
            usize::try_from(size).map_err(|_| BackupEnvelopeError::LimitExceeded)?,
        ));
        self.copy_section(index, &mut *bytes)?;
        Ok(bytes)
    }

    /// Decrypts one section chunk by chunk into `out`. Every chunk is
    /// authenticated before it is written; the content hash is checked at
    /// the end, so a caller discards `out` on any error.
    pub fn copy_section(
        &mut self,
        index: usize,
        out: &mut dyn Write,
    ) -> Result<(), BackupEnvelopeError> {
        let entry = self
            .manifest
            .entries
            .get(index)
            .ok_or(BackupEnvelopeError::InvalidMetadata)?;
        let aad = entry_aad(&self.binding, index)?;
        self.input
            .seek(SeekFrom::Start(self.offsets[index]))
            .map_err(|_| BackupEnvelopeError::Io)?;
        let mut hasher = blake3::Hasher::new();
        let mut remaining = entry.plaintext_bytes;
        let mut counter = 0u32;
        let mut encrypted = vec![0u8; CHUNK_BYTES + TAG_BYTES];
        loop {
            let chunk = usize::try_from(remaining.min(CHUNK_BYTES as u64))
                .map_err(|_| BackupEnvelopeError::LimitExceeded)?;
            let last = remaining <= CHUNK_BYTES as u64;
            let buffer = &mut encrypted[..chunk + TAG_BYTES];
            self.input
                .read_exact(buffer)
                .map_err(|_| BackupEnvelopeError::InvalidEnvelope)?;
            let plaintext = Zeroizing::new(
                self.cipher
                    .decrypt(
                        &chunk_nonce(&entry.nonce_prefix, counter, last),
                        Payload {
                            msg: buffer,
                            aad: &aad,
                        },
                    )
                    .map_err(|_| BackupEnvelopeError::Authentication)?,
            );
            hasher.update(&plaintext);
            out.write_all(&plaintext)
                .map_err(|_| BackupEnvelopeError::Io)?;
            remaining -= chunk as u64;
            if last {
                break;
            }
            counter = counter
                .checked_add(1)
                .ok_or(BackupEnvelopeError::InvalidEnvelope)?;
        }
        if ContentHash::parse(hasher.finalize().to_hex().to_string()).as_ref()
            != Ok(&entry.content_hash)
        {
            return Err(BackupEnvelopeError::Authentication);
        }
        Ok(())
    }
}

pub fn seal_backup(
    app_version: impl Into<String>,
    created_at: TimestampMillis,
    password: &str,
    sections: Vec<BackupSection>,
) -> Result<Vec<u8>, BackupEnvelopeError> {
    let mut writer = BackupWriter::new(Vec::new(), app_version, created_at, password)?;
    for section in &sections {
        writer.append_bytes(&section.name, &section.schema, &section.bytes)?;
    }
    writer.finish()
}

pub fn open_backup(
    bytes: &[u8],
    password: &str,
) -> Result<Vec<BackupSection>, BackupEnvelopeError> {
    let mut reader = BackupReader::open(std::io::Cursor::new(bytes), password)?;
    let mut sections = Vec::with_capacity(reader.manifest.entries.len());
    for (index, info) in reader.sections().into_iter().enumerate() {
        sections.push(BackupSection {
            name: info.name,
            schema: info.schema,
            bytes: reader.read_section(index)?,
        });
    }
    Ok(sections)
}

/// Checks the plaintext frame of a v2 backup without the password: magic,
/// KDF parameters in range, footer and a consistent manifest position.
pub fn verify_backup_frame(input: &mut (impl Read + Seek)) -> Result<(), BackupEnvelopeError> {
    read_layout(input).map(|_| ())
}

struct BackupLayout {
    kdf: BackupKdf,
    binding: [u8; 32],
    manifest_nonce: [u8; NONCE_BYTES],
    manifest_offset: u64,
    manifest_len: usize,
}

fn read_layout(input: &mut (impl Read + Seek)) -> Result<BackupLayout, BackupEnvelopeError> {
    let total = input
        .seek(SeekFrom::End(0))
        .map_err(|_| BackupEnvelopeError::Io)?;
    let minimum = (HEADER_BYTES + TAG_BYTES + FOOTER_BYTES) as u64;
    if total < minimum {
        return Err(BackupEnvelopeError::InvalidEnvelope);
    }
    input
        .seek(SeekFrom::Start(0))
        .map_err(|_| BackupEnvelopeError::Io)?;
    let mut header = [0u8; HEADER_BYTES];
    input
        .read_exact(&mut header)
        .map_err(|_| BackupEnvelopeError::InvalidEnvelope)?;
    let kdf = decode_header(&header)?;
    input
        .seek(SeekFrom::Start(total - FOOTER_BYTES as u64))
        .map_err(|_| BackupEnvelopeError::Io)?;
    let mut footer = [0u8; FOOTER_BYTES];
    input
        .read_exact(&mut footer)
        .map_err(|_| BackupEnvelopeError::InvalidEnvelope)?;
    if footer[NONCE_BYTES + 4..] != FOOTER_MAGIC {
        return Err(BackupEnvelopeError::InvalidEnvelope);
    }
    let manifest_nonce: [u8; NONCE_BYTES] = footer[..NONCE_BYTES]
        .try_into()
        .map_err(|_| BackupEnvelopeError::InvalidEnvelope)?;
    let manifest_len = u32::from_be_bytes(
        footer[NONCE_BYTES..NONCE_BYTES + 4]
            .try_into()
            .map_err(|_| BackupEnvelopeError::InvalidEnvelope)?,
    ) as usize;
    if manifest_len <= TAG_BYTES || manifest_len > MAX_MANIFEST_BYTES + TAG_BYTES {
        return Err(BackupEnvelopeError::LimitExceeded);
    }
    let manifest_offset = (total - FOOTER_BYTES as u64)
        .checked_sub(manifest_len as u64)
        .filter(|offset| *offset >= HEADER_BYTES as u64)
        .ok_or(BackupEnvelopeError::InvalidEnvelope)?;
    Ok(BackupLayout {
        kdf,
        binding: *blake3::hash(&header).as_bytes(),
        manifest_nonce,
        manifest_offset,
        manifest_len,
    })
}

fn encode_header(kdf: &BackupKdf) -> [u8; HEADER_BYTES] {
    let mut header = [0u8; HEADER_BYTES];
    header[..MAGIC.len()].copy_from_slice(&MAGIC);
    let mut position = MAGIC.len();
    for value in [kdf.memory_kib, kdf.iterations, kdf.lanes] {
        header[position..position + 4].copy_from_slice(&value.to_be_bytes());
        position += 4;
    }
    header[position..].copy_from_slice(&kdf.salt);
    header
}

fn decode_header(header: &[u8; HEADER_BYTES]) -> Result<BackupKdf, BackupEnvelopeError> {
    if header[..MAGIC.len()] != MAGIC {
        return Err(BackupEnvelopeError::InvalidEnvelope);
    }
    let value = |at: usize| {
        u32::from_be_bytes(
            header[at..at + 4]
                .try_into()
                .expect("header field is four bytes"),
        )
    };
    let kdf = BackupKdf {
        memory_kib: value(MAGIC.len()),
        iterations: value(MAGIC.len() + 4),
        lanes: value(MAGIC.len() + 8),
        salt: header[MAGIC.len() + 12..]
            .try_into()
            .expect("header salt length"),
    };
    if !(MIN_ARGON2_MEMORY_KIB..=MAX_ARGON2_MEMORY_KIB).contains(&kdf.memory_kib)
        || !(1..=MAX_ARGON2_ITERATIONS).contains(&kdf.iterations)
        || !(1..=MAX_ARGON2_LANES).contains(&kdf.lanes)
    {
        return Err(BackupEnvelopeError::InvalidMetadata);
    }
    Ok(kdf)
}

fn encrypted_len(plaintext_bytes: u64) -> Result<u64, BackupEnvelopeError> {
    let chunks = plaintext_bytes.div_ceil(CHUNK_BYTES as u64).max(1);
    chunks
        .checked_mul(TAG_BYTES as u64)
        .and_then(|tags| tags.checked_add(plaintext_bytes))
        .ok_or(BackupEnvelopeError::LimitExceeded)
}

fn chunk_nonce(prefix: &[u8; ENTRY_NONCE_PREFIX_BYTES], counter: u32, last: bool) -> XNonce {
    let mut nonce = [0u8; NONCE_BYTES];
    nonce[..ENTRY_NONCE_PREFIX_BYTES].copy_from_slice(prefix);
    nonce[ENTRY_NONCE_PREFIX_BYTES..NONCE_BYTES - 1].copy_from_slice(&counter.to_be_bytes());
    nonce[NONCE_BYTES - 1] = u8::from(last);
    *XNonce::from_slice(&nonce)
}

fn fill(source: &mut dyn Read, buffer: &mut [u8]) -> Result<usize, BackupEnvelopeError> {
    let mut filled = 0;
    while filled < buffer.len() {
        match source.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(BackupEnvelopeError::Io),
        }
    }
    Ok(filled)
}

fn validate_manifest(manifest: &BackupManifest) -> Result<(), BackupEnvelopeError> {
    if manifest.version != BACKUP_ENVELOPE_VERSION {
        return Err(BackupEnvelopeError::InvalidMetadata);
    }
    validate_app_version(&manifest.app_version)?;
    if manifest.entries.is_empty() || manifest.entries.len() > MAX_BACKUP_ENTRIES {
        return Err(BackupEnvelopeError::LimitExceeded);
    }
    let mut names = HashSet::with_capacity(manifest.entries.len());
    let mut prefixes = HashSet::with_capacity(manifest.entries.len());
    let mut total = 0u64;
    for entry in &manifest.entries {
        validate_entry_name(&entry.name)?;
        validate_schema(&entry.schema)?;
        if !names.insert(entry.name.as_str()) {
            return Err(BackupEnvelopeError::InvalidEntryName);
        }
        if !prefixes.insert(entry.nonce_prefix) {
            return Err(BackupEnvelopeError::InvalidMetadata);
        }
        if entry.plaintext_bytes > MAX_BACKUP_ENTRY_BYTES
            || ContentHash::parse(entry.content_hash.as_str()).as_ref() != Ok(&entry.content_hash)
        {
            return Err(BackupEnvelopeError::LimitExceeded);
        }
        total = total
            .checked_add(entry.plaintext_bytes)
            .filter(|total| *total <= MAX_BACKUP_TOTAL_BYTES)
            .ok_or(BackupEnvelopeError::LimitExceeded)?;
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

#[cfg(test)]
fn hash_bytes(bytes: &[u8]) -> ContentHash {
    ContentHash::parse(blake3::hash(bytes).to_hex().to_string())
        .expect("BLAKE3 produces a valid content hash")
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
                if hash_reader(&mut file)? != *expected_hash {
                    file.restart().map_err(BackupArchiveStoreError::Platform)?;
                    return Err(BackupArchiveStoreError::HashMismatch);
                }
                if let Err(error) = verify_backup_frame(&mut file) {
                    if error != BackupEnvelopeError::Io {
                        file.restart().map_err(BackupArchiveStoreError::Platform)?;
                    }
                    return Err(BackupArchiveStoreError::Envelope(error));
                }
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
        let max = Some(MAX_BACKUP_TOTAL_BYTES)
            .and_then(|total| {
                let chunks =
                    u64::try_from(MAX_BACKUP_ENTRIES).ok()? + total.div_ceil(CHUNK_BYTES as u64);
                total
                    .checked_add(chunks.checked_mul(TAG_BYTES as u64)?)?
                    .checked_add(
                        (HEADER_BYTES + MAX_MANIFEST_BYTES + TAG_BYTES + FOOTER_BYTES) as u64,
                    )
            })
            .ok_or(BackupArchiveStoreError::LimitExceeded)?;
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
            BackupSection::new(
                "data/personas.json",
                "persona.snapshot.v1",
                br#"[{"title":"Private persona"}]"#.to_vec(),
            ),
            BackupSection::new(
                "media/blake3/asset.bin",
                "media.blob.v1",
                b"exact media bytes".to_vec(),
            ),
            BackupSection::new("data/empty.json", "empty.v1", Vec::new()),
        ]
    }

    fn envelope(password: &str) -> Vec<u8> {
        seal_backup("1.2.3", TimestampMillis::new(1234), password, sections()).expect("seal backup")
    }

    #[test]
    fn encrypted_envelope_round_trips_with_a_private_manifest() {
        let bytes = envelope("correct horse battery staple");
        assert_eq!(
            detect_backup_format(&bytes),
            Ok(BackupFormatVersion::CurrentV2)
        );
        verify_backup_frame(&mut std::io::Cursor::new(&bytes)).expect("frame");
        let reader =
            BackupReader::open(std::io::Cursor::new(&bytes), "correct horse battery staple")
                .expect("open reader");
        let info = reader.info();
        assert_eq!(info.version, 2);
        assert_eq!(info.created_at, TimestampMillis::new(1234));
        assert_eq!(info.entry_count, 3);
        assert_eq!(
            info.plaintext_bytes,
            sections()
                .iter()
                .map(|section| section.bytes.len() as u64)
                .sum::<u64>()
        );
        assert_eq!(
            open_backup(&bytes, "correct horse battery staple").expect("open backup"),
            sections()
        );
        for needle in [
            &b"data/personas.json"[..],
            b"media.blob.v1",
            b"Private persona",
            b"exact media bytes",
            hash_bytes(b"exact media bytes").as_str().as_bytes(),
        ] {
            assert!(
                !bytes.windows(needle.len()).any(|window| window == needle),
                "plaintext leaked"
            );
        }
        assert!(!format!("{:?}", sections()[0]).contains("Private persona"));
    }

    #[test]
    fn large_sections_stream_in_chunks() {
        let large = (0..(CHUNK_BYTES * 2 + 7))
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        let exact = vec![9u8; CHUNK_BYTES];
        let mut writer =
            BackupWriter::new(Vec::new(), "1.2.3", TimestampMillis::new(1), "password")
                .expect("writer");
        writer
            .append_section("media/large", "media.v1", &mut large.as_slice())
            .expect("large section");
        writer
            .append_bytes("media/exact", "media.v1", &exact)
            .expect("exact chunk section");
        let bytes = writer.finish().expect("finish");
        let mut reader =
            BackupReader::open(std::io::Cursor::new(&bytes), "password").expect("reader");
        let mut copied = Vec::new();
        reader.copy_section(0, &mut copied).expect("copy large");
        assert_eq!(copied, large);
        assert_eq!(*reader.read_section(1).expect("read exact"), exact);
        assert_eq!(reader.sections()[0].content_hash, hash_bytes(&large));
    }

    #[test]
    fn unversioned_legacy_zip_is_classified_as_version_one() {
        assert_eq!(
            detect_backup_format(b"PK\x03\x04legacy archive bytes"),
            Ok(BackupFormatVersion::LegacyV1)
        );
        assert_eq!(
            detect_backup_format(b"unrecognized backup"),
            Err(BackupEnvelopeError::InvalidEnvelope)
        );
    }

    #[test]
    fn envelope_rejects_wrong_password_tampering_truncation_and_bad_names() {
        let bytes = envelope("correct password");
        assert_eq!(
            open_backup(&bytes, "wrong password"),
            Err(BackupEnvelopeError::Authentication)
        );
        for position in [HEADER_BYTES + 3, bytes.len() - FOOTER_BYTES - 1] {
            let mut tampered = bytes.clone();
            tampered[position] ^= 1;
            assert_eq!(
                open_backup(&tampered, "correct password"),
                Err(BackupEnvelopeError::Authentication)
            );
        }
        let mut salt_changed = bytes.clone();
        salt_changed[HEADER_BYTES - 1] ^= 1;
        assert_eq!(
            open_backup(&salt_changed, "correct password"),
            Err(BackupEnvelopeError::Authentication)
        );
        let mut weak = bytes.clone();
        weak[MAGIC.len()..MAGIC.len() + 4].copy_from_slice(&1024u32.to_be_bytes());
        assert_eq!(
            open_backup(&weak, "correct password"),
            Err(BackupEnvelopeError::InvalidMetadata)
        );
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            open_backup(&trailing, "correct password"),
            Err(BackupEnvelopeError::InvalidEnvelope)
        );
        let mut truncated = bytes[..HEADER_BYTES + 20].to_vec();
        truncated.extend_from_slice(&bytes[bytes.len() - FOOTER_BYTES..]);
        assert!(open_backup(&truncated, "correct password").is_err());

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
    fn a_failed_section_poisons_the_writer() {
        struct Failing;
        impl Read for Failing {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("source failed"))
            }
        }
        let mut writer =
            BackupWriter::new(Vec::new(), "1.2.3", TimestampMillis::new(1), "password")
                .expect("writer");
        writer.append_bytes("data/a", "a.v1", b"first").expect("a");
        assert_eq!(
            writer.append_section("data/b", "b.v1", &mut Failing),
            Err(BackupEnvelopeError::Io)
        );
        assert_eq!(
            writer.append_bytes("data/c", "c.v1", b"third"),
            Err(BackupEnvelopeError::InvalidEnvelope)
        );
        assert_eq!(
            writer.finish().map(|_| ()),
            Err(BackupEnvelopeError::InvalidEnvelope)
        );
    }

    #[test]
    fn swapped_entries_fail_authentication() {
        let mut writer =
            BackupWriter::new(Vec::new(), "1.2.3", TimestampMillis::new(1), "password")
                .expect("writer");
        writer.append_bytes("data/a", "a.v1", b"first!").expect("a");
        writer.append_bytes("data/b", "b.v1", b"second").expect("b");
        let bytes = writer.finish().expect("finish");
        let entry = 6 + TAG_BYTES;
        let mut swapped = bytes.clone();
        swapped[HEADER_BYTES..HEADER_BYTES + entry]
            .copy_from_slice(&bytes[HEADER_BYTES + entry..HEADER_BYTES + 2 * entry]);
        swapped[HEADER_BYTES + entry..HEADER_BYTES + 2 * entry]
            .copy_from_slice(&bytes[HEADER_BYTES..HEADER_BYTES + entry]);
        assert_eq!(
            open_backup(&swapped, "password"),
            Err(BackupEnvelopeError::Authentication)
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
