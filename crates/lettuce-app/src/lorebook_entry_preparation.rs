use std::collections::HashSet;

use lettuce_characters::{CharacterRepository, PersonaRepository};
use lettuce_context::{LorebookEntry, LorebookRepository, PromptDocument, PromptVariable};
use lettuce_conversations::{
    ConversationKind, ConversationReader, MessagePart, MessageRenderSource, MessageRole,
    ResolvedInferenceProfile, SnapshotSelection, TimelineItem,
};
use lettuce_creation::{
    LorebookEntryFallbackFormat, LorebookEntryPromptValues, LorebookEntryRunRepository,
    LorebookEntrySource,
};
use lettuce_jobs::JobStore;
use lettuce_memory::{
    MemoryItem, MemoryRepository, MemoryRepositoryError, MemorySummaryRepository,
};
use lettuce_models::{CapabilityStatus, Modality};
use lettuce_types::{
    ConversationId, LorebookId, MemoryId, MessageId, PageLimit, PageRequest, RequestId,
    TimestampMillis,
};

use crate::{
    LorebookEntryAdmission, LorebookEntryAdmissionCoordinator, LorebookEntryAdmissionError,
    LorebookEntryAdmissionRequest,
};

#[derive(Debug, Clone)]
pub struct LorebookEntryPreparationRequest<'a> {
    pub request_id: RequestId,
    pub conversation_id: ConversationId,
    pub lorebook_id: LorebookId,
    pub selected_message_ids: Vec<MessageId>,
    pub selected_memory_ids: Vec<MemoryId>,
    pub source: LorebookEntrySource,
    pub include_memory_summary: bool,
    pub direction_prompt: Option<String>,
    pub force: bool,
    pub time_awareness_enabled: bool,
    pub profile: ResolvedInferenceProfile,
    pub prompt: &'a PromptDocument,
    pub fallback_format: LorebookEntryFallbackFormat,
    pub now: TimestampMillis,
}

