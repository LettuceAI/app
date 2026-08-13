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

/// Mutable authored fields accepted by a persona revision.
///
/// Identity, lifecycle, media associations, and revision metadata are owned by
/// the repository and therefore cannot be supplied by a caller-owned update.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonaDraftUpdate {
    pub title: String,
    pub description: String,
    pub nickname: Option<String>,
    pub design_description: Option<String>,
    pub avatar_crop: Option<Crop>,
    pub image_recommendation: Option<ImageRecommendation>,
}

impl PersonaDraftUpdate {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_authored_fields(
            &self.title,
            &self.description,
            self.nickname.as_ref(),
            self.design_description.as_ref(),
            self.avatar_crop,
            self.image_recommendation.as_ref(),
        )
    }
}

fn validate_authored_fields(
    title: &str,
    description: &str,
    nickname: Option<&String>,
    design_description: Option<&String>,
    avatar_crop: Option<Crop>,
    image_recommendation: Option<&ImageRecommendation>,
) -> Result<(), ValidationError> {
    validate_name("persona.title", title)?;
    if description.trim().is_empty() {
        return Err(ValidationError::Blank {
            field: "persona.description",
        });
    }
    validate_text("persona.description", description)?;
    if let Some(nickname) = nickname {
        validate_text("persona.nickname", nickname)?;
    }
    if let Some(description) = design_description {
        validate_text("persona.design_description", description)?;
    }
    if let Some(crop) = avatar_crop {
        crop.validate()?;
    }
    if let Some(recommendation) = image_recommendation {
        recommendation.validate()?;
    }
    Ok(())
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

/// The revisioned singleton that records the application's default persona.
/// It is separate from a persona's authored revision so changing the default
/// does not mutate the selected persona.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonaDefaultState {
    pub persona_id: Option<PersonaId>,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl PersonaDefaultState {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_revision_timestamps(
            "persona.default.timestamps",
            self.revision,
            self.created_at,
            self.updated_at,
        )
    }
}

/// A consistent read of the default singleton and its selected persona.
/// Adapters must materialize both values from one read snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonaDefaultSnapshot {
    pub state: PersonaDefaultState,
    pub persona: Option<Persona>,
}

