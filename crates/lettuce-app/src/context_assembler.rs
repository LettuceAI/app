//! Provider-neutral context assembly.
//!
//! This module is intentionally the last application-side step before model
//! admission.  It reads only conversation aggregates and protected launch
//! snapshots; provider selection, memory retrieval, and inference belong to
//! later application ports.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use lettuce_characters::{CharacterRepository, PersonaRepository};
use lettuce_companions::{
    CompanionPromptStateInput, CompanionScheduledNoteRepository, CompanionStateOwner,
    CompanionStateRepository, SoulOwner, SoulRepository, active_scheduled_notes,
    render_prompt_state, render_scheduled_notes_block,
};
use lettuce_context::{
    DetectionPolicy, KeywordMatchMode, LorebookSnapshotActivationEntry,
    LorebookSnapshotActivationSource, PromptBehaviorVersion, PromptConditionContext, PromptEntry,
    PromptEntryChatMode, PromptEntryCondition, PromptEntryImageSlot, PromptEntryInfoSource,
    PromptEntryPayload, PromptEntryPosition, PromptEntryRole, PromptPurpose, PromptRenderContext,
    PromptRenderValues, PromptSnapshot, PromptVariable, RenderedPromptMessage,
    render_prompt_snapshot, resolve_lorebook_snapshot_activation,
};
use lettuce_conversations::{
    AnnotationPayload, BranchStatus, ContextAssemblyError, ContextAttributions,
    ContextBudgetReport, ContextRequest, ConversationAggregate, ConversationKind,
    ConversationReader, ConversationSnapshotMaterializer, EffectiveConversationSettings,
    GenerationOperation, LorebookAttribution, MessagePart, MessageRenderSource, MessageRole,
    PromptAttribution, ProviderContextPart, ProviderNeutralContext, ProviderNeutralMessage,
    SnapshotDocumentBody, SnapshotDocumentKind, SnapshotSelection, TimelineItem,
};
use lettuce_conversations::{
    CharacterSnapshotBodyV1, ConversationParticipant, LorebookLaunchSnapshot,
    LorebookSnapshotBodyV1, PersonaSnapshotBodyV1, PromptEntryConditionV1, PromptEntryImageSlotV1,
    PromptEntryPayloadV1, PromptEntryPositionV1, PromptEntryRoleV1, PromptLaunchSnapshot,
    PromptSnapshotBodyV1, SceneLaunchSnapshot, ScenePartV1, SceneSnapshotBodyV1,
};
use lettuce_types::{ConversationId, ConversationParticipantId, MessageId, TimestampMillis};

/// Concrete context assembly service. Its dependency is a set of domain ports;
/// this type deliberately has no database, provider, model, or memory port.
#[derive(Debug)]
pub struct ConversationContextAssembler<'a, S> {
    sources: &'a S,
}

impl<'a, S> ConversationContextAssembler<'a, S> {
    pub fn new(sources: &'a S) -> Self {
        Self { sources }
    }
}

