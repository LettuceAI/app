//! Prompt library commands: reads, authored writes with the required
//! placeholder check, hard delete, built-in reset, the app default, the
//! placeholder registry, validation, previews and the default character
//! rules.

use lettuce_context::{
    LifecycleStatus, PromptBehaviorVersion, PromptDocument, PromptEntry, PromptEntryChatMode,
    PromptEntryCondition, PromptEntryDraft, PromptEntryEdit, PromptEntryImageSlot,
    PromptEntryInfoSource, PromptEntryPayload, PromptEntryPosition, PromptEntryRole,
    PromptLibraryQuery, PromptMetadataDraft, PromptProvenance, PromptPurpose, PromptRepository,
    SceneImageProtocolKind,
};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode, ApiErrorDetails};
use lettuce_types::{CharacterId, PageRequest, PersonaId, PromptDocumentId, PromptEntryId};

use super::ApiContext;
use super::error::{IntoApiError, api_error, invalid_field, missing_placeholders, parse_id};
use super::lorebooks::{Failure, lifecycle, revision};
use super::mapping;
use super::messages::operation;

pub(super) const fn purpose(kind: dto::PromptKind) -> PromptPurpose {
    use PromptPurpose as P;
    use dto::PromptKind as K;
    match kind {
        K::DirectChat => P::DirectChat,
        K::CompanionChat => P::CompanionChat,
        K::GroupChatRoleplay => P::GroupChatRoleplay,
        K::GroupChatConversational => P::GroupChatConversational,
        K::DynamicMemorySummarizer => P::DynamicMemorySummarizer,
        K::DynamicMemoryManager => P::DynamicMemoryManager,
        K::ReplyHelperRoleplay => P::ReplyHelperRoleplay,
        K::ReplyHelperConversational => P::ReplyHelperConversational,
        K::LorebookEntryWriter => P::LorebookEntryWriter,
        K::LorebookKeywordGenerator => P::LorebookKeywordGenerator,
        K::LorebookGeneratorPlanner => P::LorebookGeneratorPlanner,
        K::LorebookGeneratorWriter => P::LorebookGeneratorWriter,
        K::LorebookGeneratorRefine => P::LorebookGeneratorRefine,
        K::LorebookGeneratorCoherence => P::LorebookGeneratorCoherence,
        K::AvatarGeneration => P::AvatarGeneration,
        K::AvatarEditRequest => P::AvatarEditRequest,
        K::SceneGeneration => P::SceneGeneration,
        K::ScenePromptWriter => P::ScenePromptWriter,
        K::DesignReferenceWriter => P::DesignReferenceWriter,
        K::CompanionSoulWriter => P::CompanionSoulWriter,
        K::CompanionGrowthcycle => P::CompanionGrowthcycle,
        K::CompanionConsolidation => P::CompanionConsolidation,
        K::RuntimeText => P::RuntimeText,
    }
}

pub(super) const fn kind(purpose: PromptPurpose) -> dto::PromptKind {
    use PromptPurpose as P;
    use dto::PromptKind as K;
    match purpose {
        P::Undefined | P::DirectChat => K::DirectChat,
        P::CompanionChat => K::CompanionChat,
        P::GroupChatRoleplay => K::GroupChatRoleplay,
        P::GroupChatConversational => K::GroupChatConversational,
        P::DynamicMemorySummarizer => K::DynamicMemorySummarizer,
        P::DynamicMemoryManager => K::DynamicMemoryManager,
        P::ReplyHelperRoleplay => K::ReplyHelperRoleplay,
        P::ReplyHelperConversational => K::ReplyHelperConversational,
        P::LorebookEntryWriter => K::LorebookEntryWriter,
        P::LorebookKeywordGenerator => K::LorebookKeywordGenerator,
        P::LorebookGeneratorPlanner => K::LorebookGeneratorPlanner,
        P::LorebookGeneratorWriter => K::LorebookGeneratorWriter,
        P::LorebookGeneratorRefine => K::LorebookGeneratorRefine,
        P::LorebookGeneratorCoherence => K::LorebookGeneratorCoherence,
        P::AvatarGeneration => K::AvatarGeneration,
        P::AvatarEditRequest => K::AvatarEditRequest,
        P::SceneGeneration => K::SceneGeneration,
        P::ScenePromptWriter => K::ScenePromptWriter,
        P::DesignReferenceWriter => K::DesignReferenceWriter,
        P::CompanionSoulWriter => K::CompanionSoulWriter,
        P::CompanionGrowthcycle => K::CompanionGrowthcycle,
        P::CompanionConsolidation => K::CompanionConsolidation,
        P::RuntimeText => K::RuntimeText,
    }
}

