use std::{fmt, str::FromStr, time::Duration};

use lettuce_types::{
    AssetId, CharacterId, ConversationId, GenerationCandidateId, GenerationTurnId, GroupId, JobId,
    MediaBlobId, MemoryId, ModelProfileId, OperationId, RequestId, TimestampMillis,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Timestamp = TimestampMillis;

const MAX_SAFE_LABEL_CHARS: usize = 128;
const MAX_SAFE_ID_CHARS: usize = 128;

fn validate_safe_text(value: &str, max: usize, allow_spaces: bool) -> Result<(), SafeTextError> {
    if value.is_empty() {
        return Err(SafeTextError::Empty);
    }
    if value.chars().count() > max {
        return Err(SafeTextError::TooLong);
    }
    if value.chars().any(|character| {
        character.is_control()
            || (!allow_spaces && character.is_whitespace())
            || matches!(character, '/' | '\\' | '\0' | ':' | '?')
    }) {
        return Err(SafeTextError::UnsafeCharacters);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SafeTextError {
    #[error("value must not be empty")]
    Empty,
    #[error("value is too long")]
    TooLong,
    #[error("value contains unsafe characters")]
    UnsafeCharacters,
}

/// Bounded caller-attested text for events or IPC DTOs; it is not intrinsically
/// secret-safe. Adapters should prefer machine-facing codes or i18n keys.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SafeLabel(String);

impl fmt::Debug for SafeLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SafeLabel")
            .field("length", &self.0.chars().count())
            .finish()
    }
}

impl SafeLabel {
    pub fn new(value: impl Into<String>) -> Result<Self, SafeTextError> {
        let value = value.into();
        validate_safe_text(&value, MAX_SAFE_LABEL_CHARS, true)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SafeLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for SafeLabel {
    type Err = SafeTextError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for SafeLabel {
    type Error = SafeTextError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SafeLabel> for String {
    fn from(value: SafeLabel) -> Self {
        value.0
    }
}

/// A bounded identifier used by domain references. It is not a path or a
/// serialized payload.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SubjectId(String);

impl fmt::Debug for SubjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SubjectId(<redacted>)")
    }
}

impl SubjectId {
    pub fn new(value: impl Into<String>) -> Result<Self, SafeTextError> {
        let value = value.into();
        validate_safe_text(&value, MAX_SAFE_ID_CHARS, false)?;
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(SafeTextError::UnsafeCharacters);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SubjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for SubjectId {
    type Err = SafeTextError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for SubjectId {
    type Error = SafeTextError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SubjectId> for String {
    fn from(value: SubjectId) -> Self {
        value.0
    }
}

/// Caller-selected key used to coalesce equivalent submissions.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IdempotencyKey(String);

impl fmt::Debug for IdempotencyKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("IdempotencyKey(<redacted>)")
    }
}

impl IdempotencyKey {
    pub fn new(value: impl Into<String>) -> Result<Self, SafeTextError> {
        let value = value.into();
        validate_safe_text(&value, MAX_SAFE_ID_CHARS, false)?;
        if !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'@' | b'+')
        }) {
            return Err(SafeTextError::UnsafeCharacters);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IdempotencyKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for IdempotencyKey {
    type Err = SafeTextError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for IdempotencyKey {
    type Error = SafeTextError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<IdempotencyKey> for String {
    fn from(value: IdempotencyKey) -> Self {
        value.0
    }
}

macro_rules! opaque_uuid {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            #[must_use]
            pub const fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }
    };
}

opaque_uuid!(LeaseId);
opaque_uuid!(WorkerId);
opaque_uuid!(CorrelationId);
opaque_uuid!(OutcomeId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventSeq(u64);

impl EventSeq {
    pub const FIRST: Self = Self(1);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Result<Self, SequenceOverflow> {
        self.0.checked_add(1).map(Self).ok_or(SequenceOverflow)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("event sequence overflow")]
pub struct SequenceOverflow;

impl Default for EventSeq {
    fn default() -> Self {
        Self::FIRST
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttemptNo(u32);

impl AttemptNo {
    pub const QUEUED: Self = Self(0);

    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    pub fn next(self) -> Result<Self, AttemptOverflow> {
        self.0.checked_add(1).map(Self).ok_or(AttemptOverflow)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("attempt number overflow")]
pub struct AttemptOverflow;

impl Default for AttemptNo {
    fn default() -> Self {
        Self::QUEUED
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    ArtifactInstall,
    ArtifactVerify,
    RuntimePrepare,
    ModelLoad,
    MemoryExtraction,
    MemoryConsolidation,
    CompanionGrowth,
    CompanionConsolidation,
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
    EmbeddingBenchmark,
    Maintenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobSubject {
    pub kind: SubjectKind,
    pub id: SubjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<SafeLabel>,
}

impl JobSubject {
    pub fn new(kind: SubjectKind, id: impl Into<String>) -> Result<Self, SafeTextError> {
        Ok(Self {
            kind,
            id: SubjectId::new(id)?,
            display: None,
        })
    }

    pub fn with_display(mut self, display: impl Into<String>) -> Result<Self, SafeTextError> {
        self.display = Some(SafeLabel::new(display)?);
        Ok(self)
    }

    #[must_use]
    pub fn from_uuid(kind: SubjectKind, id: Uuid) -> Self {
        Self {
            kind,
            id: SubjectId(id.to_string()),
            display: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceClass {
    Network,
    DiskRead,
    DiskWrite,
    Cpu,
    Gpu,
    ModelLoad,
    Process,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryPolicy {
    Resume,
    Restart,
    Compensate,
    MarkInterrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationPolicy {
    Cooperative,
    UntilIrreversibleStage,
    NotCancellable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum JobPriority {
    Background,
    #[default]
    Normal,
    Interactive,
    Recovery,
    Cleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
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

impl JobState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }

    pub fn can_transition_to(self, next: Self) -> Result<(), TransitionError> {
        let legal = match self {
            Self::Queued => matches!(
                next,
                Self::Claimed | Self::CancellationRequested | Self::Interrupted
            ),
            Self::Claimed => matches!(
                next,
                Self::Running | Self::CancellationRequested | Self::Queued | Self::Interrupted
            ),
            Self::Running => matches!(
                next,
                Self::CancellationRequested
                    | Self::CleaningUp
                    | Self::Succeeded
                    | Self::Failed
                    | Self::Queued
                    | Self::Interrupted
            ),
            Self::CancellationRequested => matches!(next, Self::CleaningUp | Self::Interrupted),
            Self::CleaningUp => matches!(next, Self::Cancelled | Self::Failed | Self::Interrupted),
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Interrupted => false,
        };
        legal.then_some(()).ok_or(TransitionError {
            from: self,
            to: next,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("illegal job transition from {from:?} to {to:?}")]
pub struct TransitionError {
    pub from: JobState,
    pub to: JobState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct FiniteFraction(f64);

impl FiniteFraction {
    pub fn new(value: f64) -> Result<Self, FractionError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(FractionError);
        }
        Ok(Self(value))
    }

    pub fn from_ratio(completed: u64, total: u64) -> Result<Self, FractionError> {
        if total == 0 || completed > total {
            return Err(FractionError);
        }
        Self::new(completed as f64 / total as f64)
    }

    #[must_use]
    pub const fn value(&self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for FiniteFraction {
    type Error = FractionError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<FiniteFraction> for f64 {
    fn from(value: FiniteFraction) -> Self {
        value.0
    }
}

impl Eq for FiniteFraction {}

impl PartialOrd for FiniteFraction {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("fraction must be finite and between zero and one")]
pub struct FractionError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitsProgress {
    pub completed: u64,
    pub total: Option<u64>,
}

impl UnitsProgress {
    pub fn new(completed: u64, total: Option<u64>) -> Result<Self, ProgressError> {
        if total.is_some_and(|total| completed > total) {
            return Err(ProgressError::ExceedsTotal);
        }
        Ok(Self { completed, total })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BytesProgress {
    pub completed: u64,
    pub total: Option<u64>,
}

impl BytesProgress {
    pub fn new(completed: u64, total: Option<u64>) -> Result<Self, ProgressError> {
        if total.is_some_and(|total| completed > total) {
            return Err(ProgressError::ExceedsTotal);
        }
        Ok(Self { completed, total })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProgressSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<UnitsProgress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<BytesProgress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fraction: Option<FiniteFraction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<SafeLabel>,
}

impl ProgressSnapshot {
    pub fn validate(&self) -> Result<(), ProgressError> {
        if self
            .units
            .as_ref()
            .is_some_and(|value| value.total.is_some_and(|total| value.completed > total))
            || self
                .bytes
                .as_ref()
                .is_some_and(|value| value.total.is_some_and(|total| value.completed > total))
        {
            return Err(ProgressError::ExceedsTotal);
        }
        Ok(())
    }

    pub fn is_monotonic_from(&self, previous: &Self) -> bool {
        let units_ok = match (&previous.units, &self.units) {
            (None, None) => true,
            (Some(_), None) => false,
            (Some(old), Some(new)) => new.completed >= old.completed,
            (None, Some(_)) => true,
        };
        let bytes_ok = match (&previous.bytes, &self.bytes) {
            (None, None) => true,
            (Some(_), None) => false,
            (Some(old), Some(new)) => new.completed >= old.completed,
            (None, Some(_)) => true,
        };
        let fraction_ok = match (&previous.fraction, &self.fraction) {
            (None, None) => true,
            (Some(_), None) => false,
            (Some(old), Some(new)) => new >= old,
            (None, Some(_)) => true,
        };
        units_ok && bytes_ok && fraction_ok
    }

    #[must_use]
    pub fn preserving_omitted_from(&self, previous: &Self) -> Self {
        Self {
            units: self.units.clone().or_else(|| previous.units.clone()),
            bytes: self.bytes.clone().or_else(|| previous.bytes.clone()),
            fraction: self.fraction.clone().or_else(|| previous.fraction.clone()),
            message: self.message.clone().or_else(|| previous.message.clone()),
        }
    }

    pub fn has_compatible_totals_with(&self, previous: &Self) -> bool {
        let units_ok = match (&previous.units, &self.units) {
            (Some(old), Some(new)) => match (old.total, new.total) {
                (Some(old_total), Some(new_total)) => old_total == new_total,
                (Some(_), None) => false,
                (None, _) => true,
            },
            _ => true,
        };
        let bytes_ok = match (&previous.bytes, &self.bytes) {
            (Some(old), Some(new)) => match (old.total, new.total) {
                (Some(old_total), Some(new_total)) => old_total == new_total,
                (Some(_), None) => false,
                (None, _) => true,
            },
            _ => true,
        };
        units_ok && bytes_ok
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProgressError {
    #[error("progress exceeds its declared total")]
    ExceedsTotal,
    #[error("progress regressed within a stage")]
    Regression,
    #[error("progress updates must use an explicit stage change to reset counters")]
    StageResetRequired,
    #[error("progress total changed within a stage")]
    TotalChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageSnapshot {
    pub name: SafeLabel,
    pub irreversible: bool,
}

impl StageSnapshot {
    pub fn new(name: impl Into<String>, irreversible: bool) -> Result<Self, SafeTextError> {
        Ok(Self {
            name: SafeLabel::new(name)?,
            irreversible,
        })
    }
}

impl Default for StageSnapshot {
    fn default() -> Self {
        Self::new("queued", false).expect("constant stage label is safe")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationReason {
    User,
    Shutdown,
    Parent,
    Timeout,
    Recovery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CancellationView {
    pub requested: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<CancellationReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_at: Option<Timestamp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobErrorCode {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobError {
    pub code: JobErrorCode,
    pub retryable: bool,
    pub message: SafeLabel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

impl JobError {
    pub fn new(
        code: JobErrorCode,
        retryable: bool,
        message: impl Into<String>,
    ) -> Result<Self, SafeTextError> {
        Ok(Self {
            code,
            retryable,
            message: SafeLabel::new(message)?,
            retry_after_ms: None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarningCode {
    PartialOutput,
    RecoveryRequired,
    OptionalChildFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutcomeRef {
    CreatedProposal(GenerationCandidateId),
    ArtifactInstallation(AssetId),
    GeneratedAssetSet(AssetId),
    TransferReport(OperationId),
    SyncReport(OperationId),
    SpeechAsset(MediaBlobId),
    MemoryRun(MemoryId),
    GenerationTurn(GenerationTurnId),
    Checkpoint(OutcomeId),
    Request(RequestId),
    Conversation(ConversationId),
    Group(GroupId),
    Character(CharacterId),
    ModelProfile(ModelProfileId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobOutcome {
    Success {
        result_ref: OutcomeRef,
    },
    Partial {
        result_ref: OutcomeRef,
        warnings: Vec<WarningCode>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobSpec {
    pub kind: JobKind,
    pub subject: JobSubject,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<IdempotencyKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<JobId>,
    pub correlation_id: CorrelationId,
    pub input_ref: OutcomeRef,
    pub priority: JobPriority,
    pub recovery_policy: RecoveryPolicy,
    pub cancellation_policy: CancellationPolicy,
    pub resources: Vec<ResourceClass>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SpecError {
    #[error("a job must declare at least one resource class")]
    EmptyResources,
    #[error("a job cannot declare the same resource class more than once")]
    DuplicateResource,
}

/// Alias used by the store contract and application adapters.
pub type NewJob = JobSpec;

impl JobSpec {
    #[must_use]
    pub fn new(kind: JobKind, subject: JobSubject, input_ref: OutcomeRef) -> Self {
        Self {
            kind,
            subject,
            idempotency_key: None,
            parent_id: None,
            correlation_id: CorrelationId::new(),
            input_ref,
            priority: JobPriority::Normal,
            recovery_policy: RecoveryPolicy::Restart,
            cancellation_policy: CancellationPolicy::Cooperative,
            resources: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_idempotency_key(mut self, key: IdempotencyKey) -> Self {
        self.idempotency_key = Some(key);
        self
    }

    #[must_use]
    pub fn with_resources(mut self, resources: Vec<ResourceClass>) -> Self {
        self.resources = resources;
        self
    }

    pub fn validate(&self) -> Result<(), SpecError> {
        if self.resources.is_empty() {
            return Err(SpecError::EmptyResources);
        }
        for (index, resource) in self.resources.iter().enumerate() {
            if self.resources[index + 1..].contains(resource) {
                return Err(SpecError::DuplicateResource);
            }
        }
        Ok(())
    }

    #[must_use]
    pub const fn with_parent(mut self, parent_id: JobId) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    #[must_use]
    pub const fn with_priority(mut self, priority: JobPriority) -> Self {
        self.priority = priority;
        self
    }

    #[must_use]
    pub const fn with_policies(
        mut self,
        recovery_policy: RecoveryPolicy,
        cancellation_policy: CancellationPolicy,
    ) -> Self {
        self.recovery_policy = recovery_policy;
        self.cancellation_policy = cancellation_policy;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimRef {
    pub job_id: JobId,
    pub worker_id: WorkerId,
    pub attempt: AttemptNo,
    pub lease_id: LeaseId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobSnapshot {
    pub id: JobId,
    pub kind: JobKind,
    pub subject: JobSubject,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<IdempotencyKey>,
    pub state: JobState,
    pub stage: StageSnapshot,
    pub progress: ProgressSnapshot,
    pub attempt: AttemptNo,
    pub recovery_policy: RecoveryPolicy,
    pub cancellation_policy: CancellationPolicy,
    pub resources: Vec<ResourceClass>,
    pub cancellation: CancellationView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<JobOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JobError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<JobId>,
    pub children: Vec<ChildLink>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<ClaimRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_expires_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_ref: Option<OutcomeRef>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildLink {
    pub child_id: JobId,
    pub required: bool,
}

impl JobSnapshot {
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub claim: ClaimRef,
    pub lease_expires_at: Timestamp,
    pub input_ref: OutcomeRef,
    pub recovery_policy: RecoveryPolicy,
    pub cancellation_policy: CancellationPolicy,
    pub resources: Vec<ResourceClass>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceAvailability {
    pub network: bool,
    pub disk_read: bool,
    pub disk_write: bool,
    pub cpu: bool,
    pub gpu: bool,
    pub model_load: bool,
    pub process: bool,
}

impl ResourceAvailability {
    #[must_use]
    pub const fn all() -> Self {
        Self {
            network: true,
            disk_read: true,
            disk_write: true,
            cpu: true,
            gpu: true,
            model_load: true,
            process: true,
        }
    }

    #[must_use]
    pub const fn none() -> Self {
        Self {
            network: false,
            disk_read: false,
            disk_write: false,
            cpu: false,
            gpu: false,
            model_load: false,
            process: false,
        }
    }

    #[must_use]
    pub const fn allows(self, class: ResourceClass) -> bool {
        match class {
            ResourceClass::Network => self.network,
            ResourceClass::DiskRead => self.disk_read,
            ResourceClass::DiskWrite => self.disk_write,
            ResourceClass::Cpu => self.cpu,
            ResourceClass::Gpu => self.gpu,
            ResourceClass::ModelLoad => self.model_load,
            ResourceClass::Process => self.process,
        }
    }
}

pub fn timestamp_after(
    now: Timestamp,
    duration: Duration,
) -> Result<Timestamp, TimeArithmeticError> {
    let millis = i64::try_from(duration.as_millis()).map_err(|_| TimeArithmeticError)?;
    now.get()
        .checked_add(millis)
        .map(Timestamp::new)
        .ok_or(TimeArithmeticError)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("timestamp arithmetic overflow")]
pub struct TimeArithmeticError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_refs_are_rejected() {
        assert!(SubjectId::new("/tmp/secret").is_err());
        assert!(IdempotencyKey::new("prompt/path").is_err());
        assert!(SafeLabel::new("line\nvalue").is_err());
    }

    #[test]
    fn progress_requires_explicit_stage_reset() {
        let old = ProgressSnapshot {
            units: Some(UnitsProgress::new(9, Some(10)).expect("valid")),
            ..ProgressSnapshot::default()
        };
        let new = ProgressSnapshot {
            units: Some(UnitsProgress::new(1, Some(2)).expect("valid")),
            ..ProgressSnapshot::default()
        };
        assert!(!new.is_monotonic_from(&old));
    }

    #[test]
    fn fraction_rejects_nan_and_out_of_range() {
        assert!(FiniteFraction::new(f64::NAN).is_err());
        assert!(FiniteFraction::new(1.1).is_err());
        assert!(FiniteFraction::new(0.5).is_ok());
    }

    #[test]
    fn terminal_states_are_immutable() {
        for state in [
            JobState::Succeeded,
            JobState::Failed,
            JobState::Cancelled,
            JobState::Interrupted,
        ] {
            assert!(state.can_transition_to(JobState::Running).is_err());
        }
    }

    #[test]
    fn transition_matrix_is_exhaustive() {
        let states = [
            JobState::Queued,
            JobState::Claimed,
            JobState::Running,
            JobState::CancellationRequested,
            JobState::CleaningUp,
            JobState::Succeeded,
            JobState::Failed,
            JobState::Cancelled,
            JobState::Interrupted,
        ];
        let legal = [
            (JobState::Queued, JobState::Claimed),
            (JobState::Queued, JobState::CancellationRequested),
            (JobState::Queued, JobState::Interrupted),
            (JobState::Claimed, JobState::Running),
            (JobState::Claimed, JobState::CancellationRequested),
            (JobState::Claimed, JobState::Queued),
            (JobState::Claimed, JobState::Interrupted),
            (JobState::Running, JobState::CancellationRequested),
            (JobState::Running, JobState::CleaningUp),
            (JobState::Running, JobState::Succeeded),
            (JobState::Running, JobState::Failed),
            (JobState::Running, JobState::Queued),
            (JobState::Running, JobState::Interrupted),
            (JobState::CancellationRequested, JobState::CleaningUp),
            (JobState::CancellationRequested, JobState::Interrupted),
            (JobState::CleaningUp, JobState::Cancelled),
            (JobState::CleaningUp, JobState::Failed),
            (JobState::CleaningUp, JobState::Interrupted),
        ];
        for from in states {
            for to in states {
                assert_eq!(
                    from.can_transition_to(to).is_ok(),
                    legal.contains(&(from, to))
                );
            }
        }
    }

    #[test]
    fn serde_cannot_bypass_validation() {
        assert!(serde_json::from_str::<SubjectId>(r#""../secret""#).is_err());
        assert!(serde_json::from_str::<IdempotencyKey>(r#""prompt/path""#).is_err());
        assert!(serde_json::from_str::<SafeLabel>(r#""line\nvalue""#).is_err());
        assert!(serde_json::from_str::<FiniteFraction>("2.0").is_err());
    }

    #[test]
    fn caller_attested_text_is_redacted_in_debug_output() {
        let label = SafeLabel::new("secret-prompt").expect("bounded text");
        let subject = SubjectId::new("secret-id").expect("bounded id");
        let key = IdempotencyKey::new("secret-key").expect("bounded key");
        assert!(!format!("{label:?}").contains("secret-prompt"));
        assert!(!format!("{subject:?}").contains("secret-id"));
        assert!(!format!("{key:?}").contains("secret-key"));
    }
}
