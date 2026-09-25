use lettuce_characters::{CharacterRepository, RepositoryError as CharacterRepositoryError};
use lettuce_companions::{
    CompanionContinueRepositoryError, CompanionConversationContinuer, CompanionConversationSender,
    CompanionSendRepositoryError, CompanionStateReplacement, CompanionStateRepository,
    CompanionStateRepositoryError, CompanionTurnEffectSeed, CompanionTurnInput,
    PreparedCompanionContinue, PreparedCompanionSend, apply_turn, signals_from_classification,
    unavailable_signal_bundle,
};
use lettuce_conversations::{
    ContinueConversation, ContinueConversationResult, ConversationReader, ConversationRepository,
    ConversationRepositoryError, MemoryModeSnapshot, MessagePart, OperationKind, SendConversation,
    SendConversationResult, resolve_effective_settings,
};
use lettuce_jobs::handle::CancellationToken;
use lettuce_types::TimestampMillis;

use crate::{CompanionEmotionEngine, CompanionEmotionGenerationError};

#[derive(Debug)]
pub struct CompanionTurnCoordinator<'a, S, E: ?Sized> {
    sources: &'a S,
    emotion: Option<&'a E>,
}

impl<S, E> CompanionTurnCoordinator<'_, S, E>
where
    S: ConversationRepository
        + CharacterRepository
        + CompanionStateRepository
        + CompanionConversationContinuer,
    E: ?Sized,
{
    pub fn begin_continue(
        &self,
        command: &ContinueConversation,
        now: TimestampMillis,
    ) -> Result<ContinueConversationResult, CompanionTurnError> {
        lettuce_conversations::ConversationMutation::Continue(command.clone())
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        if self
            .sources
            .operation_record(
                command.conversation_id,
                OperationKind::Continue,
                &command.operation,
            )?
            .is_some()
        {
            return self
                .sources
                .begin_continue(command, now)
                .map_err(Into::into);
        }
        let aggregate = ConversationReader::get(self.sources, command.conversation_id)?;
        if aggregate.conversation.revision != command.expected_revision {
            return Err(ConversationRepositoryError::StaleRevision {
                expected: command.expected_revision,
                actual: aggregate.conversation.revision,
            }
            .into());
        }
        let Some(owner) =
            crate::companion::companion_clock::companion_state_owner(&aggregate.conversation)
        else {
            return self
                .sources
                .begin_continue(command, now)
                .map_err(Into::into);
        };
        let companion =
            companion_state(self.sources, &aggregate.conversation, owner, now)?.is_some();
        let dynamic = resolve_effective_settings(&aggregate.conversation, None)
            .map_err(ConversationRepositoryError::Invalid)?
            .memory
            .is_some_and(|memory| memory.mode == MemoryModeSnapshot::Dynamic);
        if !companion || !dynamic {
            return self
                .sources
                .begin_continue(command, now)
                .map_err(Into::into);
        }
        let prepared = PreparedCompanionContinue::new(command.clone())?;
        CompanionConversationContinuer::begin_companion_continue(self.sources, prepared, now)
            .map_err(Into::into)
    }
}

impl<'a, S, E: ?Sized> CompanionTurnCoordinator<'a, S, E> {
    #[must_use]
    pub const fn new(sources: &'a S, emotion: Option<&'a E>) -> Self {
        Self { sources, emotion }
    }
}

