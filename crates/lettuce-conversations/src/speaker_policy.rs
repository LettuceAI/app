use std::collections::HashSet;

use lettuce_types::ConversationParticipantId;

use crate::content::MessageRole;
use crate::ports::{SpeakerParticipantState, SpeakerPolicyRequest};
use crate::snapshot::GroupSpeakerSelectionSnapshot;
use crate::{SelectedSpeakerDecision, SpeakerDecisionMethod, SpeakerFallback};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SpeakerSelectionError {
    #[error("speaker participants contain a duplicate id")]
    DuplicateParticipantId,
    #[error("forced speaker is not a participant")]
    UnknownForcedSpeaker,
    #[error("mention source is not a participant")]
    UnknownMentionSource,
    #[error("explicit speaker is not eligible")]
    IneligibleExplicitSpeaker,
    #[error("no eligible, unmuted speaker is available")]
    NoAvailableSpeaker,
    #[error("speaker selection requires an explicit speaker")]
    ExplicitSpeakerRequired,
    #[error("llm selection requires an external policy decision")]
    LlmRequiresExternalPolicy,
}

pub fn select_group_speaker(
    request: &SpeakerPolicyRequest,
    policy: GroupSpeakerSelectionSnapshot,
) -> Result<SelectedSpeakerDecision, SpeakerSelectionError> {
    validate_request(request)?;

    if let Some(participant_id) = request.forced_speaker {
        return explicit_decision(request, participant_id);
    }
    if let Some(participant_id) = request.mention_source {
        return explicit_decision(request, participant_id);
    }

    match policy {
        GroupSpeakerSelectionSnapshot::Director => {
            Err(SpeakerSelectionError::ExplicitSpeakerRequired)
        }
        GroupSpeakerSelectionSnapshot::DirectorAction => {
            Err(SpeakerSelectionError::ExplicitSpeakerRequired)
        }
        GroupSpeakerSelectionSnapshot::Llm => Err(SpeakerSelectionError::LlmRequiresExternalPolicy),
        GroupSpeakerSelectionSnapshot::Heuristic => {
            automatic_decision(request, AutomaticPolicy::Heuristic)
        }
        GroupSpeakerSelectionSnapshot::RoundRobin => {
            automatic_decision(request, AutomaticPolicy::RoundRobin)
        }
    }
}

fn validate_request(request: &SpeakerPolicyRequest) -> Result<(), SpeakerSelectionError> {
    let mut ids = HashSet::with_capacity(request.participants.len());
    for participant in &request.participants {
        if !ids.insert(participant.id) {
            return Err(SpeakerSelectionError::DuplicateParticipantId);
        }
    }
    if request.forced_speaker.is_some_and(|id| !ids.contains(&id)) {
        return Err(SpeakerSelectionError::UnknownForcedSpeaker);
    }
    if request.mention_source.is_some_and(|id| !ids.contains(&id)) {
        return Err(SpeakerSelectionError::UnknownMentionSource);
    }
    Ok(())
}

fn explicit_decision(
    request: &SpeakerPolicyRequest,
    participant_id: ConversationParticipantId,
) -> Result<SelectedSpeakerDecision, SpeakerSelectionError> {
    let participant = request
        .participants
        .iter()
        .find(|participant| participant.id == participant_id)
        .expect("explicit speaker was validated as a participant");
    if !participant.eligible {
        return Err(SpeakerSelectionError::IneligibleExplicitSpeaker);
    }
    Ok(SelectedSpeakerDecision {
        participant_id,
        method: SpeakerDecisionMethod::Explicit,
        fallback: SpeakerFallback::None,
        reference: None,
        rationale_summary: None,
        decision_model: None,
        usage_event_id: None,
    })
}