#[async_trait]
impl<S> lettuce_conversations::ContextAssembler for ConversationContextAssembler<'_, S>
where
    S: ConversationReader
        + ConversationSnapshotMaterializer
        + CharacterRepository
        + PersonaRepository
        + SoulRepository
        + CompanionStateRepository
        + CompanionScheduledNoteRepository,
{
    async fn assemble(
        &self,
        request: ContextRequest,
    ) -> Result<ProviderNeutralContext, ContextAssemblyError> {
        request
            .validate()
            .map_err(|_| ContextAssemblyError::InvalidRequest)?;
        let aggregate = ConversationReader::get(self.sources, request.conversation_id)
            .map_err(|_| ContextAssemblyError::ConversationUnavailable)?;
        validate_aggregate_and_path(&aggregate, &request)?;
        validate_timeline_items(&request)?;
        validate_message_ancestry(&aggregate.branches, &request)?;

        let settings = lettuce_conversations::resolve_effective_settings(
            &aggregate.conversation,
            request
                .selected_speaker
                .as_ref()
                .map(|speaker| speaker.participant_id),
        )
        .map_err(|error| {
            if matches!(
                error,
                lettuce_conversations::ValidationError::InvalidReference {
                    field: "effective_settings.selected_speaker"
                }
            ) {
                ContextAssemblyError::MissingSpeaker
            } else {
                ContextAssemblyError::InvalidRequest
            }
        })?;

        let snapshot = SnapshotBundle::load(
            self.sources,
            &aggregate,
            request.conversation_id,
            &settings,
            request
                .selected_speaker
                .as_ref()
                .map(|speaker| speaker.participant_id),
        )?;
        let (selected_window, omitted_messages, scene_timeline) =
            select_timeline(&aggregate, &request)?;
        let (scene, scene_direction) = snapshot.scene_values(&scene_timeline)?;
        let effective_at = source_effective_time(&request)?;
        let companion_state = self.companion_prompt_state(&aggregate, &snapshot, effective_at)?;
        let scheduled_notes =
            self.companion_scheduled_notes(&aggregate, &snapshot, effective_at)?;

        let recent_text = selected_window
            .iter()
            .filter_map(active_text)
            .rev()
            .take(10)
            .collect::<Vec<_>>();
        let recent_text = recent_text.into_iter().rev().collect::<Vec<_>>();
        let latest_user_message = selected_window
            .iter()
            .rev()
            .filter(|item| item.message.role == MessageRole::User)
            .find_map(active_text);

        let lore_sources = snapshot
            .lorebooks
            .iter()
            .enumerate()
            .map(|(source_order, (reference, body))| lorebook_source(reference, body, source_order))
            .collect::<Result<Vec<_>, _>>()?;
        let lore_activation = resolve_lorebook_snapshot_activation(
            &lore_sources,
            &recent_text,
            latest_user_message.as_deref(),
        )
        .map_err(|_| ContextAssemblyError::LorebookActivation)?;
        let lorebook_text = lore_activation
            .entries
            .iter()
            .map(|entry| entry.entry.content.trim())
            .filter(|content| !content.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");

        let (prompt, rendered_prompt) = if let Some((reference, body)) = snapshot.prompt.as_ref() {
            let document = body.clone();
            let render_context = PromptRenderContext {
                conditions: prompt_conditions(
                    &aggregate,
                    &request,
                    &snapshot,
                    &settings,
                    &lorebook_text,
                    &scene,
                    &scene_direction,
                    &recent_text,
                    selected_window.len(),
                    companion_state.is_some(),
                    scheduled_notes.is_some(),
                ),
                values: prompt_values(
                    &aggregate,
                    &snapshot,
                    &settings,
                    &scene,
                    &scene_direction,
                    &lorebook_text,
                    &request,
                    request.swap_roles,
                    companion_state.as_deref(),
                    scheduled_notes.as_deref(),
                ),
            };
            let rendered = render_prompt_snapshot(&document, &render_context).map_err(|error| {
                tracing::warn!(?error, "prompt snapshot rendering failed");
                ContextAssemblyError::PromptRender
            })?;
            (Some((reference.clone(), document)), rendered)
        } else {
            (None, Default::default())
        };

        let (mut messages, mut in_chat_messages) = prompt_messages(&rendered_prompt)?;
        let author_note = settings.author_note.as_deref().unwrap_or_default();
        let author_consumed = author_note_consumed(&rendered_prompt, prompt.as_ref());
        if !author_note.trim().is_empty() && !author_consumed {
            let attribution_name = author_note_attribution_name(&aggregate, &snapshot, &request);
            in_chat_messages.push((
                1,
                text_message(
                    MessageRole::System,
                    &format!(
                        "# Author Note\nThe following is private session-level guidance from {attribution_name}. Treat it as hidden continuity and writing context for this chat. Use its facts naturally when relevant, including answering with those facts when the conversation calls for them, but do not say they came from an author note or hidden instruction.\n\n{}",
                        author_note.trim()
                    ),
                ),
            ));
        }
        let lore_consumed = lorebook_consumed(&rendered_prompt, prompt.as_ref());
        if !lorebook_text.trim().is_empty() && !lore_consumed {
            in_chat_messages.push((
                0,
                text_message(
                    MessageRole::System,
                    &format!("# World Information\n{}", lorebook_text.trim()),
                ),
            ));
        }
        let memory_summary = request
            .memory
            .as_ref()
            .and_then(|memory| memory.summary.as_deref())
            .unwrap_or_default();
        let memory_keys = request
            .memory
            .as_ref()
            .map(|memory| memory.key_memories.join("\n"))
            .unwrap_or_default();
        let memory_summary_consumed = rendered_entry_consumed(
            &rendered_prompt,
            prompt.as_ref(),
            &["{{context_summary}}", "{{memory_summary}}"],
        );
        let memory_keys_consumed = rendered_entry_consumed(
            &rendered_prompt,
            prompt.as_ref(),
            &["{{key_memories}}", "{{selected_memories}}"],
        );
        let mut memory_used = false;
        if !memory_summary.trim().is_empty() && !memory_summary_consumed {
            in_chat_messages.push((
                1,
                text_message(
                    MessageRole::System,
                    &format!("# Memory Summary\n{}", memory_summary.trim()),
                ),
            ));
            memory_used = true;
        } else if !memory_summary.trim().is_empty() {
            memory_used = true;
        }
        if !memory_keys.trim().is_empty() && !memory_keys_consumed {
            in_chat_messages.push((
                1,
                text_message(
                    MessageRole::System,
                    &format!("# Key Memories\n{}", memory_keys.trim()),
                ),
            ));
            memory_used = true;
        } else if !memory_keys.trim().is_empty() {
            memory_used = true;
        }
        let scheduled_notes_consumed =
            rendered_entry_consumed(&rendered_prompt, prompt.as_ref(), &["{{scheduled_notes}}"]);
        if let Some(notes) = scheduled_notes.as_deref()
            && !scheduled_notes_consumed
        {
            in_chat_messages.push((0, text_message(MessageRole::System, notes)));
        }
        if prompt
            .as_ref()
            .is_some_and(|(_, document)| document.condense)
        {
            condense_prompt_messages(&mut messages);
        }

        let character_names = snapshot
            .characters
            .iter()
            .map(|(participant, _body)| (participant.id, participant.display_name.clone()))
            .collect::<HashMap<_, _>>();
        let selected_speaker = request
            .selected_speaker
            .as_ref()
            .map(|speaker| speaker.participant_id);
        let mut transcript = Vec::new();
        for item in &selected_window {
            if let Some(message) = provider_message(
                item,
                &aggregate,
                &character_names,
                selected_speaker,
                request.swap_roles,
            )? {
                transcript.push(message);
            }
        }
        let final_transcript_role = transcript.last().map(|message| message.role);
        messages.append(&mut transcript);
        insert_in_chat_messages(&mut messages, in_chat_messages);

        if let Some(instruction) = operation_instruction(
            request.operation,
            request.guidance.as_deref(),
            final_transcript_role,
        ) {
            messages.push(text_message(MessageRole::User, &instruction));
        }

        let attributions = ContextAttributions {
            prompt: prompt.map(|(reference, _document)| PromptAttribution {
                document_id: reference.source_id,
                revision: reference.source_revision,
                selected_entry_ids: rendered_prompt
                    .relative
                    .iter()
                    .chain(rendered_prompt.in_chat.iter())
                    .map(|entry| entry.entry_id)
                    .collect(),
            }),
            lorebooks: lore_attributions(&lore_activation),
            memory: memory_used
                .then(|| {
                    request
                        .memory
                        .as_ref()
                        .map(|memory| memory.attribution.clone())
                })
                .flatten(),
        };
        let budget = budget_report(&messages, omitted_messages)?;
        let context = ProviderNeutralContext {
            messages,
            attributions,
            budget,
        };
        context.validate().map_err(map_output_validation)?;
        Ok(context)
    }
}

impl<S> ConversationContextAssembler<'_, S>
where
    S: CharacterRepository
        + PersonaRepository
        + SoulRepository
        + CompanionStateRepository
        + CompanionScheduledNoteRepository,
{
    fn companion_prompt_state(
        &self,
        aggregate: &ConversationAggregate,
        snapshot: &SnapshotBundle,
        effective_at: TimestampMillis,
    ) -> Result<Option<String>, ContextAssemblyError> {
        let ConversationKind::Direct(details) = &aggregate.conversation.kind else {
            return Ok(None);
        };
        if snapshot.characters.first().is_none_or(|(_, character)| {
            character.interaction_mode != lettuce_conversations::InteractionModeV1::Companion
        }) {
            return Ok(None);
        }
        let character = CharacterRepository::get(self.sources, details.character.source_id)
            .map_err(|_| ContextAssemblyError::ConversationUnavailable)?
            .ok_or(ContextAssemblyError::ConversationUnavailable)?;
        let config = character
            .character
            .defaults
            .companion_soul
            .unwrap_or_default();
        let persona_id = match &details.persona {
            SnapshotSelection::Inherited(persona) | SnapshotSelection::Explicit(persona) => {
                Some(persona.source_id)
            }
            SnapshotSelection::Disabled => None,
        };
        let state = CompanionStateRepository::get(
            self.sources,
            CompanionStateOwner {
                conversation_id: aggregate.conversation.id,
                character_id: character.character.id,
                persona_id,
            },
        )
        .map_err(|_| ContextAssemblyError::ConversationUnavailable)?
        .ok_or(ContextAssemblyError::ConversationUnavailable)?;
        let episode = CompanionStateRepository::get_continuity_episode(
            self.sources,
            aggregate.conversation.id,
        )
        .map_err(|_| ContextAssemblyError::ConversationUnavailable)?
        .filter(|episode| {
            episode.character_id == character.character.id && episode.persona_id == persona_id
        })
        .ok_or(ContextAssemblyError::ConversationUnavailable)?;
        let soul = SoulRepository::get(self.sources, SoulOwner::Character(character.character.id))
            .map_err(|_| ContextAssemblyError::ConversationUnavailable)?
            .ok_or(ContextAssemblyError::ConversationUnavailable)?;
        let partner_name = persona_id
            .map(|id| PersonaRepository::get(self.sources, id))
            .transpose()
            .map_err(|_| ContextAssemblyError::ConversationUnavailable)?
            .flatten()
            .map(|persona| persona.title);
        Ok(Some(render_prompt_state(&CompanionPromptStateInput {
            character_name: &character.character.profile.name,
            partner_name: partner_name.as_deref(),
            soul: &config.soul,
            soul_state: &soul,
            runtime_state: &state.state,
            style_notes: &config.prompting.style_notes,
            continuity_episode: episode.episode_index,
            effective_at,
        })))
    }

    fn companion_scheduled_notes(
        &self,
        aggregate: &ConversationAggregate,
        snapshot: &SnapshotBundle,
        effective_at: TimestampMillis,
    ) -> Result<Option<String>, ContextAssemblyError> {
        let ConversationKind::Direct(details) = &aggregate.conversation.kind else {
            return Ok(None);
        };
        if snapshot.characters.first().is_none_or(|(_, character)| {
            character.interaction_mode != lettuce_conversations::InteractionModeV1::Companion
        }) {
            return Ok(None);
        }
        let notes = self
            .sources
            .list_scheduled_notes(details.character.source_id)
            .map_err(|_| ContextAssemblyError::ConversationUnavailable)?;
        let active = active_scheduled_notes(notes, effective_at)
            .map_err(|_| ContextAssemblyError::ConversationUnavailable)?;
        Ok(render_scheduled_notes_block(&active))
    }
}

fn source_effective_time(
    request: &ContextRequest,
) -> Result<TimestampMillis, ContextAssemblyError> {
    request
        .timeline
        .iter()
        .find(|item| item.message.id == request.source_message_id)
        .map(|item| item.message.effective_time)
        .ok_or(ContextAssemblyError::InvalidTimeline)
}

fn author_note_attribution_name(
    aggregate: &ConversationAggregate,
    snapshot: &SnapshotBundle,
    request: &ContextRequest,
) -> String {
    if request.swap_roles && !aggregate.conversation.kind.is_group() {
        return selected_character(snapshot, request)
            .map(|character| character.name.clone())
            .unwrap_or_else(|| "character".into());
    }
    snapshot
        .persona
        .as_ref()
        .map(|persona| persona.title.clone())
        .unwrap_or_else(|| "user".into())
}

fn operation_instruction(
    operation: GenerationOperation,
    guidance: Option<&str>,
    final_transcript_role: Option<MessageRole>,
) -> Option<String> {
    if let Some(guidance) = guidance.filter(|value| !value.trim().is_empty()) {
        return Some(guidance.to_owned());
    }
    match operation {
        GenerationOperation::Continue if final_transcript_role != Some(MessageRole::User) => {
            Some("Continue the conversation from the current head.".into())
        }
        GenerationOperation::Send
        | GenerationOperation::Regenerate
        | GenerationOperation::Continue => None,
    }
}

fn map_output_validation(error: lettuce_conversations::ValidationError) -> ContextAssemblyError {
    match error {
        lettuce_conversations::ValidationError::TooMany { .. }
        | lettuce_conversations::ValidationError::TooLarge { .. }
        | lettuce_conversations::ValidationError::OutOfBounds { .. } => {
            ContextAssemblyError::SizeLimit
        }
        _ => ContextAssemblyError::InvalidTimeline,
    }
}

fn validate_aggregate_and_path(
    aggregate: &ConversationAggregate,
    request: &ContextRequest,
) -> Result<(), ContextAssemblyError> {
    aggregate
        .validate()
        .map_err(|_| ContextAssemblyError::ConversationUnavailable)?;
    let selected = aggregate
        .branches
        .iter()
        .find(|branch| branch.id == request.branch_id)
        .ok_or(ContextAssemblyError::InvalidTimeline)?;
    if selected.status != BranchStatus::Active {
        return Err(ContextAssemblyError::InvalidTimeline);
    }
    let mut actual = Vec::new();
    let mut current = Some(selected.id);
    while let Some(branch_id) = current {
        let branch = aggregate
            .branches
            .iter()
            .find(|branch| branch.id == branch_id)
            .ok_or(ContextAssemblyError::InvalidTimeline)?;
        actual.push(branch.id);
        current = branch.parent_branch_id;
        if actual.len() > aggregate.branches.len() {
            return Err(ContextAssemblyError::InvalidTimeline);
        }
    }
    actual.reverse();
    if actual != request.branch_path {
        return Err(ContextAssemblyError::InvalidTimeline);
    }
    Ok(())
}

fn validate_timeline_items(request: &ContextRequest) -> Result<(), ContextAssemblyError> {
    let mut ids = HashSet::new();
    for item in &request.timeline {
        if !ids.insert(item.message.id) {
            return Err(ContextAssemblyError::InvalidTimeline);
        }
        match item.message.active_render_source {
            MessageRenderSource::Revision(id) => {
                let revision = item
                    .active_revision
                    .as_ref()
                    .ok_or(ContextAssemblyError::InvalidTimeline)?;
                if revision.id != id
                    || revision.message_id != item.message.id
                    || item.active_candidate.is_some()
                {
                    return Err(ContextAssemblyError::InvalidTimeline);
                }
                revision
                    .validate()
                    .map_err(|_| ContextAssemblyError::InvalidTimeline)?;
            }
            MessageRenderSource::Candidate(id) => {
                let candidate = item
                    .active_candidate
                    .as_ref()
                    .ok_or(ContextAssemblyError::InvalidTimeline)?;
                if candidate.id != id
                    || candidate.message_id != item.message.id
                    || item.active_revision.is_some()
                {
                    return Err(ContextAssemblyError::InvalidTimeline);
                }
                candidate
                    .validate()
                    .map_err(|_| ContextAssemblyError::InvalidTimeline)?;
            }
        }
    }
    Ok(())
}

/// Prove that the supplied timeline is the exact parent chain ending at the
/// operation's source/head. Branch IDs alone are insufficient: a sibling or
/// an ancestor message after a fork can otherwise be smuggled into the
/// timestamp-sorted window.
fn validate_message_ancestry(
    branches: &[lettuce_conversations::ConversationBranch],
    request: &ContextRequest,
) -> Result<(), ContextAssemblyError> {
    let branch = branches
        .iter()
        .find(|branch| branch.id == request.branch_id)
        .ok_or(ContextAssemblyError::InvalidTimeline)?;
    let target_id = match request.operation {
        GenerationOperation::Continue => branch.head_message_id.or(Some(request.source_message_id)),
        GenerationOperation::Send | GenerationOperation::Regenerate => {
            Some(request.source_message_id)
        }
    }
    .ok_or(ContextAssemblyError::InvalidTimeline)?;
    let by_id = request
        .timeline
        .iter()
        .map(|item| (item.message.id, item))
        .collect::<HashMap<_, _>>();
    let path_indexes = request
        .branch_path
        .iter()
        .enumerate()
        .map(|(index, branch_id)| (*branch_id, index))
        .collect::<HashMap<_, _>>();
    let mut ancestry = HashSet::new();
    let mut current = Some(target_id);
    while let Some(message_id) = current {
        if !ancestry.insert(message_id) {
            return Err(ContextAssemblyError::InvalidTimeline);
        }
        let item = by_id
            .get(&message_id)
            .ok_or(ContextAssemblyError::InvalidTimeline)?;
        let current_index = *path_indexes
            .get(&item.message.branch_id)
            .ok_or(ContextAssemblyError::InvalidTimeline)?;
        current = item.message.parent_message_id;
        let Some(parent_id) = current else {
            if current_index != 0 {
                return Err(ContextAssemblyError::InvalidTimeline);
            }
            break;
        };
        let parent = by_id
            .get(&parent_id)
            .ok_or(ContextAssemblyError::InvalidTimeline)?;
        let parent_index = *path_indexes
            .get(&parent.message.branch_id)
            .ok_or(ContextAssemblyError::InvalidTimeline)?;
        if parent_index > current_index {
            return Err(ContextAssemblyError::InvalidTimeline);
        }
        if parent_index == current_index {
            continue;
        }
        if parent_index + 1 != current_index {
            return Err(ContextAssemblyError::InvalidTimeline);
        }
        let child_branch = branches
            .iter()
            .find(|branch| branch.id == item.message.branch_id)
            .ok_or(ContextAssemblyError::InvalidTimeline)?;
        if child_branch.fork_message_id != Some(parent_id) {
            return Err(ContextAssemblyError::InvalidTimeline);
        }
    }
    if request
        .timeline
        .iter()
        .any(|item| !ancestry.contains(&item.message.id))
    {
        return Err(ContextAssemblyError::InvalidTimeline);
    }
    Ok(())
}

fn select_timeline<'a>(
    aggregate: &ConversationAggregate,
    request: &'a ContextRequest,
) -> Result<(Vec<&'a TimelineItem>, usize, Vec<&'a TimelineItem>), ContextAssemblyError> {
    let branch = aggregate
        .branches
        .iter()
        .find(|branch| branch.id == request.branch_id)
        .ok_or(ContextAssemblyError::InvalidTimeline)?;
    let head_id = match request.operation {
        GenerationOperation::Continue => branch.head_message_id.or(Some(request.source_message_id)),
        GenerationOperation::Send | GenerationOperation::Regenerate => {
            Some(request.source_message_id)
        }
    };
    let head_index = request
        .timeline
        .iter()
        .position(|item| Some(item.message.id) == head_id)
        .ok_or(ContextAssemblyError::InvalidTimeline)?;
    if !matches!(
        request.timeline[head_index].message.visibility,
        lettuce_conversations::MessageVisibility::Visible
    ) {
        return Err(ContextAssemblyError::InvalidTimeline);
    }
    if matches!(request.operation, GenerationOperation::Continue)
        && branch.head_message_id.is_some()
        && branch.head_message_id != Some(request.source_message_id)
    {
        return Err(ContextAssemblyError::InvalidTimeline);
    }
    let mut ordered = request.timeline.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|left| message_order(left));
    let head_position = ordered
        .iter()
        .position(|item| item.message.id == request.timeline[head_index].message.id)
        .ok_or(ContextAssemblyError::InvalidTimeline)?;
    ordered.truncate(head_position + 1);
    if matches!(request.operation, GenerationOperation::Regenerate) {
        ordered.retain(|item| item.message.id != request.source_message_id);
    }

    let visible = ordered
        .into_iter()
        .filter(|item| {
            !matches!(
                item.message.visibility,
                lettuce_conversations::MessageVisibility::Hidden
                    | lettuce_conversations::MessageVisibility::Tombstoned
            )
        })
        .collect::<Vec<_>>();
    // Scene messages are domain timeline records used to resolve the
    // effective scene and its latest valid edit. They are not transcript
    // messages and must not consume the bounded chat window.
    let scenes = visible
        .iter()
        .filter(|item| item.message.role == MessageRole::Scene)
        .copied()
        .collect::<Vec<_>>();
    let visible = visible
        .into_iter()
        .filter(|item| item.message.role != MessageRole::Scene)
        .collect::<Vec<_>>();
    let mut selected = visible
        .iter()
        .filter(|item| item.message.pinned)
        .copied()
        .collect::<Vec<_>>();
    let mut non_pinned = visible
        .iter()
        .filter(|item| !item.message.pinned)
        .copied()
        .collect::<Vec<_>>();
    let limit = request.window.recent_non_pinned_limit;
    let mut omitted_messages = non_pinned.len().saturating_sub(limit);
    if non_pinned.len() > limit {
        non_pinned = non_pinned.split_off(non_pinned.len() - limit);
    }
    selected.extend(non_pinned);
    selected.sort_by_key(|left| message_order(left));
    selected.dedup_by_key(|item| item.message.id);
    let required_id = match request.operation {
        GenerationOperation::Send => Some(request.source_message_id),
        GenerationOperation::Continue => branch.head_message_id.or(Some(request.source_message_id)),
        GenerationOperation::Regenerate => None,
    };
    if let Some(required_id) = required_id {
        if !selected.iter().any(|item| item.message.id == required_id) {
            let required = request
                .timeline
                .iter()
                .find(|item| item.message.id == required_id)
                .ok_or(ContextAssemblyError::InvalidTimeline)?;
            if omitted_messages > 0
                && !required.message.pinned
                && required.message.role != MessageRole::Scene
            {
                omitted_messages -= 1;
            }
            selected.push(required);
            selected.sort_by_key(|left| message_order(left));
        }
    }
    Ok((selected, omitted_messages, scenes))
}