const fn image_slot(slot: PromptEntryImageSlot) -> dto::PromptImageSlot {
    match slot {
        PromptEntryImageSlot::Character => dto::PromptImageSlot::Character,
        PromptEntryImageSlot::Persona => dto::PromptImageSlot::Persona,
        PromptEntryImageSlot::ChatBackground => dto::PromptImageSlot::ChatBackground,
        PromptEntryImageSlot::Avatar => dto::PromptImageSlot::Avatar,
        PromptEntryImageSlot::References => dto::PromptImageSlot::References,
    }
}

const fn slot_of(slot: dto::PromptImageSlot) -> PromptEntryImageSlot {
    match slot {
        dto::PromptImageSlot::Character => PromptEntryImageSlot::Character,
        dto::PromptImageSlot::Persona => PromptEntryImageSlot::Persona,
        dto::PromptImageSlot::ChatBackground => PromptEntryImageSlot::ChatBackground,
        dto::PromptImageSlot::Avatar => PromptEntryImageSlot::Avatar,
        dto::PromptImageSlot::References => PromptEntryImageSlot::References,
    }
}

const fn role(role: PromptEntryRole) -> dto::PromptEntryRole {
    match role {
        PromptEntryRole::System => dto::PromptEntryRole::System,
        PromptEntryRole::User => dto::PromptEntryRole::User,
        PromptEntryRole::Assistant => dto::PromptEntryRole::Assistant,
    }
}

const fn position(position: PromptEntryPosition) -> dto::PromptEntryPosition {
    match position {
        PromptEntryPosition::Relative => dto::PromptEntryPosition::Relative,
        PromptEntryPosition::InChat => dto::PromptEntryPosition::InChat,
        PromptEntryPosition::Conditional => dto::PromptEntryPosition::Conditional,
        PromptEntryPosition::Interval => dto::PromptEntryPosition::Interval,
    }
}

fn condition(value: &PromptEntryCondition) -> dto::PromptCondition {
    use PromptEntryCondition as C;
    use dto::PromptCondition as D;
    match value {
        C::ChatMode { value } => D::ChatMode {
            value: match value {
                PromptEntryChatMode::Direct => dto::PromptChatMode::Direct,
                PromptEntryChatMode::Group => dto::PromptChatMode::Group,
            },
        },
        C::InfoSource { value } => D::InfoSource {
            value: match value {
                PromptEntryInfoSource::Messages => dto::PromptInfoSource::Messages,
                PromptEntryInfoSource::Memory => dto::PromptInfoSource::Memory,
                PromptEntryInfoSource::Mixed => dto::PromptInfoSource::Mixed,
            },
        },
        C::SceneGenerationEnabled { value } => D::SceneGenerationEnabled { value: *value },
        C::AvatarGenerationEnabled { value } => D::AvatarGenerationEnabled { value: *value },
        C::IsLocalImageGenerationModel { value } => {
            D::IsLocalImageGenerationModel { value: *value }
        }
        C::IsSceneGenerationLocalImageModel { value } => {
            D::IsSceneGenerationLocalImageModel { value: *value }
        }
        C::SceneImageProtocol { value } => D::SceneImageProtocol {
            value: match value {
                SceneImageProtocolKind::Remote => dto::PromptSceneImageProtocol::Remote,
                SceneImageProtocolKind::Local => dto::PromptSceneImageProtocol::Local,
            },
        },
        C::HasScene { value } => D::HasScene { value: *value },
        C::HasSceneDirection { value } => D::HasSceneDirection { value: *value },
        C::HasPersona { value } => D::HasPersona { value: *value },
        C::MessageCountAtLeast { value } => D::MessageCountAtLeast { value: *value },
        C::ParticipantCountAtLeast { value } => D::ParticipantCountAtLeast { value: *value },
        C::KeywordAny { values } => D::KeywordAny {
            values: values.clone(),
        },
        C::KeywordAll { values } => D::KeywordAll {
            values: values.clone(),
        },
        C::KeywordNone { values } => D::KeywordNone {
            values: values.clone(),
        },
        C::DynamicMemoryEnabled { value } => D::DynamicMemoryEnabled { value: *value },
        C::HasMemorySummary { value } => D::HasMemorySummary { value: *value },
        C::HasKeyMemories { value } => D::HasKeyMemories { value: *value },
        C::HasLorebookContent { value } => D::HasLorebookContent { value: *value },
        C::DoesAuthorNoteExists { value } => D::DoesAuthorNoteExists { value: *value },
        C::HasActiveScheduledNote { value } => D::HasActiveScheduledNote { value: *value },
        C::HasSubjectDescription { value } => D::HasSubjectDescription { value: *value },
        C::HasCurrentDescription { value } => D::HasCurrentDescription { value: *value },
        C::HasCharacterReferenceImages { value } => {
            D::HasCharacterReferenceImages { value: *value }
        }
        C::HasChatBackground { value } => D::HasChatBackground { value: *value },
        C::HasPersonaReferenceImages { value } => D::HasPersonaReferenceImages { value: *value },
        C::HasCharacterReferenceText { value } => D::HasCharacterReferenceText { value: *value },
        C::HasPersonaReferenceText { value } => D::HasPersonaReferenceText { value: *value },
        C::InputScopeAny { values } => D::InputScopeAny {
            values: values.clone(),
        },
        C::OutputScopeAny { values } => D::OutputScopeAny {
            values: values.clone(),
        },
        C::ProviderIdAny { values } => D::ProviderIdAny {
            values: values.clone(),
        },
        C::ReasoningEnabled { value } => D::ReasoningEnabled { value: *value },
        C::VisionEnabled { value } => D::VisionEnabled { value: *value },
        C::IsTimeAwarenessEnabled { value } => D::IsTimeAwarenessEnabled { value: *value },
        C::IsCompanionMode { value } => D::IsCompanionMode { value: *value },
        C::All { conditions } => D::All {
            conditions: conditions.iter().map(condition).collect(),
        },
        C::Any { conditions } => D::Any {
            conditions: conditions.iter().map(condition).collect(),
        },
        C::Not { condition: inner } => D::Not {
            condition: Box::new(condition(inner)),
        },
    }
}