impl PersonaDefaultSnapshot {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.state.validate()?;
        match (self.state.persona_id, &self.persona) {
            (None, None) => Ok(()),
            (Some(expected_id), Some(persona)) => {
                if expected_id != persona.id {
                    return Err(ValidationError::InvalidReference {
                        field: "persona.default.persona_id",
                    });
                }
                if persona.status != LifecycleStatus::Active {
                    return Err(ValidationError::InvalidValue {
                        field: "persona.default.persona.status",
                    });
                }
                persona.validate()
            }
            _ => Err(ValidationError::InvalidReference {
                field: "persona.default.persona",
            }),
        }
    }
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
        validate_authored_fields(
            &self.title,
            &self.description,
            self.nickname.as_ref(),
            self.design_description.as_ref(),
            self.avatar_crop,
            self.image_recommendation.as_ref(),
        )?;
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
    use super::{
        Persona, PersonaDefaultSnapshot, PersonaDefaultState, PersonaDraftUpdate, PersonaMediaLink,
        PersonaMediaSlot,
    };
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

    #[test]
    fn persona_draft_round_trips_and_excludes_repository_fields() {
        let draft = PersonaDraftUpdate {
            title: "Writer".into(),
            description: "A writer".into(),
            nickname: Some("W".into()),
            design_description: Some("Ink-stained".into()),
            avatar_crop: None,
            image_recommendation: None,
        };
        draft.validate().expect("draft should validate");
        let encoded = serde_json::to_string(&draft).expect("draft serializes");
        assert_eq!(
            serde_json::from_str::<PersonaDraftUpdate>(&encoded).expect("draft decodes"),
            draft
        );
        for field in [
            "id",
            "status",
            "media",
            "revision",
            "created_at",
            "updated_at",
        ] {
            let value = format!(
                r#"{{"title":"Writer","description":"A writer","nickname":null,"design_description":null,"avatar_crop":null,"image_recommendation":null,"{field}":null}}"#
            );
            assert!(
                serde_json::from_str::<PersonaDraftUpdate>(&value).is_err(),
                "draft unexpectedly accepted repository field {field}"
            );
        }
    }

    #[test]
    fn persona_draft_uses_authored_persona_validation() {
        let draft = PersonaDraftUpdate {
            title: " ".into(),
            description: "A writer".into(),
            nickname: None,
            design_description: None,
            avatar_crop: None,
            image_recommendation: None,
        };
        assert_eq!(
            draft.validate(),
            Err(ValidationError::Blank {
                field: "persona.title"
            })
        );
        let draft = PersonaDraftUpdate {
            title: "Writer".into(),
            description: " ".into(),
            nickname: None,
            design_description: None,
            avatar_crop: None,
            image_recommendation: None,
        };
        assert_eq!(
            draft.validate(),
            Err(ValidationError::Blank {
                field: "persona.description"
            })
        );
    }

    #[test]
    fn default_state_validates_revision_and_timestamp_order() {
        let mut state = PersonaDefaultState {
            persona_id: Some(PersonaId::new()),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(5),
            updated_at: TimestampMillis::new(5),
        };
        state.validate().expect("default state should validate");
        let encoded = serde_json::to_string(&state).expect("default state serializes");
        assert_eq!(
            serde_json::from_str::<PersonaDefaultState>(&encoded).expect("default state decodes"),
            state
        );
        assert!(
            serde_json::from_str::<PersonaDefaultState>(
                r#"{"persona_id":null,"revision":1,"created_at":5,"updated_at":5,"extra":true}"#
            )
            .is_err()
        );
        state.updated_at = TimestampMillis::new(4);
        assert!(matches!(
            state.validate(),
            Err(ValidationError::InvalidTimestampOrder {
                field: "persona.default.timestamps"
            })
        ));
        state.updated_at = TimestampMillis::new(5);
        state.revision = Revision::new(0);
        assert_eq!(state.validate(), Err(ValidationError::ZeroRevision));
    }

    #[test]
    fn default_snapshot_requires_matching_active_persona_or_no_persona() {
        let persona = Persona::new(
            PersonaId::new(),
            "Writer".into(),
            "A writer".into(),
            TimestampMillis::new(5),
        )
        .expect("persona should validate");
        let state = PersonaDefaultState {
            persona_id: Some(persona.id),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(5),
            updated_at: TimestampMillis::new(5),
        };
        let snapshot = PersonaDefaultSnapshot {
            state: state.clone(),
            persona: Some(persona.clone()),
        };
        snapshot.validate().expect("matching active snapshot");
        let encoded = serde_json::to_string(&snapshot).expect("snapshot serializes");
        assert_eq!(
            serde_json::from_str::<PersonaDefaultSnapshot>(&encoded).expect("snapshot decodes"),
            snapshot
        );
        assert!(serde_json::from_str::<PersonaDefaultSnapshot>(
            r#"{"state":{"persona_id":null,"revision":1,"created_at":5,"updated_at":5},"persona":null,"extra":true}"#
        )
        .is_err());
        PersonaDefaultSnapshot {
            state: PersonaDefaultState {
                persona_id: None,
                ..state.clone()
            },
            persona: None,
        }
        .validate()
        .expect("empty default snapshot");

        let mismatched = PersonaDefaultSnapshot {
            state: PersonaDefaultState {
                persona_id: Some(PersonaId::new()),
                ..state.clone()
            },
            persona: Some(persona.clone()),
        };
        assert!(matches!(
            mismatched.validate(),
            Err(ValidationError::InvalidReference {
                field: "persona.default.persona_id"
            })
        ));
        let missing = PersonaDefaultSnapshot {
            state,
            persona: None,
        };
        assert!(matches!(
            missing.validate(),
            Err(ValidationError::InvalidReference {
                field: "persona.default.persona"
            })
        ));
        let mut archived = persona;
        archived.status = LifecycleStatus::Archived;
        let archived_snapshot = PersonaDefaultSnapshot {
            state: PersonaDefaultState {
                persona_id: Some(archived.id),
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(5),
                updated_at: TimestampMillis::new(5),
            },
            persona: Some(archived),
        };
        assert!(matches!(
            archived_snapshot.validate(),
            Err(ValidationError::InvalidValue {
                field: "persona.default.persona.status"
            })
        ));
    }
}
