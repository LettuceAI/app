use std::collections::HashSet;

use lettuce_types::{
    AssetId, CharacterId, GroupId, Revision, SceneAssetLinkId, SceneId, SceneVariantId,
    TimestampMillis,
};
use serde::{Deserialize, Serialize};

use crate::constants::{
    MAX_COLLECTION_ITEMS, validate_collection, validate_contiguous, validate_text,
};
use crate::{LifecycleStatus, ValidationError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SceneOwner {
    Character(CharacterId),
    Group(GroupId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScenePart {
    Text { text: String },
    InlineAsset { link_id: SceneAssetLinkId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneDocumentV1 {
    pub format_version: u32,
    pub parts: Vec<ScenePart>,
}

impl SceneDocumentV1 {
    pub fn new(parts: Vec<ScenePart>) -> Result<Self, ValidationError> {
        let document = Self {
            format_version: 1,
            parts,
        };
        document.validate()?;
        Ok(document)
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.format_version != 1 {
            return Err(ValidationError::UnsupportedVersion {
                field: "scene_document",
                version: self.format_version,
            });
        }
        validate_collection("scene_document.parts", &self.parts, MAX_COLLECTION_ITEMS)?;
        for part in &self.parts {
            if let ScenePart::Text { text } = part {
                validate_text("scene_document.text", text)?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn inline_link_ids(&self) -> Vec<SceneAssetLinkId> {
        self.parts
            .iter()
            .filter_map(|part| match part {
                ScenePart::InlineAsset { link_id } => Some(*link_id),
                ScenePart::Text { .. } => None,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneAssetSlot {
    Background,
    Inline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneAssetLink {
    pub id: SceneAssetLinkId,
    pub asset_id: AssetId,
    pub slot: SceneAssetSlot,
    pub ordinal: u32,
}

impl SceneAssetLink {
    fn validate(&self) -> Result<(), ValidationError> {
        if self.slot == SceneAssetSlot::Background && self.ordinal != 0 {
            return Err(ValidationError::Invariant {
                field: "scene.background.ordinal",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    pub id: SceneId,
    pub owner: SceneOwner,
    pub status: LifecycleStatus,
    pub ordinal: u32,
    pub content: SceneDocumentV1,
    pub direction: Option<String>,
    pub selected_variant_id: Option<SceneVariantId>,
    pub assets: Vec<SceneAssetLink>,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl Scene {
    pub fn new(
        id: SceneId,
        owner: SceneOwner,
        ordinal: u32,
        content: SceneDocumentV1,
        created_at: TimestampMillis,
    ) -> Result<Self, ValidationError> {
        let scene = Self {
            id,
            owner,
            status: LifecycleStatus::Active,
            ordinal,
            content,
            direction: None,
            selected_variant_id: None,
            assets: Vec::new(),
            revision: Revision::INITIAL,
            created_at,
            updated_at: created_at,
        };
        scene.validate()?;
        Ok(scene)
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        self.content.validate()?;
        if let Some(direction) = &self.direction {
            validate_text("scene.direction", direction)?;
        }
        validate_collection("scene.assets", &self.assets, MAX_COLLECTION_ITEMS)?;
        let mut backgrounds = 0;
        let mut link_ids = HashSet::new();
        let mut inline_ordinals = Vec::new();
        for link in &self.assets {
            link.validate()?;
            if !link_ids.insert(link.id) {
                return Err(ValidationError::Duplicate {
                    field: "scene.asset_ids",
                });
            }
            match link.slot {
                SceneAssetSlot::Background => backgrounds += 1,
                SceneAssetSlot::Inline => inline_ordinals.push(link.ordinal),
            }
        }
        if backgrounds > 1 {
            return Err(ValidationError::Invariant {
                field: "scene.background",
            });
        }
        inline_ordinals.sort_unstable();
        validate_contiguous("scene.inline_asset.order", inline_ordinals)?;
        let linked_inline_ids: HashSet<_> = self
            .assets
            .iter()
            .filter(|link| link.slot == SceneAssetSlot::Inline)
            .map(|link| link.id)
            .collect();
        validate_document_inline_links(
            &self.content,
            &linked_inline_ids,
            "scene.document.inline_link_ids",
        )?;
        Ok(())
    }

    pub fn validate_selected_variant(
        &self,
        variants: &[SceneVariant],
    ) -> Result<(), ValidationError> {
        self.validate()?;
        let mut ids = HashSet::new();
        let linked_inline_ids: HashSet<_> = self
            .assets
            .iter()
            .filter(|link| link.slot == SceneAssetSlot::Inline)
            .map(|link| link.id)
            .collect();
        let mut referenced_inline_ids: HashSet<_> =
            self.content.inline_link_ids().into_iter().collect();
        for variant in variants {
            if variant.scene_id != self.id {
                return Err(ValidationError::InvalidReference {
                    field: "scene.variant.scene_id",
                });
            }
            variant.validate()?;
            validate_document_inline_links(
                &variant.content,
                &linked_inline_ids,
                "scene_variant.document.inline_link_ids",
            )?;
            referenced_inline_ids.extend(variant.content.inline_link_ids());
            ids.insert(variant.id);
        }
        if self
            .selected_variant_id
            .is_some_and(|id| !ids.contains(&id))
        {
            return Err(ValidationError::InvalidReference {
                field: "scene.selected_variant_id",
            });
        }
        if referenced_inline_ids != linked_inline_ids {
            return Err(ValidationError::InvalidReference {
                field: "scene.document.inline_link_ids",
            });
        }
        Ok(())
    }
}

fn validate_document_inline_links(
    document: &SceneDocumentV1,
    linked_inline_ids: &HashSet<SceneAssetLinkId>,
    field: &'static str,
) -> Result<(), ValidationError> {
    let inline_link_ids = document.inline_link_ids();
    let unique_inline_link_ids: HashSet<_> = inline_link_ids.iter().copied().collect();
    if unique_inline_link_ids.len() != inline_link_ids.len() {
        return Err(ValidationError::Duplicate { field });
    }
    if !unique_inline_link_ids.is_subset(linked_inline_ids) {
        return Err(ValidationError::InvalidReference { field });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneVariant {
    pub id: SceneVariantId,
    pub scene_id: SceneId,
    pub ordinal: u32,
    pub content: SceneDocumentV1,
    pub direction: Option<String>,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl SceneVariant {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.content.validate()?;
        if let Some(direction) = &self.direction {
            validate_text("scene_variant.direction", direction)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Scene, SceneAssetLink, SceneAssetSlot, SceneDocumentV1, SceneOwner, ScenePart, SceneVariant,
    };
    use crate::LifecycleStatus;
    use lettuce_types::{
        AssetId, CharacterId, Revision, SceneAssetLinkId, SceneId, TimestampMillis,
    };

    #[test]
    fn inline_document_links_must_have_association_rows() {
        let link_id = SceneAssetLinkId::new();
        let scene = Scene {
            id: SceneId::new(),
            owner: SceneOwner::Character(CharacterId::new()),
            status: LifecycleStatus::Active,
            ordinal: 0,
            content: SceneDocumentV1::new(vec![ScenePart::InlineAsset { link_id }])
                .expect("inline scene should validate"),
            direction: None,
            selected_variant_id: None,
            assets: Vec::new(),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(0),
            updated_at: TimestampMillis::new(0),
        };
        assert!(scene.validate().is_err());
        let mut scene = scene;
        scene.assets.push(SceneAssetLink {
            id: link_id,
            asset_id: AssetId::new(),
            slot: SceneAssetSlot::Inline,
            ordinal: 0,
        });
        assert!(scene.validate().is_ok());
    }

    #[test]
    fn variants_and_base_scene_share_one_complete_asset_link_set() {
        let base_link_id = SceneAssetLinkId::new();
        let variant_link_id = SceneAssetLinkId::new();
        let scene_id = SceneId::new();
        let scene = Scene {
            id: scene_id,
            owner: SceneOwner::Character(CharacterId::new()),
            status: LifecycleStatus::Active,
            ordinal: 0,
            content: SceneDocumentV1::new(vec![ScenePart::InlineAsset {
                link_id: base_link_id,
            }])
            .expect("base document should validate"),
            direction: None,
            selected_variant_id: None,
            assets: vec![
                SceneAssetLink {
                    id: base_link_id,
                    asset_id: AssetId::new(),
                    slot: SceneAssetSlot::Inline,
                    ordinal: 0,
                },
                SceneAssetLink {
                    id: variant_link_id,
                    asset_id: AssetId::new(),
                    slot: SceneAssetSlot::Inline,
                    ordinal: 1,
                },
            ],
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(0),
            updated_at: TimestampMillis::new(0),
        };
        let variant = SceneVariant {
            id: lettuce_types::SceneVariantId::new(),
            scene_id,
            ordinal: 0,
            content: SceneDocumentV1::new(vec![ScenePart::InlineAsset {
                link_id: variant_link_id,
            }])
            .expect("variant document should validate"),
            direction: None,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(0),
            updated_at: TimestampMillis::new(0),
        };
        assert!(
            scene
                .validate_selected_variant(std::slice::from_ref(&variant))
                .is_ok()
        );

        let dangling = SceneVariant {
            content: SceneDocumentV1::new(vec![ScenePart::InlineAsset {
                link_id: SceneAssetLinkId::new(),
            }])
            .expect("variant document should validate"),
            ..variant
        };
        assert!(scene.validate_selected_variant(&[dangling]).is_err());
    }
}