impl<S, E> CompanionTurnCoordinator<'_, S, E>
where
    S: ConversationRepository
        + CharacterRepository
        + CompanionStateRepository
        + CompanionConversationSender,
    E: CompanionEmotionEngine + ?Sized,
{
    pub fn begin_send(
        &self,
        command: &SendConversation,
        now: TimestampMillis,
        cancellation: &CancellationToken,
    ) -> Result<SendConversationResult, CompanionTurnError> {
        command
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        if self
            .sources
            .operation_record(
                command.conversation_id,
                OperationKind::Send,
                &command.operation,
            )?
            .is_some()
        {
            return self.sources.begin_send(command, now).map_err(Into::into);
        }

        let aggregate = ConversationReader::get(self.sources, command.conversation_id)?;
        if aggregate.conversation.revision != command.expected_revision {
            return Err(ConversationRepositoryError::StaleRevision {
                expected: command.expected_revision,
                actual: aggregate.conversation.revision,
            }
            .into());
        }
        let Some(owner) =
            crate::companion::companion_clock::companion_state_owner(&aggregate.conversation)
        else {
            return self.sources.begin_send(command, now).map_err(Into::into);
        };
        let clock = crate::companion::companion_clock::companion_clock_context(
            self.sources,
            &aggregate.conversation,
        )
        .map_err(|_| CompanionTurnError::CharacterMissing)?;
        let Some(snapshot) = companion_state(self.sources, &aggregate.conversation, owner, now)?
        else {
            return self.sources.begin_send(command, now).map_err(Into::into);
        };
        let config = CharacterRepository::get(self.sources, owner.character_id)?
            .ok_or(CompanionTurnError::CharacterMissing)?
            .character
            .defaults
            .companion_soul
            .unwrap_or_default();
        let effective_now = clock.effective_now(now);
        let text = classification_text(&command.message.parts);
        let bundle = match self.emotion {
            Some(engine) => match engine.classify_emotion(&text, cancellation) {
                Ok(Some(classification)) => signals_from_classification(&classification),
                Ok(None) => unavailable_signal_bundle(),
                Err(CompanionEmotionGenerationError::Unavailable) => {
                    tracing::warn!(
                        conversation_id = %command.conversation_id,
                        "companion emotion classifier unavailable; using neutral update"
                    );
                    unavailable_signal_bundle()
                }
                Err(CompanionEmotionGenerationError::Cancelled) => {
                    return Err(CompanionTurnError::Cancelled);
                }
            },
            None => unavailable_signal_bundle(),
        };
        let transition = apply_turn(
            &snapshot.state,
            &config.soul.baseline_affect,
            &config.soul.regulation_style,
            &config.relationship_defaults,
            &CompanionTurnInput {
                signals: bundle.signals,
                emotion_delta: bundle.emotion_delta,
                relationship_delta: bundle.relationship_delta,
                confidence: bundle.confidence,
                now: effective_now,
            },
        );
        let effect_seed = resolve_effective_settings(&aggregate.conversation, None)
            .map_err(ConversationRepositoryError::Invalid)?
            .memory
            .is_some_and(|memory| memory.mode == MemoryModeSnapshot::Dynamic)
            .then(|| CompanionTurnEffectSeed::from_transition(&transition));
        let prepared = PreparedCompanionSend::new(
            command.clone(),
            owner,
            CompanionStateReplacement {
                expected_session_revision: snapshot.session_revision,
                expected_relationship_revision: snapshot.relationship_revision,
                state: transition.current,
                applied_at: now,
            },
            effect_seed,
        )?;
        CompanionConversationSender::begin_companion_send(self.sources, prepared, now)
            .map_err(Into::into)
    }
}

/// The conversation's companion state, created from the character's companion
/// settings when a companion chat has none yet (legacy `current_state` falling
/// back to `default_state`); `None` when the chat is not a companion chat.
fn companion_state<S>(
    sources: &S,
    conversation: &lettuce_conversations::Conversation,
    owner: lettuce_companions::CompanionStateOwner,
    now: TimestampMillis,
) -> Result<Option<lettuce_companions::CompanionStateSnapshot>, CompanionTurnError>
where
    S: CharacterRepository + CompanionStateRepository + ?Sized,
{
    if let Some(snapshot) = CompanionStateRepository::get(sources, owner)? {
        return Ok(Some(snapshot));
    }
    if !crate::companion::companion_clock::companion_clock_context(sources, conversation)
        .map_err(|_| CompanionTurnError::CharacterMissing)?
        .companion
    {
        return Ok(None);
    }
    let config = CharacterRepository::get(sources, owner.character_id)?
        .ok_or(CompanionTurnError::CharacterMissing)?
        .character
        .defaults
        .companion_soul
        .unwrap_or_default();
    CompanionStateRepository::create(
        sources,
        owner,
        lettuce_companions::initial_runtime_state(
            &config.soul.baseline_affect,
            &config.soul.regulation_style,
            &config.relationship_defaults,
        ),
        now,
    )
    .map(Some)
    .map_err(Into::into)
}

fn classification_text(parts: &[MessagePart]) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionTurnError {
    #[error("conversation operation failed: {0}")]
    Conversation(#[from] ConversationRepositoryError),
    #[error("character repository failed: {0}")]
    Character(#[from] CharacterRepositoryError),
    #[error("companion state repository failed: {0:?}")]
    State(CompanionStateRepositoryError),
    #[error("companion send repository failed: {0:?}")]
    Send(CompanionSendRepositoryError),
    #[error("companion continue repository failed: {0:?}")]
    Continue(CompanionContinueRepositoryError),
    #[error("companion character is missing")]
    CharacterMissing,
    #[error("companion emotion classification was cancelled")]
    Cancelled,
}

impl From<CompanionStateRepositoryError> for CompanionTurnError {
    fn from(error: CompanionStateRepositoryError) -> Self {
        Self::State(error)
    }
}

impl From<CompanionSendRepositoryError> for CompanionTurnError {
    fn from(error: CompanionSendRepositoryError) -> Self {
        Self::Send(error)
    }
}

impl From<CompanionContinueRepositoryError> for CompanionTurnError {
    fn from(error: CompanionContinueRepositoryError) -> Self {
        Self::Continue(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_text_preserves_part_order_and_ignores_non_text_parts() {
        let parts = vec![
            MessagePart::Text { text: "one".into() },
            MessagePart::MediaAsset {
                asset_id: lettuce_types::AssetId::new(),
                role: lettuce_conversations::MediaAssetRole::Attachment,
            },
            MessagePart::Text { text: "two".into() },
        ];
        assert_eq!(classification_text(&parts), "one\ntwo");
    }
}