fn condition_of(value: dto::PromptCondition) -> PromptEntryCondition {
    use PromptEntryCondition as C;
    use dto::PromptCondition as D;
    match value {
        D::ChatMode { value } => C::ChatMode {
            value: match value {
                dto::PromptChatMode::Direct => PromptEntryChatMode::Direct,
                dto::PromptChatMode::Group => PromptEntryChatMode::Group,
            },
        },
        D::InfoSource { value } => C::InfoSource {
            value: match value {
                dto::PromptInfoSource::Messages => PromptEntryInfoSource::Messages,
                dto::PromptInfoSource::Memory => PromptEntryInfoSource::Memory,
                dto::PromptInfoSource::Mixed => PromptEntryInfoSource::Mixed,
            },
        },
        D::SceneGenerationEnabled { value } => C::SceneGenerationEnabled { value },
        D::AvatarGenerationEnabled { value } => C::AvatarGenerationEnabled { value },
        D::IsLocalImageGenerationModel { value } => C::IsLocalImageGenerationModel { value },
        D::IsSceneGenerationLocalImageModel { value } => {
            C::IsSceneGenerationLocalImageModel { value }
        }
        D::SceneImageProtocol { value } => C::SceneImageProtocol {
            value: match value {
                dto::PromptSceneImageProtocol::Remote => SceneImageProtocolKind::Remote,
                dto::PromptSceneImageProtocol::Local => SceneImageProtocolKind::Local,
            },
        },
        D::HasScene { value } => C::HasScene { value },
        D::HasSceneDirection { value } => C::HasSceneDirection { value },
        D::HasPersona { value } => C::HasPersona { value },
        D::MessageCountAtLeast { value } => C::MessageCountAtLeast { value },
        D::ParticipantCountAtLeast { value } => C::ParticipantCountAtLeast { value },
        D::KeywordAny { values } => C::KeywordAny { values },
        D::KeywordAll { values } => C::KeywordAll { values },
        D::KeywordNone { values } => C::KeywordNone { values },
        D::DynamicMemoryEnabled { value } => C::DynamicMemoryEnabled { value },
        D::HasMemorySummary { value } => C::HasMemorySummary { value },
        D::HasKeyMemories { value } => C::HasKeyMemories { value },
        D::HasLorebookContent { value } => C::HasLorebookContent { value },
        D::DoesAuthorNoteExists { value } => C::DoesAuthorNoteExists { value },
        D::HasActiveScheduledNote { value } => C::HasActiveScheduledNote { value },
        D::HasSubjectDescription { value } => C::HasSubjectDescription { value },
        D::HasCurrentDescription { value } => C::HasCurrentDescription { value },
        D::HasCharacterReferenceImages { value } => C::HasCharacterReferenceImages { value },
        D::HasChatBackground { value } => C::HasChatBackground { value },
        D::HasPersonaReferenceImages { value } => C::HasPersonaReferenceImages { value },
        D::HasCharacterReferenceText { value } => C::HasCharacterReferenceText { value },
        D::HasPersonaReferenceText { value } => C::HasPersonaReferenceText { value },
        D::InputScopeAny { values } => C::InputScopeAny { values },
        D::OutputScopeAny { values } => C::OutputScopeAny { values },
        D::ProviderIdAny { values } => C::ProviderIdAny { values },
        D::ReasoningEnabled { value } => C::ReasoningEnabled { value },
        D::VisionEnabled { value } => C::VisionEnabled { value },
        D::IsTimeAwarenessEnabled { value } => C::IsTimeAwarenessEnabled { value },
        D::IsCompanionMode { value } => C::IsCompanionMode { value },
        D::All { conditions } => C::All {
            conditions: conditions.into_iter().map(condition_of).collect(),
        },
        D::Any { conditions } => C::Any {
            conditions: conditions.into_iter().map(condition_of).collect(),
        },
        D::Not { condition } => C::Not {
            condition: Box::new(condition_of(*condition)),
        },
    }
}