fn message_order(
    item: &TimelineItem,
) -> (
    lettuce_types::TimestampMillis,
    lettuce_types::TimestampMillis,
    lettuce_types::TimestampMillis,
    MessageId,
) {
    (
        item.message.effective_time,
        item.message.logical_time,
        item.message.created_at,
        item.message.id,
    )
}

struct SnapshotBundle {
    characters: Vec<(ConversationParticipant, CharacterSnapshotBodyV1)>,
    persona: Option<PersonaSnapshotBodyV1>,
    prompt: Option<(PromptLaunchSnapshot, PromptSnapshot)>,
    scene: Option<(SceneLaunchSnapshot, SceneSnapshotBodyV1)>,
    lorebooks: Vec<(LorebookLaunchSnapshot, LorebookSnapshotBodyV1)>,
}

impl SnapshotBundle {
    fn load<M: ConversationSnapshotMaterializer>(
        materializer: &M,
        aggregate: &ConversationAggregate,
        conversation_id: ConversationId,
        settings: &EffectiveConversationSettings,
        selected_speaker: Option<ConversationParticipantId>,
    ) -> Result<Self, ContextAssemblyError> {
        let mut characters = Vec::new();
        match &aggregate.conversation.kind {
            ConversationKind::Direct(details) => {
                let body =
                    materialize_character(materializer, conversation_id, &details.character)?;
                let participant = aggregate
                    .conversation
                    .participants
                    .iter()
                    .find(|participant| {
                        participant.role == lettuce_conversations::ParticipantRole::Character
                    })
                    .cloned()
                    .ok_or(ContextAssemblyError::MissingSpeaker)?;
                characters.push((participant, body));
            }
            ConversationKind::Group(details) => {
                let selected_speaker =
                    selected_speaker.ok_or(ContextAssemblyError::MissingSpeaker)?;
                if !aggregate
                    .conversation
                    .participants
                    .iter()
                    .any(|participant| {
                        participant.id == selected_speaker
                            && participant.role == lettuce_conversations::ParticipantRole::Character
                    })
                {
                    return Err(ContextAssemblyError::MissingSpeaker);
                }
                for member in &details.group.members {
                    let body =
                        materialize_character(materializer, conversation_id, &member.character)?;
                    let participant = aggregate
                        .conversation
                        .participants
                        .iter()
                        .find(|participant| {
                            participant.source
                                == lettuce_conversations::ParticipantSource::Character(
                                    member.character.source_id,
                                )
                        })
                        .cloned()
                        .ok_or(ContextAssemblyError::MissingSpeaker)?;
                    characters.push((participant, body));
                }
            }
        }
        let persona = settings
            .persona
            .as_ref()
            .map(|snapshot| materialize_persona(materializer, conversation_id, snapshot))
            .transpose()?;
        let prompt = settings
            .prompt
            .as_ref()
            .map(|snapshot| {
                materialize_prompt(materializer, conversation_id, snapshot)
                    .and_then(|body| prompt_document(snapshot, &body))
                    .map(|document| (snapshot.clone(), document))
            })
            .transpose()?;
        let scene = settings
            .scene
            .as_ref()
            .map(|snapshot| {
                materialize_scene(materializer, conversation_id, snapshot)
                    .map(|body| (snapshot.clone(), body))
            })
            .transpose()?;
        let lorebooks = settings
            .lorebooks
            .iter()
            .map(|snapshot| {
                materialize_lorebook(materializer, conversation_id, snapshot)
                    .map(|body| (snapshot.clone(), body))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            characters,
            persona,
            prompt,
            scene,
            lorebooks,
        })
    }

    fn scene_values(
        &self,
        timeline: &[&TimelineItem],
    ) -> Result<(String, String), ContextAssemblyError> {
        let Some((snapshot, body)) = self.scene.as_ref() else {
            return Ok((String::new(), String::new()));
        };
        let (mut content, mut direction) = selected_scene_content(body);
        let scene_override = timeline
            .iter()
            .rfind(|item| {
                item.message.scene_edited
                    && item.initial_origin.as_ref().is_some_and(|origin| {
                        matches!(
                            origin,
                            lettuce_conversations::InitialMessageOrigin::SelectedScene {
                                snapshot_ref
                            } if snapshot_ref == &snapshot.snapshot_ref
                        )
                    })
                    && parts(item).iter().any(|part| {
                        matches!(
                            part,
                            MessagePart::Annotation { annotation }
                                if matches!(
                                    annotation.payload,
                                    AnnotationPayload::SceneEdited {
                                        scene_id: Some(scene_id),
                                        ..
                                    } if scene_id == snapshot.source_id
                                )
                        )
                    })
            })
            .map(|item| {
                let mut text = Vec::new();
                let mut override_direction = None;
                for part in parts(item) {
                    match part {
                        MessagePart::Text { text: value } => text.push(value),
                        MessagePart::Annotation { annotation } => {
                            if let AnnotationPayload::SceneEdited {
                                scene_id,
                                direction,
                            } = annotation.payload
                            {
                                if scene_id == Some(snapshot.source_id) {
                                    override_direction = direction;
                                }
                            }
                        }
                        MessagePart::MediaAsset { .. } => {}
                        MessagePart::ReasoningSummary { .. }
                        | MessagePart::ToolCall { .. }
                        | MessagePart::ToolResult { .. } => {}
                    }
                }
                (text.join("\n"), override_direction)
            });
        if let Some((override_content, override_direction)) = scene_override {
            if override_content.trim().is_empty() {
                return Err(ContextAssemblyError::InvalidTimeline);
            }
            content = override_content;
            if let Some(override_direction) = override_direction {
                direction = override_direction;
            }
        }
        Ok((content, direction))
    }
}

