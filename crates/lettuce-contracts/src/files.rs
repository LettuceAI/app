use serde::{Deserialize, Serialize};

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