#[derive(Debug, thiserror::Error)]
pub enum LorebookEntryPreparationError {
    #[error("lorebook entry generation source input is invalid")]
    InvalidInput,
    #[error("lorebook entry generation source data is unavailable")]
    SourceUnavailable,
    #[error("lorebook entry generation memory source failed: {0}")]
    Memory(#[from] MemoryRepositoryError),
    #[error("lorebook entry generation admission failed: {0}")]
    Admission(#[from] LorebookEntryAdmissionError),
}

#[derive(Debug)]
pub struct LorebookEntryPreparationCoordinator<'a, S: ?Sized, J: ?Sized> {
    sources: &'a S,
    jobs: &'a J,
}

impl<'a, S: ?Sized, J: ?Sized> LorebookEntryPreparationCoordinator<'a, S, J> {
    #[must_use]
    pub const fn new(sources: &'a S, jobs: &'a J) -> Self {
        Self { sources, jobs }
    }
}

impl<S, J> LorebookEntryPreparationCoordinator<'_, S, J>
where
    S: ConversationReader
        + CharacterRepository
        + PersonaRepository
        + LorebookRepository
        + MemoryRepository
        + MemorySummaryRepository
        + LorebookEntryRunRepository
        + crate::runtime_text::RuntimeTextSource
        + ?Sized,
    J: JobStore + ?Sized,
{
    pub fn prepare_and_admit(
        &self,
        request: LorebookEntryPreparationRequest<'_>,
    ) -> Result<LorebookEntryAdmission, LorebookEntryPreparationError> {
        validate_profile(&request.profile)?;
        let text = crate::runtime_text::RuntimeText::load(
            self.sources,
            crate::BuiltInPromptId::LorebookRuntime,
        )
        .map_err(|_| LorebookEntryPreparationError::SourceUnavailable)?;
        let fragment = |key: &str, variables: Vec<(PromptVariable, String)>| {
            text.render_with(key, variables)
                .map_err(|_| LorebookEntryPreparationError::SourceUnavailable)
        };
        let none = fragment("lorebook_none", Vec::new())?;
        let aggregate = ConversationReader::get(self.sources, request.conversation_id)
            .map_err(|_| LorebookEntryPreparationError::SourceUnavailable)?;
        let conversation = &aggregate.conversation;
        let ConversationKind::Direct(details) = &conversation.kind else {
            return Err(LorebookEntryPreparationError::InvalidInput);
        };
        let character_id = details.character.source_id;
        let character = CharacterRepository::get(self.sources, character_id)
            .map_err(|_| LorebookEntryPreparationError::SourceUnavailable)?
            .ok_or(LorebookEntryPreparationError::SourceUnavailable)?;
        let persona_id = match &details.persona {
            SnapshotSelection::Inherited(persona) | SnapshotSelection::Explicit(persona) => {
                PersonaRepository::get(self.sources, persona.source_id)
                    .map_err(|_| LorebookEntryPreparationError::SourceUnavailable)?
                    .ok_or(LorebookEntryPreparationError::SourceUnavailable)?;
                Some(persona.source_id)
            }
            SnapshotSelection::Disabled => None,
        };
        let lorebook = LorebookRepository::get(self.sources, request.lorebook_id)
            .map_err(|_| LorebookEntryPreparationError::SourceUnavailable)?
            .ok_or(LorebookEntryPreparationError::SourceUnavailable)?;

        let messages_enabled = matches!(
            request.source,
            LorebookEntrySource::Messages | LorebookEntrySource::Mixed
        );
        let memory_enabled = matches!(
            request.source,
            LorebookEntrySource::Memory | LorebookEntrySource::Mixed
        );
        let selected_message_ids = if messages_enabled {
            reject_duplicates(&request.selected_message_ids)?;
            request.selected_message_ids.clone()
        } else {
            Vec::new()
        };
        let selected_memory_ids = if memory_enabled {
            reject_duplicates(&request.selected_memory_ids)?;
            request.selected_memory_ids.clone()
        } else {
            Vec::new()
        };

        let selected_messages = if selected_message_ids.is_empty() {
            none.clone()
        } else {
            let timeline = load_timeline(
                self.sources,
                request.conversation_id,
                conversation.active_branch_id,
            )?;
            format_selected_messages(
                &timeline,
                &selected_message_ids,
                request.time_awareness_enabled,
                &fragment,
            )?
        };

        let (selected_memories, memory_summary) = if memory_enabled {
            let memory =
                MemoryRepository::get_for_conversation(self.sources, request.conversation_id)?;
            match memory {
                Some(memory) => {
                    let selected = format_selected_memories(
                        &memory.items,
                        &selected_memory_ids,
                        request.now,
                        request.time_awareness_enabled,
                        &none,
                        &fragment,
                    )?;
                    let summary = if request.include_memory_summary {
                        MemorySummaryRepository::get_summary(self.sources, memory.id)?
                            .map(|summary| summary.text.trim().to_owned())
                            .filter(|summary| !summary.is_empty())
                            .unwrap_or_else(|| none.clone())
                    } else {
                        none.clone()
                    };
                    (selected, summary)
                }
                None if selected_memory_ids.is_empty() => (none.clone(), none.clone()),
                None => return Err(LorebookEntryPreparationError::InvalidInput),
            }
        } else {
            (none.clone(), none.clone())
        };

        validate_source_inputs(
            request.source,
            &selected_message_ids,
            &selected_messages,
            &selected_memories,
            &memory_summary,
            &none,
        )?;

        let prompt_values = LorebookEntryPromptValues {
            lorebook_name: lorebook.book.name,
            character_name: character.character.profile.name,
            session_title: conversation.title.clone(),
            existing_entries: format_existing_entries(&lorebook.entries, &none, &fragment)?,
            direction_prompt: request
                .direction_prompt
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map_or_else(|| none.clone(), str::to_owned),
            selected_messages,
            memory_summary,
            selected_memories,
            none_marker: none.clone(),
        };
        LorebookEntryAdmissionCoordinator::new(self.sources, self.jobs)
            .admit(LorebookEntryAdmissionRequest {
                request_id: request.request_id,
                conversation_id: request.conversation_id,
                lorebook_id: request.lorebook_id,
                character_id,
                persona_id,
                selected_message_ids,
                selected_memory_ids,
                source: request.source,
                include_memory_summary: request.include_memory_summary,
                time_awareness_enabled: request.time_awareness_enabled,
                force: request.force,
                profile: request.profile,
                prompt: request.prompt,
                prompt_values,
                fallback_format: request.fallback_format,
                now: request.now,
            })
            .map_err(Into::into)
    }
}

fn validate_profile(
    profile: &ResolvedInferenceProfile,
) -> Result<(), LorebookEntryPreparationError> {
    if profile
        .chat_profile
        .capabilities
        .input_modalities
        .get(Modality::Text)
        != CapabilityStatus::Supported
        || profile
            .chat_profile
            .capabilities
            .output_modalities
            .get(Modality::Text)
            != CapabilityStatus::Supported
    {
        return Err(LorebookEntryPreparationError::InvalidInput);
    }
    Ok(())
}

fn reject_duplicates<T: Eq + std::hash::Hash>(
    values: &[T],
) -> Result<(), LorebookEntryPreparationError> {
    let mut seen = HashSet::with_capacity(values.len());
    if values.iter().any(|value| !seen.insert(value)) {
        return Err(LorebookEntryPreparationError::InvalidInput);
    }
    Ok(())
}

fn load_timeline<R: ConversationReader + ?Sized>(
    repository: &R,
    conversation_id: ConversationId,
    branch_id: lettuce_types::ConversationBranchId,
) -> Result<Vec<TimelineItem>, LorebookEntryPreparationError> {
    let mut cursor = None;
    let mut items = Vec::new();
    loop {
        let page = repository
            .timeline_page(
                conversation_id,
                branch_id,
                &PageRequest {
                    cursor,
                    limit: PageLimit::new(200),
                },
            )
            .map_err(|_| LorebookEntryPreparationError::SourceUnavailable)?;
        items.extend(page.items);
        let Some(next) = page.next_cursor else {
            break;
        };
        cursor = Some(next);
    }
    items.reverse();
    Ok(items)
}

type Fragment<'a> = dyn Fn(&str, Vec<(PromptVariable, String)>) -> Result<String, LorebookEntryPreparationError>
    + 'a;

fn format_selected_messages(
    timeline: &[TimelineItem],
    selected_ids: &[MessageId],
    time_awareness_enabled: bool,
    fragment: &Fragment<'_>,
) -> Result<String, LorebookEntryPreparationError> {
    let selected: HashSet<_> = selected_ids.iter().copied().collect();
    let messages: Vec<_> = timeline
        .iter()
        .filter(|item| selected.contains(&item.message.id))
        .collect();
    if messages.len() != selected_ids.len() {
        return Err(LorebookEntryPreparationError::InvalidInput);
    }
    Ok(messages
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let content = active_text(item);
            let content = if content.trim().is_empty() {
                fragment("lorebook_empty_message", Vec::new())?
            } else {
                content.trim().to_owned()
            };
            let content = if time_awareness_enabled {
                format!(
                    "{} {}",
                    crate::companion_memory_inference::format_message_timestamp(
                        item.message.effective_time
                    ),
                    content
                )
            } else {
                content
            };
            fragment(
                "lorebook_selected_message",
                vec![
                    (PromptVariable::ItemNumber, (index + 1).to_string()),
                    (
                        PromptVariable::MessageRole,
                        role_label(item.message.role).to_owned(),
                    ),
                    (PromptVariable::MessageText, content),
                ],
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .join("\n"))
}

fn active_text(item: &TimelineItem) -> String {
    let parts = match item.message.active_render_source {
        MessageRenderSource::Revision(_) => item.active_revision.as_ref().map(|value| &value.parts),
        MessageRenderSource::Candidate(_) => {
            item.active_candidate.as_ref().map(|value| &value.parts)
        }
    };
    parts
        .into_iter()
        .flatten()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

const fn role_label(role: MessageRole) -> &'static str {
    match role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::System => "system",
        MessageRole::Scene => "scene",
    }
}

fn format_selected_memories(
    memories: &[MemoryItem],
    selected_ids: &[MemoryId],
    effective_now: TimestampMillis,
    time_awareness_enabled: bool,
    none: &str,
    fragment: &Fragment<'_>,
) -> Result<String, LorebookEntryPreparationError> {
    if selected_ids.is_empty() {
        return Ok(none.to_owned());
    }
    let selected: HashSet<_> = selected_ids.iter().copied().collect();
    let matches: Vec<_> = memories
        .iter()
        .filter(|memory| selected.contains(&memory.id))
        .collect();
    if matches.len() != selected_ids.len() {
        return Err(LorebookEntryPreparationError::InvalidInput);
    }
    let lines = matches
        .iter()
        .filter(|memory| !memory.text.trim().is_empty())
        .enumerate()
        .map(|(index, memory)| {
            let rendered = if time_awareness_enabled {
                crate::memory_prompt::memory_prompt_line(memory, effective_now).with_observed()
            } else {
                format!("- {}", memory.text.trim())
            };
            fragment(
                "lorebook_selected_memory",
                vec![
                    (PromptVariable::ItemNumber, (index + 1).to_string()),
                    (
                        PromptVariable::MemoryText,
                        rendered.trim_start_matches("- ").to_owned(),
                    ),
                ],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(if lines.is_empty() {
        none.to_owned()
    } else {
        lines.join("\n")
    })
}

fn format_existing_entries(
    entries: &[LorebookEntry],
    none: &str,
    fragment: &Fragment<'_>,
) -> Result<String, LorebookEntryPreparationError> {
    if entries.is_empty() {
        return Ok(none.to_owned());
    }
    Ok(entries
        .iter()
        .map(|entry| {
            let title = if entry.title.trim().is_empty() {
                match entry.keywords.first() {
                    Some(keyword) => keyword.clone(),
                    None => fragment("lorebook_untitled_entry", Vec::new())?,
                }
            } else {
                entry.title.trim().to_owned()
            };
            let keywords = if entry.always_active {
                fragment("lorebook_always_active", Vec::new())?
            } else if entry.keywords.is_empty() {
                fragment("lorebook_no_keywords", Vec::new())?
            } else {
                fragment(
                    "lorebook_keyword_list",
                    vec![(PromptVariable::EntryKeywords, entry.keywords.join(", "))],
                )?
            };
            fragment(
                "lorebook_existing_entry",
                vec![
                    (PromptVariable::EntryTitle, title),
                    (PromptVariable::EntryKeywords, keywords),
                    (
                        PromptVariable::EntryContent,
                        entry.content.trim().to_owned(),
                    ),
                ],
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .join("\n"))
}

fn validate_source_inputs(
    source: LorebookEntrySource,
    selected_message_ids: &[MessageId],
    selected_messages: &str,
    selected_memories: &str,
    memory_summary: &str,
    none: &str,
) -> Result<(), LorebookEntryPreparationError> {
    let invalid = match source {
        LorebookEntrySource::Messages => selected_message_ids.is_empty(),
        LorebookEntrySource::Memory => selected_memories == none && memory_summary == none,
        LorebookEntrySource::Mixed => {
            selected_messages == none && selected_memories == none && memory_summary == none
        }
    };
    if invalid {
        return Err(LorebookEntryPreparationError::InvalidInput);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use lettuce_context::{KeywordMatchMode, LorebookEntry};
    use lettuce_types::{LorebookEntryId, LorebookId, Revision, TimestampMillis};

    use super::format_existing_entries;

    #[test]
    fn legacy_existing_entry_format_is_preserved() {
        let lorebook_id = LorebookId::new();
        let entry =
            |title: &str, always_active: bool, keywords: Vec<String>, content: &str, ordinal| {
                LorebookEntry {
                    id: LorebookEntryId::new(),
                    lorebook_id,
                    title: title.into(),
                    enabled: true,
                    always_active,
                    keywords,
                    case_sensitive: false,
                    match_mode: KeywordMatchMode::Literal,
                    content: content.into(),
                    priority: 0,
                    ordinal,
                    revision: Revision::INITIAL,
                    created_at: TimestampMillis::new(1),
                    updated_at: TimestampMillis::new(1),
                }
            };
        let catalog = crate::BuiltInPromptCatalog::bundled().expect("catalog");
        let seed = catalog.seed(crate::BuiltInPromptId::LorebookRuntime);
        let fragment = |key: &str, variables: Vec<(lettuce_context::PromptVariable, String)>| {
            let entry = seed
                .entries
                .iter()
                .find(|entry| entry.built_in_entry_key.as_deref() == Some(key))
                .expect("catalog fragment");
            let mut values = lettuce_context::PromptRenderValues::default();
            values.purpose_values.extend(variables);
            Ok(lettuce_context::render_prompt_text(
                lettuce_context::PromptPurpose::RuntimeText,
                &entry.content,
                &values,
            )
            .expect("render fragment"))
        };
        assert_eq!(
            format_existing_entries(
                &[
                    entry("", false, vec!["Harbour".into()], "", 0),
                    entry("  Brass key  ", true, Vec::new(), "  Opens the gate.  ", 1),
                    entry("Fog", false, Vec::new(), "Dense at dawn.", 2),
                ],
                "(none)",
                &fragment,
            )
            .expect("format entries"),
            "- Harbour (keywords: Harbour)\n- Brass key (always active): Opens the gate.\n- Fog (no keywords): Dense at dawn."
        );
    }
}