fn entry_view(entry: &PromptEntry) -> dto::PromptEntryView {
    dto::PromptEntryView {
        id: entry.id.to_string(),
        built_in_key: entry.built_in_entry_key.clone(),
        name: entry.name.clone(),
        role: role(entry.role),
        content: entry.content.clone(),
        enabled: entry.enabled,
        position: position(entry.injection_position),
        depth: entry.depth,
        conditional_min_messages: entry.conditional_min_messages,
        interval_turns: entry.interval_turns,
        system_prompt: entry.system_prompt,
        condition: entry.conditions.as_ref().map(condition),
        image_slot: entry.payload.as_ref().map(|payload| match payload {
            PromptEntryPayload::ImageSlot { slot } => image_slot(*slot),
        }),
    }
}

fn app_default(context: &ApiContext) -> Result<PromptDocumentId, ApiError> {
    let settings = lettuce_settings::GlobalSettingsStore::load(context.backend().database())
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
    Ok(settings
        .default_prompt_document_id
        .unwrap_or(context.backend().built_in_prompt_ids().app_default))
}

fn summary(
    context: &ApiContext,
    document: &PromptDocument,
    default: PromptDocumentId,
) -> Result<dto::PromptSummary, ApiError> {
    let origin = match &document.provenance {
        PromptProvenance::BuiltIn {
            key,
            seed_digest,
            authored_digest,
            required,
            protected,
            ..
        } => dto::PromptOrigin::BuiltIn {
            key: key.clone(),
            protected: *protected,
            required: *required,
            edited: seed_digest != authored_digest,
        },
        PromptProvenance::User => dto::PromptOrigin::User,
        PromptProvenance::Derived {
            source,
            source_name,
        } => dto::PromptOrigin::Derived {
            source_id: source.to_string(),
            source_name: source_name.clone(),
            source_deleted: PromptRepository::get(context.backend().database(), *source)
                .map_err(IntoApiError::into_api_error)?
                .is_none(),
        },
        PromptProvenance::Imported => dto::PromptOrigin::Imported,
    };
    Ok(dto::PromptSummary {
        id: document.id.to_string(),
        name: document.name.clone(),
        kind: kind(document.purpose),
        archived: document.status == LifecycleStatus::Archived,
        origin,
        app_default: document.id == default,
        revision: document.revision.get(),
        created_at: document.created_at.get(),
        updated_at: document.updated_at.get(),
    })
}

pub(super) fn view(
    context: &ApiContext,
    document: &PromptDocument,
) -> Result<dto::PromptView, ApiError> {
    Ok(dto::PromptView {
        prompt: summary(context, document, app_default(context)?)?,
        condense: document.condense,
        behavior: match document.behavior_version {
            PromptBehaviorVersion::LegacyV1 => dto::PromptBehavior::LegacyV1,
            PromptBehaviorVersion::DeterministicV2 => dto::PromptBehavior::DeterministicV2,
        },
        entries: document.entries.iter().map(entry_view).collect(),
    })
}

struct Input {
    metadata: PromptMetadataDraft,
    edits: Vec<PromptEntryEdit>,
}

