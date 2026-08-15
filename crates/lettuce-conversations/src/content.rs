use lettuce_types::{
    AssetId, ConversationBranchId, ConversationId, ConversationParticipantId, GenerationTurnId,
    MessageCandidateId, MessageId, MessageRevisionId, ReplayArtifactId, Revision, TimestampMillis,
    ToolExecutionId,
};
use serde::{Deserialize, Serialize};

use crate::ValidationError;
use crate::snapshot::{ModelSelectionSnapshot, ValidateSnapshot};
use crate::validation::{
    MAX_ANNOTATION_BYTES, MAX_AUTHORED_TEXT_BYTES, MAX_PARTS, MAX_REASONING_BYTES,
    validate_collection, validate_revision_timestamps, validate_text,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum MessagePart {
    Text {
        text: String,
    },
    MediaAsset {
        asset_id: AssetId,
        role: MediaAssetRole,
    },
    ReasoningSummary {
        text: String,
    },
    ToolCall {
        execution_id: ToolExecutionId,
    },
    ToolResult {
        execution_id: ToolExecutionId,
    },
    Annotation {
        annotation: Annotation,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaAssetRole {
    Inline,
    Attachment,
    Avatar,
    Scene,
    Reference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Annotation {
    pub version: u32,
    pub kind: AnnotationKind,
    pub payload: AnnotationPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationKind {
    SceneEdited,
    Safety,
    SystemNotice,
    CompanionEffect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AnnotationPayload {
    SceneEdited {
        scene_id: Option<lettuce_types::SceneId>,
        direction: Option<String>,
    },
    Safety {
        action: SafetyAction,
        code: SafetyCode,
    },
    SystemNotice {
        code: SystemNoticeCode,
        text: String,
    },
    CompanionEffect {
        effect_id: lettuce_types::CompanionEffectId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyAction {
    Allowed,
    Transformed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyCode {
    Policy,
    Provider,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemNoticeCode {
    Imported,
    Recovery,
    Moderation,
    Information,
}

impl MessagePart {
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Self::Text { text } => {
                validate_text("message_part.text", text, MAX_AUTHORED_TEXT_BYTES, true)
            }
            Self::ReasoningSummary { text } => validate_text(
                "message_part.reasoning_summary",
                text,
                MAX_REASONING_BYTES,
                true,
            ),
            Self::Annotation { annotation } => annotation.validate(),
            Self::MediaAsset { .. } | Self::ToolCall { .. } | Self::ToolResult { .. } => Ok(()),
        }
    }
}

impl Annotation {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.version != 1 {
            return Err(ValidationError::UnsupportedVersion {
                field: "message.annotation",
                version: self.version,
            });
        }
        match (&self.kind, &self.payload) {
            (AnnotationKind::SceneEdited, AnnotationPayload::SceneEdited { direction, .. }) => {
                if let Some(direction) = direction {
                    validate_text(
                        "message.annotation.scene_direction",
                        direction,
                        MAX_ANNOTATION_BYTES,
                        true,
                    )?;
                }
                Ok(())
            }
            (AnnotationKind::Safety, AnnotationPayload::Safety { .. })
            | (AnnotationKind::CompanionEffect, AnnotationPayload::CompanionEffect { .. }) => {
                Ok(())
            }
            (AnnotationKind::SystemNotice, AnnotationPayload::SystemNotice { text, .. }) => {
                validate_text(
                    "message.annotation.notice",
                    text,
                    MAX_ANNOTATION_BYTES,
                    false,
                )
            }
            _ => Err(ValidationError::InvalidValue {
                field: "message.annotation.kind_payload",
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Scene,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageVisibility {
    Visible,
    Hidden,
    Tombstoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum MessageRenderSource {
    Revision(MessageRevisionId),
    Candidate(MessageCandidateId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub id: MessageId,
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub parent_message_id: Option<MessageId>,
    pub author_participant_id: Option<ConversationParticipantId>,
    pub role: MessageRole,
    pub logical_time: TimestampMillis,
    pub effective_time: TimestampMillis,
    pub visibility: MessageVisibility,
    pub pinned: bool,
    pub scene_edited: bool,
    pub active_render_source: MessageRenderSource,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl Message {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_revision_timestamps(
            "message.timestamps",
            self.revision,
            self.created_at,
            self.updated_at,
        )?;
        match (self.role, self.author_participant_id) {
            (MessageRole::System | MessageRole::Scene, Some(_)) => {
                Err(ValidationError::InvalidReference {
                    field: "message.non_chat_author",
                })
            }
            (MessageRole::User | MessageRole::Assistant, None) => {
                Err(ValidationError::InvalidReference {
                    field: "message.participant_author",
                })
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageRevision {
    pub id: MessageRevisionId,
    pub message_id: MessageId,
    pub sequence: Revision,
    pub parts: Vec<MessagePart>,
    pub authored_at: TimestampMillis,
    pub source_turn_id: Option<GenerationTurnId>,
    pub provider_replay: Option<ReplayArtifactRef>,
}

impl MessageRevision {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.sequence.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        validate_collection("message_revision.parts", &self.parts, MAX_PARTS)?;
        for part in &self.parts {
            part.validate()?;
        }
        if let Some(replay) = &self.provider_replay {
            replay.validate()?;
            if replay.retention != ReplayRetention::Conversation {
                return Err(ValidationError::InvalidValue {
                    field: "message_revision.provider_replay.retention",
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageCandidate {
    pub id: MessageCandidateId,
    pub message_id: MessageId,
    pub turn_id: GenerationTurnId,
    pub attempt_id: lettuce_types::GenerationAttemptId,
    pub ordinal: u16,
    pub parts: Vec<MessagePart>,
    pub model: ModelSelectionSnapshot,
    pub created_at: TimestampMillis,
    pub provider_replay: Option<ReplayArtifactRef>,
}

impl MessageCandidate {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_collection("message_candidate.parts", &self.parts, MAX_PARTS)?;
        for part in &self.parts {
            part.validate()?;
        }
        self.model.validate_snapshot("message_candidate.model")?;
        if let Some(replay) = &self.provider_replay {
            replay.validate()?;
            if replay.retention != ReplayRetention::Conversation {
                return Err(ValidationError::InvalidValue {
                    field: "message_candidate.provider_replay.retention",
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayArtifactRef {
    pub artifact_id: ReplayArtifactId,
    pub digest: lettuce_types::ContentHash,
    pub schema_version: u32,
    pub byte_size: u64,
    pub retention: ReplayRetention,
    pub codec: ReplayCodec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayRetention {
    Conversation,
    Ephemeral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayCodec {
    Json,
    Cbor,
    Binary,
}

impl ReplayArtifactRef {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.schema_version != 1 {
            return Err(ValidationError::UnsupportedVersion {
                field: "replay_artifact_ref",
                version: self.schema_version,
            });
        }
        if self.byte_size > 16 * 1024 * 1024 {
            return Err(ValidationError::TooLarge {
                field: "replay_artifact_ref.byte_size",
            });
        }
        Ok(())
    }
}