fn materialize_character<M: ConversationSnapshotMaterializer>(
    materializer: &M,
    id: ConversationId,
    snapshot: &lettuce_conversations::CharacterLaunchSnapshot,
) -> Result<CharacterSnapshotBodyV1, ContextAssemblyError> {
    snapshot
        .validate()
        .map_err(|_| ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Character,
        })?;
    let body = materializer
        .materialize_character(id, snapshot)
        .map_err(|error| map_snapshot_error(error, SnapshotDocumentKind::Character))?;
    body.validate()
        .map_err(|_| ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Character,
        })?;
    if body.character_id != snapshot.source_id {
        return Err(ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Character,
        });
    }
    Ok(body)
}

fn materialize_persona<M: ConversationSnapshotMaterializer>(
    materializer: &M,
    id: ConversationId,
    snapshot: &lettuce_conversations::PersonaLaunchSnapshot,
) -> Result<PersonaSnapshotBodyV1, ContextAssemblyError> {
    snapshot
        .validate()
        .map_err(|_| ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Persona,
        })?;
    let body = materializer
        .materialize_persona(id, snapshot)
        .map_err(|error| map_snapshot_error(error, SnapshotDocumentKind::Persona))?;
    body.validate()
        .map_err(|_| ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Persona,
        })?;
    if body.persona_id != snapshot.source_id {
        return Err(ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Persona,
        });
    }
    Ok(body)
}

fn materialize_prompt<M: ConversationSnapshotMaterializer>(
    materializer: &M,
    id: ConversationId,
    snapshot: &PromptLaunchSnapshot,
) -> Result<PromptSnapshotBodyV1, ContextAssemblyError> {
    snapshot
        .validate()
        .map_err(|_| ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Prompt,
        })?;
    let body = materializer
        .materialize_prompt(id, snapshot)
        .map_err(|error| map_snapshot_error(error, SnapshotDocumentKind::Prompt))?;
    body.validate()
        .map_err(|_| ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Prompt,
        })?;
    if body.prompt_id != snapshot.source_id {
        return Err(ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Prompt,
        });
    }
    Ok(body)
}

fn materialize_scene<M: ConversationSnapshotMaterializer>(
    materializer: &M,
    id: ConversationId,
    snapshot: &SceneLaunchSnapshot,
) -> Result<SceneSnapshotBodyV1, ContextAssemblyError> {
    snapshot
        .validate()
        .map_err(|_| ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Scene,
        })?;
    let body = materializer
        .materialize_scene(id, snapshot)
        .map_err(|error| map_snapshot_error(error, SnapshotDocumentKind::Scene))?;
    body.validate()
        .map_err(|_| ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Scene,
        })?;
    if body.scene_id != snapshot.source_id {
        return Err(ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Scene,
        });
    }
    Ok(body)
}

fn materialize_lorebook<M: ConversationSnapshotMaterializer>(
    materializer: &M,
    id: ConversationId,
    snapshot: &LorebookLaunchSnapshot,
) -> Result<LorebookSnapshotBodyV1, ContextAssemblyError> {
    snapshot
        .validate()
        .map_err(|_| ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Lorebook,
        })?;
    let body = materializer
        .materialize_lorebook(id, snapshot)
        .map_err(|error| map_snapshot_error(error, SnapshotDocumentKind::Lorebook))?;
    body.validate()
        .map_err(|_| ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Lorebook,
        })?;
    if body.lorebook_id != snapshot.source_id {
        return Err(ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Lorebook,
        });
    }
    Ok(body)
}

fn map_snapshot_error(
    error: lettuce_conversations::ArtifactError,
    kind: SnapshotDocumentKind,
) -> ContextAssemblyError {
    match error {
        lettuce_conversations::ArtifactError::NotFound
        | lettuce_conversations::ArtifactError::Storage => {
            ContextAssemblyError::SnapshotUnavailable { kind }
        }
        _ => ContextAssemblyError::SnapshotInvalid { kind },
    }
}

fn lorebook_source(
    reference: &LorebookLaunchSnapshot,
    body: &LorebookSnapshotBodyV1,
    source_order: usize,
) -> Result<LorebookSnapshotActivationSource, ContextAssemblyError> {
    let entries = body
        .entries
        .iter()
        .map(|entry| LorebookSnapshotActivationEntry {
            entry_id: entry.entry_id,
            title: entry.title.clone(),
            enabled: entry.enabled,
            always_active: entry.always_active,
            keywords: entry.keywords.clone(),
            case_sensitive: entry.case_sensitive,
            match_mode: match entry.match_mode {
                lettuce_conversations::KeywordMatchModeV1::Literal => KeywordMatchMode::Literal,
                lettuce_conversations::KeywordMatchModeV1::Regex => KeywordMatchMode::Regex,
            },
            content: entry.content.clone(),
            priority: entry.priority,
            ordinal: entry.ordinal,
        })
        .collect();
    Ok(LorebookSnapshotActivationSource {
        lorebook_id: reference.source_id,
        root_revision: reference.source_revision,
        source_order,
        detection_policy: match body.detection_policy {
            lettuce_conversations::DetectionPolicyV1::RecentMessageWindow => {
                DetectionPolicy::RecentMessageWindow
            }
            lettuce_conversations::DetectionPolicyV1::LatestUserMessage => {
                DetectionPolicy::LatestUserMessage
            }
        },
        behavior_version: match body.behavior_version {
            lettuce_conversations::LorebookBehaviorVersionV1::LegacyV1 => {
                lettuce_context::LorebookBehaviorVersion::LegacyV1
            }
            lettuce_conversations::LorebookBehaviorVersionV1::DeterministicV2 => {
                lettuce_context::LorebookBehaviorVersion::DeterministicV2
            }
        },
        entries,
    })
}

fn prompt_document(
    reference: &PromptLaunchSnapshot,
    body: &PromptSnapshotBodyV1,
) -> Result<PromptSnapshot, ContextAssemblyError> {
    let entries = body
        .entries
        .iter()
        .map(prompt_entry)
        .collect::<Result<Vec<_>, _>>()?;
    let document = PromptSnapshot {
        id: reference.source_id,
        purpose: prompt_purpose(body.purpose),
        entries,
        condense: body.condense,
        behavior_version: match body.behavior_version {
            lettuce_conversations::PromptBehaviorVersionV1::LegacyV1 => {
                PromptBehaviorVersion::LegacyV1
            }
            lettuce_conversations::PromptBehaviorVersionV1::DeterministicV2 => {
                PromptBehaviorVersion::DeterministicV2
            }
        },
        revision: reference.source_revision,
    };
    document
        .validate()
        .map_err(|_| ContextAssemblyError::SnapshotInvalid {
            kind: SnapshotDocumentKind::Prompt,
        })?;
    Ok(document)
}

fn prompt_entry(
    entry: &lettuce_conversations::PromptEntryV1,
) -> Result<PromptEntry, ContextAssemblyError> {
    Ok(PromptEntry {
        id: entry.entry_id,
        built_in_entry_key: entry.built_in_entry_key.clone(),
        name: entry.name.clone(),
        role: match entry.role {
            PromptEntryRoleV1::System => PromptEntryRole::System,
            PromptEntryRoleV1::User => PromptEntryRole::User,
            PromptEntryRoleV1::Assistant => PromptEntryRole::Assistant,
        },
        content: entry.content.clone(),
        enabled: entry.enabled,
        injection_position: match entry.injection_position {
            PromptEntryPositionV1::Relative => PromptEntryPosition::Relative,
            PromptEntryPositionV1::InChat => PromptEntryPosition::InChat,
            PromptEntryPositionV1::Conditional => PromptEntryPosition::Conditional,
            PromptEntryPositionV1::Interval => PromptEntryPosition::Interval,
        },
        depth: entry.depth,
        conditional_min_messages: entry.conditional_min_messages,
        interval_turns: entry.interval_turns,
        system_prompt: entry.system_prompt,
        conditions: entry.condition.as_ref().map(prompt_condition),
        payload: entry.payload.as_ref().map(prompt_payload).transpose()?,
    })
}