fn input(prompt: dto::PromptInput) -> Result<Input, ApiError> {
    if prompt.kind == dto::PromptKind::RuntimeText {
        return Err(invalid_field(
            "kind",
            "runtime text prompts belong to the app catalog",
        ));
    }
    let edits = prompt
        .entries
        .into_iter()
        .map(|entry| {
            Ok(PromptEntryEdit {
                entry_id: entry
                    .entry_id
                    .as_deref()
                    .map(|id| parse_id::<PromptEntryId>(id, "entry_id"))
                    .transpose()?,
                draft: PromptEntryDraft {
                    built_in_entry_key: None,
                    name: entry.name,
                    role: match entry.role {
                        dto::PromptEntryRole::System => PromptEntryRole::System,
                        dto::PromptEntryRole::User => PromptEntryRole::User,
                        dto::PromptEntryRole::Assistant => PromptEntryRole::Assistant,
                    },
                    content: entry.content,
                    enabled: entry.enabled,
                    injection_position: match entry.position {
                        dto::PromptEntryPosition::Relative => PromptEntryPosition::Relative,
                        dto::PromptEntryPosition::InChat => PromptEntryPosition::InChat,
                        dto::PromptEntryPosition::Conditional => PromptEntryPosition::Conditional,
                        dto::PromptEntryPosition::Interval => PromptEntryPosition::Interval,
                    },
                    depth: entry.depth,
                    conditional_min_messages: entry.conditional_min_messages,
                    interval_turns: entry.interval_turns,
                    system_prompt: entry.system_prompt,
                    conditions: entry.condition.map(condition_of),
                    payload: entry.image_slot.map(|slot| PromptEntryPayload::ImageSlot {
                        slot: slot_of(slot),
                    }),
                },
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Input {
        metadata: PromptMetadataDraft {
            name: prompt.name,
            purpose: purpose(prompt.kind),
            condense: prompt.condense,
            behavior_version: match prompt.behavior {
                dto::PromptBehavior::LegacyV1 => PromptBehaviorVersion::LegacyV1,
                dto::PromptBehavior::DeterministicV2 => PromptBehaviorVersion::DeterministicV2,
            },
        },
        edits,
    })
}

fn missing(input: &Input) -> Vec<String> {
    let entries = input
        .edits
        .iter()
        .map(|edit| PromptEntry {
            id: PromptEntryId::new(),
            built_in_entry_key: None,
            name: edit.draft.name.clone(),
            role: edit.draft.role,
            content: edit.draft.content.clone(),
            enabled: edit.draft.enabled,
            injection_position: edit.draft.injection_position,
            depth: edit.draft.depth,
            conditional_min_messages: edit.draft.conditional_min_messages,
            interval_turns: edit.draft.interval_turns,
            system_prompt: edit.draft.system_prompt,
            conditions: edit.draft.conditions.clone(),
            payload: edit.draft.payload.clone(),
        })
        .collect::<Vec<_>>();
    lettuce_context::missing_required_placeholders(input.metadata.purpose, &entries)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn require_placeholders(input: &Input) -> Result<(), ApiError> {
    let missing = missing(input);
    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing_placeholders(missing))
    }
}

pub async fn prompts_list(
    context: &ApiContext,
    request: dto::PromptsListRequest,
) -> Result<dto::PromptPage, ApiError> {
    context
        .blocking(move |context| {
            let page = PromptRepository::page(
                context.backend().database(),
                PromptLibraryQuery {
                    page: PageRequest {
                        cursor: request.cursor,
                        limit: mapping::page_limit(request.limit),
                    },
                    status: lifecycle(request.lifecycle),
                    purpose: request.kind.map(purpose),
                },
            )
            .map_err(|error| match error {
                lettuce_context::PromptRepositoryError::Failure(_) => {
                    invalid_field("cursor", "the cursor is not valid")
                }
                error => error.into_api_error(),
            })?;
            let default = app_default(context)?;
            Ok(dto::PromptPage {
                items: page
                    .items
                    .iter()
                    .map(|document| summary(context, document, default))
                    .collect::<Result<_, _>>()?,
                next_cursor: page.next_cursor,
            })
        })
        .await
}

fn load(context: &ApiContext, id: PromptDocumentId) -> Result<PromptDocument, ApiError> {
    PromptRepository::get(context.backend().database(), id)
        .map_err(IntoApiError::into_api_error)?
        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "prompt not found"))
}

pub async fn prompt_get(
    context: &ApiContext,
    request: dto::PromptGetRequest,
) -> Result<dto::PromptView, ApiError> {
    let id = parse_id(&request.prompt_id, "prompt_id")?;
    context
        .blocking(move |context| view(context, &load(context, id)?))
        .await
}

/// Creates a user prompt. A prompt missing a placeholder its kind requires
/// is refused with the missing names; the operation key replays the first
/// result.
pub async fn prompt_create(
    context: &ApiContext,
    request: dto::PromptCreateRequest,
) -> Result<dto::PromptView, ApiError> {
    let bytes = serde_json::to_vec(&request)
        .map_err(|_| api_error(ApiErrorCode::Internal, "the request could not be encoded"))?;
    let token = operation(
        request.client_operation_id.clone(),
        &[b"prompt_create", &bytes],
    )?;
    let input = input(request.prompt)?;
    if input.edits.iter().any(|edit| edit.entry_id.is_some()) {
        return Err(invalid_field("entry_id", "a new prompt has no entries yet"));
    }
    require_placeholders(&input)?;
    let key = request.client_operation_id;
    context
        .blocking(move |context| {
            let now = context.now();
            let default = app_default(context)?;
            context
                .backend()
                .database()
                .commit_api_operation(
                    "prompt_create",
                    &key,
                    token.request_digest.as_str(),
                    now,
                    |scope| {
                        let document = scope
                            .create_prompt(
                                input.metadata,
                                input.edits.into_iter().map(|edit| edit.draft).collect(),
                                now,
                            )
                            .map_err(|error| Failure(error.into_api_error()))?;
                        Ok::<_, Failure>(dto::PromptView {
                            prompt: summary(context, &document, default).map_err(Failure)?,
                            condense: document.condense,
                            behavior: match document.behavior_version {
                                PromptBehaviorVersion::LegacyV1 => dto::PromptBehavior::LegacyV1,
                                PromptBehaviorVersion::DeterministicV2 => {
                                    dto::PromptBehavior::DeterministicV2
                                }
                            },
                            entries: document.entries.iter().map(entry_view).collect(),
                        })
                    },
                )
                .map_err(|failure| failure.0)
        })
        .await
}

