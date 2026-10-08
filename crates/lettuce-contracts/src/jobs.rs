use serde::{Deserialize, Serialize};

use crate::AssetRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum JobKindDto {
    ArtifactInstall,
    ArtifactVerify,
    RuntimePrepare,
    ModelLoad,
    MemoryExtraction,
    MemoryConsolidation,
    CompanionGrowth,
    CompanionConsolidation,
    CompanionSoulWriter,
    ConversationGeneration,
    VectorIndexBuild,
    CreationRun,
    ImageGenerate,
    MediaTransform,
    TransferImport,
    TransferExport,
    BackupExport,
    BackupRestore,
    SyncSession,
    SpeechTranscribe,
    SpeechSynthesize,
    SpeechVoiceCreate,
    EmbeddingBenchmark,
    Maintenance,
    ModelPull,
    ModelsFolderMove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum JobStateDto {
    Queued,
    Claimed,
    Running,
    CancellationRequested,
    CleaningUp,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum JobSubjectKindDto {
    Conversation,
    Group,
    MemorySpace,
    CreationProject,
    ArtifactInstall,
    ImageRequest,
    TransferPlan,
    Backup,
    Peer,
    SpeechRequest,
    Runtime,
    ModelProfile,
    Maintenance,
    ProviderModel,
}

/// What a job works on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct JobSubjectDto {
    pub kind: JobSubjectKindDto,
    pub id: String,
}

/// What the download center shows for a model job: the repository file a
/// download installs, the model an Ollama server pulls, the folders a move
/// goes between.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum JobSubjectDetail {
    ModelDownload {
        repo: String,
        file: String,
        display_name: String,
    },
    ModelPull {
        provider_account_id: String,
        model: String,
    },
    ModelsFolderMove {
        from: String,
        to: String,
    },
    ImageBundle {
        bundle_id: String,
        display_name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum JobProgressUnit {
    Bytes,
    Items,
    Permille,
}

/// The job's progress within its current stage, in bytes, items or
/// thousandths; `label_code` names the stage (such as `download`, `verify`
/// or `install`) for the frontend to localize.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct JobProgressDto {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub current: u64,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub total: Option<u64>,
    pub unit: Option<JobProgressUnit>,
    pub label_code: Option<String>,
    /// Download speed, sent with `JobUpdated` and watch events while bytes
    /// arrive.
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub bytes_per_second: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum JobFailureCode {
    Cancelled,
    InvalidInput,
    Authentication,
    CapabilityUnavailable,
    IntegrityFailure,
    ResourceUnavailable,
    LeaseLost,
    WorkerFailed,
    StorageFailure,
    SafetyRefusal,
    TimedOut,
    Unknown,
}

/// Why a chat feature job failed, where the user can act on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum JobFailureReason {
    HelpMeReplyDisabled,
    HelpMeReplyNoHistory,
    HelpMeReplyNoModel,
    HelpMeReplyNoReply,
    ScenePromptDisabled,
    ScenePromptNoModel,
    ScenePromptNoReply,
    SceneImageDisabled,
    SceneImageNoModel,
    SceneImageNoImage,
    DesignReferenceNoModel,
    DesignReferenceNoImages,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct JobFailureDto {
    pub code: JobFailureCode,
    pub retryable: bool,
    /// What a chat feature job needs the user to change.
    pub reason: Option<JobFailureReason>,
    /// The optional model whose absence failed the job.
    pub model: Option<crate::RequiredModel>,
    /// Why Hugging Face refused a download.
    pub hugging_face: Option<crate::HfFailure>,
    /// Why an Ollama pull failed.
    pub ollama: Option<crate::OllamaFailure>,
    /// Why an image job failed.
    pub image: Option<crate::ImageFailure>,
    /// Why a speech job failed.
    pub speech: Option<crate::SpeechFailure>,
}

/// What a finished job produced, where the job kind has a typed result.
/// `ModelInstalled` names a downloaded model's path and, when the download
/// asked for one, the llama.cpp model it became.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum JobResultDto {
    VoiceCreated {
        voice_id: String,
    },
    ArtifactInstalled,
    Asset {
        asset: AssetRef,
    },
    Transcription {
        transcription: crate::TranscriptionView,
    },
    GenerationTurn {
        turn_id: String,
    },
    Conversation {
        conversation_id: String,
    },
    Group {
        group_id: String,
    },
    Character {
        character_id: String,
    },
    ModelProfile {
        model_profile_id: String,
    },
    ModelInstalled {
        model_path: String,
        model_profile_id: Option<String>,
    },
    ModelPulled {
        model: String,
    },
    ModelsFolderMoved {
        path: String,
        moved_entries: u32,
        rewired_models: u32,
    },
    /// The Soul draft a Soul writer job produced, which the caller merges
    /// into its unsaved draft.
    CompanionSoulDraft {
        draft: Box<crate::CompanionSoulDraft>,
    },
    /// The text a help-me-reply or scene prompt job wrote, cleaned.
    GeneratedText {
        text: String,
    },
    /// The entry a lorebook entry draft job wrote; nothing is saved.
    LorebookEntryDraft {
        draft: crate::LorebookEntryDraftResult,
    },
    /// The lorebook entry draft job found nothing worth an entry.
    LorebookNoEntry {
        reason: Option<String>,
    },
    /// The keywords a keyword draft job proposed.
    LorebookKeywords {
        keywords: Vec<String>,
    },
    /// The lorebook project a planner, writer or coherence job advanced.
    LorebookProject {
        project_id: String,
    },
    /// The images an image generation job stored.
    ImageGeneration {
        images: Vec<crate::GeneratedImage>,
        rejected_outputs: u32,
    },
    ImageUpscaled {
        upscaled: crate::ImageUpscaled,
    },
    LoraDiscovered {
        discovered: crate::LoraDiscovered,
    },
    Runnability {
        verdict: Box<crate::SdRunnability>,
    },
    /// What an image bundle install ended in.
    ImageBundle {
        bundle_id: String,
        state: crate::ImageBundleState,
        model_id: Option<String>,
        setup_error: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct JobView {
    pub id: String,
    pub kind: JobKindDto,
    pub subject: JobSubjectDto,
    pub subject_detail: Option<JobSubjectDetail>,
    pub state: JobStateDto,
    pub progress: JobProgressDto,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
    pub failure: Option<JobFailureDto>,
    pub result: Option<JobResultDto>,
}

/// Jobs, most recently created first. An empty or missing `kinds` or
/// `states` matches every value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct JobsListRequest {
    pub kinds: Option<Vec<JobKindDto>>,
    pub states: Option<Vec<JobStateDto>>,
    pub subject: Option<JobSubjectDto>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct JobPage {
    pub items: Vec<JobView>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct JobGetRequest {
    pub job_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct JobCancelRequest {
    pub job_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct JobWatchRequest {
    pub job_id: String,
}

/// Returned by every command that starts background work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct JobAccepted {
    pub job_id: String,
}

/// The stream `job_watch` attaches. It starts with the job's current state;
/// `Completed`, `Failed` and `Cancelled` are the last event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum JobEvent {
    ModelLoading {
        stage: crate::ModelLoadStage,
        status: crate::ModelLoadStatus,
        percent: u8,
        model_name: String,
        gpus: Option<Vec<crate::ModelLoadGpuProgress>>,
    },
    Notice {
        code: crate::RuntimeNoticeCode,
    },
    Throughput {
        #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
        tokens: u64,
        tokens_per_second: f64,
    },
    Progress {
        job: JobView,
    },
    TextDelta {
        text: Option<String>,
        reasoning: Option<String>,
    },
    /// A running local image generation's progress.
    ImageProgress {
        progress: crate::ImageProgress,
    },
    Completed {
        job: JobView,
    },
    Failed {
        job: JobView,
    },
    Cancelled {
        job: JobView,
    },
}
