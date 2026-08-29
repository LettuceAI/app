use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            #[must_use]
            pub const fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self)
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }

        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

define_id!(AssetId);
define_id!(CharacterId);
define_id!(ConversationId);
define_id!(ConversationBranchId);
define_id!(ConversationParticipantId);
define_id!(ConversationStarterId);
define_id!(GenerationCandidateId);
define_id!(MessageCandidateId);
define_id!(GenerationTurnId);
define_id!(GenerationAttemptId);
define_id!(GroupId);
define_id!(JobId);
define_id!(LorebookId);
define_id!(LorebookEntryId);
define_id!(MediaBlobId);
define_id!(MemoryId);
define_id!(MemoryRevisionId);
define_id!(MemorySpaceId);
define_id!(MessageId);
define_id!(MessageRevisionId);
define_id!(ModelProfileId);
define_id!(ModelArtifactId);
define_id!(OperationId);
define_id!(ParticipantId);
define_id!(UsageEventId);
define_id!(CompanionEffectId);
define_id!(PersonaId);
define_id!(PromptDocumentId);
define_id!(PromptEntryId);
define_id!(ProviderAccountId);
define_id!(RequestId);
define_id!(SceneAssetLinkId);
define_id!(SceneId);
define_id!(SceneVariantId);
define_id!(StarterMessageId);
define_id!(ToolExecutionId);
define_id!(VoiceProfileId);
define_id!(ReplayArtifactId);
define_id!(SnapshotArtifactId);
define_id!(OperationRecordId);
define_id!(OutboxEventId);

#[cfg(test)]
mod tests {
    use super::ConversationId;

    #[test]
    fn id_round_trips_through_text() {
        let id = ConversationId::new();
        let parsed = id.to_string().parse::<ConversationId>();

        assert_eq!(parsed, Ok(id));
    }
}
