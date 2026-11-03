use lettuce_context::{PromptDocument, PromptLookupResult, PromptRepository, PromptRepositoryError};
use lettuce_types::PromptDocumentId;

use crate::generation::built_in_prompts::{BuiltInPromptId, active_built_in_prompt};

pub(crate) fn selected_prompt<R: PromptRepository + ?Sized>(
    repository: &R,
    selected: Option<PromptDocumentId>,
    built_in: BuiltInPromptId,
) -> Result<Option<PromptDocument>, PromptRepositoryError> {
    match selected {
        Some(id) => match repository.lookup_exact(id, built_in.purpose())? {
            PromptLookupResult::Available { document } => Ok(Some(document)),
            _ => Err(PromptRepositoryError::NotFound),
        },
        None => active_built_in_prompt(repository, built_in),
    }
}

#[cfg(test)]
pub(crate) fn custom_prompt(repository: &lettuce_database::Database, built_in: BuiltInPromptId, marker: &str) -> PromptDocument {
    let source = active_built_in_prompt(repository, built_in).expect("built-in").expect("stored");
    let entries = source.entries.into_iter().enumerate().map(|(index, entry)| lettuce_context::PromptEntryDraft {
        built_in_entry_key: None, name: entry.name, role: entry.role,
        content: if index == 0 { format!("{marker}\n{}", entry.content) } else { entry.content },
        enabled: entry.enabled, injection_position: entry.injection_position, depth: entry.depth,
        conditional_min_messages: entry.conditional_min_messages, interval_turns: entry.interval_turns,
        system_prompt: false, conditions: entry.conditions, payload: entry.payload,
    }).collect();
    repository.create_user_draft(lettuce_context::PromptMetadataDraft {
        name: marker.into(), purpose: source.purpose, condense: source.condense, behavior_version: source.behavior_version,
    }, entries, lettuce_types::TimestampMillis::new(2)).expect("custom prompt")
}
