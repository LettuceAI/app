use serde::{Deserialize, Serialize};

use crate::AssetRef;

/// A file the user picked: a filesystem path, or a platform URI the host
/// resolves (such as an Android `content://` URI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct FileSource {
    pub uri: String,
}

/// Where an export is written, as chosen by the save dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct FileTarget {
    pub uri: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    CharacterCard,
    PersonaFile,
    Lorebook,
    PromptPreset,
    ChatJsonl,
    BackupV1,
    BackupV2,
    LegacyDatabase,
    Image,
    Audio,
    GgufModel,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct FilesInspectRequest {
    pub source: FileSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct FileInspection {
    pub name: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size: u64,
    pub kind: FileKind,
}

/// What a picked image or audio file is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum AssetIngestRole {
    Avatar,
    Background,
    Attachment,
    VoiceExample,
    ReferenceImage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AssetsIngestRequest {
    pub source: FileSource,
    pub role: AssetIngestRole,
}

/// What a file picker offers, which sets its extension and type filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum FilePickKind {
    Any,
    Image,
    Audio,
    Json,
    CharacterCard,
    ChatLog,
    Backup,
    GgufModel,
    DiffusionModel,
    Certificate,
    Document,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct FilesPickOpenRequest {
    pub kinds: Vec<FilePickKind>,
    pub multiple: bool,
}

/// The picked files; empty when the user cancelled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct FilesPicked {
    pub sources: Vec<FileSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct FilesPickSaveRequest {
    pub suggested_name: String,
    pub kind: FilePickKind,
}

/// The chosen destination; none when the user cancelled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct FileSavePicked {
    pub target: Option<FileTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MediaSaveToRequest {
    pub asset_id: String,
    pub target: FileTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MediaLibraryRemoveRequest {
    pub asset_id: String,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MediaLibraryRole {
    Image,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MediaLibraryRetention {
    Persistent,
    Library,
    Temporary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MediaLibraryAssetKind {
    AvatarOriginal,
    BackgroundImage,
    Illustration,
    LorebookIcon,
    MessageImage,
    MessageAudio,
    GeneratedImage,
    SynthesizedSpeech,
    OtherImage,
    OtherAudio,
    SourceDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MediaLibraryListRequest {
    pub kind: Option<MediaLibraryRole>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MediaLibraryItem {
    pub asset: AssetRef,
    pub role: MediaLibraryRole,
    pub asset_kind: MediaLibraryAssetKind,
    pub retention: MediaLibraryRetention,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub expires_at: Option<i64>,
    pub mime_type: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub byte_size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub duration_ms: Option<u64>,
    pub source_label: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
    pub references: Vec<MediaReferenceView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MediaLibraryPage {
    pub items: Vec<MediaLibraryItem>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MediaReferenceKind {
    Character,
    Persona,
    Group,
    Scene,
    Conversation,
    Creation,
    Speech,
    Image,
    Memory,
    Companion,
    Model,
    Settings,
    Lorebook,
    Prompt,
    Job,
    LegacyImport,
    Sync,
    Usage,
    Transfer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MediaReferenceView {
    pub kind: MediaReferenceKind,
    pub owner_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MediaFailureReason {
    HostUnavailable,
    AssetMissing,
    BlobMissing,
    ObjectMissing,
    NotReady,
    InvalidMetadata,
    ProtectedTarget,
    Storage,
}
