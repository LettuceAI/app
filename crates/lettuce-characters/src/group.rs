use lettuce_types::{
    AssetId, CharacterId, GroupId, ModelProfileId, PromptDocumentId, Revision, SceneId,
    TimestampMillis,
};
use serde::{Deserialize, Serialize};

use crate::constants::{
    MAX_COLLECTION_ITEMS, validate_collection, validate_contiguous, validate_name, validate_unique,
};
use crate::presentation::ChatAppearanceV1;
use crate::{
    ChatMode, LifecycleStatus, MemoryPolicy, Selection, SpeakerSelection, ValidationError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupMember {
    pub character_id: CharacterId,
    pub ordinal: u32,
    pub muted: bool,
    pub model_profile_override: Option<ModelProfileId>,
}

impl GroupMember {
    pub fn validate(&self) -> Result<(), ValidationError> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupProfile {
    pub id: GroupId,
    pub status: LifecycleStatus,
    pub name: String,
    pub chat_mode: ChatMode,
    pub persona: Selection<lettuce_types::PersonaId>,
    pub speaker_selection: SpeakerSelection,
    pub memory_policy: MemoryPolicy,
    pub disable_character_lorebooks: bool,
    pub group_conversation_prompt_id: Option<PromptDocumentId>,
    pub group_roleplay_prompt_id: Option<PromptDocumentId>,
    pub presentation: ChatAppearanceV1,
    pub members: Vec<GroupMember>,
    pub starting_scene_id: Option<SceneId>,
    pub background_asset_id: Option<AssetId>,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl GroupProfile {
    pub fn new(
        id: GroupId,
        name: String,
        members: Vec<GroupMember>,
        created_at: TimestampMillis,
    ) -> Result<Self, ValidationError> {
        let group = Self {
            id,
            status: LifecycleStatus::Active,
            name,
            chat_mode: ChatMode::Roleplay,
            persona: Selection::Inherit,
            speaker_selection: SpeakerSelection::RoundRobin,
            memory_policy: MemoryPolicy::Manual,
            disable_character_lorebooks: false,
            group_conversation_prompt_id: None,
            group_roleplay_prompt_id: None,
            presentation: ChatAppearanceV1::default(),
            members,
            starting_scene_id: None,
            background_asset_id: None,
            revision: Revision::INITIAL,
            created_at,
            updated_at: created_at,
        };
        group.validate()?;
        Ok(group)
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_name("group.name", &self.name)?;
        validate_collection("group.members", &self.members, MAX_COLLECTION_ITEMS)?;
        if self.members.len() < 2 {
            return Err(ValidationError::Invariant {
                field: "group.minimum_members",
            });
        }
        validate_unique(
            "group.member_character_ids",
            self.members.iter().map(|member| member.character_id),
        )?;
        validate_contiguous(
            "group.member_order",
            self.members.iter().map(|member| member.ordinal),
        )?;
        if self.members.iter().all(|member| member.muted) {
            return Err(ValidationError::Invariant {
                field: "group.active_member",
            });
        }
        for member in &self.members {
            member.validate()?;
        }
        self.presentation.validate()?;
        Ok(())
    }

    pub fn bump_revision(&mut self, now: TimestampMillis) -> Result<(), ValidationError> {
        self.revision = self.revision.next()?;
        self.updated_at = now;
        Ok(())
    }

    #[must_use]
    pub fn active_member_count(&self) -> usize {
        self.members.iter().filter(|member| !member.muted).count()
    }
}

#[cfg(test)]
mod tests {
    use super::{GroupMember, GroupProfile};
    use crate::{
        ChatAppearanceV1, ChatMode, LifecycleStatus, MemoryPolicy, Selection, SpeakerSelection,
    };
    use lettuce_types::{CharacterId, GroupId, Revision, TimestampMillis};

    fn member(ordinal: u32, muted: bool) -> GroupMember {
        GroupMember {
            character_id: CharacterId::new(),
            ordinal,
            muted,
            model_profile_override: None,
        }
    }

    fn group(members: Vec<GroupMember>) -> GroupProfile {
        GroupProfile {
            id: GroupId::new(),
            status: LifecycleStatus::Active,
            name: "Cast".into(),
            chat_mode: ChatMode::Roleplay,
            persona: Selection::Inherit,
            speaker_selection: SpeakerSelection::RoundRobin,
            memory_policy: MemoryPolicy::Manual,
            disable_character_lorebooks: false,
            group_conversation_prompt_id: None,
            group_roleplay_prompt_id: None,
            presentation: ChatAppearanceV1::default(),
            members,
            starting_scene_id: None,
            background_asset_id: None,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(0),
            updated_at: TimestampMillis::new(0),
        }
    }

    #[test]
    fn groups_require_two_members_and_one_active_member() {
        assert!(group(vec![member(0, false)]).validate().is_err());
        assert!(
            group(vec![member(0, true), member(1, true)])
                .validate()
                .is_err()
        );
        assert!(
            group(vec![member(0, false), member(1, true)])
                .validate()
                .is_ok()
        );
    }
}
