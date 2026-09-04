use std::collections::HashSet;

use chrono::{Local, LocalResult, TimeZone};
use lettuce_characters::{CharacterRepository, PersonaRepository};
use lettuce_context::{LorebookEntry, LorebookRepository, PromptDocument};
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
        + ?Sized,
    J: JobStore + ?Sized,
{
    pub fn prepare_and_admit(
        &self,
        request: LorebookEntryPreparationRequest<'_>,
    ) -> Result<LorebookEntryAdmission, LorebookEntryPreparationError> {
        validate_profile(&request.profile)?;
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
            "(none)".to_owned()
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
                    )?;
                    let summary = if request.include_memory_summary {
                        MemorySummaryRepository::get_summary(self.sources, memory.id)?
                            .map(|summary| summary.text.trim().to_owned())
                            .filter(|summary| !summary.is_empty())
                            .unwrap_or_else(|| "(none)".to_owned())
                    } else {
                        "(none)".to_owned()
                    };
                    (selected, summary)
                }
                None if selected_memory_ids.is_empty() => {
                    ("(none)".to_owned(), "(none)".to_owned())
                }
                None => return Err(LorebookEntryPreparationError::InvalidInput),
            }
        } else {
            ("(none)".to_owned(), "(none)".to_owned())
        };

        validate_source_inputs(
            request.source,
            &selected_message_ids,
            &selected_messages,
            &selected_memories,
            &memory_summary,
        )?;

        let prompt_values = LorebookEntryPromptValues {
            lorebook_name: lorebook.book.name,
            character_name: character.character.profile.name,
            session_title: conversation.title.clone(),
            existing_entries: format_existing_entries(&lorebook.entries),
            direction_prompt: request
                .direction_prompt
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("(none)")
                .to_owned(),
            selected_messages,
            memory_summary,
            selected_memories,
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

fn format_selected_messages(
    timeline: &[TimelineItem],
    selected_ids: &[MessageId],
    time_awareness_enabled: bool,
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
                "[empty message]".to_owned()
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
            format!(
                "{}. {}: {}",
                index + 1,
                role_label(item.message.role),
                content
            )
        })
        .collect::<Vec<_>>()
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
) -> Result<String, LorebookEntryPreparationError> {
    if selected_ids.is_empty() {
        return Ok("(none)".to_owned());
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
        .enumerate()
        .filter_map(|(index, memory)| {
            let text = memory.text.trim();
            if text.is_empty() {
                return None;
            }
            let rendered = if time_awareness_enabled {
                format_memory_for_prompt(memory, effective_now)
            } else {
                format!("- {text}")
            };
            Some(format!(
                "{}. {}",
                index + 1,
                rendered.trim_start_matches("- ")
            ))
        })
        .collect::<Vec<_>>();
    Ok(if lines.is_empty() {
        "(none)".to_owned()
    } else {
        lines.join("\n")
    })
}

fn format_memory_for_prompt(memory: &MemoryItem, effective_now: TimestampMillis) -> String {
    let mut line = format!("- {}", memory.text);
    if let Some(observed_at) = memory.observed_at {
        let observed = match Local.timestamp_millis_opt(observed_at.get()) {
            LocalResult::Single(datetime) | LocalResult::Ambiguous(datetime, _) => datetime,
            LocalResult::None => Local::now(),
        };
        let relative = humanize_relative(effective_now.get() - observed_at.get());
        line.push_str(&format!(
            " (observed {}, {})",
            observed.format("%Y-%m-%d %H:%M"),
            relative
        ));
    }
    line
}

fn humanize_relative(delta_ms: i64) -> String {
    let future = delta_ms < 0;
    let seconds = delta_ms.unsigned_abs() / 1000;
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    const MONTH: u64 = 30 * DAY;
    const YEAR: u64 = 365 * DAY;

    if seconds < 45 {
        return "just now".to_owned();
    }
    let (count, unit) = if seconds < HOUR {
        (seconds / MINUTE, "minute")
    } else if seconds < DAY {
        (seconds / HOUR, "hour")
    } else if seconds < WEEK {
        (seconds / DAY, "day")
    } else if seconds < MONTH {
        (seconds / WEEK, "week")
    } else if seconds < YEAR {
        (seconds / MONTH, "month")
    } else {
        (seconds / YEAR, "year")
    };
    let count = count.max(1);
    if unit == "day" && count == 1 {
        return if future { "tomorrow" } else { "yesterday" }.to_owned();
    }
    let plural = if count == 1 { "" } else { "s" };
    if future {
        format!("in {count} {unit}{plural}")
    } else {
        format!("{count} {unit}{plural} ago")
    }
}

fn format_existing_entries(entries: &[LorebookEntry]) -> String {
    if entries.is_empty() {
        return "(none)".to_owned();
    }
    entries
        .iter()
        .map(|entry| {
            let title = if entry.title.trim().is_empty() {
                entry
                    .keywords
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Untitled entry".to_owned())
            } else {
                entry.title.trim().to_owned()
            };
            let keywords = if entry.always_active {
                "always active".to_owned()
            } else if entry.keywords.is_empty() {
                "no keywords".to_owned()
            } else {
                format!("keywords: {}", entry.keywords.join(", "))
            };
            let content = entry.content.trim();
            if content.is_empty() {
                format!("- {title} ({keywords})")
            } else {
                format!("- {title} ({keywords}): {content}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn validate_source_inputs(
    source: LorebookEntrySource,
    selected_message_ids: &[MessageId],
    selected_messages: &str,
    selected_memories: &str,
    memory_summary: &str,
) -> Result<(), LorebookEntryPreparationError> {
    let invalid = match source {
        LorebookEntrySource::Messages => selected_message_ids.is_empty(),
        LorebookEntrySource::Memory => selected_memories == "(none)" && memory_summary == "(none)",
        LorebookEntrySource::Mixed => {
            selected_messages == "(none)"
                && selected_memories == "(none)"
                && memory_summary == "(none)"
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

    use super::{format_existing_entries, humanize_relative};

    #[test]
    fn legacy_relative_time_thresholds_are_preserved() {
        assert_eq!(humanize_relative(44_000), "just now");
        assert_eq!(humanize_relative(60_000), "1 minute ago");
        assert_eq!(humanize_relative(86_400_000), "yesterday");
        assert_eq!(humanize_relative(-86_400_000), "tomorrow");
        assert_eq!(humanize_relative(14 * 86_400_000), "2 weeks ago");
    }

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
        assert_eq!(
            format_existing_entries(&[
                entry("", false, vec!["Harbour".into()], "", 0),
                entry("  Brass key  ", true, Vec::new(), "  Opens the gate.  ", 1),
                entry("Fog", false, Vec::new(), "Dense at dawn.", 2),
            ]),
            "- Harbour (keywords: Harbour)\n- Brass key (always active): Opens the gate.\n- Fog (no keywords): Dense at dawn."
        );
    }
}
