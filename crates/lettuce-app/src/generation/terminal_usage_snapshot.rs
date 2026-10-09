use lettuce_conversations::{
    Conversation, ConversationKind, GenerationOperation, GenerationTurn, ParticipantSource,
    UsageOutcome, UsageRecordSnapshot,
};
use lettuce_usage::{JobInferenceUsage, JobInferenceUsageResult};

pub(super) fn terminal_snapshot(
    turn: &GenerationTurn,
    conversation: &Conversation,
    records: &[JobInferenceUsage],
    outcome: UsageOutcome,
    error_message: Option<String>,
) -> UsageRecordSnapshot {
    let mut snapshot = records
        .iter()
        .rev()
        .find_map(|record| match &record.result {
            Some(JobInferenceUsageResult::Response { snapshot, .. }) => {
                snapshot.as_deref().cloned()
            }
            _ => None,
        })
        .or_else(|| records.first().and_then(|record| record.snapshot.clone()))
        .unwrap_or_default();
    if snapshot.character_id.is_none() {
        match &conversation.kind {
            ConversationKind::Direct(details) => {
                snapshot.character_id = Some(details.character.source_id);
                snapshot.character_name = Some(details.character.name.clone());
            }
            ConversationKind::Group(_) => {
                if let Some(speaker) = &turn.selected_speaker
                    && let Some(participant) = conversation
                        .participants
                        .iter()
                        .find(|participant| participant.id == speaker.participant_id)
                    && let ParticipantSource::Character(id) = participant.source
                {
                    snapshot.character_id = Some(id);
                    snapshot.character_name = Some(participant.display_name.clone());
                }
            }
        }
    }
    if snapshot.model_name.is_none() {
        snapshot.model_name = turn
            .resolved_model
            .as_ref()
            .map(|model| model.display_name.clone());
    }
    if snapshot.operation_kind.is_none() {
        let group = matches!(conversation.kind, ConversationKind::Group(_));
        snapshot.operation_kind = Some(
            match (group, turn.operation) {
                (false, GenerationOperation::Send) => "chat",
                (false, GenerationOperation::Continue) => "continue",
                (false, GenerationOperation::Regenerate) => "regenerate",
                (true, GenerationOperation::Send) => "group_chat_message",
                (true, GenerationOperation::Continue) => "group_chat_continue",
                (true, GenerationOperation::Regenerate) => "group_chat_regenerate",
            }
            .into(),
        );
    }
    if snapshot.provider_response_id.is_none() {
        snapshot.provider_response_id =
            records
                .iter()
                .rev()
                .find_map(|record| match &record.result {
                    Some(JobInferenceUsageResult::Response {
                        provider_response_id,
                        ..
                    }) => provider_response_id.clone(),
                    _ => None,
                });
    }
    if snapshot.finish_reason.is_none() {
        snapshot.finish_reason = match outcome {
            UsageOutcome::Failed => Some("error".into()),
            UsageOutcome::Cancelled | UsageOutcome::Interrupted => Some("aborted".into()),
            UsageOutcome::Succeeded => None,
        };
    }
    snapshot.error_message = error_message;
    snapshot
}