fn prompt_condition(condition: &PromptEntryConditionV1) -> PromptEntryCondition {
    match condition {
        PromptEntryConditionV1::ChatMode { value } => PromptEntryCondition::ChatMode {
            value: match value {
                lettuce_conversations::PromptEntryChatModeV1::Direct => PromptEntryChatMode::Direct,
                lettuce_conversations::PromptEntryChatModeV1::Group => PromptEntryChatMode::Group,
            },
        },
        PromptEntryConditionV1::InfoSource { value } => PromptEntryCondition::InfoSource {
            value: match value {
                lettuce_conversations::PromptEntryInfoSourceV1::Messages => {
                    PromptEntryInfoSource::Messages
                }
                lettuce_conversations::PromptEntryInfoSourceV1::Memory => {
                    PromptEntryInfoSource::Memory
                }
                lettuce_conversations::PromptEntryInfoSourceV1::Mixed => {
                    PromptEntryInfoSource::Mixed
                }
            },
        },
        PromptEntryConditionV1::SceneGenerationEnabled { value } => {
            PromptEntryCondition::SceneGenerationEnabled { value: *value }
        }
        PromptEntryConditionV1::AvatarGenerationEnabled { value } => {
            PromptEntryCondition::AvatarGenerationEnabled { value: *value }
        }
        PromptEntryConditionV1::IsLocalImageGenerationModel { value } => {
            PromptEntryCondition::IsLocalImageGenerationModel { value: *value }
        }
        PromptEntryConditionV1::IsSceneGenerationLocalImageModel { value } => {
            PromptEntryCondition::IsSceneGenerationLocalImageModel { value: *value }
        }
        PromptEntryConditionV1::HasScene { value } => {
            PromptEntryCondition::HasScene { value: *value }
        }
        PromptEntryConditionV1::HasSceneDirection { value } => {
            PromptEntryCondition::HasSceneDirection { value: *value }
        }
        PromptEntryConditionV1::HasPersona { value } => {
            PromptEntryCondition::HasPersona { value: *value }
        }
        PromptEntryConditionV1::MessageCountAtLeast { value } => {
            PromptEntryCondition::MessageCountAtLeast { value: *value }
        }
        PromptEntryConditionV1::ParticipantCountAtLeast { value } => {
            PromptEntryCondition::ParticipantCountAtLeast { value: *value }
        }
        PromptEntryConditionV1::KeywordAny { values } => PromptEntryCondition::KeywordAny {
            values: values.clone(),
        },
        PromptEntryConditionV1::KeywordAll { values } => PromptEntryCondition::KeywordAll {
            values: values.clone(),
        },
        PromptEntryConditionV1::KeywordNone { values } => PromptEntryCondition::KeywordNone {
            values: values.clone(),
        },
        PromptEntryConditionV1::DynamicMemoryEnabled { value } => {
            PromptEntryCondition::DynamicMemoryEnabled { value: *value }
        }
        PromptEntryConditionV1::HasMemorySummary { value } => {
            PromptEntryCondition::HasMemorySummary { value: *value }
        }
        PromptEntryConditionV1::HasKeyMemories { value } => {
            PromptEntryCondition::HasKeyMemories { value: *value }
        }
        PromptEntryConditionV1::HasLorebookContent { value } => {
            PromptEntryCondition::HasLorebookContent { value: *value }
        }
        PromptEntryConditionV1::DoesAuthorNoteExists { value } => {
            PromptEntryCondition::DoesAuthorNoteExists { value: *value }
        }
        PromptEntryConditionV1::HasActiveScheduledNote { value } => {
            PromptEntryCondition::HasActiveScheduledNote { value: *value }
        }
        PromptEntryConditionV1::HasSubjectDescription { value } => {
            PromptEntryCondition::HasSubjectDescription { value: *value }
        }
        PromptEntryConditionV1::HasCurrentDescription { value } => {
            PromptEntryCondition::HasCurrentDescription { value: *value }
        }
        PromptEntryConditionV1::HasCharacterReferenceImages { value } => {
            PromptEntryCondition::HasCharacterReferenceImages { value: *value }
        }
        PromptEntryConditionV1::HasChatBackground { value } => {
            PromptEntryCondition::HasChatBackground { value: *value }
        }
        PromptEntryConditionV1::HasPersonaReferenceImages { value } => {
            PromptEntryCondition::HasPersonaReferenceImages { value: *value }
        }
        PromptEntryConditionV1::HasCharacterReferenceText { value } => {
            PromptEntryCondition::HasCharacterReferenceText { value: *value }
        }
        PromptEntryConditionV1::HasPersonaReferenceText { value } => {
            PromptEntryCondition::HasPersonaReferenceText { value: *value }
        }
        PromptEntryConditionV1::InputScopeAny { values } => PromptEntryCondition::InputScopeAny {
            values: values.clone(),
        },
        PromptEntryConditionV1::OutputScopeAny { values } => PromptEntryCondition::OutputScopeAny {
            values: values.clone(),
        },
        PromptEntryConditionV1::ProviderIdAny { values } => PromptEntryCondition::ProviderIdAny {
            values: values.clone(),
        },
        PromptEntryConditionV1::ReasoningEnabled { value } => {
            PromptEntryCondition::ReasoningEnabled { value: *value }
        }
        PromptEntryConditionV1::VisionEnabled { value } => {
            PromptEntryCondition::VisionEnabled { value: *value }
        }
        PromptEntryConditionV1::IsTimeAwarenessEnabled { value } => {
            PromptEntryCondition::IsTimeAwarenessEnabled { value: *value }
        }
        PromptEntryConditionV1::IsCompanionMode { value } => {
            PromptEntryCondition::IsCompanionMode { value: *value }
        }
        PromptEntryConditionV1::All { conditions } => PromptEntryCondition::All {
            conditions: conditions.iter().map(prompt_condition).collect(),
        },
        PromptEntryConditionV1::Any { conditions } => PromptEntryCondition::Any {
            conditions: conditions.iter().map(prompt_condition).collect(),
        },
        PromptEntryConditionV1::Not { condition } => PromptEntryCondition::Not {
            condition: Box::new(prompt_condition(condition)),
        },
    }
}

fn prompt_payload(
    payload: &PromptEntryPayloadV1,
) -> Result<PromptEntryPayload, ContextAssemblyError> {
    Ok(match payload {
        PromptEntryPayloadV1::ImageSlot { slot } => PromptEntryPayload::ImageSlot {
            slot: match slot {
                PromptEntryImageSlotV1::Character => PromptEntryImageSlot::Character,
                PromptEntryImageSlotV1::Persona => PromptEntryImageSlot::Persona,
                PromptEntryImageSlotV1::ChatBackground => PromptEntryImageSlot::ChatBackground,
                PromptEntryImageSlotV1::Avatar => PromptEntryImageSlot::Avatar,
                PromptEntryImageSlotV1::References => PromptEntryImageSlot::References,
            },
        },
    })
}

fn prompt_purpose(purpose: lettuce_conversations::PromptPurposeV1) -> PromptPurpose {
    match purpose {
        lettuce_conversations::PromptPurposeV1::DirectChat => PromptPurpose::DirectChat,
        lettuce_conversations::PromptPurposeV1::CompanionChat => PromptPurpose::CompanionChat,
        lettuce_conversations::PromptPurposeV1::GroupChatConversational => {
            PromptPurpose::GroupChatConversational
        }
        lettuce_conversations::PromptPurposeV1::GroupChatRoleplay => {
            PromptPurpose::GroupChatRoleplay
        }
        lettuce_conversations::PromptPurposeV1::Undefined => PromptPurpose::Undefined,
        lettuce_conversations::PromptPurposeV1::DynamicMemorySummarizer => {
            PromptPurpose::DynamicMemorySummarizer
        }
        lettuce_conversations::PromptPurposeV1::DynamicMemoryManager => {
            PromptPurpose::DynamicMemoryManager
        }
        lettuce_conversations::PromptPurposeV1::ReplyHelperRoleplay => {
            PromptPurpose::ReplyHelperRoleplay
        }
        lettuce_conversations::PromptPurposeV1::ReplyHelperConversational => {
            PromptPurpose::ReplyHelperConversational
        }
        lettuce_conversations::PromptPurposeV1::LorebookEntryWriter => {
            PromptPurpose::LorebookEntryWriter
        }
        lettuce_conversations::PromptPurposeV1::LorebookKeywordGenerator => {
            PromptPurpose::LorebookKeywordGenerator
        }
        lettuce_conversations::PromptPurposeV1::LorebookGeneratorPlanner => {
            PromptPurpose::LorebookGeneratorPlanner
        }
        lettuce_conversations::PromptPurposeV1::LorebookGeneratorWriter => {
            PromptPurpose::LorebookGeneratorWriter
        }
        lettuce_conversations::PromptPurposeV1::LorebookGeneratorRefine => {
            PromptPurpose::LorebookGeneratorRefine
        }
        lettuce_conversations::PromptPurposeV1::LorebookGeneratorCoherence => {
            PromptPurpose::LorebookGeneratorCoherence
        }
        lettuce_conversations::PromptPurposeV1::AvatarGeneration => PromptPurpose::AvatarGeneration,
        lettuce_conversations::PromptPurposeV1::AvatarEditRequest => {
            PromptPurpose::AvatarEditRequest
        }
        lettuce_conversations::PromptPurposeV1::SceneGeneration => PromptPurpose::SceneGeneration,
        lettuce_conversations::PromptPurposeV1::ScenePromptWriter => {
            PromptPurpose::ScenePromptWriter
        }
        lettuce_conversations::PromptPurposeV1::DesignReferenceWriter => {
            PromptPurpose::DesignReferenceWriter
        }
        lettuce_conversations::PromptPurposeV1::CompanionSoulWriter => {
            PromptPurpose::CompanionSoulWriter
        }
        lettuce_conversations::PromptPurposeV1::CompanionGrowthcycle => {
            PromptPurpose::CompanionGrowthcycle
        }
        lettuce_conversations::PromptPurposeV1::CompanionConsolidation => {
            PromptPurpose::CompanionConsolidation
        }
    }
}

