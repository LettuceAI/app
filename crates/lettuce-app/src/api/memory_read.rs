use std::collections::HashMap;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{ConversationKind, ConversationReader};
use lettuce_jobs::handle::CancellationToken;
use lettuce_jobs::{JobCatalog, JobKind, JobListFilter, JobSnapshot, JobState};
use lettuce_memory::{
    MemoryOrigin, MemoryReadRepository, MemoryReadScope, MemoryRepository, MemoryToolOutcome,
};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{PageLimit, PageRequest};

use super::error::{IntoApiError, api_error, parse_id};
use super::{ApiContext, memory::memory_error};

fn origin(value: MemoryOrigin) -> dto::MemoryOrigin {
    match value {
        MemoryOrigin::User => dto::MemoryOrigin::User,
        MemoryOrigin::Model => dto::MemoryOrigin::Model,
        MemoryOrigin::Import => dto::MemoryOrigin::Import,
    }
}

fn category(value: lettuce_memory::MemoryCategory) -> dto::MemoryCategory {
    use dto::MemoryCategory as D;
    use lettuce_memory::MemoryCategory as M;
    match value {
        M::CharacterTrait => D::CharacterTrait,
        M::Relationship => D::Relationship,
        M::PlotEvent => D::PlotEvent,
        M::WorldDetail => D::WorldDetail,
        M::Preference => D::Preference,
        M::Other => D::Other,
        M::Milestone => D::Milestone,
        M::Boundary => D::Boundary,
        M::Profile => D::Profile,
        M::Routine => D::Routine,
        M::Episodic => D::Episodic,
        M::EmotionalSnapshot => D::EmotionalSnapshot,
    }
}

pub(super) fn latest_memory_job(
    context: &ApiContext,
    scope: &MemoryReadScope,
) -> Result<Option<JobSnapshot>, ApiError> {
    let database = context.backend().database();
    let mut cursor = None;
    loop {
        let page = database
            .list_jobs(&JobListFilter {
                kinds: vec![JobKind::MemoryExtraction],
                states: vec![],
                subject: None,
                page: PageRequest {
                    limit: PageLimit::new(200),
                    cursor,
                },
            })
            .map_err(IntoApiError::into_api_error)?;
        for job in page.items {
            let Ok(conversation) = job.subject.id.as_str().parse() else {
                continue;
            };
            if !scope.space_conversations.contains(&conversation) {
                continue;
            }
            let detail = database
                .job_detail(job.id)
                .map_err(|_| {
                    api_error(
                        ApiErrorCode::Internal,
                        "memory job detail could not be read",
                    )
                })?
                .ok_or_else(|| {
                    api_error(ApiErrorCode::Internal, "memory job admission is missing")
                })?;
            let batch =
                crate::companion::companion_memory_job::decode_memory_admission(detail.detail)
                    .map_err(|_| {
                        api_error(ApiErrorCode::Internal, "memory job admission is invalid")
                    })?;
            if scope.pooled || batch.branch_id == scope.branch_id {
                return Ok(Some(job));
            }
        }
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return Ok(None),
        }
    }
}

pub(crate) fn failure(job: &JobSnapshot) -> Option<dto::MemoryFailureCode> {
    let error = job.error.as_ref()?;
    use dto::MemoryFailureCode as F;
    Some(match error.message.as_str() {
        "embedding-model-unavailable" => F::EmbeddingUnavailable,
        "memory-model-required" => F::ModelMissing,
        "memory-model-invalid" => F::ModelInvalid,
        "memory-prompt-required" => F::PromptMissing,
        "memory-settings-invalid" => F::SettingsInvalid,
        "companion-memory-provider-unavailable" => F::ProviderUnavailable,
        "companion-memory-provider-rejected" => F::ProviderRejected,
        "companion-memory-empty-response" => F::EmptyResponse,
        "companion-memory-round-limit" => F::RoundLimit,
        "companion-memory-tool-failed" => F::ToolFailed,
        "companion-memory-recovery-failed" => F::StorageFailure,
        _ => match error.code {
            lettuce_jobs::JobErrorCode::LeaseLost => F::LeaseLost,
            lettuce_jobs::JobErrorCode::TimedOut => F::TimedOut,
            lettuce_jobs::JobErrorCode::StorageFailure => F::StorageFailure,
            _ => F::Internal,
        },
    })
}

pub(crate) fn cycle_status(state: JobState) -> dto::MemoryCycleStatus {
    match state {
        JobState::Queued => dto::MemoryCycleStatus::Queued,
        JobState::Claimed
        | JobState::Running
        | JobState::CancellationRequested
        | JobState::CleaningUp => dto::MemoryCycleStatus::Processing,
        JobState::Succeeded => dto::MemoryCycleStatus::Complete,
        JobState::Failed => dto::MemoryCycleStatus::Failed,
        JobState::Cancelled => dto::MemoryCycleStatus::Cancelled,
        JobState::Interrupted => dto::MemoryCycleStatus::Interrupted,
    }
}