fn automatic_decision(
    request: &SpeakerPolicyRequest,
    policy: AutomaticPolicy,
) -> Result<SelectedSpeakerDecision, SpeakerSelectionError> {
    let available: Vec<&SpeakerParticipantState> = request
        .participants
        .iter()
        .filter(|participant| participant.eligible && !participant.muted)
        .collect();
    let (participant_id, method) = match policy {
        AutomaticPolicy::Heuristic => (
            heuristic_speaker(request, &available),
            SpeakerDecisionMethod::Heuristic,
        ),
        AutomaticPolicy::RoundRobin => (
            round_robin_speaker(request, &available),
            SpeakerDecisionMethod::RoundRobin,
        ),
    };
    let participant_id = participant_id.ok_or(SpeakerSelectionError::NoAvailableSpeaker)?;
    Ok(SelectedSpeakerDecision {
        participant_id,
        method,
        fallback: SpeakerFallback::None,
        reference: None,
        rationale_summary: None,
        decision_model: None,
        usage_event_id: None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutomaticPolicy {
    Heuristic,
    RoundRobin,
}

fn heuristic_speaker(
    request: &SpeakerPolicyRequest,
    available: &[&SpeakerParticipantState],
) -> Option<ConversationParticipantId> {
    let total_speaks: u64 = request
        .participants
        .iter()
        .map(|participant| u64::from(participant.speak_count))
        .sum();
    let participant_count = available.len() as u64;
    let latest_assistant_index = request
        .timeline
        .iter()
        .rposition(|item| item.message.role == MessageRole::Assistant);

    let mut best: Option<(ConversationParticipantId, i64)> = None;
    for participant in available {
        let count = u64::from(participant.speak_count);
        let mut score = 100_000_i64;

        if total_speaks > 0 {
            let expected_gap = total_speaks.saturating_sub(count.saturating_mul(participant_count));
            if expected_gap > 0 {
                score += ((expected_gap * 200_000) / (total_speaks * participant_count)) as i64;
            } else if count.saturating_mul(participant_count) * 2 > total_speaks * 3 {
                score -= 20_000;
            }
        }

        match recency_turns_ago(request, participant, latest_assistant_index) {
            None => score += 50_000,
            Some(0) => score -= 30_000,
            Some(1) => score -= 15_000,
            Some(turns) if turns >= 3 => score += 10_000,
            Some(_) => {}
        }

        if best.is_none_or(|(_, best_score)| score > best_score) {
            best = Some((participant.id, score));
        }
    }
    best.map(|(participant_id, _)| participant_id)
}

fn recency_turns_ago(
    request: &SpeakerPolicyRequest,
    participant: &SpeakerParticipantState,
    latest_assistant_index: Option<usize>,
) -> Option<u64> {
    let last_index = request.timeline.iter().rposition(|item| {
        item.message.role == MessageRole::Assistant
            && item.message.author_participant_id == Some(participant.id)
    });
    if let (Some(last_index), Some(latest_index)) = (last_index, latest_assistant_index) {
        return Some(
            request.timeline[last_index + 1..=latest_index]
                .iter()
                .filter(|item| item.message.role == MessageRole::Assistant)
                .count() as u64,
        );
    }
    if request.prior_speaker == Some(participant.id) {
        return Some(0);
    }
    participant.last_spoke_turn?;
    Some(2)
}

fn round_robin_speaker(
    request: &SpeakerPolicyRequest,
    available: &[&SpeakerParticipantState],
) -> Option<ConversationParticipantId> {
    if available.is_empty() {
        return None;
    }
    let prior_index = request
        .participants
        .iter()
        .position(|participant| Some(participant.id) == request.prior_speaker);
    let start = prior_index.map_or(0, |index| index + 1);
    request
        .participants
        .iter()
        .cycle()
        .skip(start)
        .take(request.participants.len())
        .find(|participant| participant.eligible && !participant.muted)
        .map(|participant| participant.id)
}

#[cfg(test)]
mod tests {
    use lettuce_types::{
        ConversationBranchId, ConversationId, MessageCandidateId, MessageId, Revision,
        TimestampMillis,
    };

    use super::*;
    use crate::content::{Message, MessageRenderSource, MessageRole, MessageVisibility};
    use crate::{GenerationOperation, TimelineItem};

    fn participant(
        id: ConversationParticipantId,
        speak_count: u32,
        eligible: bool,
        muted: bool,
    ) -> SpeakerParticipantState {
        SpeakerParticipantState {
            id,
            eligible,
            muted,
            speak_count,
            last_spoke_turn: None,
            last_spoke_at: None,
        }
    }

    fn request(participants: Vec<SpeakerParticipantState>) -> SpeakerPolicyRequest {
        SpeakerPolicyRequest {
            conversation_id: ConversationId::new(),
            branch_id: ConversationBranchId::new(),
            operation: GenerationOperation::Send,
            forced_speaker: None,
            mention_source: None,
            participants,
            prior_speaker: None,
            timeline: Vec::new(),
        }
    }

    fn assistant_message(participant_id: ConversationParticipantId, at: i64) -> TimelineItem {
        TimelineItem {
            message: Message {
                id: MessageId::new(),
                conversation_id: ConversationId::new(),
                branch_id: ConversationBranchId::new(),
                parent_message_id: None,
                author_participant_id: Some(participant_id),
                role: MessageRole::Assistant,
                logical_time: TimestampMillis::new(at),
                effective_time: TimestampMillis::new(at),
                visibility: MessageVisibility::Visible,
                pinned: false,
                scene_edited: false,
                active_render_source: MessageRenderSource::Candidate(MessageCandidateId::new()),
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(at),
                updated_at: TimestampMillis::new(at),
            },
            active_revision: None,
            active_candidate: None,
            initial_origin: None,
        }
    }

    #[test]
    fn explicit_muted_speaker_is_allowed() {
        let muted = ConversationParticipantId::new();
        let mut request = request(vec![participant(muted, 0, true, true)]);
        request.forced_speaker = Some(muted);
        let decision = select_group_speaker(&request, GroupSpeakerSelectionSnapshot::Heuristic)
            .expect("eligible explicit speaker should be selected");
        assert_eq!(decision.participant_id, muted);
        assert_eq!(decision.method, SpeakerDecisionMethod::Explicit);
        assert_eq!(decision.fallback, SpeakerFallback::None);
        assert!(decision.reference.is_none());
        assert!(decision.decision_model.is_none());
        assert!(decision.usage_event_id.is_none());
    }

    #[test]
    fn invalid_explicit_and_duplicate_participants_are_rejected() {
        let first = ConversationParticipantId::new();
        let mut unknown = request(vec![participant(first, 0, true, false)]);
        unknown.forced_speaker = Some(ConversationParticipantId::new());
        assert_eq!(
            select_group_speaker(&unknown, GroupSpeakerSelectionSnapshot::Heuristic),
            Err(SpeakerSelectionError::UnknownForcedSpeaker)
        );

        unknown.forced_speaker = None;
        unknown.mention_source = Some(ConversationParticipantId::new());
        assert_eq!(
            select_group_speaker(&unknown, GroupSpeakerSelectionSnapshot::Heuristic),
            Err(SpeakerSelectionError::UnknownMentionSource)
        );

        unknown.mention_source = Some(first);
        unknown.participants[0].eligible = false;
        assert_eq!(
            select_group_speaker(&unknown, GroupSpeakerSelectionSnapshot::Heuristic),
            Err(SpeakerSelectionError::IneligibleExplicitSpeaker)
        );

        let duplicate = request(vec![
            participant(first, 0, true, false),
            participant(first, 0, true, false),
        ]);
        assert_eq!(
            select_group_speaker(&duplicate, GroupSpeakerSelectionSnapshot::Heuristic),
            Err(SpeakerSelectionError::DuplicateParticipantId)
        );
    }

    #[test]
    fn heuristic_balances_participation_and_skips_muted_participants() {
        let loud = ConversationParticipantId::new();
        let quiet = ConversationParticipantId::new();
        let mut balanced = request(vec![
            participant(loud, 10, true, false),
            participant(quiet, 0, true, false),
        ]);
        assert_eq!(
            select_group_speaker(&balanced, GroupSpeakerSelectionSnapshot::Heuristic)
                .expect("quiet participant should win")
                .participant_id,
            quiet
        );

        balanced.participants[1].muted = true;
        assert_eq!(
            select_group_speaker(&balanced, GroupSpeakerSelectionSnapshot::Heuristic)
                .expect("unmuted participant should remain available")
                .participant_id,
            loud
        );
    }

    #[test]
    fn heuristic_ties_follow_participant_order() {
        let first = ConversationParticipantId::new();
        let second = ConversationParticipantId::new();
        let request = request(vec![
            participant(first, 0, true, false),
            participant(second, 0, true, false),
        ]);
        assert_eq!(
            select_group_speaker(&request, GroupSpeakerSelectionSnapshot::Heuristic)
                .expect("first participant should win a tie")
                .participant_id,
            first
        );
    }

    #[test]
    fn heuristic_uses_timeline_recency() {
        let recent = ConversationParticipantId::new();
        let old = ConversationParticipantId::new();
        let mut request = request(vec![
            participant(recent, 0, true, false),
            participant(old, 0, true, false),
        ]);
        request.timeline = vec![
            assistant_message(old, 1),
            assistant_message(recent, 2),
            assistant_message(old, 3),
        ];
        assert_eq!(
            select_group_speaker(&request, GroupSpeakerSelectionSnapshot::Heuristic)
                .expect("less recent participant should win")
                .participant_id,
            recent
        );
    }

    #[test]
    fn heuristic_penalizes_prior_speaker_without_timeline() {
        let prior = ConversationParticipantId::new();
        let other = ConversationParticipantId::new();
        let mut request = request(vec![
            participant(prior, 0, true, false),
            participant(other, 0, true, false),
        ]);
        request.prior_speaker = Some(prior);
        assert_eq!(
            select_group_speaker(&request, GroupSpeakerSelectionSnapshot::Heuristic)
                .expect("other participant should win")
                .participant_id,
            other
        );
    }

    #[test]
    fn round_robin_wraps_skips_muted_and_starts_after_unknown_prior() {
        let first = ConversationParticipantId::new();
        let second = ConversationParticipantId::new();
        let third = ConversationParticipantId::new();
        let mut request = request(vec![
            participant(first, 0, true, false),
            participant(second, 0, true, false),
            participant(third, 0, true, false),
        ]);
        request.prior_speaker = Some(third);
        assert_eq!(
            select_group_speaker(&request, GroupSpeakerSelectionSnapshot::RoundRobin)
                .expect("round robin should wrap")
                .participant_id,
            first
        );

        request.prior_speaker = Some(first);
        request.participants[1].muted = true;
        assert_eq!(
            select_group_speaker(&request, GroupSpeakerSelectionSnapshot::RoundRobin)
                .expect("round robin should skip muted")
                .participant_id,
            third
        );

        request.prior_speaker = Some(ConversationParticipantId::new());
        assert_eq!(
            select_group_speaker(&request, GroupSpeakerSelectionSnapshot::RoundRobin)
                .expect("unknown prior should start at the first available")
                .participant_id,
            first
        );
    }

    #[test]
    fn automatic_selection_reports_unavailable_cast() {
        let first = ConversationParticipantId::new();
        let request = request(vec![participant(first, 0, false, false)]);
        assert_eq!(
            select_group_speaker(&request, GroupSpeakerSelectionSnapshot::Heuristic),
            Err(SpeakerSelectionError::NoAvailableSpeaker)
        );
    }

    #[test]
    fn director_and_llm_require_external_or_explicit_selection() {
        let request = request(vec![participant(
            ConversationParticipantId::new(),
            0,
            true,
            false,
        )]);
        assert_eq!(
            select_group_speaker(&request, GroupSpeakerSelectionSnapshot::Director),
            Err(SpeakerSelectionError::ExplicitSpeakerRequired)
        );
        assert_eq!(
            select_group_speaker(&request, GroupSpeakerSelectionSnapshot::DirectorAction),
            Err(SpeakerSelectionError::ExplicitSpeakerRequired)
        );
        assert_eq!(
            select_group_speaker(&request, GroupSpeakerSelectionSnapshot::Llm),
            Err(SpeakerSelectionError::LlmRequiresExternalPolicy)
        );
    }
}
