use std::collections::HashSet;

use lettuce_types::{AssetId, PersonaId, Revision, TimestampMillis};
use serde::{Deserialize, Serialize};

use crate::character::ImageRecommendation;
use crate::constants::{
    MAX_COLLECTION_ITEMS, validate_collection, validate_contiguous, validate_name,
    validate_revision_timestamps, validate_text,
};
use crate::presentation::Crop;
use crate::{LifecycleStatus, ValidationError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonaMediaSlot {
    Avatar,
    DesignReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonaMediaLink {
    pub asset_id: AssetId,
    pub slot: PersonaMediaSlot,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PersonaMedia {
    pub links: Vec<PersonaMediaLink>,
}

impl PersonaMedia {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_collection("persona.media", &self.links, MAX_COLLECTION_ITEMS)?;
        let mut avatars = 0;
        let mut references = Vec::new();
        for link in &self.links {
            match link.slot {
                PersonaMediaSlot::Avatar => avatars += 1,
                PersonaMediaSlot::DesignReference => references.push(link.ordinal),
            }
        }
        if avatars > 1 {
            return Err(ValidationError::Invariant {
                field: "persona.media.avatar",
            });
        }
        let mut reference_asset_ids = HashSet::new();
        let mut all_asset_ids = HashSet::new();
        for link in &self.links {
            if !all_asset_ids.insert(link.asset_id) {
                return Err(ValidationError::Duplicate {
                    field: "persona.media.asset_ids",
                });
            }
        }
        if self
            .links
            .iter()
            .filter(|link| link.slot == PersonaMediaSlot::DesignReference)
            .any(|link| !reference_asset_ids.insert(link.asset_id))
        {
            return Err(ValidationError::Duplicate {
                field: "persona.media.design_reference.asset_ids",
            });
        }
        references.sort_unstable();
        validate_contiguous("persona.media.design_reference.order", references)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Persona {
    pub id: PersonaId,
    pub status: LifecycleStatus,
    pub title: String,
    pub description: String,
    pub nickname: Option<String>,
    pub design_description: Option<String>,
    pub avatar_crop: Option<Crop>,
    pub image_recommendation: Option<ImageRecommendation>,
    pub media: PersonaMedia,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl Persona {
    pub fn new(
        id: PersonaId,
        title: String,
        description: String,
        created_at: TimestampMillis,
    ) -> Result<Self, ValidationError> {
        let persona = Self {
            id,
            status: LifecycleStatus::Active,
            title,
            description,
            nickname: None,
            design_description: None,
            avatar_crop: None,
            image_recommendation: None,
            media: PersonaMedia::default(),
            revision: Revision::INITIAL,
            created_at,
            updated_at: created_at,
        };
        persona.validate()?;
        Ok(persona)
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_revision_timestamps(
            "persona.timestamps",
            self.revision,
            self.created_at,
            self.updated_at,
        )?;
        validate_name("persona.title", &self.title)?;
        if self.description.trim().is_empty() {
            return Err(ValidationError::Blank {
                field: "persona.description",
            });
        }
        validate_text("persona.description", &self.description)?;
        if let Some(nickname) = &self.nickname {
            validate_text("persona.nickname", nickname)?;
        }
        if let Some(description) = &self.design_description {
            validate_text("persona.design_description", description)?;
        }
        if let Some(crop) = self.avatar_crop {
            crop.validate()?;
        }
        if let Some(recommendation) = &self.image_recommendation {
            recommendation.validate()?;
        }
        self.media.validate()
    }

    pub fn bump_revision(&mut self, now: TimestampMillis) -> Result<(), ValidationError> {
        self.revision = self.revision.next()?;
        self.updated_at = now;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Persona, PersonaMediaLink, PersonaMediaSlot};
    use crate::{LifecycleStatus, PersonaMedia, ValidationError};
    use lettuce_types::{AssetId, PersonaId, Revision, TimestampMillis};

    #[test]
    fn description_is_required() {
        let persona = Persona {
            id: PersonaId::new(),
            status: LifecycleStatus::Active,
            title: "Writer".into(),
            description: " ".into(),
            nickname: None,
            design_description: None,
            avatar_crop: None,
            image_recommendation: None,
            media: PersonaMedia::default(),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(0),
            updated_at: TimestampMillis::new(0),
        };
        assert!(persona.validate().is_err());
    }

    #[test]
    fn design_references_are_unique_assets_in_contiguous_order() {
        let asset_id = AssetId::new();
        let media = PersonaMedia {
            links: vec![
                PersonaMediaLink {
                    asset_id,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 0,
                },
                PersonaMediaLink {
                    asset_id,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 1,
                },
            ],
        };
        assert!(media.validate().is_err());
    }

    #[test]
    fn media_asset_ids_are_unique_across_avatar_and_design_slots() {
        let asset_id = AssetId::new();
        let media = PersonaMedia {
            links: vec![
                PersonaMediaLink {
                    asset_id,
                    slot: PersonaMediaSlot::Avatar,
                    ordinal: 0,
                },
                PersonaMediaLink {
                    asset_id,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 0,
                },
            ],
        };
        assert!(media.validate().is_err());
    }

    #[test]
    fn authored_persona_rejects_reversed_timestamps() {
        let mut persona = Persona {
            id: PersonaId::new(),
            status: LifecycleStatus::Active,
            title: "Writer".into(),
            description: "A writer".into(),
            nickname: None,
            design_description: None,
            avatar_crop: None,
            image_recommendation: None,
            media: PersonaMedia::default(),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(5),
            updated_at: TimestampMillis::new(4),
        };
        assert!(matches!(
            persona.validate(),
            Err(ValidationError::InvalidTimestampOrder {
                field: "persona.timestamps"
            })
        ));
        persona.revision = Revision::new(0);
        assert_eq!(persona.validate(), Err(ValidationError::ZeroRevision));
    }
}