fn selected_scene_content(body: &SceneSnapshotBodyV1) -> (String, String) {
    let (parts, direction) = body
        .selected_variant_id
        .and_then(|id| {
            body.variants
                .iter()
                .find(|variant| variant.variant_id == id)
        })
        .map(|variant| {
            (
                &variant.content,
                variant.direction.clone().or_else(|| body.direction.clone()),
            )
        })
        .unwrap_or((&body.content, body.direction.clone()));
    (
        parts
            .iter()
            .filter_map(|part| match part {
                ScenePartV1::Text { text } => Some(text.as_str()),
                ScenePartV1::InlineAsset { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        direction.unwrap_or_default(),
    )
}

#[allow(clippy::too_many_arguments)]
fn prompt_conditions(
    aggregate: &ConversationAggregate,
    request: &ContextRequest,
    snapshot: &SnapshotBundle,
    settings: &EffectiveConversationSettings,
    lorebook_text: &str,
    scene: &str,
    scene_direction: &str,
    recent_text: &[String],
    selected_message_count: usize,
    companion_mode_enabled: bool,
    has_active_scheduled_note: bool,
) -> PromptConditionContext {
    let runtime = &request.prompt_runtime;
    let character = selected_character(snapshot, request);
    let recent = recent_text.join("\n");
    PromptConditionContext {
        chat_mode: if aggregate.conversation.kind.is_group() {
            PromptEntryChatMode::Group
        } else {
            PromptEntryChatMode::Direct
        },
        info_source: if request.memory.is_some() {
            PromptEntryInfoSource::Mixed
        } else {
            PromptEntryInfoSource::Messages
        },
        scene_generation_enabled: runtime.scene_generation_enabled,
        avatar_generation_enabled: runtime.avatar_generation_enabled,
        is_local_image_generation_model: runtime.is_local_image_generation_model,
        is_scene_generation_local_image_model: runtime.is_scene_generation_local_image_model,
        has_scene: !scene.trim().is_empty(),
        has_scene_direction: !scene_direction.trim().is_empty(),
        has_persona: snapshot.persona.is_some() && settings.persona.is_some(),
        message_count: selected_message_count,
        participant_count: aggregate.conversation.participants.len(),
        recent_text: recent,
        dynamic_memory_enabled: runtime.dynamic_memory_enabled,
        has_memory_summary: request.memory.as_ref().is_some_and(|memory| {
            memory
                .summary
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
        }),
        has_key_memories: request
            .memory
            .as_ref()
            .is_some_and(|memory| !memory.key_memories.is_empty()),
        has_lorebook_content: !lorebook_text.trim().is_empty(),
        does_author_note_exists: settings
            .author_note
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty()),
        has_active_scheduled_note,
        has_subject_description: character
            .and_then(|body| body.description.as_ref())
            .is_some_and(|value| !value.trim().is_empty()),
        has_current_description: character
            .and_then(|body| body.description.as_ref())
            .is_some_and(|value| !value.trim().is_empty()),
        has_character_reference_images: character.is_some_and(|body| !body.media.is_empty()),
        has_chat_background: false,
        has_persona_reference_images: snapshot
            .persona
            .as_ref()
            .is_some_and(|body| !body.media.is_empty()),
        has_character_reference_text: character
            .and_then(|body| body.design_description.as_ref())
            .is_some_and(|value| !value.trim().is_empty()),
        has_persona_reference_text: snapshot
            .persona
            .as_ref()
            .and_then(|body| body.design_description.as_ref())
            .is_some_and(|value| !value.trim().is_empty()),
        input_scopes: runtime.input_scopes.clone(),
        output_scopes: runtime.output_scopes.clone(),
        provider_id: runtime.provider_id.clone(),
        reasoning_enabled: request.capabilities.reasoning
            == lettuce_models::CapabilityStatus::Supported,
        vision_enabled: request.capabilities.input_modalities.image
            == lettuce_models::CapabilityStatus::Supported,
        time_awareness_enabled: runtime.time_awareness_enabled,
        companion_mode_enabled: companion_mode_enabled || runtime.companion_mode_enabled,
    }
}

#[allow(clippy::too_many_arguments)]
fn prompt_values(
    aggregate: &ConversationAggregate,
    snapshot: &SnapshotBundle,
    settings: &EffectiveConversationSettings,
    scene: &str,
    scene_direction: &str,
    lorebook_text: &str,
    request: &ContextRequest,
    swap_roles: bool,
    companion_state: Option<&str>,
    scheduled_notes: Option<&str>,
) -> PromptRenderValues {
    let character = selected_character(snapshot, request);
    let user = aggregate
        .conversation
        .participants
        .iter()
        .find(|participant| participant.role == lettuce_conversations::ParticipantRole::User);
    let character_description = character
        .map(|body| {
            [
                body.description.as_deref(),
                body.definition.as_deref(),
                body.design_description.as_deref(),
            ]
            .into_iter()
            .flatten()
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
        })
        .unwrap_or_default();
    let authored_user_name = user
        .map(|participant| participant.display_name.clone())
        .unwrap_or_default();
    let authored_user_description = user
        .and_then(|participant| participant.authored_description.clone())
        .unwrap_or_default();
    let persona_name = snapshot
        .persona
        .as_ref()
        .map(|body| body.title.clone())
        .unwrap_or_else(|| authored_user_name.clone());
    let persona_description = snapshot
        .persona
        .as_ref()
        .map(|body| {
            [
                Some(body.description.as_str()),
                body.design_description.as_deref(),
            ]
            .into_iter()
            .flatten()
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
        })
        .unwrap_or_else(|| authored_user_description.clone());
    let selected_name = request
        .selected_speaker
        .as_ref()
        .and_then(|decision| {
            snapshot
                .characters
                .iter()
                .find(|(participant, _)| participant.id == decision.participant_id)
        })
        .map(|(_, body)| body.name.clone())
        .unwrap_or_else(|| character.map(|body| body.name.clone()).unwrap_or_default());
    let swap_roles = swap_roles && !aggregate.conversation.kind.is_group();
    let (character_name, character_description, persona_name, persona_description) = if swap_roles {
        (
            persona_name.clone(),
            persona_description.clone(),
            selected_name.clone(),
            character_description.clone(),
        )
    } else {
        (
            selected_name.clone(),
            character_description,
            persona_name,
            persona_description,
        )
    };
    let (user_name, user_description, ai_name, ai_description) = if swap_roles {
        (
            selected_name.clone(),
            character_description.clone(),
            snapshot
                .persona
                .as_ref()
                .map(|body| body.title.clone())
                .unwrap_or_else(|| authored_user_name.clone()),
            snapshot
                .persona
                .as_ref()
                .map(|body| {
                    [
                        Some(body.description.as_str()),
                        body.design_description.as_deref(),
                    ]
                    .into_iter()
                    .flatten()
                    .filter(|value| !value.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n")
                })
                .unwrap_or_else(|| authored_user_description.clone()),
        )
    } else {
        (
            authored_user_name,
            authored_user_description,
            selected_name.clone(),
            character_description.clone(),
        )
    };
    let mut values = PromptRenderValues {
        character_name,
        character_description,
        persona_name,
        persona_description,
        scene: scene.to_owned(),
        scene_direction: scene_direction.to_owned(),
        lorebook: lorebook_text.to_owned(),
        author_note: settings.author_note.clone().unwrap_or_default(),
        content_rules: request
            .prompt_values
            .content_rules
            .clone()
            .unwrap_or_default(),
        context_summary: request
            .memory
            .as_ref()
            .and_then(|memory| memory.summary.clone())
            .unwrap_or_default(),
        key_memories: request
            .memory
            .as_ref()
            .map(|memory| memory.key_memories.join("\n"))
            .unwrap_or_default(),
        user_name,
        user_description,
        ai_description,
        ai_name,
        ..PromptRenderValues::default()
    };
    for (variable, value) in [
        (
            PromptVariable::CompanionState,
            companion_state
                .map(str::to_owned)
                .or_else(|| request.prompt_values.companion_state.clone()),
        ),
        (
            PromptVariable::ScheduledNotes,
            scheduled_notes.map(str::to_owned),
        ),
        (PromptVariable::Date, request.prompt_values.date.clone()),
        (
            PromptVariable::DateFull,
            request.prompt_values.date_full.clone(),
        ),
        (
            PromptVariable::Weekday,
            request.prompt_values.weekday.clone(),
        ),
        (
            PromptVariable::TimeHour,
            request.prompt_values.time_hour.clone(),
        ),
        (
            PromptVariable::TimeMinute,
            request.prompt_values.time_minute.clone(),
        ),
        (
            PromptVariable::TimeSecond,
            request.prompt_values.time_second.clone(),
        ),
        (
            PromptVariable::TimeFull,
            request.prompt_values.time_full.clone(),
        ),
        (
            PromptVariable::Time12HourFormat,
            request.prompt_values.time_12hour_format.clone(),
        ),
        (
            PromptVariable::TimeTimezone,
            request.prompt_values.time_timezone.clone(),
        ),
        (
            PromptVariable::TimeTimezoneName,
            request.prompt_values.time_timezone_name.clone(),
        ),
        (
            PromptVariable::DatetimeIso,
            request.prompt_values.datetime_iso.clone(),
        ),
    ] {
        if let Some(value) = value {
            values.purpose_values.insert(variable, value);
        }
    }
    if aggregate.conversation.kind.is_group() {
        values.purpose_values.insert(
            PromptVariable::GroupCharacters,
            snapshot
                .characters
                .iter()
                .map(|(_, body)| body.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    values
}

fn selected_character<'a>(
    snapshot: &'a SnapshotBundle,
    request: &ContextRequest,
) -> Option<&'a CharacterSnapshotBodyV1> {
    request
        .selected_speaker
        .as_ref()
        .and_then(|decision| {
            snapshot
                .characters
                .iter()
                .find(|(participant, _)| participant.id == decision.participant_id)
                .map(|(_, body)| body)
        })
        .or_else(|| snapshot.characters.first().map(|(_, body)| body))
}

#[allow(clippy::type_complexity)]
fn prompt_messages(
    rendered: &lettuce_context::RenderedPrompt,
) -> Result<
    (
        Vec<ProviderNeutralMessage>,
        Vec<(u32, ProviderNeutralMessage)>,
    ),
    ContextAssemblyError,
> {
    let messages = rendered
        .relative
        .iter()
        .map(rendered_message)
        .collect::<Result<Vec<_>, _>>()?;
    let in_chat = rendered
        .in_chat
        .iter()
        .map(|entry| rendered_message(entry).map(|message| (entry.depth, message)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((messages, in_chat))
}

fn insert_in_chat_messages(
    messages: &mut Vec<ProviderNeutralMessage>,
    in_chat: Vec<(u32, ProviderNeutralMessage)>,
) {
    let base_len = messages.len();
    // Rendered prompt order is the stable tie-break for equal insertion depth.
    let mut inserts = in_chat
        .into_iter()
        .enumerate()
        .map(|(index, (depth, message))| (base_len.saturating_sub(depth as usize), index, message))
        .collect::<Vec<_>>();
    inserts.sort_by_key(|(position, index, _)| (*position, *index));
    for (offset, (position, _, message)) in inserts.into_iter().enumerate() {
        messages.insert((position + offset).min(messages.len()), message);
    }
}

fn rendered_message(
    entry: &RenderedPromptMessage,
) -> Result<ProviderNeutralMessage, ContextAssemblyError> {
    if entry.payload.is_some() {
        return Err(ContextAssemblyError::UnsupportedPart);
    }
    Ok(ProviderNeutralMessage {
        role: match entry.role {
            PromptEntryRole::System => MessageRole::System,
            PromptEntryRole::User => MessageRole::User,
            PromptEntryRole::Assistant => MessageRole::Assistant,
        },
        parts: vec![ProviderContextPart::Text {
            text: entry.content.trim().to_owned(),
        }],
    })
}

fn text_message(role: MessageRole, text: &str) -> ProviderNeutralMessage {
    ProviderNeutralMessage {
        role,
        parts: vec![ProviderContextPart::Text {
            text: text.trim().to_owned(),
        }],
    }
}

fn condense_prompt_messages(messages: &mut Vec<ProviderNeutralMessage>) {
    let mut condensed: Vec<ProviderNeutralMessage> = Vec::with_capacity(messages.len());
    for message in messages.drain(..) {
        if let Some(last) = condensed.last_mut()
            && last.role == message.role
            && last.parts.len() == 1
            && message.parts.len() == 1
            && let (
                ProviderContextPart::Text { text: left },
                ProviderContextPart::Text { text: right },
            ) = (&mut last.parts[0], &message.parts[0])
        {
            if !left.is_empty() && !right.is_empty() {
                left.push_str("\n\n");
            }
            left.push_str(right);
            continue;
        }
        condensed.push(message);
    }
    *messages = condensed;
}

fn author_note_consumed(
    rendered: &lettuce_context::RenderedPrompt,
    prompt: Option<&(PromptLaunchSnapshot, PromptSnapshot)>,
) -> bool {
    rendered_entry_consumed(rendered, prompt, &["{{author_note}}"])
}

fn lorebook_consumed(
    rendered: &lettuce_context::RenderedPrompt,
    prompt: Option<&(PromptLaunchSnapshot, PromptSnapshot)>,
) -> bool {
    rendered_entry_consumed(rendered, prompt, &["{{lorebook}}"])
}

fn rendered_entry_consumed(
    rendered: &lettuce_context::RenderedPrompt,
    prompt: Option<&(PromptLaunchSnapshot, PromptSnapshot)>,
    needles: &[&str],
) -> bool {
    let Some((_, document)) = prompt else {
        return false;
    };
    rendered
        .relative
        .iter()
        .chain(rendered.in_chat.iter())
        .any(|rendered| {
            document
                .entries
                .iter()
                .find(|entry| entry.id == rendered.entry_id)
                .is_some_and(|entry| {
                    !rendered.content.trim().is_empty()
                        && needles.iter().any(|needle| entry.content.contains(needle))
                })
        })
}

fn provider_message(
    item: &TimelineItem,
    aggregate: &ConversationAggregate,
    character_names: &HashMap<ConversationParticipantId, String>,
    selected_speaker: Option<ConversationParticipantId>,
    swap_roles: bool,
) -> Result<Option<ProviderNeutralMessage>, ContextAssemblyError> {
    if item.message.role == MessageRole::Scene {
        return Ok(None);
    }
    let role = match (&aggregate.conversation.kind, item.message.role) {
        (ConversationKind::Direct(_), MessageRole::User) if swap_roles => MessageRole::Assistant,
        (ConversationKind::Direct(_), MessageRole::Assistant) if swap_roles => MessageRole::User,
        (ConversationKind::Group(_), MessageRole::Assistant | MessageRole::User) => {
            if item.message.author_participant_id == selected_speaker {
                MessageRole::Assistant
            } else {
                MessageRole::User
            }
        }
        (_, role) => role,
    };
    let mut context_parts = provider_context_parts(parts(item))?;
    if matches!(aggregate.conversation.kind, ConversationKind::Group(_))
        && role == MessageRole::User
        && item
            .message
            .author_participant_id
            .is_some_and(|id| character_names.contains_key(&id))
    {
        let participant_id = item
            .message
            .author_participant_id
            .ok_or(ContextAssemblyError::InvalidTimeline)?;
        let character_name = character_names
            .get(&participant_id)
            .ok_or(ContextAssemblyError::MissingSpeaker)?;
        let prefix = format!("{character_name}: ");
        if let Some(ProviderContextPart::Text { text }) = context_parts.first_mut() {
            *text = format!("{prefix}{text}");
        } else {
            context_parts.insert(0, ProviderContextPart::Text { text: prefix });
        }
    }
    if context_parts.is_empty() {
        return Ok(None);
    }
    Ok(Some(ProviderNeutralMessage {
        role,
        parts: context_parts,
    }))
}

fn provider_context_parts(
    parts: Vec<MessagePart>,
) -> Result<Vec<ProviderContextPart>, ContextAssemblyError> {
    let mut context_parts = Vec::new();
    for part in parts {
        match part {
            MessagePart::Text { text } => context_parts.push(ProviderContextPart::Text { text }),
            MessagePart::MediaAsset { asset_id, role } => {
                context_parts.push(ProviderContextPart::MediaAsset { asset_id, role })
            }
            MessagePart::ReasoningSummary { .. } => {}
            MessagePart::Annotation { annotation }
                if matches!(annotation.payload, AnnotationPayload::SceneEdited { .. }) => {}
            MessagePart::ToolCall { .. }
            | MessagePart::ToolResult { .. }
            | MessagePart::Annotation { .. } => return Err(ContextAssemblyError::UnsupportedPart),
        }
    }
    Ok(context_parts)
}

fn parts(item: &TimelineItem) -> Vec<MessagePart> {
    match item.message.active_render_source {
        MessageRenderSource::Revision(_) => item
            .active_revision
            .as_ref()
            .map(|revision| revision.parts.clone())
            .unwrap_or_default(),
        MessageRenderSource::Candidate(_) => item
            .active_candidate
            .as_ref()
            .map(|candidate| candidate.parts.clone())
            .unwrap_or_default(),
    }
}

fn active_text(item: &&TimelineItem) -> Option<String> {
    let text = parts(item)
        .into_iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    (!text.trim().is_empty()).then_some(text)
}

fn lore_attributions(
    activation: &lettuce_context::MultiLorebookSnapshotActivation,
) -> Vec<LorebookAttribution> {
    activation
        .sources
        .iter()
        .filter_map(|source| {
            let entries = activation
                .entries
                .iter()
                .filter(|entry| entry.source.lorebook_id == source.lorebook_id)
                .map(|entry| entry.entry.entry_id)
                .collect::<Vec<_>>();
            (!entries.is_empty()).then_some(LorebookAttribution {
                lorebook_id: source.lorebook_id,
                revision: source.root_revision,
                activated_entry_ids: entries,
            })
        })
        .collect()
}

fn budget_report(
    messages: &[ProviderNeutralMessage],
    omitted: usize,
) -> Result<ContextBudgetReport, ContextAssemblyError> {
    let input_bytes = messages
        .iter()
        .flat_map(|message| message.parts.iter())
        .map(|part| match part {
            ProviderContextPart::Text { text } => text.len(),
            ProviderContextPart::MediaAsset { .. } => 16,
            ProviderContextPart::ToolCall(call) => {
                call.name.len()
                    + call.provider_call_id.as_ref().map_or(0, String::len)
                    + serde_json::to_vec(&call.arguments).map_or(0, |value| value.len())
            }
            ProviderContextPart::ToolResult(result) => {
                result.name.len()
                    + result.provider_call_id.as_ref().map_or(0, String::len)
                    + serde_json::to_vec(&result.output.value).map_or(0, |value| value.len())
            }
        })
        .sum::<usize>();
    if input_bytes > 16 * 1024 * 1024 || input_bytes > u32::MAX as usize {
        return Err(ContextAssemblyError::SizeLimit);
    }
    Ok(ContextBudgetReport {
        selected_messages: messages
            .len()
            .try_into()
            .map_err(|_| ContextAssemblyError::SizeLimit)?,
        omitted_messages: omitted
            .try_into()
            .map_err(|_| ContextAssemblyError::SizeLimit)?,
        input_bytes: input_bytes as u32,
        estimated_input_tokens: (input_bytes as u64)
            .div_ceil(4)
            .try_into()
            .map_err(|_| ContextAssemblyError::SizeLimit)?,
        truncated: omitted > 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_context::{PromptEntryImageSlot, PromptEntryPayload, RenderedPrompt};
    use lettuce_types::{CharacterId, LorebookEntryId, LorebookId, PromptEntryId, SceneId};

    #[test]
    fn provider_history_omits_reasoning_without_dropping_visible_text() {
        assert_eq!(
            provider_context_parts(vec![
                MessagePart::ReasoningSummary {
                    text: "private chain".into(),
                },
                MessagePart::Text {
                    text: "visible answer".into(),
                },
            ]),
            Ok(vec![ProviderContextPart::Text {
                text: "visible answer".into(),
            }])
        );
        assert_eq!(
            provider_context_parts(vec![MessagePart::ReasoningSummary {
                text: "private chain".into(),
            }]),
            Ok(Vec::new())
        );
    }

    #[test]
    fn in_chat_entries_are_inserted_by_depth_with_stable_order() {
        let relative = ProviderNeutralMessage {
            role: MessageRole::System,
            parts: vec![ProviderContextPart::Text {
                text: "rules".into(),
            }],
        };
        let transcript = ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::Text {
                text: "head".into(),
            }],
        };
        let mut messages = vec![relative, transcript];
        insert_in_chat_messages(
            &mut messages,
            vec![
                (
                    1,
                    ProviderNeutralMessage {
                        role: MessageRole::System,
                        parts: vec![ProviderContextPart::Text {
                            text: "near".into(),
                        }],
                    },
                ),
                (
                    0,
                    ProviderNeutralMessage {
                        role: MessageRole::System,
                        parts: vec![ProviderContextPart::Text {
                            text: "latest".into(),
                        }],
                    },
                ),
            ],
        );
        let texts = messages
            .iter()
            .map(|message| match &message.parts[0] {
                ProviderContextPart::Text { text } => text.clone(),
                ProviderContextPart::MediaAsset { .. } => "media".into(),
                ProviderContextPart::ToolCall(_) => "tool_call".into(),
                ProviderContextPart::ToolResult(_) => "tool_result".into(),
            })
            .collect::<Vec<_>>();
        assert_eq!(texts, ["rules", "near", "head", "latest"]);
    }

    #[test]
    fn prompt_payloads_are_rejected_in_provider_neutral_context() {
        let rendered = RenderedPrompt {
            relative: vec![lettuce_context::RenderedPromptMessage {
                entry_id: PromptEntryId::new(),
                role: PromptEntryRole::System,
                content: String::new(),
                depth: 0,
                payload: Some(PromptEntryPayload::ImageSlot {
                    slot: PromptEntryImageSlot::Character,
                }),
            }],
            ..RenderedPrompt::default()
        };
        assert_eq!(
            prompt_messages(&rendered),
            Err(ContextAssemblyError::UnsupportedPart)
        );
    }

    #[test]
    fn lorebook_attribution_contains_only_activated_entry_ids() {
        let first_book = LorebookId::new();
        let second_book = LorebookId::new();
        let first_entry = LorebookEntryId::new();
        let second_entry = LorebookEntryId::new();
        let source = |book, order| lettuce_context::ResolvedLorebookSnapshotSource {
            lorebook_id: book,
            root_revision: lettuce_types::Revision::INITIAL,
            source_order: order,
            detection_policy: DetectionPolicy::RecentMessageWindow,
            behavior_version: lettuce_context::LorebookBehaviorVersion::LegacyV1,
        };
        let first = source(first_book, 0);
        let second = source(second_book, 1);
        let entry = |id, source| lettuce_context::ResolvedLorebookSnapshotEntry {
            entry: LorebookSnapshotActivationEntry {
                entry_id: id,
                title: "entry".into(),
                enabled: true,
                always_active: true,
                keywords: Vec::new(),
                case_sensitive: false,
                match_mode: KeywordMatchMode::Literal,
                content: "body".into(),
                priority: 0,
                ordinal: 0,
            },
            source,
            matched_keywords: Vec::new(),
            always_active: true,
        };
        let activation = lettuce_context::MultiLorebookSnapshotActivation {
            entries: vec![
                entry(first_entry, first.clone()),
                entry(second_entry, second.clone()),
            ],
            sources: vec![first, second],
            activated_lorebook_ids: vec![first_book, second_book],
            activated_entry_ids: vec![first_entry, second_entry],
        };
        let attribution = lore_attributions(&activation);
        assert_eq!(attribution.len(), 2);
        assert_eq!(attribution[0].activated_entry_ids, vec![first_entry]);
        assert_eq!(attribution[1].activated_entry_ids, vec![second_entry]);
    }

    #[test]
    fn budget_reports_bounded_estimate_and_window_truncation() {
        let messages = vec![text_message(MessageRole::User, "12345678")];
        let report = budget_report(&messages, 3).expect("budget");
        assert_eq!(report.input_bytes, 8);
        assert_eq!(report.estimated_input_tokens, 2);
        assert_eq!(report.omitted_messages, 3);
        assert!(report.truncated);
    }

    #[test]
    fn operation_instruction_is_explicit_except_continue_after_non_user_head() {
        assert_eq!(
            operation_instruction(
                GenerationOperation::Regenerate,
                None,
                Some(MessageRole::Assistant)
            ),
            None
        );
        assert_eq!(
            operation_instruction(
                GenerationOperation::Regenerate,
                Some("retry"),
                Some(MessageRole::Assistant)
            ),
            Some("retry".into())
        );
        assert_eq!(
            operation_instruction(GenerationOperation::Continue, None, Some(MessageRole::User)),
            None
        );
        assert_eq!(
            operation_instruction(
                GenerationOperation::Continue,
                None,
                Some(MessageRole::Assistant)
            ),
            Some("Continue the conversation from the current head.".into())
        );
    }

    #[test]
    fn ancestry_rejects_a_parent_branch_message_after_the_fork() {
        let conversation_id = ConversationId::new();
        let root_branch = lettuce_types::ConversationBranchId::new();
        let child_branch = lettuce_types::ConversationBranchId::new();
        let root_message = MessageId::new();
        let child_message = MessageId::new();
        let injected = MessageId::new();
        let item = |id, branch_id, parent_message_id| TimelineItem {
            message: lettuce_conversations::Message {
                id,
                conversation_id,
                branch_id,
                parent_message_id,
                author_participant_id: None,
                role: MessageRole::System,
                logical_time: lettuce_types::TimestampMillis::UNIX_EPOCH,
                effective_time: lettuce_types::TimestampMillis::UNIX_EPOCH,
                visibility: lettuce_conversations::MessageVisibility::Visible,
                pinned: false,
                scene_edited: false,
                active_render_source: MessageRenderSource::Revision(
                    lettuce_types::MessageRevisionId::new(),
                ),
                revision: lettuce_types::Revision::INITIAL,
                created_at: lettuce_types::TimestampMillis::UNIX_EPOCH,
                updated_at: lettuce_types::TimestampMillis::UNIX_EPOCH,
            },
            active_revision: None,
            active_candidate: None,
            initial_origin: None,
        };
        let request = ContextRequest {
            conversation_id,
            branch_id: child_branch,
            branch_path: vec![root_branch, child_branch],
            source_message_id: child_message,
            operation: GenerationOperation::Continue,
            swap_roles: false,
            guidance: None,
            window: lettuce_conversations::ContextWindowPolicy::default(),
            selected_speaker: None,
            capabilities: lettuce_models::ModelCapabilities::default(),
            safety: lettuce_conversations::SafetyContext::Standard,
            prompt_runtime: lettuce_conversations::PromptRuntimeFacts::default(),
            prompt_values: lettuce_conversations::PromptRuntimeValues::default(),
            memory: None,
            timeline: vec![
                item(root_message, root_branch, None),
                item(child_message, child_branch, Some(root_message)),
                item(injected, root_branch, Some(child_message)),
            ],
        };
        let branches = vec![
            lettuce_conversations::ConversationBranch {
                id: root_branch,
                conversation_id,
                parent_branch_id: None,
                fork_message_id: None,
                head_message_id: None,
                status: BranchStatus::Active,
                revision: lettuce_types::Revision::INITIAL,
                created_at: lettuce_types::TimestampMillis::UNIX_EPOCH,
                updated_at: lettuce_types::TimestampMillis::UNIX_EPOCH,
            },
            lettuce_conversations::ConversationBranch {
                id: child_branch,
                conversation_id,
                parent_branch_id: Some(root_branch),
                fork_message_id: Some(root_message),
                head_message_id: Some(child_message),
                status: BranchStatus::Active,
                revision: lettuce_types::Revision::INITIAL,
                created_at: lettuce_types::TimestampMillis::UNIX_EPOCH,
                updated_at: lettuce_types::TimestampMillis::UNIX_EPOCH,
            },
        ];
        assert_eq!(
            validate_message_ancestry(&branches, &request),
            Err(ContextAssemblyError::InvalidTimeline)
        );
    }

    #[test]
    fn scene_variant_and_latest_valid_edit_are_resolved_without_transcript_emission() {
        let scene_id = SceneId::new();
        let variant_id = lettuce_types::SceneVariantId::new();
        let snapshot = SceneLaunchSnapshot {
            snapshot_ref: lettuce_conversations::ProtectedSnapshotRef {
                source: lettuce_conversations::SnapshotSource::Scene(scene_id),
                source_revision: lettuce_types::Revision::INITIAL,
                artifact_id: lettuce_types::SnapshotArtifactId::new(),
                digest: lettuce_types::ContentHash::parse("ab".repeat(32)).expect("digest"),
                schema_version: 1,
                byte_size: 1,
            },
            source_id: scene_id,
            source_revision: lettuce_types::Revision::INITIAL,
            title: "Opening".into(),
        };
        let body = SceneSnapshotBodyV1 {
            scene_id,
            owner: lettuce_conversations::SceneOwnerV1::Character(CharacterId::new()),
            ordinal: 0,
            content: vec![ScenePartV1::Text {
                text: "base scene".into(),
            }],
            direction: Some("base direction".into()),
            selected_variant_id: Some(variant_id),
            variants: vec![lettuce_conversations::SceneVariantBodyV1 {
                variant_id,
                ordinal: 0,
                content: vec![ScenePartV1::Text {
                    text: "selected scene".into(),
                }],
                direction: Some("selected direction".into()),
            }],
            assets: Vec::new(),
        };
        let message_id = MessageId::new();
        let revision_id = lettuce_types::MessageRevisionId::new();
        let mut item = TimelineItem {
            message: lettuce_conversations::Message {
                id: message_id,
                conversation_id: ConversationId::new(),
                branch_id: lettuce_types::ConversationBranchId::new(),
                parent_message_id: None,
                author_participant_id: None,
                role: MessageRole::Scene,
                logical_time: lettuce_types::TimestampMillis::UNIX_EPOCH,
                effective_time: lettuce_types::TimestampMillis::UNIX_EPOCH,
                visibility: lettuce_conversations::MessageVisibility::Visible,
                pinned: false,
                scene_edited: true,
                active_render_source: MessageRenderSource::Revision(revision_id),
                revision: lettuce_types::Revision::INITIAL,
                created_at: lettuce_types::TimestampMillis::UNIX_EPOCH,
                updated_at: lettuce_types::TimestampMillis::UNIX_EPOCH,
            },
            active_revision: Some(lettuce_conversations::MessageRevision {
                id: revision_id,
                message_id,
                sequence: lettuce_types::Revision::INITIAL,
                parts: vec![
                    MessagePart::Annotation {
                        // The scene revision carries the edited scene text; the
                        // annotation carries the optional direction override.
                        annotation: lettuce_conversations::Annotation {
                            version: 1,
                            kind: lettuce_conversations::AnnotationKind::SceneEdited,
                            payload: AnnotationPayload::SceneEdited {
                                scene_id: Some(scene_id),
                                direction: Some("edited direction".into()),
                            },
                        },
                    },
                    MessagePart::Text {
                        text: "edited scene".into(),
                    },
                ],
                authored_at: lettuce_types::TimestampMillis::UNIX_EPOCH,
                source_turn_id: None,
                provider_replay: None,
            }),
            active_candidate: None,
            initial_origin: Some(lettuce_conversations::InitialMessageOrigin::SelectedScene {
                snapshot_ref: snapshot.snapshot_ref.clone(),
            }),
        };
        let bundle = SnapshotBundle {
            characters: Vec::new(),
            persona: None,
            prompt: None,
            scene: Some((snapshot, body)),
            lorebooks: Vec::new(),
        };
        let (scene, direction) = bundle.scene_values(&[&item]).expect("scene");
        assert_eq!(scene, "edited scene");
        assert_eq!(direction, "edited direction");

        let MessagePart::Annotation { annotation } =
            &mut item.active_revision.as_mut().expect("revision").parts[0]
        else {
            panic!("scene annotation");
        };
        annotation.payload = AnnotationPayload::SceneEdited {
            scene_id: Some(SceneId::new()),
            direction: Some("wrong scene direction".into()),
        };
        let (scene, direction) = bundle.scene_values(&[&item]).expect("scene");
        assert_eq!(scene, "selected scene");
        assert_eq!(direction, "selected direction");
    }
}