/// Replaces a prompt's metadata and entries under its revision, with the
/// same placeholder check as create.
pub async fn prompt_update(
    context: &ApiContext,
    request: dto::PromptUpdateRequest,
) -> Result<dto::PromptView, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id = parse_id(&request.prompt_id, "prompt_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let input = input(request.prompt)?;
    require_placeholders(&input)?;
    context
        .blocking(move |context| {
            let document = context
                .backend()
                .database()
                .commit_api_operation(
                    "prompt_update",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .update_prompt(id, expected, input.metadata, input.edits, context.now())
                            .map_err(|error| Failure(error.into_api_error()))
                    },
                )
                .map_err(|error| error.0)?;
            view(context, &document)
        })
        .await
}

/// Hard deletes a prompt. Protected and required built-ins are refused;
/// selections of it return to their defaults in the same transaction and
/// turn history keeps its name.
pub async fn prompt_delete(
    context: &ApiContext,
    request: dto::PromptDeleteRequest,
) -> Result<dto::SourceDeleteResult, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id = parse_id(&request.prompt_id, "prompt_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let result = context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "prompt_delete",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .delete_prompt(id, expected, context.now())
                            .map(super::lorebooks::deletion)
                            .map_err(|error| Failure(error.into_api_error()))
                    },
                )
                .map_err(|error| error.0)
        })
        .await?;
    super::lorebooks::emit_deletion(context, &result);
    context.emit(dto::ApiEvent::PromptsChanged);
    Ok(result)
}

pub async fn prompt_builtin_reset(
    context: &ApiContext,
    request: dto::PromptBuiltinResetRequest,
) -> Result<dto::PromptBuiltinResetResult, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    context
        .blocking(move |context| {
            let catalog = crate::BuiltInPromptCatalog::bundled()
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            let (key, seeds, expected) = match request {
                dto::PromptBuiltinResetRequest::One {
                    client_operation_id,
                    prompt_id,
                    expected_revision,
                } => {
                    let id = parse_id(&prompt_id, "prompt_id")?;
                    let current = load(context, id)?;
                    let PromptProvenance::BuiltIn { key, .. } = current.provenance else {
                        return Err(invalid_field("prompt_id", "the prompt is not built in"));
                    };
                    let builtin = crate::BuiltInPromptId::from_key_or_alias(&key)
                        .ok_or_else(|| invalid_field("prompt_id", "the prompt is not built in"))?;
                    (
                        client_operation_id,
                        vec![catalog.seed(builtin).clone()],
                        Some((id, revision(expected_revision, "expected_revision")?)),
                    )
                }
                dto::PromptBuiltinResetRequest::All {
                    client_operation_id,
                } => (client_operation_id, catalog.seeds().to_vec(), None),
            };
            let documents = context
                .backend()
                .database()
                .commit_api_operation(
                    "prompt_builtin_reset",
                    &key,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .reset_builtin_prompts(
                                lettuce_context::BuiltInReconcileRequest {
                                    seeds,
                                    mode: lettuce_context::BuiltInReconcileMode::ResetToSeed,
                                },
                                expected,
                                context.now(),
                            )
                            .map(|outcomes| {
                                outcomes
                                    .into_iter()
                                    .map(|outcome| outcome.document)
                                    .collect::<Vec<_>>()
                            })
                            .map_err(|error| Failure(error.into_api_error()))
                    },
                )
                .map_err(|error| error.0)?;
            Ok(dto::PromptBuiltinResetResult {
                prompts: documents
                    .iter()
                    .map(|document| view(context, document))
                    .collect::<Result<_, _>>()?,
            })
        })
        .await
}

