//! Provider-neutral context assembly.
//!
//! This module is intentionally the last application-side step before model
//! admission.  It reads the conversation aggregate, its launch snapshots, and
//! the live prompts, persona and lorebooks each turn uses; provider selection,
//! memory retrieval, and inference belong to later application ports.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use lettuce_characters::{CharacterRepository, GroupRepository, PersonaRepository};
use lettuce_companions::{
    CompanionPromptStateInput, CompanionScheduledNoteRepository, CompanionStateRepository,
    SoulOwner, SoulRepository, active_scheduled_notes, prompt_state,
};
use lettuce_context::{
    CharacterLorebookBindingRepository, GroupLorebookBindingRepository, LorebookActivationSource,
    LorebookRepository, LorebookSourceProvenance, PersonaLorebookBindingRepository,
    PromptBehaviorVersion, PromptConditionContext, PromptEntry, PromptEntryChatMode,
    PromptEntryCondition, PromptEntryImageSlot, PromptEntryInfoSource, PromptEntryPayload,
    PromptEntryPosition, PromptEntryRole, PromptPurpose, PromptRenderContext, PromptRenderValues,
    PromptSnapshot, PromptVariable, RenderedPromptMessage, ResolvedLorebookEntry,
    render_prompt_snapshot, resolve_lorebook_activation,
};
use lettuce_context::{PromptRepository, RenderedPrompt, render_prompt};
use lettuce_conversations::{
    AnnotationPayload, BranchStatus, ContextAssemblyError, ContextAttributions,
    ContextBudgetReport, ContextRequest, ConversationAggregate, ConversationKind,
    ConversationReader, ConversationSnapshotMaterializer, EffectiveConversationSettings,
    GenerationOperation, LorebookAttribution, MemoryPromptLine, MessagePart, MessageRenderSource,
    MessageRole, PromptAttribution, ProviderContextPart, ProviderNeutralContext,
    ProviderNeutralMessage, SettingProvenance, SnapshotDocumentBody, SnapshotDocumentKind,
    SnapshotSelection, TimelineItem,
};
use lettuce_conversations::{
    CharacterSnapshotBodyV1, ConversationParticipant, PersonaSnapshotBodyV1,
    PromptEntryConditionV1, PromptEntryImageSlotV1, PromptEntryPayloadV1, PromptEntryPositionV1,
    PromptEntryRoleV1, PromptLaunchSnapshot, PromptSnapshotBodyV1, SceneLaunchSnapshot,
    ScenePartV1, SceneSnapshotBodyV1,
};
use lettuce_types::{
    CharacterId, ConversationId, ConversationParticipantId, MessageId, PersonaId, TimestampMillis,
};

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
        + CompanionScheduledNoteRepository
        + PromptRepository
        + lettuce_settings::GlobalSettingsStore
        + GroupRepository
        + LorebookRepository
        + CharacterLorebookBindingRepository
        + PersonaLorebookBindingRepository
        + GroupLorebookBindingRepository,
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

        let mut settings = lettuce_conversations::resolve_effective_settings(
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

        let direct = matches!(aggregate.conversation.kind, ConversationKind::Direct(_));
        settings.prompt = None;
        let mut snapshot = SnapshotBundle::load(
            self.sources,
            &aggregate,
            request.conversation_id,
            &settings,
            request
                .selected_speaker
                .as_ref()
                .map(|speaker| speaker.participant_id),
        )?;
        let speaker_character = speaker_character(&aggregate.conversation, &request);
        let unavailable = |_| ContextAssemblyError::ConversationUnavailable;
        let live_group =
            crate::generation::live_sources::live_group(self.sources, &aggregate.conversation)
                .map_err(unavailable)?;
        let persona = crate::generation::live_sources::live_persona(
            self.sources,
            &aggregate.conversation,
            live_group.as_ref().and_then(|group| group.profile.as_ref()),
        )
        .map_err(unavailable)?;
        snapshot.persona = persona.as_ref().map(crate::launch::documents::persona_body);
        snapshot.prompt = match &live_group {
            None => self.live_direct_prompt(&aggregate.conversation, &snapshot)?,
            Some(group) => {
                self.live_group_prompt(&aggregate.conversation, group, speaker_character)?
            }
        };
        snapshot.read_live_characters(self.sources, live_group.as_ref())?;
        let TimelineSelection {
            window: selected_window,
            omitted_messages,
            scenes: scene_timeline,
            history,
            visible,
        } = select_timeline(&aggregate.branches, &request)?;
        let (mut scene, mut scene_direction) = snapshot.scene_values(&scene_timeline)?;
        if let Some(group) = &live_group {
            if group.chat_mode == lettuce_conversations::GroupChatModeSnapshot::Conversation {
                scene.clear();
                scene_direction.clear();
            }
            scene = resolve_member_mentions(&scene, &snapshot.group_members);
        }
        let effective_at = source_effective_time(&request)?;
        let companion_state = self.companion_prompt_state(&aggregate, effective_at)?;
        let scheduled_notes = self.companion_scheduled_notes(&aggregate, effective_at)?;

        let keyword_window = if direct { &visible } else { &history };
        let recent_text = keyword_window
            .iter()
            .filter_map(active_text)
            .rev()
            .take(lettuce_context::LEGACY_RECENT_MESSAGE_LIMIT)
            .collect::<Vec<_>>();
        let recent_text = recent_text.into_iter().rev().collect::<Vec<_>>();
        let latest_user_message = history
            .iter()
            .rev()
            .filter(|item| item.message.role == MessageRole::User)
            .find_map(active_text);

        let mut lore_entries: Vec<ResolvedLorebookEntry> = Vec::new();
        for tier in self.live_lorebook_tiers(
            &aggregate.conversation,
            speaker_character,
            persona.as_ref().map(|persona| persona.id),
            live_group
                .as_ref()
                .is_some_and(|group| group.disable_character_lorebooks),
        )? {
            let activation =
                resolve_lorebook_activation(&tier, &recent_text, latest_user_message.as_deref())
                    .map_err(|_| ContextAssemblyError::LorebookActivation)?;
            for entry in activation.entries {
                if !lore_entries
                    .iter()
                    .any(|active| active.entry.id == entry.entry.id)
                {
                    lore_entries.push(entry);
                }
            }
        }
        let lorebook_text = lore_entries
            .iter()
            .map(|entry| entry.entry.content.trim())
            .filter(|content| !content.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");

        let group = matches!(aggregate.conversation.kind, ConversationKind::Group(_));
        let conditions = prompt_conditions(
            &aggregate,
            &request,
            &snapshot,
            &settings,
            &lorebook_text,
            &scene,
            &scene_direction,
            &recent_text,
            request
                .prompt_runtime
                .conversation_message_count
                .unwrap_or(selected_window.len()),
            companion_state.is_some(),
            scheduled_notes.is_some(),
        );
        let memory_text = request
            .memory
            .as_ref()
            .filter(|memory| !memory.key_memories.is_empty())
            .map(|_| {
                crate::generation::runtime_text::RuntimeText::load(
                    self.sources,
                    crate::BuiltInPromptId::MemoryRuntime,
                )
                .map_err(|_| ContextAssemblyError::RuntimeTextUnavailable)
            })
            .transpose()?;
        let memory_lines = |lines: &[MemoryPromptLine], observed: bool| {
            let Some(text) = memory_text.as_ref() else {
                return Ok(String::new());
            };
            lines
                .iter()
                .map(|line| crate::memory::memory_prompt::render_memory_line(text, line, observed))
                .collect::<Result<Vec<_>, _>>()
                .map(|lines| {
                    lines
                        .into_iter()
                        .filter(|line| !line.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .map_err(runtime_text_error)
        };
        let key_lines = request
            .memory
            .as_ref()
            .map(|memory| memory_lines(&memory.key_memories, !group))
            .transpose()?
            .unwrap_or_default();
        let memory_summary = request
            .memory
            .as_ref()
            .and_then(|memory| memory.summary.as_deref())
            .unwrap_or_default()
            .trim()
            .to_owned();
        let mut runtime_values = prompt_values(
            &aggregate,
            &snapshot,
            &settings,
            &scene,
            &scene_direction,
            lorebook_text.trim(),
            &request,
            request.swap_roles,
            companion_state.as_deref(),
            scheduled_notes.as_deref(),
        );
        runtime_values.author_note = runtime_values.author_note.trim().to_owned();
        runtime_values.context_summary = memory_summary.clone();
        runtime_values.key_memories = key_lines.clone();
        resolve_substituted_values(&mut runtime_values, group);
        let names = PromptRenderValues {
            character_name: runtime_values.character_name.clone(),
            persona_name: runtime_values.persona_name.clone(),
            user_name: runtime_values.user_name.clone(),
            ..PromptRenderValues::default()
        };
        let guidance = request
            .guidance
            .as_deref()
            .map(str::trim)
            .filter(|guidance| !guidance.is_empty());
        if let Some(guidance) = guidance {
            runtime_values
                .purpose_values
                .insert(PromptVariable::RegenerateGuidance, guidance.to_owned());
        }
        let group_user_name = runtime_values.persona_name.clone();
        let runtime = RuntimeSections::render(
            self.sources,
            &PromptRenderContext {
                conditions: conditions.clone(),
                values: runtime_values,
            },
        )?;

        let (prompt, rendered_prompt) = if let Some(document) = snapshot.prompt.as_ref() {
            let mut values = prompt_values(
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
            );
            for (variable, binding, filler) in [
                (
                    PromptVariable::CharacterLoraKeywords,
                    &request.prompt_values.character_scene_lora,
                    "scene_lora_primary_subject",
                ),
                (
                    PromptVariable::PersonaLoraKeywords,
                    &request.prompt_values.persona_scene_lora,
                    "scene_lora_secondary_subject",
                ),
            ] {
                let value = match binding {
                    Some(lettuce_conversations::SceneLoraBinding::Keywords(keywords)) => {
                        keywords.clone()
                    }
                    Some(lettuce_conversations::SceneLoraBinding::NoLora) => {
                        runtime.fragment(filler, [])?.unwrap_or_default()
                    }
                    None => continue,
                };
                values.purpose_values.insert(variable, value);
            }
            values.key_memories = match runtime.section("runtime_group_key_memories") {
                Some(section) if group && !key_lines.is_empty() => section.text,
                _ if group => String::new(),
                _ => key_lines.clone(),
            };
            if direct {
                values
                    .purpose_values
                    .retain(|variable, _| variable.is_allowed_for(document.purpose));
            } else {
                let list = group_characters(&snapshot.group_members, speaker_character, &runtime)?;
                let list = values.resolve_names(&list);
                values
                    .purpose_values
                    .insert(PromptVariable::GroupCharacters, list);
            }
            resolve_substituted_values(&mut values, group);
            let without_scene;
            let rendered_document = if direct
                && settings.scene.is_none()
                && !scene_timeline
                    .iter()
                    .any(|item| active_text(item).is_some())
            {
                without_scene = PromptSnapshot {
                    entries: document
                        .entries
                        .iter()
                        .filter(|entry| !has_scene_placeholder(&entry.content))
                        .cloned()
                        .collect(),
                    ..document.clone()
                };
                &without_scene
            } else {
                document
            };
            let render_context = PromptRenderContext { conditions, values };
            let rendered =
                render_prompt_snapshot(rendered_document, &render_context).map_err(|error| {
                    tracing::warn!(?error, "prompt snapshot rendering failed");
                    ContextAssemblyError::PromptRender
                })?;
            (Some(document), rendered)
        } else {
            (None, Default::default())
        };

        let (mut messages, in_chat) = prompt_messages(&rendered_prompt)?;
        let mut placement = Placement {
            turn_context: rendered_prompt
                .in_chat
                .iter()
                .map(|entry| {
                    !prompt
                        .and_then(|document| {
                            document
                                .entries
                                .iter()
                                .find(|authored| authored.id == entry.entry_id)
                        })
                        .is_some_and(|authored| {
                            matches!(
                                authored.injection_position,
                                PromptEntryPosition::Conditional | PromptEntryPosition::Interval
                            )
                        })
                })
                .collect(),
            in_chat,
            relative: Vec::new(),
        };
        let mut place = |section: Option<RuntimeSection>| placement.place(section);
        let summary_placeholder = template_has_placeholder(prompt, "{{context_summary}}");
        let keys_placeholder = template_has_placeholder(prompt, "{{key_memories}}");
        let memory_used = if group {
            (!memory_summary.is_empty() && summary_placeholder)
                || (!key_lines.is_empty() && keys_placeholder)
        } else {
            !memory_summary.is_empty() || !key_lines.is_empty()
        };
        if !group && !memory_summary.is_empty() && !summary_placeholder {
            place(runtime.section("runtime_context_summary"));
        }
        if !group && !key_lines.is_empty() && !keys_placeholder {
            place(runtime.section("runtime_key_memories"));
        }
        if !lorebook_text.trim().is_empty() && !template_has_placeholder(prompt, "{{lorebook}}") {
            place(runtime.section("runtime_world_information"));
        }
        let author_note = settings.author_note.as_deref().unwrap_or_default();
        if !author_note.trim().is_empty() && !template_has_placeholder(prompt, "{{author_note}}") {
            place(runtime.section(if group {
                "runtime_group_author_note"
            } else {
                "runtime_author_note"
            }));
        }
        if companion_state.is_some() && !template_has_placeholder(prompt, "{{companion_state}}") {
            place(runtime.section("runtime_companion_state"));
        }
        if scheduled_notes.is_some() && !template_has_placeholder(prompt, "{{scheduled_notes}}") {
            place(runtime.section("runtime_scheduled_notes"));
        }
        let condense_point = prompt
            .is_some_and(|document| document.condense)
            .then_some((placement.relative.len(), placement.in_chat.len()));
        let mut place = |section: Option<RuntimeSection>| placement.place(section);
        if request.swap_roles && !group {
            place(runtime.section("runtime_swap_places"));
        }
        let selected_speaker = request
            .selected_speaker
            .as_ref()
            .map(|speaker| speaker.participant_id);
        let last_message = selected_window
            .iter()
            .rev()
            .map(|item| &item.message)
            .find(|message| message.role != MessageRole::Scene);
        if group {
            if selected_window.is_empty() {
                place(runtime.section("runtime_group_begin"));
            }
            if request.operation == GenerationOperation::Continue
                && last_message.is_some_and(|message| {
                    message.role == MessageRole::Assistant
                        && message.author_participant_id == selected_speaker
                })
            {
                place(runtime.section("runtime_group_continue_same_speaker"));
            }
        } else if request.operation == GenerationOperation::Continue
            && last_message.is_none_or(|message| message.role != MessageRole::User)
        {
            place(runtime.section("runtime_continue_instruction"));
        }
        if guidance.is_some() {
            place(runtime.section("runtime_regenerate_instruction"));
        }
        let Placement {
            relative: mut runtime_relative,
            mut in_chat,
            turn_context,
        } = placement;
        if let Some((relative_end, in_chat_end)) = condense_point {
            let mut relative = std::mem::take(&mut messages);
            relative.extend(runtime_relative.drain(..relative_end));
            let mut late = Vec::new();
            let mut post = Vec::new();
            let mut kept = Vec::new();
            for (index, (depth, message)) in in_chat.drain(..).enumerate() {
                if index >= in_chat_end {
                    post.push((depth, message));
                } else if group || (depth == 0 && turn_context[index]) {
                    late.push(message);
                } else {
                    kept.push((depth, message));
                }
            }
            if group {
                relative.append(&mut late);
            }
            messages.extend(condensed_system_message(relative));
            in_chat = kept;
            in_chat.extend(condensed_system_message(late).map(|message| (0, message)));
            in_chat.extend(post);
        }
        if direct
            && request.operation == GenerationOperation::Send
            && !request.prompt_runtime.dynamic_memory_enabled
            && !key_lines.is_empty()
        {
            if let Some(section) = runtime.section("runtime_relevant_memories") {
                in_chat.insert(0, (section.depth.unwrap_or_default(), section.message));
            }
        }
        messages.append(&mut runtime_relative);

        let character_names = snapshot
            .characters
            .iter()
            .map(|(participant, body)| (participant.id, body.name.clone()))
            .collect::<HashMap<_, _>>();
        let mut transcript = Vec::new();
        for item in &selected_window {
            if let Some(message) = provider_message(
                item,
                &aggregate,
                &character_names,
                selected_speaker,
                request.swap_roles,
                &runtime,
                &group_user_name,
                !group && request.prompt_runtime.time_awareness_enabled,
            )? {
                transcript.push(message);
            }
        }
        insert_in_chat_messages(&mut transcript, in_chat);
        messages.append(&mut transcript);
        if direct {
            for part in messages.iter_mut().flat_map(|message| &mut message.parts) {
                if let ProviderContextPart::Text { text } = part {
                    *text = names.resolve_names(text);
                }
            }
        }

        let attributions = ContextAttributions {
            prompt: prompt.map(|document| PromptAttribution {
                document_id: document.id,
                revision: document.revision,
                selected_entry_ids: rendered_prompt
                    .relative
                    .iter()
                    .chain(rendered_prompt.in_chat.iter())
                    .map(|entry| entry.entry_id)
                    .collect(),
            }),
            lorebooks: lore_attributions(&lore_entries),
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
        + CompanionScheduledNoteRepository
        + PromptRepository
        + lettuce_settings::GlobalSettingsStore
        + GroupRepository
        + LorebookRepository
        + CharacterLorebookBindingRepository
        + PersonaLorebookBindingRepository
        + GroupLorebookBindingRepository,
{
    /// The system prompt of a direct chat, resolved from live sources on
    /// every turn (legacy `build_system_prompt_entries`); the stored launch
    /// and override snapshots only record which prompt the chat selected.
    /// A companion chat follows `policy::companion_prompt` and ignores the
    /// selection. Any other direct chat follows `policy::direct_prompt`: the
    /// chat's selection, then the live character's direct prompt, then the app
    /// default chain; a prompt the chat disabled yields none. The selection is
    /// a current override, else the launch prompt when the launch pinned one
    /// like legacy's session template: a starter's explicit prompt, or an
    /// inherited prompt that is the character's direct prompt in the launch
    /// character snapshot, so it runs before the bundle's characters are
    /// replaced with their live records. A launch that fell back to the app
    /// default chain pinned nothing, as legacy left the session template empty.
    fn live_direct_prompt(
        &self,
        conversation: &lettuce_conversations::Conversation,
        snapshot: &SnapshotBundle,
    ) -> Result<Option<PromptSnapshot>, ContextAssemblyError> {
        let ConversationKind::Direct(details) = &conversation.kind else {
            return Ok(None);
        };
        let companion =
            crate::companion::companion_clock::companion_clock_context(self.sources, conversation)
                .map_err(|_| ContextAssemblyError::ConversationUnavailable)?
                .companion;
        let character = CharacterRepository::get(self.sources, details.character.source_id)
            .map_err(|_| ContextAssemblyError::ConversationUnavailable)?
            .ok_or(ContextAssemblyError::ConversationUnavailable)?;
        let unavailable = ContextAssemblyError::SnapshotUnavailable {
            kind: SnapshotDocumentKind::Prompt,
        };
        let app_default = lettuce_settings::GlobalSettingsStore::load(self.sources)
            .map_err(|_| unavailable)?
            .default_prompt_document_id;
        let defaults = &character.character.defaults;
        let document = if companion {
            Some(
                crate::launch::policy::companion_prompt(
                    self.sources,
                    defaults.companion_soul.as_ref(),
                    app_default,
                )
                .map_err(|_| unavailable)?
                .ok_or(unavailable)?,
            )
        } else {
            let current = conversation
                .current_settings
                .as_ref()
                .map(|settings| (settings.prompt_provenance, settings.prompt.as_ref()));
            let selected = match current {
                Some((SettingProvenance::Disabled, _)) => return Ok(None),
                Some((SettingProvenance::CurrentOverride, prompt)) => {
                    prompt.map(|prompt| prompt.source_id)
                }
                _ => match &details.prompt {
                    SnapshotSelection::Explicit(prompt) => Some(prompt.source_id),
                    SnapshotSelection::Inherited(prompt)
                        if snapshot.characters.first().is_some_and(|(_, character)| {
                            character.direct_prompt_id == Some(prompt.source_id)
                        }) =>
                    {
                        Some(prompt.source_id)
                    }
                    SnapshotSelection::Inherited(_) | SnapshotSelection::Disabled => None,
                },
            };
            crate::launch::policy::direct_prompt(
                self.sources,
                selected,
                defaults.direct_prompt_id,
                app_default,
            )
            .map_err(|_| unavailable)?
        };
        document
            .map(|document| {
                prompt_document(
                    document.id,
                    document.revision,
                    &crate::launch::documents::prompt_body(&document),
                )
            })
            .transpose()
    }

    /// The prompt a group speaker generates with, read live each turn like
    /// legacy's group template lookup: the conversation's own selection, the
    /// speaker's group prompt, then the group's (`policy::group_prompt`). A
    /// prompt the conversation disabled yields none.
    fn live_group_prompt(
        &self,
        conversation: &lettuce_conversations::Conversation,
        live: &crate::generation::live_sources::LiveGroup,
        speaker: Option<CharacterId>,
    ) -> Result<Option<PromptSnapshot>, ContextAssemblyError> {
        let unavailable = || ContextAssemblyError::ConversationUnavailable;
        let selected = match conversation
            .current_settings
            .as_ref()
            .map(|settings| (settings.prompt_provenance, settings.prompt.as_ref()))
        {
            Some((SettingProvenance::Disabled, _)) => return Ok(None),
            Some((SettingProvenance::CurrentOverride, prompt)) => {
                prompt.map(|prompt| prompt.source_id)
            }
            _ => None,
        };
        let roleplay = live.chat_mode == lettuce_conversations::GroupChatModeSnapshot::Roleplay;
        let member = speaker
            .map(|id| CharacterRepository::get(self.sources, id))
            .transpose()
            .map_err(|_| unavailable())?
            .flatten()
            .and_then(|character| {
                let defaults = character.character.defaults;
                if roleplay {
                    defaults.group_roleplay_prompt_id
                } else {
                    defaults.group_conversation_prompt_id
                }
            });
        let group = live.profile.as_ref().and_then(|group| {
            if roleplay {
                group.group_roleplay_prompt_id
            } else {
                group.group_conversation_prompt_id
            }
        });
        crate::launch::policy::group_prompt(self.sources, live.chat_mode, [selected, member, group])
            .map_err(|_| unavailable())?
            .map(|document| {
                prompt_document(
                    document.id,
                    document.revision,
                    &crate::launch::documents::prompt_body(&document),
                )
            })
            .transpose()
    }

    /// The lorebooks a turn activates, read live each turn in ordered tiers.
    /// A direct chat has one tier (legacy `get_lorebook_content`): the chat's
    /// own selection, else the character's enabled bindings then the persona's.
    /// A group chat has two (legacy `get_group_active_lorebook_entries`): the
    /// conversation's own selection, else the group's bindings; then the
    /// speaker's bindings unless the group disables character lorebooks. A
    /// disabled selection is an empty own selection, as legacy's empty
    /// `lorebook_ids`. Missing and archived books are skipped by the activation.
    fn live_lorebook_tiers(
        &self,
        conversation: &lettuce_conversations::Conversation,
        speaker: Option<CharacterId>,
        persona: Option<PersonaId>,
        disable_character_lorebooks: bool,
    ) -> Result<Vec<Vec<LorebookActivationSource>>, ContextAssemblyError> {
        let own_provenance = LorebookSourceProvenance::Conversation {
            id: conversation.id,
        };
        let own = match conversation
            .current_settings
            .as_ref()
            .map(|settings| (settings.lorebooks_provenance, settings.lorebooks.as_ref()))
        {
            Some((SettingProvenance::Disabled, _)) => Some(Vec::new()),
            Some((SettingProvenance::CurrentOverride, books)) => Some(
                books
                    .into_iter()
                    .flatten()
                    .map(|book| (own_provenance, book.source_id))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        };
        let binding_error = |_| ContextAssemblyError::ConversationUnavailable;
        let enabled = |bindings: Vec<lettuce_context::LorebookBinding>,
                       provenance: LorebookSourceProvenance| {
            crate::launch::policy::enabled_lorebooks(&bindings)
                .into_iter()
                .map(move |id| (provenance, id))
        };
        let tiers = match &conversation.kind {
            ConversationKind::Direct(details) => {
                let books = match (own, &details.lorebooks) {
                    (Some(books), _) => books,
                    (None, SnapshotSelection::Disabled) => Vec::new(),
                    (None, SnapshotSelection::Explicit(books)) => books
                        .iter()
                        .map(|book| (own_provenance, book.source_id))
                        .collect(),
                    (None, SnapshotSelection::Inherited(_)) => {
                        let character = details.character.source_id;
                        let mut books = enabled(
                            self.sources
                                .list_character_bindings(character)
                                .map_err(binding_error)?,
                            LorebookSourceProvenance::Character { id: character },
                        )
                        .collect::<Vec<_>>();
                        if let Some(persona) = persona {
                            books.extend(enabled(
                                self.sources
                                    .list_persona_bindings(persona)
                                    .map_err(binding_error)?,
                                LorebookSourceProvenance::Persona { id: persona },
                            ));
                        }
                        books
                    }
                };
                vec![books]
            }
            ConversationKind::Group(details) => {
                let group_id = details.group.source_id;
                let group_books = match (own, &details.group.lorebooks) {
                    (Some(books), _) => books,
                    (None, SnapshotSelection::Disabled) => Vec::new(),
                    (None, _) => enabled(
                        self.sources
                            .list_group_bindings(group_id)
                            .map_err(binding_error)?,
                        LorebookSourceProvenance::Group { id: group_id },
                    )
                    .collect(),
                };
                let speaker_books = match speaker {
                    Some(character) if !disable_character_lorebooks => enabled(
                        self.sources
                            .list_character_bindings(character)
                            .map_err(binding_error)?,
                        LorebookSourceProvenance::Character { id: character },
                    )
                    .collect(),
                    _ => Vec::new(),
                };
                vec![group_books, speaker_books]
            }
        };
        tiers
            .into_iter()
            .map(|tier| {
                tier.into_iter()
                    .map(|(provenance, lorebook_id)| {
                        Ok(LorebookActivationSource {
                            provenance,
                            lorebook_id,
                            details: LorebookRepository::get(self.sources, lorebook_id)
                                .map_err(|_| ContextAssemblyError::ConversationUnavailable)?,
                        })
                    })
                    .collect()
            })
            .collect()
    }

    /// The companion state block of a companion chat (legacy
    /// `is_companion_mode`, see `companion_clock`). The relationship and the
    /// partner are those of the persona the chat uses now; a chat, Soul or
    /// episode that has no stored state yet renders the defaults of the
    /// character's companion settings.
    fn companion_prompt_state(
        &self,
        aggregate: &ConversationAggregate,
        effective_at: TimestampMillis,
    ) -> Result<Option<String>, ContextAssemblyError> {
        let Some(owner) =
            crate::companion::companion_clock::companion_state_owner(&aggregate.conversation)
        else {
            return Ok(None);
        };
        if !crate::companion::companion_clock::companion_clock_context(
            self.sources,
            &aggregate.conversation,
        )
        .map_err(|_| ContextAssemblyError::ConversationUnavailable)?
        .companion
        {
            return Ok(None);
        }
        let character = CharacterRepository::get(self.sources, owner.character_id)
            .map_err(|_| ContextAssemblyError::ConversationUnavailable)?
            .ok_or(ContextAssemblyError::ConversationUnavailable)?;
        let config = character
            .character
            .defaults
            .companion_soul
            .unwrap_or_default();
        let persona_id = owner.persona_id;
        let state = CompanionStateRepository::get(self.sources, owner)
            .map_err(|_| ContextAssemblyError::ConversationUnavailable)?
            .map_or_else(
                || {
                    lettuce_companions::initial_runtime_state(
                        &config.soul.baseline_affect,
                        &config.soul.regulation_style,
                        &config.relationship_defaults,
                    )
                },
                |snapshot| snapshot.state,
            );
        let episode_index = CompanionStateRepository::get_continuity_episode(
            self.sources,
            aggregate.conversation.id,
        )
        .map_err(|_| ContextAssemblyError::ConversationUnavailable)?
        .filter(|episode| episode.character_id == character.character.id)
        .map_or(0, |episode| episode.episode_index);
        let soul = SoulRepository::get(
            self.sources,
            SoulOwner::for_conversation(
                character.character.id,
                aggregate.conversation.id,
                config.share_soul_growth_across_chats,
            ),
        )
        .map_err(|_| ContextAssemblyError::ConversationUnavailable)?
        .map_or_else(
            || lettuce_companions::initial_soul_state(Some(&config), effective_at),
            Ok,
        )
        .map_err(|_| ContextAssemblyError::ConversationUnavailable)?;
        let partner_name = persona_id
            .map(|id| PersonaRepository::get(self.sources, id))
            .transpose()
            .map_err(|_| ContextAssemblyError::ConversationUnavailable)?
            .flatten()
            .map(|persona| persona.title);
        let facts = prompt_state(&CompanionPromptStateInput {
            soul: &config.soul,
            soul_state: &soul,
            runtime_state: &state,
            style_notes: &config.prompting.style_notes,
            continuity_episode: episode_index,
            effective_at,
        });
        crate::companion::companion_prompt_text::render_companion_state(
            &companion_runtime_text(self.sources)?,
            &character.character.profile.name,
            partner_name.as_deref(),
            &facts,
        )
        .map(|state| Some(state).filter(|state| !state.is_empty()))
        .map_err(runtime_text_error)
    }

    fn companion_scheduled_notes(
        &self,
        aggregate: &ConversationAggregate,
        effective_at: TimestampMillis,
    ) -> Result<Option<String>, ContextAssemblyError> {
        let ConversationKind::Direct(details) = &aggregate.conversation.kind else {
            return Ok(None);
        };
        if !crate::companion::companion_clock::companion_clock_context(
            self.sources,
            &aggregate.conversation,
        )
        .map_err(|_| ContextAssemblyError::ConversationUnavailable)?
        .companion
        {
            return Ok(None);
        }
        let notes = self
            .sources
            .list_scheduled_notes(details.character.source_id)
            .map_err(|_| ContextAssemblyError::ConversationUnavailable)?;
        let active = active_scheduled_notes(notes, effective_at)
            .map_err(|_| ContextAssemblyError::ConversationUnavailable)?;
        if active.is_empty() {
            return Ok(None);
        }
        crate::companion::companion_prompt_text::render_scheduled_notes(
            &companion_runtime_text(self.sources)?,
            &active,
        )
        .map_err(runtime_text_error)
    }
}

fn companion_runtime_text<S: PromptRepository + ?Sized>(
    sources: &S,
) -> Result<crate::generation::runtime_text::RuntimeText, ContextAssemblyError> {
    crate::generation::runtime_text::RuntimeText::load(
        sources,
        crate::BuiltInPromptId::CompanionRuntime,
    )
    .map_err(|_| ContextAssemblyError::RuntimeTextUnavailable)
}

fn runtime_text_error(
    error: crate::generation::runtime_text::RuntimeTextError,
) -> ContextAssemblyError {
    match error {
        crate::generation::runtime_text::RuntimeTextError::Unavailable => {
            ContextAssemblyError::RuntimeTextUnavailable
        }
        crate::generation::runtime_text::RuntimeTextError::Render => {
            ContextAssemblyError::PromptRender
        }
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

struct RuntimeSection {
    text: String,
    message: ProviderNeutralMessage,
    depth: Option<u32>,
}

/// The built-in chat runtime prompt, rendered once per turn. Rust decides
/// which of its sections a turn injects; their text, role and placement come
/// from the catalog document, so a user edit or disabled entry is honored.
struct RuntimeSections {
    text: crate::generation::runtime_text::RuntimeText,
    rendered: RenderedPrompt,
}

impl RuntimeSections {
    fn render<S: PromptRepository + ?Sized>(
        sources: &S,
        context: &PromptRenderContext,
    ) -> Result<Self, ContextAssemblyError> {
        let text = crate::generation::runtime_text::RuntimeText::load(
            sources,
            crate::BuiltInPromptId::ChatRuntime,
        )
        .map_err(|_| ContextAssemblyError::RuntimeTextUnavailable)?;
        let rendered = render_prompt(text.document(), context).map_err(|error| {
            tracing::warn!(?error, "chat runtime prompt rendering failed");
            ContextAssemblyError::PromptRender
        })?;
        Ok(Self { text, rendered })
    }

    /// A rendered fragment, or `None` when its entry was removed or disabled.
    fn fragment(
        &self,
        key: &str,
        variables: impl IntoIterator<Item = (PromptVariable, String)>,
    ) -> Result<Option<String>, ContextAssemblyError> {
        let mut values = PromptRenderValues::default();
        values.purpose_values.extend(variables);
        self.text
            .render(key, &values)
            .map_err(|_| ContextAssemblyError::PromptRender)
    }

    fn section(&self, key: &str) -> Option<RuntimeSection> {
        let entry_id = self
            .text
            .document()
            .entries
            .iter()
            .find(|entry| entry.built_in_entry_key.as_deref() == Some(key))?
            .id;
        let relative = self
            .rendered
            .relative
            .iter()
            .find(|message| message.entry_id == entry_id)
            .map(|message| (message, None));
        let (message, depth) = relative.or_else(|| {
            self.rendered
                .in_chat
                .iter()
                .find(|message| message.entry_id == entry_id)
                .map(|message| (message, Some(message.depth)))
        })?;
        let provider_message = rendered_message(message).ok()?;
        Some(RuntimeSection {
            text: message.content.trim().to_owned(),
            message: provider_message,
            depth,
        })
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
/// window.
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

struct TimelineSelection<'a> {
    window: Vec<&'a TimelineItem>,
    omitted_messages: usize,
    scenes: Vec<&'a TimelineItem>,
    history: Vec<&'a TimelineItem>,
    /// Every visible message, scene messages included, in conversation order.
    visible: Vec<&'a TimelineItem>,
}

fn select_timeline<'a>(
    branches: &[lettuce_conversations::ConversationBranch],
    request: &'a ContextRequest,
) -> Result<TimelineSelection<'a>, ContextAssemblyError> {
    let branch = branches
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
    let head_visibility = request.timeline[head_index].message.visibility;
    let hidden_continue_head = matches!(request.operation, GenerationOperation::Continue)
        && head_visibility == lettuce_conversations::MessageVisibility::Hidden;
    if head_visibility != lettuce_conversations::MessageVisibility::Visible && !hidden_continue_head
    {
        return Err(ContextAssemblyError::InvalidTimeline);
    }
    if matches!(request.operation, GenerationOperation::Continue)
        && branch.head_message_id.is_some()
        && branch.head_message_id != Some(request.source_message_id)
    {
        return Err(ContextAssemblyError::InvalidTimeline);
    }
    let mut ordered = conversation_order(request, request.timeline[head_index].message.id)?;
    let positions = ordered
        .iter()
        .enumerate()
        .map(|(position, item)| (item.message.id, position))
        .collect::<HashMap<_, _>>();
    let message_order = |item: &&TimelineItem| positions.get(&item.message.id).copied();
    if matches!(request.operation, GenerationOperation::Regenerate) {
        ordered.retain(|item| item.message.id != request.source_message_id);
    }

    let all_visible = ordered
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
    let scenes = all_visible
        .iter()
        .filter(|item| item.message.role == MessageRole::Scene)
        .copied()
        .collect::<Vec<_>>();
    let history = all_visible
        .iter()
        .filter(|item| item.message.role != MessageRole::Scene)
        .copied()
        .collect::<Vec<_>>();
    let visible = &history;
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
    selected.sort_by_key(message_order);
    selected.dedup_by_key(|item| item.message.id);
    let required_id = match request.operation {
        GenerationOperation::Send => Some(request.source_message_id),
        GenerationOperation::Continue if hidden_continue_head => None,
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
            selected.sort_by_key(message_order);
        }
    }
    Ok(TimelineSelection {
        window: selected,
        omitted_messages,
        scenes,
        history,
        visible: all_visible,
    })
}

/// The parent chain ending at `head_id`, root first: the conversation order
/// legacy sent history in. Message times never reorder it, so a message
/// stamped earlier by a moved companion clock keeps its place.
fn conversation_order(
    request: &ContextRequest,
    head_id: MessageId,
) -> Result<Vec<&TimelineItem>, ContextAssemblyError> {
    let by_id = request
        .timeline
        .iter()
        .map(|item| (item.message.id, item))
        .collect::<HashMap<_, _>>();
    let mut chain = Vec::new();
    let mut current = Some(head_id);
    while let Some(message_id) = current {
        if chain.len() >= by_id.len() {
            return Err(ContextAssemblyError::InvalidTimeline);
        }
        let item = *by_id
            .get(&message_id)
            .ok_or(ContextAssemblyError::InvalidTimeline)?;
        chain.push(item);
        current = item.message.parent_message_id;
    }
    chain.reverse();
    Ok(chain)
}

struct SnapshotBundle {
    characters: Vec<(ConversationParticipant, CharacterSnapshotBodyV1)>,
    persona: Option<PersonaSnapshotBodyV1>,
    prompt: Option<PromptSnapshot>,
    scene: Option<(SceneLaunchSnapshot, SceneSnapshotBodyV1)>,
    /// A group's current members in cast order, for `{{group_characters}}`.
    group_members: Vec<CharacterSnapshotBodyV1>,
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
                materialize_prompt(materializer, conversation_id, snapshot).and_then(|body| {
                    prompt_document(snapshot.source_id, snapshot.source_revision, &body)
                })
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
        let group_members = match &aggregate.conversation.kind {
            ConversationKind::Group(_) => characters.iter().map(|(_, body)| body.clone()).collect(),
            ConversationKind::Direct(_) => Vec::new(),
        };
        Ok(Self {
            characters,
            persona,
            prompt,
            scene,
            group_members,
        })
    }

    /// Replaces each character body with its current record, as legacy read
    /// the characters every turn, and lists a group's current members in cast
    /// order. A record that no longer exists keeps its launch body; a group
    /// that no longer exists keeps its launch members.
    fn read_live_characters<S: CharacterRepository + ?Sized>(
        &mut self,
        sources: &S,
        group: Option<&crate::generation::live_sources::LiveGroup>,
    ) -> Result<(), ContextAssemblyError> {
        let live = |id: CharacterId| {
            CharacterRepository::get(sources, id)
                .map(|details| {
                    details
                        .map(|details| crate::launch::documents::character_body(&details.character))
                })
                .map_err(|_| ContextAssemblyError::ConversationUnavailable)
        };
        for (_, body) in &mut self.characters {
            if let Some(current) = live(body.character_id)? {
                *body = current;
            }
        }
        match group.and_then(|group| group.profile.as_ref()) {
            Some(profile) => {
                let mut members = profile.members.iter().collect::<Vec<_>>();
                members.sort_by_key(|member| member.ordinal);
                self.group_members = members
                    .into_iter()
                    .map(|member| live(member.character_id))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect();
            }
            None => {
                for body in &mut self.group_members {
                    if let Some(current) = live(body.character_id)? {
                        *body = current;
                    }
                }
            }
        }
        Ok(())
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

fn prompt_document(
    id: lettuce_types::PromptDocumentId,
    revision: lettuce_types::Revision,
    body: &PromptSnapshotBodyV1,
) -> Result<PromptSnapshot, ContextAssemblyError> {
    let entries = body
        .entries
        .iter()
        .map(prompt_entry)
        .collect::<Result<Vec<_>, _>>()?;
    let document = PromptSnapshot {
        id,
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
        revision,
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
        PromptEntryConditionV1::SceneImageProtocol { value } => {
            PromptEntryCondition::SceneImageProtocol {
                value: match value {
                    lettuce_conversations::SceneImageProtocolV1::Remote => {
                        lettuce_context::SceneImageProtocolKind::Remote
                    }
                    lettuce_conversations::SceneImageProtocolV1::Local => {
                        lettuce_context::SceneImageProtocolKind::Local
                    }
                },
            }
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
        lettuce_conversations::PromptPurposeV1::RuntimeText => PromptPurpose::RuntimeText,
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
        scene_image_protocol: runtime.scene_image_protocol.map(|protocol| match protocol {
            lettuce_conversations::SceneImageProtocol::Remote => {
                lettuce_context::SceneImageProtocolKind::Remote
            }
            lettuce_conversations::SceneImageProtocol::Local => {
                lettuce_context::SceneImageProtocolKind::Local
            }
        }),
        has_scene: !scene.trim().is_empty(),
        has_scene_direction: !scene_direction.trim().is_empty(),
        has_persona: snapshot.persona.is_some(),
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
        reasoning_enabled: runtime.reasoning_enabled,
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
        .and_then(|body| body.definition.as_deref().or(body.description.as_deref()))
        .map(str::trim)
        .unwrap_or_default()
        .to_owned();
    let persona_name = snapshot.persona.as_ref().map_or_else(
        || {
            user.map(|participant| participant.display_name.clone())
                .unwrap_or_default()
        },
        |body| body.title.clone(),
    );
    let persona_description = snapshot.persona.as_ref().map_or_else(
        || {
            user.and_then(|participant| participant.authored_description.clone())
                .unwrap_or_default()
        },
        |body| body.description.trim().to_owned(),
    );
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
            persona_name,
            persona_description,
            selected_name,
            character_description,
        )
    } else {
        (
            selected_name,
            character_description,
            persona_name,
            persona_description,
        )
    };
    let mut values = PromptRenderValues {
        user_name: persona_name.clone(),
        user_description: persona_description.clone(),
        ai_name: character_name.clone(),
        ai_description: character_description.clone(),
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
    values
}

/// Legacy `has_scene_placeholder` (`prompt_engine.rs` 3174-3178).
fn has_scene_placeholder(content: &str) -> bool {
    content.contains("{{scene}}")
        || content.contains("{{scene_direction}}")
        || content.contains("{{direction}}")
}

/// Legacy `{{group_characters}}` (`group_chat_manager/mod.rs` 5268-5288), one
/// newline-terminated line per member other than the speaker: the member's
/// definition, else its description; an empty one falls back to the
/// personality summary (the first 200 characters of the same text, legacy
/// `load_characters_info` 4511-4527); a member with neither is its name alone.
fn group_characters(
    members: &[CharacterSnapshotBodyV1],
    speaker: Option<CharacterId>,
    runtime: &RuntimeSections,
) -> Result<String, ContextAssemblyError> {
    let mut list = String::new();
    for body in members {
        if Some(body.character_id) == speaker {
            continue;
        }
        let definition = body
            .definition
            .as_deref()
            .filter(|definition| !definition.is_empty());
        let description = definition.or(body.description.as_deref());
        let summary = description.map(|source| {
            let summary = source.chars().take(200).collect::<String>();
            if summary.len() < source.len() {
                summary + "..."
            } else {
                source.to_owned()
            }
        });
        let name = (PromptVariable::CharacterName, body.name.clone());
        let line = match description
            .filter(|description| !description.is_empty())
            .map(str::to_owned)
            .or(summary)
        {
            Some(description) => runtime.fragment(
                "runtime_group_character_line",
                [name, (PromptVariable::CharacterDescription, description)],
            )?,
            None => runtime.fragment("runtime_group_character_name_line", [name])?,
        };
        if let Some(line) = line {
            list.push_str(&line);
            list.push('\n');
        }
    }
    Ok(list)
}

/// Legacy `replace_character_name_placeholders` (`group_chat_manager/mod.rs`
/// 5549-5577): a `{{@"Name"}}` token in a group's starting scene becomes the
/// member's name; a token naming no member stays as written.
fn resolve_member_mentions(content: &str, members: &[CharacterSnapshotBodyV1]) -> String {
    const OPEN: &str = "{{@\"";
    const CLOSE: &str = "\"}}";
    let mut resolved = String::with_capacity(content.len());
    let mut rest = content;
    while let Some(start) = rest.find(OPEN) {
        let name_start = start + OPEN.len();
        let Some(length) = rest[name_start..].find(CLOSE) else {
            break;
        };
        let name = &rest[name_start..name_start + length];
        let end = name_start + length + CLOSE.len();
        resolved.push_str(&rest[..start]);
        if members.iter().any(|body| body.name == name) {
            resolved.push_str(name);
        } else {
            resolved.push_str(&rest[start..end]);
        }
        rest = &rest[end..];
    }
    resolved.push_str(rest);
    resolved
}

/// Identity tokens inside values the renderer substitutes verbatim, resolved
/// the way legacy did: a direct chat ran `apply_identity_placeholders` over its
/// rendered prompt (`prompt_engine.rs` 4580) and author note (3314-3365); a
/// group chat replaced `{{char}}`, `{{persona}}` and `{{user}}` in each
/// rendered entry and every identity token in its author note
/// (`group_chat_manager/mod.rs` 5060-5100, 5490-5495).
fn resolve_substituted_values(values: &mut PromptRenderValues, group: bool) {
    let resolve = |values: &PromptRenderValues, text: &str| {
        if group {
            values.resolve_names(text)
        } else {
            values.resolve_identity(text)
        }
    };
    let lorebook = resolve(values, &values.lorebook);
    let context_summary = resolve(values, &values.context_summary);
    let key_memories = resolve(values, &values.key_memories);
    let author_note = values.resolve_identity(&values.author_note);
    values.lorebook = lorebook;
    values.context_summary = context_summary;
    values.key_memories = key_memories;
    values.author_note = author_note;
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

/// Runtime sections a turn injects, collected in placement order.
/// `turn_context` marks the in-chat messages legacy's condense could merge
/// into its turn-context message: every one except a conditional or interval
/// template entry.
struct Placement {
    relative: Vec<ProviderNeutralMessage>,
    in_chat: Vec<(u32, ProviderNeutralMessage)>,
    turn_context: Vec<bool>,
}

impl Placement {
    fn place(&mut self, section: Option<RuntimeSection>) {
        let Some(section) = section else {
            return;
        };
        match section.depth {
            None => self.relative.push(section.message),
            Some(depth) => {
                self.in_chat.push((depth, section.message));
                self.turn_context.push(true);
            }
        }
    }
}

/// Legacy `condense_entries_into_single_system_message`: the non-empty text
/// of the messages as one system message.
fn condensed_system_message(
    messages: Vec<ProviderNeutralMessage>,
) -> Option<ProviderNeutralMessage> {
    let text = messages
        .iter()
        .flat_map(|message| &message.parts)
        .filter_map(|part| match part {
            ProviderContextPart::Text { text } => Some(text.trim()),
            _ => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    (!text.is_empty()).then(|| ProviderNeutralMessage {
        role: MessageRole::System,
        parts: vec![ProviderContextPart::Text { text }],
    })
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

pub(crate) fn condense_prompt_messages(messages: &mut Vec<ProviderNeutralMessage>) {
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

fn template_has_placeholder(prompt: Option<&PromptSnapshot>, placeholder: &str) -> bool {
    prompt.is_some_and(|document| {
        document
            .entries
            .iter()
            .any(|entry| entry.content.contains(placeholder))
    })
}

#[allow(clippy::too_many_arguments)]
fn provider_message(
    item: &TimelineItem,
    aggregate: &ConversationAggregate,
    character_names: &HashMap<ConversationParticipantId, String>,
    selected_speaker: Option<ConversationParticipantId>,
    swap_roles: bool,
    runtime: &RuntimeSections,
    group_user_name: &str,
    time_stamps: bool,
) -> Result<Option<ProviderNeutralMessage>, ContextAssemblyError> {
    if item.message.role == MessageRole::Scene {
        return Ok(None);
    }
    if aggregate.conversation.kind.is_group()
        && item.message.role == MessageRole::Assistant
        && item.message.author_participant_id.is_none()
    {
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
    if time_stamps
        && matches!(
            item.message.role,
            MessageRole::User | MessageRole::Assistant
        )
    {
        let stamp = crate::companion::companion_memory_inference::format_message_timestamp(
            item.message.effective_time,
        );
        match context_parts.iter_mut().find_map(|part| match part {
            ProviderContextPart::Text { text } => Some(text),
            _ => None,
        }) {
            Some(text) if text.is_empty() => *text = stamp,
            Some(text) => *text = format!("{stamp} {text}"),
            None => context_parts.insert(0, ProviderContextPart::Text { text: stamp }),
        }
    }
    if matches!(aggregate.conversation.kind, ConversationKind::Group(_))
        && role == MessageRole::User
        && matches!(
            item.message.role,
            MessageRole::User | MessageRole::Assistant
        )
    {
        let speaker = if item.message.role == MessageRole::User {
            group_user_name.to_owned()
        } else {
            match item
                .message
                .author_participant_id
                .and_then(|id| character_names.get(&id))
            {
                Some(name) => name.clone(),
                None => runtime
                    .fragment("runtime_group_unknown_speaker", [])?
                    .unwrap_or_default(),
            }
        };
        let first_text = context_parts
            .iter()
            .position(|part| matches!(part, ProviderContextPart::Text { .. }));
        let message_text = match first_text.map(|index| &context_parts[index]) {
            Some(ProviderContextPart::Text { text }) => text.clone(),
            _ => String::new(),
        };
        if let Some(prefixed) = runtime.fragment(
            "runtime_group_message_prefix",
            [
                (PromptVariable::SpeakerName, speaker),
                (PromptVariable::MessageText, message_text),
            ],
        )? {
            if let Some(index) = first_text {
                context_parts.remove(index);
            }
            context_parts.insert(0, ProviderContextPart::Text { text: prefixed });
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

/// The per-turn record of the books used: each book with an activated
/// entry, at the revision read this turn, in activation order.
fn lore_attributions(entries: &[ResolvedLorebookEntry]) -> Vec<LorebookAttribution> {
    let mut attributions: Vec<LorebookAttribution> = Vec::new();
    for entry in entries {
        match attributions
            .iter_mut()
            .find(|attribution| attribution.lorebook_id == entry.source.lorebook_id)
        {
            Some(attribution) => attribution.activated_entry_ids.push(entry.entry.id),
            None => attributions.push(LorebookAttribution {
                lorebook_id: entry.source.lorebook_id,
                revision: entry.source.book_revision,
                activated_entry_ids: vec![entry.entry.id],
            }),
        }
    }
    attributions
}

/// The character a group turn speaks as; `None` for a direct chat.
fn speaker_character(
    conversation: &lettuce_conversations::Conversation,
    request: &ContextRequest,
) -> Option<CharacterId> {
    let speaker = request.selected_speaker.as_ref()?.participant_id;
    conversation
        .participants
        .iter()
        .find(|participant| participant.id == speaker)
        .and_then(|participant| match participant.source {
            lettuce_conversations::ParticipantSource::Character(id) => Some(id),
            _ => None,
        })
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
    fn lorebook_attribution_groups_activated_entries_by_book() {
        let first_book = LorebookId::new();
        let second_book = LorebookId::new();
        let entry_ids = [
            LorebookEntryId::new(),
            LorebookEntryId::new(),
            LorebookEntryId::new(),
        ];
        let entry = |id, book, revision| ResolvedLorebookEntry {
            entry: lettuce_context::LorebookEntry {
                id,
                lorebook_id: book,
                title: "entry".into(),
                enabled: true,
                always_active: true,
                keywords: Vec::new(),
                case_sensitive: false,
                match_mode: lettuce_context::KeywordMatchMode::Literal,
                content: "body".into(),
                priority: 0,
                ordinal: 0,
                revision: lettuce_types::Revision::INITIAL,
                created_at: lettuce_types::TimestampMillis::UNIX_EPOCH,
                updated_at: lettuce_types::TimestampMillis::UNIX_EPOCH,
            },
            source: lettuce_context::ResolvedLorebookSource {
                provenance: LorebookSourceProvenance::Conversation {
                    id: ConversationId::new(),
                },
                lorebook_id: book,
                book_revision: lettuce_types::Revision::new(revision),
                source_order: 0,
            },
            matched_keywords: Vec::new(),
            always_active: true,
        };
        let attribution = lore_attributions(&[
            entry(entry_ids[0], first_book, 3),
            entry(entry_ids[1], second_book, 5),
            entry(entry_ids[2], first_book, 3),
        ]);
        assert_eq!(attribution.len(), 2);
        assert_eq!(attribution[0].lorebook_id, first_book);
        assert_eq!(attribution[0].revision.get(), 3);
        assert_eq!(
            attribution[0].activated_entry_ids,
            vec![entry_ids[0], entry_ids[2]]
        );
        assert_eq!(attribution[1].activated_entry_ids, vec![entry_ids[1]]);
    }

    #[test]
    fn budget_reports_bounded_estimate_and_window_truncation() {
        let messages = vec![ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::Text {
                text: "12345678".into(),
            }],
        }];
        let report = budget_report(&messages, 3).expect("budget");
        assert_eq!(report.input_bytes, 8);
        assert_eq!(report.estimated_input_tokens, 2);
        assert_eq!(report.omitted_messages, 3);
        assert!(report.truncated);
    }

    #[test]
    fn continue_from_a_hidden_head_answers_the_last_visible_message() {
        let conversation_id = ConversationId::new();
        let branch_id = lettuce_types::ConversationBranchId::new();
        let user = MessageId::new();
        let hidden = MessageId::new();
        let item = |id, role, visibility, parent_message_id, at| TimelineItem {
            message: lettuce_conversations::Message {
                id,
                conversation_id,
                branch_id,
                parent_message_id,
                author_participant_id: None,
                role,
                logical_time: lettuce_types::TimestampMillis::new(at),
                effective_time: lettuce_types::TimestampMillis::new(at),
                visibility,
                pinned: false,
                scene_edited: false,
                active_render_source: MessageRenderSource::Revision(
                    lettuce_types::MessageRevisionId::new(),
                ),
                revision: lettuce_types::Revision::INITIAL,
                created_at: lettuce_types::TimestampMillis::new(at),
                updated_at: lettuce_types::TimestampMillis::new(at),
            },
            active_revision: None,
            active_candidate: None,
            initial_origin: None,
        };
        let request = ContextRequest {
            conversation_id,
            branch_id,
            branch_path: vec![branch_id],
            source_message_id: hidden,
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
                item(
                    user,
                    MessageRole::User,
                    lettuce_conversations::MessageVisibility::Visible,
                    None,
                    1,
                ),
                item(
                    hidden,
                    MessageRole::System,
                    lettuce_conversations::MessageVisibility::Hidden,
                    Some(user),
                    2,
                ),
            ],
        };
        let branches = vec![lettuce_conversations::ConversationBranch {
            id: branch_id,
            conversation_id,
            parent_branch_id: None,
            fork_message_id: None,
            head_message_id: Some(hidden),
            status: BranchStatus::Active,
            revision: lettuce_types::Revision::INITIAL,
            created_at: lettuce_types::TimestampMillis::UNIX_EPOCH,
            updated_at: lettuce_types::TimestampMillis::UNIX_EPOCH,
        }];
        let selection = select_timeline(&branches, &request)
            .expect("legacy continuation skipped model-invisible messages");
        assert_eq!(
            selection
                .window
                .iter()
                .map(|item| item.message.id)
                .collect::<Vec<_>>(),
            vec![user]
        );
    }

    #[test]
    fn a_message_stamped_before_its_history_keeps_the_whole_history() {
        let conversation_id = ConversationId::new();
        let branch_id = lettuce_types::ConversationBranchId::new();
        let ids = [MessageId::new(), MessageId::new(), MessageId::new()];
        let item = |index: usize, role, at| TimelineItem {
            message: lettuce_conversations::Message {
                id: ids[index],
                conversation_id,
                branch_id,
                parent_message_id: index.checked_sub(1).map(|parent| ids[parent]),
                author_participant_id: None,
                role,
                logical_time: lettuce_types::TimestampMillis::new(at),
                effective_time: lettuce_types::TimestampMillis::new(at),
                visibility: lettuce_conversations::MessageVisibility::Visible,
                pinned: false,
                scene_edited: false,
                active_render_source: MessageRenderSource::Revision(
                    lettuce_types::MessageRevisionId::new(),
                ),
                revision: lettuce_types::Revision::INITIAL,
                created_at: lettuce_types::TimestampMillis::new(at),
                updated_at: lettuce_types::TimestampMillis::new(at),
            },
            active_revision: None,
            active_candidate: None,
            initial_origin: None,
        };
        let request = ContextRequest {
            conversation_id,
            branch_id,
            branch_path: vec![branch_id],
            source_message_id: ids[2],
            operation: GenerationOperation::Send,
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
                item(0, MessageRole::User, 5_000),
                item(1, MessageRole::Assistant, 6_000),
                item(2, MessageRole::User, 1_000),
            ],
        };
        let branches = vec![lettuce_conversations::ConversationBranch {
            id: branch_id,
            conversation_id,
            parent_branch_id: None,
            fork_message_id: None,
            head_message_id: Some(ids[2]),
            status: BranchStatus::Active,
            revision: lettuce_types::Revision::INITIAL,
            created_at: lettuce_types::TimestampMillis::UNIX_EPOCH,
            updated_at: lettuce_types::TimestampMillis::UNIX_EPOCH,
        }];
        let selection = select_timeline(&branches, &request)
            .expect("legacy sent history in conversation order whatever its timestamps");
        assert_eq!(
            selection
                .window
                .iter()
                .map(|item| item.message.id)
                .collect::<Vec<_>>(),
            ids.to_vec()
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
                supersedes_candidate_id: None,
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
            group_members: Vec::new(),
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
