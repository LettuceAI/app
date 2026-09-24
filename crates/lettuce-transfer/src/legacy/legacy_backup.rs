use std::{
    collections::BTreeSet,
    io::{Read, SeekFrom},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use base64::{Engine as _, engine::general_purpose};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit},
};
use lettuce_types::ContentHash;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;
use zip::ZipArchive;

use crate::MAX_BACKUP_ENTRIES;

/// A version-1 entry is one AEAD message, so each is decrypted whole.
pub const MAX_LEGACY_BACKUP_ENTRY_BYTES: usize = 512 * 1024 * 1024;
/// The decrypted documents are held together while the archive is planned.
pub const MAX_LEGACY_BACKUP_DOCUMENT_TOTAL_BYTES: usize = 512 * 1024 * 1024;

const LEGACY_MANIFEST_VERSION: u32 = 2;
const LEGACY_MARKER: &[u8] = b"LETTUCE_BACKUP_VERIFIED";
const MAX_LEGACY_MANIFEST_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupInventory {
    pub version: u32,
    pub created_at: u64,
    pub app_version: String,
    pub source_hash: ContentHash,
    pub documents: Vec<LegacyBackupDocument>,
    pub media: Vec<LegacyBackupMedia>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct LegacyBackupDocument {
    pub kind: LegacyBackupDocumentKind,
    pub bytes: Zeroizing<Vec<u8>>,
}

/// One legacy media file: its size and content hash are known up front and
/// its bytes are read on demand, so an inventory never holds the library.
#[derive(Clone)]
pub struct LegacyBackupMedia {
    pub root: LegacyBackupMediaRoot,
    pub relative_segments: Vec<String>,
    pub byte_len: u64,
    pub content_hash: ContentHash,
    source: LegacyMediaSource,
}

#[derive(Clone)]
enum LegacyMediaSource {
    Memory(Arc<Zeroizing<Vec<u8>>>),
    File(PathBuf),
    Archive(Arc<LegacyArchive>, usize),
}

struct LegacyArchive {
    zip: Mutex<ZipArchive<Box<dyn crate::BackupSource>>>,
    key: Zeroizing<[u8; 32]>,
    nonce: [u8; 24],
}

impl LegacyArchive {
    fn entry(&self, index: usize) -> Result<Zeroizing<Vec<u8>>, LegacyBackupInventoryError> {
        let mut zip = self
            .zip
            .lock()
            .map_err(|_| LegacyBackupInventoryError::InvalidArchive)?;
        let mut file = zip
            .by_index(index)
            .map_err(|_| LegacyBackupInventoryError::InvalidArchive)?;
        let encrypted = read_bounded(&mut file, MAX_LEGACY_BACKUP_ENTRY_BYTES as u64 + 16)?;
        let decrypted = Zeroizing::new(decrypt(&encrypted, &self.key, &self.nonce)?);
        if decrypted.len() > MAX_LEGACY_BACKUP_ENTRY_BYTES {
            return Err(LegacyBackupInventoryError::LimitExceeded);
        }
        Ok(decrypted)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyMediaReadError {
    #[error("legacy media file could not be read")]
    Unreadable,
    #[error("legacy media file exceeds {limit} bytes")]
    TooLarge { limit: u64 },
    #[error("legacy media changed after it was inventoried")]
    Changed,
}

impl LegacyBackupMedia {
    #[must_use]
    pub fn from_bytes(
        root: LegacyBackupMediaRoot,
        relative_segments: Vec<String>,
        bytes: Zeroizing<Vec<u8>>,
    ) -> Self {
        Self {
            root,
            relative_segments,
            byte_len: bytes.len() as u64,
            content_hash: content_hash(&bytes),
            source: LegacyMediaSource::Memory(Arc::new(bytes)),
        }
    }

    /// Hashes the file by streaming it and keeps only its path.
    pub fn from_file(
        root: LegacyBackupMediaRoot,
        relative_segments: Vec<String>,
        path: PathBuf,
        max_bytes: u64,
    ) -> Result<Self, LegacyMediaReadError> {
        let file = std::fs::File::open(&path).map_err(|_| LegacyMediaReadError::Unreadable)?;
        let mut hasher = blake3::Hasher::new();
        let byte_len = std::io::copy(&mut file.take(max_bytes.saturating_add(1)), &mut hasher)
            .map_err(|_| LegacyMediaReadError::Unreadable)?;
        if byte_len > max_bytes {
            return Err(LegacyMediaReadError::TooLarge { limit: max_bytes });
        }
        Ok(Self {
            root,
            relative_segments,
            byte_len,
            content_hash: ContentHash::parse(hasher.finalize().to_hex().to_string())
                .expect("BLAKE3 produces a valid content hash"),
            source: LegacyMediaSource::File(path),
        })
    }

    /// The bytes, checked against the inventoried size and hash.
    pub fn read(&self) -> Result<Zeroizing<Vec<u8>>, LegacyMediaReadError> {
        let bytes = match &self.source {
            LegacyMediaSource::Memory(bytes) => Zeroizing::new(bytes.to_vec()),
            LegacyMediaSource::File(path) => {
                let mut bytes = Zeroizing::new(Vec::with_capacity(
                    usize::try_from(self.byte_len).map_err(|_| LegacyMediaReadError::Changed)?,
                ));
                std::fs::File::open(path)
                    .and_then(|file| {
                        file.take(self.byte_len.saturating_add(1))
                            .read_to_end(&mut bytes)
                    })
                    .map_err(|_| LegacyMediaReadError::Unreadable)?;
                bytes
            }
            LegacyMediaSource::Archive(archive, index) => archive
                .entry(*index)
                .map_err(|_| LegacyMediaReadError::Unreadable)?,
        };
        if bytes.len() as u64 != self.byte_len || content_hash(&bytes) != self.content_hash {
            return Err(LegacyMediaReadError::Changed);
        }
        Ok(bytes)
    }
}

impl PartialEq for LegacyBackupMedia {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
            && self.relative_segments == other.relative_segments
            && self.byte_len == other.byte_len
            && self.content_hash == other.content_hash
    }
}

impl Eq for LegacyBackupMedia {}

impl std::fmt::Debug for LegacyBackupDocument {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LegacyBackupDocument")
            .field("kind", &self.kind)
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

impl std::fmt::Debug for LegacyBackupMedia {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LegacyBackupMedia")
            .field("root", &self.root)
            .field("relative_segments", &self.relative_segments)
            .field("byte_len", &self.byte_len)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LegacyBackupDocumentKind {
    Meta,
    Settings,
    ProviderCredentials,
    Models,
    AudioProviders,
    UserVoices,
    ModelPricingCache,
    Secrets,
    PromptTemplates,
    ChatTemplates,
    Personas,
    Characters,
    CompanionScheduledNotes,
    CompanionSharedMemory,
    MemoryEmbeddings,
    Sessions,
    CreationHelperSessions,
    AsrLearning,
    GroupCharacters,
    GroupSessions,
    UsageRecords,
    Lorebooks,
    CharacterLorebooks,
    /// Only the live legacy database holds it; archives never did.
    ImageLoras,
    /// Only the live legacy database holds it; archives never did.
    PlaygroundGenerations,
    /// Only the live legacy database holds it; archives never did.
    LlmGenerationMetrics,
}

impl LegacyBackupDocumentKind {
    #[must_use]
    pub const fn archive_stem(self) -> &'static str {
        match self {
            Self::Meta => "meta",
            Self::Settings => "settings",
            Self::ProviderCredentials => "provider_credentials",
            Self::Models => "models",
            Self::AudioProviders => "audio_providers",
            Self::UserVoices => "user_voices",
            Self::ModelPricingCache => "model_pricing_cache",
            Self::Secrets => "secrets",
            Self::PromptTemplates => "prompt_templates",
            Self::ChatTemplates => "chat_templates",
            Self::Personas => "personas",
            Self::Characters => "characters",
            Self::CompanionScheduledNotes => "companion_scheduled_notes",
            Self::CompanionSharedMemory => "companion_shared_memory",
            Self::MemoryEmbeddings => "memory_embeddings",
            Self::Sessions => "sessions",
            Self::CreationHelperSessions => "creation_helper_sessions",
            Self::AsrLearning => "asr_learning",
            Self::GroupCharacters => "group_characters",
            Self::GroupSessions => "group_sessions",
            Self::UsageRecords => "usage_records",
            Self::Lorebooks => "lorebooks",
            Self::CharacterLorebooks => "character_lorebooks",
            Self::ImageLoras => "image_loras",
            Self::PlaygroundGenerations => "playground_generations",
            Self::LlmGenerationMetrics => "llm_generation_metrics",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyBackupMediaRoot {
    Images,
    Avatars,
    Attachments,
    Sessions,
    GeneratedImages,
    /// Images legacy stored as `data:` URLs in database values, planned from
    /// those values.
    Inline,
}

impl LegacyBackupMediaRoot {
    #[must_use]
    pub const fn archive_name(self) -> &'static str {
        match self {
            Self::Images => "images",
            Self::Avatars => "avatars",
            Self::Attachments => "attachments",
            Self::Sessions => "sessions",
            Self::GeneratedImages => "generated_images",
            Self::Inline => "inline",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupInventoryError {
    #[error("legacy backup password is invalid")]
    InvalidPassword,
    #[error("legacy backup archive is malformed")]
    InvalidArchive,
    #[error("legacy backup manifest is unsupported or malformed")]
    InvalidManifest,
    #[error("legacy backup inventory is unsafe or inconsistent")]
    InvalidInventory,
    #[error("legacy backup exceeds its size or entry limit")]
    LimitExceeded,
    #[error("legacy backup authentication failed")]
    Authentication,
}

#[derive(Deserialize)]
struct LegacyManifest {
    version: u32,
    created_at: u64,
    app_version: String,
    encrypted: bool,
    salt: Option<String>,
    nonce: Option<String>,
}

/// Decodes a version-1 backup from a seekable source. Documents are
/// decrypted into memory; each media entry is decrypted once to record its
/// size and hash and again only when its bytes are read.
pub fn decode_legacy_backup_inventory(
    mut input: impl crate::BackupSource + 'static,
    password: &str,
) -> Result<LegacyBackupInventory, LegacyBackupInventoryError> {
    validate_password(password)?;
    input
        .seek(SeekFrom::Start(0))
        .map_err(|_| LegacyBackupInventoryError::InvalidArchive)?;
    let mut prefix = [0u8; 4];
    input
        .read_exact(&mut prefix)
        .map_err(|_| LegacyBackupInventoryError::InvalidArchive)?;
    if prefix != *b"PK\x03\x04" {
        return Err(LegacyBackupInventoryError::InvalidArchive);
    }
    input
        .seek(SeekFrom::Start(0))
        .map_err(|_| LegacyBackupInventoryError::InvalidArchive)?;
    let mut hasher = blake3::Hasher::new();
    hasher
        .update_reader(&mut input)
        .map_err(|_| LegacyBackupInventoryError::InvalidArchive)?;
    let source_hash = ContentHash::parse(hasher.finalize().to_hex().to_string())
        .expect("BLAKE3 produces a valid content hash");
    let mut archive = ZipArchive::new(Box::new(input) as Box<dyn crate::BackupSource>)
        .map_err(|_| LegacyBackupInventoryError::InvalidArchive)?;
    if archive.is_empty() || archive.len() > MAX_BACKUP_ENTRIES {
        return Err(LegacyBackupInventoryError::LimitExceeded);
    }
    validate_archive_metadata(&mut archive)?;
    let manifest_bytes =
        read_public_entry(&mut archive, "manifest.json", MAX_LEGACY_MANIFEST_BYTES)?;
    let manifest: LegacyManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| LegacyBackupInventoryError::InvalidManifest)?;
    if manifest.version != LEGACY_MANIFEST_VERSION
        || !manifest.encrypted
        || manifest.app_version.is_empty()
        || manifest.app_version.len() > 64
        || manifest.app_version.chars().any(char::is_control)
    {
        return Err(LegacyBackupInventoryError::InvalidManifest);
    }
    let salt = decode_array::<16>(manifest.salt.as_deref())?;
    let nonce = decode_array::<24>(manifest.nonce.as_deref())?;
    let key = Zeroizing::new(derive_key(password, &salt));
    let marker = read_public_entry(&mut archive, "encrypted_marker.bin", 1024)?;
    let marker_result = decrypt(&marker, &key, &nonce);
    if marker_result.as_deref() != Ok(LEGACY_MARKER) {
        return Err(LegacyBackupInventoryError::Authentication);
    }

    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|_| LegacyBackupInventoryError::InvalidArchive)?;
        if file.is_dir() {
            continue;
        }
        let name = normalize_archive_name(file.name())?;
        if matches!(name.as_str(), "manifest.json" | "encrypted_marker.bin") {
            continue;
        }
        let entry = match document_kind(&name) {
            Some(kind) => Ok(kind),
            None => Err(media_name(&name)?),
        };
        entries.push((index, entry));
    }
    let archive = Arc::new(LegacyArchive {
        zip: Mutex::new(archive),
        key,
        nonce,
    });
    let mut documents = Vec::new();
    let mut document_bytes = 0usize;
    let mut media = Vec::new();
    for (index, entry) in entries {
        let decrypted = archive.entry(index)?;
        match entry {
            Ok(kind) => {
                document_bytes = document_bytes
                    .checked_add(decrypted.len())
                    .filter(|total| *total <= MAX_LEGACY_BACKUP_DOCUMENT_TOTAL_BYTES)
                    .ok_or(LegacyBackupInventoryError::LimitExceeded)?;
                documents.push(LegacyBackupDocument {
                    kind,
                    bytes: decrypted,
                });
            }
            Err((root, relative_segments)) => media.push(LegacyBackupMedia {
                root,
                relative_segments,
                byte_len: decrypted.len() as u64,
                content_hash: content_hash(&decrypted),
                source: LegacyMediaSource::Archive(Arc::clone(&archive), index),
            }),
        }
    }
    documents.sort_by_key(|document| document.kind);
    media.sort_by(|left, right| {
        (&left.root, &left.relative_segments).cmp(&(&right.root, &right.relative_segments))
    });
    Ok(LegacyBackupInventory {
        version: 1,
        created_at: manifest.created_at,
        app_version: manifest.app_version,
        source_hash,
        documents,
        media,
    })
}

fn validate_archive_metadata(
    archive: &mut ZipArchive<Box<dyn crate::BackupSource>>,
) -> Result<(), LegacyBackupInventoryError> {
    let mut names = BTreeSet::new();
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|_| LegacyBackupInventoryError::InvalidArchive)?;
        let name = normalize_archive_name(file.name())?;
        if !names.insert(name.clone()) {
            return Err(LegacyBackupInventoryError::InvalidInventory);
        }
        if file.is_dir() {
            if !known_directory(&name) {
                return Err(LegacyBackupInventoryError::InvalidInventory);
            }
            continue;
        }
        let max_entry = u64::try_from(MAX_LEGACY_BACKUP_ENTRY_BYTES)
            .map_err(|_| LegacyBackupInventoryError::LimitExceeded)?
            .saturating_add(16);
        if file.size() > max_entry {
            return Err(LegacyBackupInventoryError::LimitExceeded);
        }
        total = total
            .checked_add(file.size())
            .filter(|total| *total <= crate::MAX_BACKUP_TOTAL_BYTES)
            .ok_or(LegacyBackupInventoryError::LimitExceeded)?;
    }
    Ok(())
}

fn read_public_entry(
    archive: &mut ZipArchive<Box<dyn crate::BackupSource>>,
    name: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, LegacyBackupInventoryError> {
    let mut file = archive
        .by_name(name)
        .map_err(|_| LegacyBackupInventoryError::InvalidInventory)?;
    if file.is_dir() || file.size() > max_bytes {
        return Err(LegacyBackupInventoryError::LimitExceeded);
    }
    read_bounded(&mut file, max_bytes)
}

fn read_bounded(
    reader: &mut impl Read,
    max_bytes: u64,
) -> Result<Vec<u8>, LegacyBackupInventoryError> {
    let mut bytes = Vec::new();
    reader
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| LegacyBackupInventoryError::InvalidArchive)?;
    if u64::try_from(bytes.len()).map_err(|_| LegacyBackupInventoryError::LimitExceeded)?
        > max_bytes
    {
        return Err(LegacyBackupInventoryError::LimitExceeded);
    }
    Ok(bytes)
}

fn document_kind(name: &str) -> Option<LegacyBackupDocumentKind> {
    let stem = name.strip_prefix("data/")?.strip_suffix(".json.enc")?;
    Some(match stem {
        "meta" => LegacyBackupDocumentKind::Meta,
        "settings" => LegacyBackupDocumentKind::Settings,
        "provider_credentials" => LegacyBackupDocumentKind::ProviderCredentials,
        "models" => LegacyBackupDocumentKind::Models,
        "audio_providers" => LegacyBackupDocumentKind::AudioProviders,
        "user_voices" => LegacyBackupDocumentKind::UserVoices,
        "model_pricing_cache" => LegacyBackupDocumentKind::ModelPricingCache,
        "secrets" => LegacyBackupDocumentKind::Secrets,
        "prompt_templates" => LegacyBackupDocumentKind::PromptTemplates,
        "chat_templates" => LegacyBackupDocumentKind::ChatTemplates,
        "personas" => LegacyBackupDocumentKind::Personas,
        "characters" => LegacyBackupDocumentKind::Characters,
        "companion_scheduled_notes" => LegacyBackupDocumentKind::CompanionScheduledNotes,
        "companion_shared_memory" => LegacyBackupDocumentKind::CompanionSharedMemory,
        "memory_embeddings" => LegacyBackupDocumentKind::MemoryEmbeddings,
        "sessions" => LegacyBackupDocumentKind::Sessions,
        "creation_helper_sessions" => LegacyBackupDocumentKind::CreationHelperSessions,
        "asr_learning" => LegacyBackupDocumentKind::AsrLearning,
        "group_characters" => LegacyBackupDocumentKind::GroupCharacters,
        "group_sessions" => LegacyBackupDocumentKind::GroupSessions,
        "usage_records" => LegacyBackupDocumentKind::UsageRecords,
        "lorebooks" => LegacyBackupDocumentKind::Lorebooks,
        "character_lorebooks" => LegacyBackupDocumentKind::CharacterLorebooks,
        _ => return None,
    })
}

fn media_name(
    name: &str,
) -> Result<(LegacyBackupMediaRoot, Vec<String>), LegacyBackupInventoryError> {
    let plain = name
        .strip_suffix(".enc")
        .ok_or(LegacyBackupInventoryError::InvalidInventory)?;
    let mut segments = plain.split('/');
    let root = match segments.next() {
        Some("images") => LegacyBackupMediaRoot::Images,
        Some("avatars") => LegacyBackupMediaRoot::Avatars,
        Some("attachments") => LegacyBackupMediaRoot::Attachments,
        Some("sessions") => LegacyBackupMediaRoot::Sessions,
        Some("generated_images") => LegacyBackupMediaRoot::GeneratedImages,
        _ => return Err(LegacyBackupInventoryError::InvalidInventory),
    };
    let relative_segments = segments.map(str::to_owned).collect::<Vec<_>>();
    if relative_segments.is_empty() {
        return Err(LegacyBackupInventoryError::InvalidInventory);
    }
    Ok((root, relative_segments))
}

fn safe_archive_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 4096
        && !name.starts_with('/')
        && !name.chars().any(char::is_control)
        && name
            .trim_end_matches('/')
            .split('/')
            .all(|segment| !segment.is_empty() && !matches!(segment, "." | ".."))
}

fn normalize_archive_name(name: &str) -> Result<String, LegacyBackupInventoryError> {
    let normalized = name.replace('\\', "/");
    if safe_archive_name(&normalized) {
        Ok(normalized)
    } else {
        Err(LegacyBackupInventoryError::InvalidInventory)
    }
}

fn known_directory(name: &str) -> bool {
    matches!(
        name.trim_end_matches('/').split('/').next(),
        Some("data" | "images" | "avatars" | "attachments" | "sessions" | "generated_images")
    )
}

fn decode_array<const N: usize>(
    value: Option<&str>,
) -> Result<[u8; N], LegacyBackupInventoryError> {
    let decoded = general_purpose::STANDARD
        .decode(value.ok_or(LegacyBackupInventoryError::InvalidManifest)?)
        .map_err(|_| LegacyBackupInventoryError::InvalidManifest)?;
    decoded
        .try_into()
        .map_err(|_| LegacyBackupInventoryError::InvalidManifest)
}

fn validate_password(password: &str) -> Result<(), LegacyBackupInventoryError> {
    if password.is_empty() || password.len() > 1024 || password.chars().any(char::is_control) {
        return Err(LegacyBackupInventoryError::InvalidPassword);
    }
    Ok(())
}

fn derive_key(password: &str, salt: &[u8; 16]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(password.as_bytes());
    hasher.update(salt);
    hasher.update(b"lettuce_backup_key_v1");
    *hasher.finalize().as_bytes()
}

fn decrypt(
    bytes: &[u8],
    key: &[u8; 32],
    nonce: &[u8; 24],
) -> Result<Vec<u8>, LegacyBackupInventoryError> {
    XChaCha20Poly1305::new(key.into())
        .decrypt(XNonce::from_slice(nonce), bytes)
        .map_err(|_| LegacyBackupInventoryError::Authentication)
}

fn content_hash(bytes: &[u8]) -> ContentHash {
    ContentHash::parse(blake3::hash(bytes).to_hex().to_string())
        .expect("BLAKE3 produces a valid content hash")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    const PASSWORD: &str = "legacy password";
    const SALT: [u8; 16] = [7; 16];
    const NONCE: [u8; 24] = [9; 24];

    fn archive(version: u32, entries: &[(&str, &[u8])]) -> Vec<u8> {
        let key = derive_key(PASSWORD, &SALT);
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, bytes) in entries {
            writer.start_file(*name, options).expect("start entry");
            writer
                .write_all(&encrypt(bytes, &key, &NONCE))
                .expect("write entry");
        }
        writer
            .start_file("encrypted_marker.bin", options)
            .expect("start marker");
        writer
            .write_all(&encrypt(LEGACY_MARKER, &key, &NONCE))
            .expect("write marker");
        writer
            .start_file("manifest.json", options)
            .expect("start manifest");
        writer
            .write_all(
                serde_json::json!({
                    "version": version,
                    "created_at": 123,
                    "app_version": "0.9.0",
                    "encrypted": true,
                    "salt": general_purpose::STANDARD.encode(SALT),
                    "nonce": general_purpose::STANDARD.encode(NONCE),
                })
                .to_string()
                .as_bytes(),
            )
            .expect("write manifest");
        writer.finish().expect("finish archive").into_inner()
    }

    fn encrypt(bytes: &[u8], key: &[u8; 32], nonce: &[u8; 24]) -> Vec<u8> {
        XChaCha20Poly1305::new(key.into())
            .encrypt(XNonce::from_slice(nonce), bytes)
            .expect("encrypt legacy fixture")
    }

    #[test]
    fn legacy_inventory_decrypts_optional_documents_and_media_from_bytes() {
        let bytes = archive(
            LEGACY_MANIFEST_VERSION,
            &[
                ("data/settings.json.enc", br#"{"theme":"dark"}"#),
                ("data/secrets.json.enc", br#"[{"value":"secret-canary"}]"#),
                ("images/shared/avatar.png.enc", b"legacy image"),
                ("avatars\\nested\\face.png.enc", b"legacy avatar"),
            ],
        );
        let inventory = decode(bytes.clone(), PASSWORD).expect("inventory");
        assert_eq!(inventory.version, 1);
        assert_eq!(inventory.created_at, 123);
        assert_eq!(inventory.app_version, "0.9.0");
        assert_eq!(inventory.documents.len(), 2);
        assert_eq!(
            inventory.documents[0].kind,
            LegacyBackupDocumentKind::Settings
        );
        assert_eq!(
            inventory.documents[1].kind,
            LegacyBackupDocumentKind::Secrets
        );
        assert!(!format!("{:?}", inventory.documents[1]).contains("secret-canary"));
        assert_eq!(inventory.media.len(), 2);
        assert_eq!(inventory.media[0].root, LegacyBackupMediaRoot::Images);
        assert_eq!(
            inventory.media[0].relative_segments,
            vec!["shared", "avatar.png"]
        );
        assert_eq!(
            &*inventory.media[0].read().expect("media bytes"),
            b"legacy image"
        );
        assert_eq!(inventory.media[1].root, LegacyBackupMediaRoot::Avatars);
        assert_eq!(
            inventory.media[1].relative_segments,
            vec!["nested", "face.png"]
        );
    }

    fn decode(
        bytes: Vec<u8>,
        password: &str,
    ) -> Result<LegacyBackupInventory, LegacyBackupInventoryError> {
        decode_legacy_backup_inventory(Cursor::new(bytes), password)
    }

    #[test]
    fn legacy_inventory_rejects_wrong_password_plaintext_and_future_versions() {
        let valid = archive(LEGACY_MANIFEST_VERSION, &[]);
        assert_eq!(
            decode(valid, "wrong password"),
            Err(LegacyBackupInventoryError::Authentication)
        );
        assert_eq!(
            decode(
                archive(
                    LEGACY_MANIFEST_VERSION,
                    &[("data/settings.json", b"plaintext")]
                ),
                PASSWORD,
            ),
            Err(LegacyBackupInventoryError::InvalidInventory)
        );
        assert_eq!(
            decode(archive(3, &[]), PASSWORD),
            Err(LegacyBackupInventoryError::InvalidManifest)
        );
    }

    #[test]
    fn legacy_inventory_rejects_unknown_and_traversing_entries() {
        assert_eq!(
            decode(
                archive(
                    LEGACY_MANIFEST_VERSION,
                    &[
                        ("images/shared.png.enc", b"one"),
                        ("images\\shared.png.enc", b"two"),
                    ]
                ),
                PASSWORD,
            ),
            Err(LegacyBackupInventoryError::InvalidInventory)
        );
        assert_eq!(
            decode(
                archive(LEGACY_MANIFEST_VERSION, &[("data/unknown.json.enc", b"{}")]),
                PASSWORD,
            ),
            Err(LegacyBackupInventoryError::InvalidInventory)
        );
        assert_eq!(
            decode(
                archive(
                    LEGACY_MANIFEST_VERSION,
                    &[("images/../secrets.txt.enc", b"secret")]
                ),
                PASSWORD,
            ),
            Err(LegacyBackupInventoryError::InvalidInventory)
        );
    }
}