pub async fn prompt_app_default_set(
    context: &ApiContext,
    request: dto::PromptAppDefaultSetRequest,
) -> Result<dto::PromptAppDefault, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id = request
        .prompt_id
        .as_deref()
        .map(|id| parse_id::<PromptDocumentId>(id, "prompt_id"))
        .transpose()?;
    let expected = revision(
        request.expected_settings_revision,
        "expected_settings_revision",
    )?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "prompt_app_default_set",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .set_default_prompt(id, expected, context.now())
                            .map(|(id, revision)| dto::PromptAppDefault {
                                prompt_id: id.map(|id| id.to_string()),
                                settings_revision: revision.get(),
                            })
                            .map_err(|error| Failure(error.into_api_error()))
                    },
                )
                .map_err(|error| error.0)
        })
        .await
}

pub async fn prompt_placeholders(
    _context: &ApiContext,
    request: dto::PromptPlaceholdersRequest,
) -> Result<dto::PromptPlaceholders, ApiError> {
    let registry = lettuce_context::prompt_placeholders(purpose(request.kind));
    Ok(dto::PromptPlaceholders {
        kind: request.kind,
        allowed: registry.allowed.into_iter().map(str::to_owned).collect(),
        required: registry.required.into_iter().map(str::to_owned).collect(),
        image_slots: registry.image_slots.into_iter().map(image_slot).collect(),
    })
}

pub async fn prompt_validate(
    _context: &ApiContext,
    request: dto::PromptValidateRequest,
) -> Result<dto::PromptValidation, ApiError> {
    Ok(dto::PromptValidation {
        missing_placeholders: missing(&input(request.prompt)?),
    })
}

fn runtime_text_error() -> ApiError {
    ApiError {
        code: ApiErrorCode::Unavailable,
        message: "the app's runtime text could not be read".into(),
        details: Some(ApiErrorDetails::RuntimeTextUnavailable),
    }
}

/// The values a preview without a conversation renders with: the given
/// character and persona, the catalog's sample memories, summary and
/// lorebook text, and the current Pure mode rules.
fn sample_context(
    context: &ApiContext,
    document: &PromptDocument,
    character: Option<CharacterId>,
    persona: Option<PersonaId>,
) -> Result<lettuce_context::PromptRenderContext, ApiError> {
    use crate::generation::runtime_text::RuntimeText;
    use lettuce_characters::{CharacterRepository, PersonaRepository};
    use lettuce_context::PromptVariable;
    let database = context.backend().database();
    let text = RuntimeText::load(database, crate::BuiltInPromptId::ChatRuntime)
        .map_err(|_| runtime_text_error())?;
    let memory_text = RuntimeText::load(database, crate::BuiltInPromptId::MemoryRuntime)
        .map_err(|_| runtime_text_error())?;
    let fragment = |key: &str| {
        text.render_with(key, Vec::new())
            .map_err(|_| runtime_text_error())
    };
    let character = character
        .map(|id| {
            CharacterRepository::get(database, id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "character not found"))
        })
        .transpose()?;
    let persona = persona
        .map(|id| {
            PersonaRepository::get(database, id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "persona not found"))
        })
        .transpose()?;
    let key_memories = [
        "preview_sample_memory_first",
        "preview_sample_memory_second",
    ]
    .into_iter()
    .map(|key| {
        let line = lettuce_conversations::MemoryPromptLine {
            text: fragment(key)?,
            observed: None,
        };
        crate::memory::memory_prompt::render_memory_line(&memory_text, &line, false)
            .map_err(|_| runtime_text_error())
    })
    .collect::<Result<Vec<_>, _>>()?
    .join("\n");
    let pure_mode = lettuce_settings::GlobalSettingsStore::load(database)
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
        .settings
        .pure_mode;
    let character_name = character
        .as_ref()
        .map(|character| character.character.profile.name.clone())
        .unwrap_or_default();
    let character_description = character
        .as_ref()
        .and_then(|character| character.character.profile.description.clone())
        .unwrap_or_default();
    let persona_name = persona
        .as_ref()
        .map(|persona| persona.title.clone())
        .unwrap_or_default();
    let persona_description = persona
        .as_ref()
        .map(|persona| persona.description.clone())
        .unwrap_or_default();
    let group = matches!(
        document.purpose,
        PromptPurpose::GroupChatRoleplay | PromptPurpose::GroupChatConversational
    );
    let mut values = lettuce_context::PromptRenderValues {
        character_name: character_name.clone(),
        character_description: character_description.clone(),
        persona_name: persona_name.clone(),
        persona_description: persona_description.clone(),
        lorebook: fragment("preview_sample_lorebook")?,
        context_summary: fragment("preview_sample_summary")?,
        key_memories,
        content_rules: crate::generation::pure_mode_rules::content_rules(database, pure_mode)
            .map_err(|_| runtime_text_error())?,
        user_name: persona_name.clone(),
        user_description: persona_description,
        ai_name: character_name.clone(),
        ai_description: character_description,
        ..lettuce_context::PromptRenderValues::default()
    };
    if group {
        values
            .purpose_values
            .insert(PromptVariable::GroupCharacters, character_name);
    }
    values
        .purpose_values
        .retain(|variable, _| variable.is_allowed_for(document.purpose));
    Ok(lettuce_context::PromptRenderContext {
        conditions: lettuce_context::PromptConditionContext {
            chat_mode: if group {
                PromptEntryChatMode::Group
            } else {
                PromptEntryChatMode::Direct
            },
            has_persona: persona.is_some(),
            has_memory_summary: true,
            has_key_memories: true,
            has_lorebook_content: true,
            ..lettuce_context::PromptConditionContext::default()
        },
        values,
    })
}

