use lettuce_types::{ConversationId, GenerationTurnId, JobId, OperationId, RequestId};
use tracing::Span;

/// Stable field name used for the logical operation name.
pub const OPERATION_FIELD: &str = "operation";
/// Stable field name used for the operation identifier.
pub const OPERATION_ID_FIELD: &str = "operation_id";
/// Stable field name used for a request identifier.
pub const REQUEST_ID_FIELD: &str = "request_id";
/// Stable field name used for a durable job identifier.
pub const JOB_ID_FIELD: &str = "job_id";
/// Stable field name used for a conversation identifier.
pub const CONVERSATION_ID_FIELD: &str = "conversation_id";
/// Stable field name used for a generation-turn identifier.
pub const GENERATION_TURN_ID_FIELD: &str = "generation_turn_id";

/// Typed correlation carried by an operation and its optional work context.
///
/// Correlation fields are identifiers only. Prompts, messages, provider
/// payloads and other user-controlled values do not belong in this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorrelationContext {
    operation_id: OperationId,
    request_id: Option<RequestId>,
    job_id: Option<JobId>,
    conversation_id: Option<ConversationId>,
    generation_turn_id: Option<GenerationTurnId>,
}

impl CorrelationContext {
    /// Creates a correlation context for a new operation.
    #[must_use]
    pub const fn new(operation_id: OperationId) -> Self {
        Self {
            operation_id,
            request_id: None,
            job_id: None,
            conversation_id: None,
            generation_turn_id: None,
        }
    }

    /// Creates a context with a freshly generated operation identifier.
    #[must_use]
    pub fn generated() -> Self {
        Self::new(OperationId::new())
    }

    #[must_use]
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    #[must_use]
    pub const fn request_id(&self) -> Option<RequestId> {
        self.request_id
    }

    #[must_use]
    pub const fn job_id(&self) -> Option<JobId> {
        self.job_id
    }

    #[must_use]
    pub const fn conversation_id(&self) -> Option<ConversationId> {
        self.conversation_id
    }

    #[must_use]
    pub const fn generation_turn_id(&self) -> Option<GenerationTurnId> {
        self.generation_turn_id
    }

    #[must_use]
    pub const fn with_request_id(mut self, request_id: RequestId) -> Self {
        self.request_id = Some(request_id);
        self
    }

    #[must_use]
    pub const fn with_job_id(mut self, job_id: JobId) -> Self {
        self.job_id = Some(job_id);
        self
    }

    #[must_use]
    pub const fn with_conversation_id(mut self, conversation_id: ConversationId) -> Self {
        self.conversation_id = Some(conversation_id);
        self
    }

    #[must_use]
    pub const fn with_generation_turn_id(mut self, generation_turn_id: GenerationTurnId) -> Self {
        self.generation_turn_id = Some(generation_turn_id);
        self
    }

    /// Creates the parent span for this operation.
    ///
    /// Only the operation name and typed identifiers are recorded. The span
    /// becomes a child of the currently entered span when one exists.
    #[must_use]
    pub fn parent_span(&self, operation: &'static str) -> Span {
        tracing::info_span!(
            "lettuce.operation",
            operation = %operation,
            operation_id = %self.operation_id,
            request_id = ?self.request_id,
            job_id = ?self.job_id,
            conversation_id = ?self.conversation_id,
            generation_turn_id = ?self.generation_turn_id,
        )
    }

    /// Alias for [`Self::parent_span`].
    #[must_use]
    pub fn span(&self, operation: &'static str) -> Span {
        self.parent_span(operation)
    }
}

impl Default for CorrelationContext {
    fn default() -> Self {
        Self::generated()
    }
}

/// Short name for [`CorrelationContext`].
pub type Correlation = CorrelationContext;