pub async fn memory_get(
    context: &ApiContext,
    request: dto::ConversationRequest,
) -> Result<dto::MemoryView, ApiError> {
    let conversation_id = parse_id(&request.conversation_id, "conversation_id")?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let conversation = ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?
                .conversation;
            database
                .get_for_branch(conversation_id, conversation.active_branch_id)
                .map_err(memory_error)?
                .ok_or_else(|| {
                    api_error(ApiErrorCode::NotFound, "conversation memory is missing")
                })?;
            let before = database
                .read_memory_scope(conversation_id, conversation.revision)
                .map_err(memory_error)?;
            crate::recount_unknown_memory_tokens(
                context.embedding().as_ref(),
                database,
                &before.memory,
                before.summary.as_ref(),
                &CancellationToken::new(),
            )
            .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            let scope = database
                .read_memory_scope(conversation_id, conversation.revision)
                .map_err(memory_error)?;
            let stored = GlobalSettingsStore::load(database)
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            let dynamic = if matches!(conversation.kind, ConversationKind::Group(_)) {
                stored.settings.effective_group_dynamic_memory()
            } else {
                &stored.settings.dynamic_memory
            };
            let latest = latest_memory_job(context, &scope)?
                .filter(|job| scope.dismissed_job != Some(job.id));
            let failure = latest.as_ref().and_then(failure);
            let mut labels = HashMap::new();
            let mut text_labels = HashMap::new();
            for cycle in &scope.cycles {
                if cycle.reverted {
                    continue;
                }
                let label = format!(
                    "{}-{}",
                    cycle.run.summary_window.start, cycle.run.summary_window.end
                );
                for result in &cycle.results {
                    if let MemoryToolOutcome::Created {
                        id,
                        short_id,
                        memories,
                    } = &result.outcome
                    {
                        labels.insert(*id, label.clone());
                        if let Some(item) = memories.iter().find(|item| item.short_id == *short_id)
                        {
                            text_labels.insert(item.text.clone(), label.clone());
                        }
                    }
                }
            }
            let since = scope.message_count.saturating_sub(scope.summary_cursor);
            Ok(dto::MemoryView {
                revision: scope.memory.revision.get(),
                items: scope
                    .memory
                    .items
                    .into_iter()
                    .map(|item| dto::MemoryItemView {
                        id: item.id.to_string(),
                        short_id: item.short_id.to_string(),
                        cycle_label: labels
                            .get(&item.id)
                            .or_else(|| text_labels.get(&item.text))
                            .cloned(),
                        text: item.text,
                        category: item.category.map(category),
                        origin: origin(item.origin),
                        pinned: item.is_pinned,
                        temperature: if item.is_cold {
                            dto::MemoryTemperature::Cold
                        } else {
                            dto::MemoryTemperature::Hot
                        },
                        observed_at: item.observed_at.map(|at| at.get()),
                        observed_time_precision: item.observed_time_precision,
                        token_count: item.token_count,
                    })
                    .collect(),
                summary: scope.summary.map(|summary| dto::MemorySummaryView {
                    text: summary.text,
                    origin: origin(summary.origin),
                    token_count: summary.token_count,
                }),
                status: dto::MemoryStatusView {
                    run_mode: match dynamic.run_mode {
                        lettuce_settings::MemoryRunMode::Auto => dto::MemoryRunMode::Auto,
                        lettuce_settings::MemoryRunMode::AskFirst => dto::MemoryRunMode::AskFirst,
                        lettuce_settings::MemoryRunMode::Manual => dto::MemoryRunMode::Manual,
                    },
                    interval: dynamic.summary_message_interval.max(1),
                    messages_since_last_cycle: since,
                    messages_until_next_cycle: u64::from(dynamic.summary_message_interval.max(1))
                        .saturating_sub(since),
                    total_conversation_messages: scope.message_count,
                    pending_approval_count: scope
                        .approval
                        .as_ref()
                        .filter(|approval| approval.pending)
                        .map(|approval| approval.prompted_message_count),
                    skipped: scope
                        .approval
                        .as_ref()
                        .is_some_and(|approval| approval.skipped),
                    latest_cycle_status: latest.as_ref().map(|job| cycle_status(job.state)),
                    latest_job_id: latest.map(|job| job.id.to_string()),
                    failure,
                    paused_reason: (failure == Some(dto::MemoryFailureCode::LeaseLost))
                        .then_some(dto::MemoryPausedReason::LeaseLost),
                },
            })
        })
        .await
}