/// Renders a prompt with entry selection and conditions: from a
/// conversation's live sources as its next turn sees them, or from sample
/// values without one.
pub async fn prompt_preview(
    context: &ApiContext,
    request: dto::PromptPreviewRequest,
) -> Result<dto::PromptPreview, ApiError> {
    let id: PromptDocumentId = parse_id(&request.prompt_id, "prompt_id")?;
    let conversation = request
        .conversation_id
        .as_deref()
        .map(|id| parse_id::<lettuce_types::ConversationId>(id, "conversation_id"))
        .transpose()?;
    let character = request
        .character_id
        .as_deref()
        .map(|id| parse_id::<CharacterId>(id, "character_id"))
        .transpose()?;
    let persona = request
        .persona_id
        .as_deref()
        .map(|id| parse_id::<PersonaId>(id, "persona_id"))
        .transpose()?;
    let handle = tokio::runtime::Handle::current();
    context
        .blocking(move |context| {
            let document = load(context, id)?;
            let (render_context, strip_scene) = match conversation {
                Some(conversation_id) => {
                    let backend = context.backend();
                    let embedding = context.embedding();
                    let runner = backend.prepared_conversation_generation_runner(
                        embedding.as_ref(),
                        context.inference(),
                        &super::worker::UnstoredReplyMedia,
                    );
                    handle
                        .block_on(runner.prompt_preview_context(
                            conversation_id,
                            &document,
                            character,
                            context.now(),
                        ))
                        .map_err(|error| {
                            api_error(ApiErrorCode::Unavailable, format!("{error:?}"))
                        })?
                }
                None => (
                    sample_context(context, &document, character, persona)?,
                    false,
                ),
            };
            let mut rendered_document = document.clone();
            if strip_scene {
                rendered_document.entries.retain(|entry| {
                    !crate::generation::context_assembler::has_scene_placeholder(&entry.content)
                });
            }
            let preview = lettuce_context::preview_prompt(&rendered_document, &render_context)
                .map_err(|error| api_error(ApiErrorCode::InvalidInput, error.to_string()))?;
            let entries = preview
                .rendered
                .relative
                .iter()
                .chain(&preview.rendered.in_chat)
                .filter_map(|message| {
                    let entry = document
                        .entries
                        .iter()
                        .find(|entry| entry.id == message.entry_id)?;
                    Some(dto::PromptPreviewEntry {
                        entry_id: entry.id.to_string(),
                        name: entry.name.clone(),
                        role: role(message.role),
                        position: position(entry.injection_position),
                        depth: message.depth,
                        text: message.content.clone(),
                    })
                })
                .collect::<Vec<_>>();
            let skipped_entry_ids = document
                .entries
                .iter()
                .filter(|entry| {
                    !entries
                        .iter()
                        .any(|shown| shown.entry_id == entry.id.to_string())
                })
                .map(|entry| entry.id.to_string())
                .collect();
            Ok(dto::PromptPreview {
                entries,
                skipped_entry_ids,
            })
        })
        .await
}

/// The rules a new character starts with. The app's runtime text failing to
/// load is `RuntimeTextUnavailable`, never an empty list.
pub async fn default_character_rules(
    context: &ApiContext,
    request: dto::DefaultCharacterRulesRequest,
) -> Result<dto::DefaultCharacterRules, ApiError> {
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let level = match request.pure_mode {
                Some(dto::PureModeLevel::Off) => lettuce_settings::PureMode::Off,
                Some(dto::PureModeLevel::Low) => lettuce_settings::PureMode::Low,
                Some(dto::PureModeLevel::Standard) => lettuce_settings::PureMode::Standard,
                Some(dto::PureModeLevel::Strict) => lettuce_settings::PureMode::Strict,
                None => {
                    lettuce_settings::GlobalSettingsStore::load(database)
                        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
                        .settings
                        .pure_mode
                }
            };
            crate::generation::pure_mode_rules::default_character_rules(database, level)
                .map(|rules| dto::DefaultCharacterRules { rules })
                .map_err(|_| runtime_text_error())
        })
        .await
}
