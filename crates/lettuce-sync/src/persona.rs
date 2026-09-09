use lettuce_characters::{
    Persona, PersonaDefaultState, PersonaMedia, PersonaMediaLink, PersonaMediaSlot,
};
use lettuce_types::{AssetId, OperationId, PersonaId, Revision};
use uuid::Uuid;

use crate::{CanonicalPayload, SyncChangeError, SyncEntity};

pub const PERSONA_SYNC_SCHEMA: &str = "persona.snapshot";
pub const PERSONA_SYNC_VERSION: u32 = 1;
pub const PERSONA_DEFAULT_SYNC_SCHEMA: &str = "persona.default";
pub const PERSONA_DEFAULT_SYNC_VERSION: u32 = 1;

const PERSONA_OPERATION_NAMESPACE: Uuid = Uuid::from_u128(0x8ea33b8a_f812_55df_9cae_12ac8f0f99a1);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PersonaSyncError {
    #[error("persona snapshot is invalid")]
    InvalidPersona,
    #[error("persona snapshot could not be encoded")]
    Encoding,
    #[error("persona snapshot payload is invalid")]
    InvalidPayload,
}

pub fn canonical_persona_payload(persona: &Persona) -> Result<CanonicalPayload, PersonaSyncError> {
    persona
        .validate()
        .map_err(|_| PersonaSyncError::InvalidPersona)?;
    let bytes = serde_json::to_vec(persona).map_err(|_| PersonaSyncError::Encoding)?;
    CanonicalPayload::new(PERSONA_SYNC_SCHEMA, PERSONA_SYNC_VERSION, bytes)
        .map_err(|_: SyncChangeError| PersonaSyncError::InvalidPayload)
}

pub fn persona_sync_entity(id: PersonaId) -> Result<SyncEntity, PersonaSyncError> {
    SyncEntity::new("persona", id.to_string()).map_err(|_| PersonaSyncError::InvalidPayload)
}

pub fn canonical_persona_default_payload(
    state: &PersonaDefaultState,
) -> Result<CanonicalPayload, PersonaSyncError> {
    state
        .validate()
        .map_err(|_| PersonaSyncError::InvalidPersona)?;
    let bytes = serde_json::to_vec(state).map_err(|_| PersonaSyncError::Encoding)?;
    CanonicalPayload::new(
        PERSONA_DEFAULT_SYNC_SCHEMA,
        PERSONA_DEFAULT_SYNC_VERSION,
        bytes,
    )
    .map_err(|_: SyncChangeError| PersonaSyncError::InvalidPayload)
}

pub fn persona_default_sync_entity() -> Result<SyncEntity, PersonaSyncError> {
    SyncEntity::new("persona_default", "application").map_err(|_| PersonaSyncError::InvalidPayload)
}

#[must_use]
pub fn persona_create_operation(id: PersonaId) -> OperationId {
    persona_operation(id, "create", Revision::INITIAL)
}

#[must_use]
pub fn persona_revise_operation(id: PersonaId, expected_revision: Revision) -> OperationId {
    persona_operation(id, "revise", expected_revision)
}

#[must_use]
pub fn persona_update_media_operation(
    id: PersonaId,
    expected_revision: Revision,
    media: &PersonaMedia,
) -> OperationId {
    let mut links = media.links.clone();
    links.sort_by_key(|link| {
        (
            media_slot_rank(link.slot),
            if link.slot == PersonaMediaSlot::Avatar {
                0
            } else {
                link.ordinal
            },
            link.asset_id.to_string(),
        )
    });
    let intent = links
        .iter()
        .map(media_link_identity)
        .collect::<Vec<_>>()
        .join(";");
    persona_intent_operation(id, "update_media", expected_revision, &intent)
}

#[must_use]
pub fn persona_attach_media_operation(
    id: PersonaId,
    expected_revision: Revision,
    link: &PersonaMediaLink,
) -> OperationId {
    persona_intent_operation(
        id,
        "attach_media",
        expected_revision,
        &media_link_identity(link),
    )
}

#[must_use]
pub fn persona_detach_media_operation(
    id: PersonaId,
    expected_revision: Revision,
    asset_id: AssetId,
    slot: PersonaMediaSlot,
) -> OperationId {
    let intent = format!("{}:{asset_id}", media_slot_identity(slot));
    persona_intent_operation(id, "detach_media", expected_revision, &intent)
}

#[must_use]
pub fn persona_reorder_media_operation(
    id: PersonaId,
    expected_revision: Revision,
    slot: PersonaMediaSlot,
    asset_id: AssetId,
    target_ordinal: u32,
) -> OperationId {
    let intent = format!("{}:{asset_id}:{target_ordinal}", media_slot_identity(slot));
    persona_intent_operation(id, "reorder_media", expected_revision, &intent)
}

#[must_use]
pub fn persona_set_default_operation(
    expected_revision: Revision,
    persona_id: PersonaId,
) -> OperationId {
    default_operation("set", expected_revision, &persona_id.to_string())
}

#[must_use]
pub fn persona_clear_default_operation(expected_revision: Revision) -> OperationId {
    default_operation("clear", expected_revision, "none")
}

#[must_use]
pub fn persona_archive_operation(
    id: PersonaId,
    expected_revision: Revision,
    expected_default_revision: Option<Revision>,
) -> OperationId {
    let intent = expected_default_revision
        .map(|revision| revision.get().to_string())
        .unwrap_or_else(|| "none".into());
    persona_intent_operation(id, "archive", expected_revision, &intent)
}

#[must_use]
pub fn persona_archive_default_operation(
    id: PersonaId,
    expected_persona_revision: Revision,
    expected_default_revision: Revision,
) -> OperationId {
    let intent = format!("{id}:{}", expected_persona_revision.get());
    default_operation("archive_clear", expected_default_revision, &intent)
}

#[must_use]
pub fn persona_restore_operation(id: PersonaId, expected_revision: Revision) -> OperationId {
    persona_operation(id, "restore", expected_revision)
}

fn persona_operation(id: PersonaId, action: &str, revision: Revision) -> OperationId {
    let name = format!("{action}\0{id}\0{}", revision.get());
    OperationId::from_uuid(Uuid::new_v5(&PERSONA_OPERATION_NAMESPACE, name.as_bytes()))
}

fn persona_intent_operation(
    id: PersonaId,
    action: &str,
    revision: Revision,
    intent: &str,
) -> OperationId {
    let name = format!("{action}\0{id}\0{}\0{intent}", revision.get());
    OperationId::from_uuid(Uuid::new_v5(&PERSONA_OPERATION_NAMESPACE, name.as_bytes()))
}

fn default_operation(action: &str, revision: Revision, intent: &str) -> OperationId {
    let name = format!("default\0{action}\0{}\0{intent}", revision.get());
    OperationId::from_uuid(Uuid::new_v5(&PERSONA_OPERATION_NAMESPACE, name.as_bytes()))
}

fn media_link_identity(link: &PersonaMediaLink) -> String {
    let ordinal = if link.slot == PersonaMediaSlot::Avatar {
        0
    } else {
        link.ordinal
    };
    format!(
        "{}:{}:{}",
        media_slot_identity(link.slot),
        link.asset_id,
        ordinal
    )
}

fn media_slot_identity(slot: PersonaMediaSlot) -> &'static str {
    match slot {
        PersonaMediaSlot::Avatar => "avatar",
        PersonaMediaSlot::DesignReference => "design_reference",
    }
}

fn media_slot_rank(slot: PersonaMediaSlot) -> u8 {
    match slot {
        PersonaMediaSlot::Avatar => 0,
        PersonaMediaSlot::DesignReference => 1,
    }
}

#[cfg(test)]
mod tests {
    use lettuce_characters::{Persona, PersonaMedia};
    use lettuce_types::{PersonaId, Revision, TimestampMillis};

    use super::*;

    fn persona() -> Persona {
        Persona {
            id: PersonaId::from_uuid(Uuid::from_u128(1)),
            status: lettuce_characters::LifecycleStatus::Active,
            title: "Writer".into(),
            description: "Writes careful prose".into(),
            nickname: Some("W".into()),
            design_description: None,
            avatar_crop: None,
            image_recommendation: None,
            media: PersonaMedia::default(),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(10),
            updated_at: TimestampMillis::new(10),
        }
    }

    #[test]
    fn persona_snapshot_keeps_the_complete_validated_aggregate() {
        let persona = persona();
        let payload = canonical_persona_payload(&persona).expect("payload");

        assert_eq!(payload.schema(), PERSONA_SYNC_SCHEMA);
        assert_eq!(payload.version(), PERSONA_SYNC_VERSION);
        assert_eq!(
            serde_json::from_slice::<Persona>(payload.bytes()).expect("decode"),
            persona
        );
    }

    #[test]
    fn persona_operation_identity_is_stable_per_mutation_boundary() {
        let id = persona().id;
        let first = AssetId::from_uuid(Uuid::from_u128(2));
        let second = AssetId::from_uuid(Uuid::from_u128(3));
        let media = PersonaMedia {
            links: vec![PersonaMediaLink {
                asset_id: first,
                slot: PersonaMediaSlot::DesignReference,
                ordinal: 0,
            }],
        };

        assert_eq!(persona_create_operation(id), persona_create_operation(id));
        assert_eq!(
            persona_revise_operation(id, Revision::new(2)),
            persona_revise_operation(id, Revision::new(2))
        );
        assert_ne!(
            persona_revise_operation(id, Revision::new(2)),
            persona_revise_operation(id, Revision::new(3))
        );
        assert_ne!(
            persona_create_operation(id),
            persona_revise_operation(id, Revision::INITIAL)
        );
        assert_eq!(
            persona_update_media_operation(id, Revision::new(2), &media),
            persona_update_media_operation(id, Revision::new(2), &media)
        );
        assert_ne!(
            persona_attach_media_operation(
                id,
                Revision::new(2),
                &PersonaMediaLink {
                    asset_id: first,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 0,
                },
            ),
            persona_attach_media_operation(
                id,
                Revision::new(2),
                &PersonaMediaLink {
                    asset_id: second,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal: 0,
                },
            )
        );
        assert_ne!(
            persona_detach_media_operation(
                id,
                Revision::new(2),
                first,
                PersonaMediaSlot::DesignReference,
            ),
            persona_reorder_media_operation(
                id,
                Revision::new(2),
                PersonaMediaSlot::DesignReference,
                first,
                0,
            )
        );
        assert_eq!(
            persona_set_default_operation(Revision::new(2), id),
            persona_set_default_operation(Revision::new(2), id)
        );
        assert_ne!(
            persona_set_default_operation(Revision::new(2), id),
            persona_set_default_operation(
                Revision::new(2),
                PersonaId::from_uuid(Uuid::from_u128(4)),
            )
        );
        assert_ne!(
            persona_set_default_operation(Revision::new(2), id),
            persona_clear_default_operation(Revision::new(2))
        );
        assert_ne!(
            persona_archive_operation(id, Revision::new(2), Some(Revision::new(3))),
            persona_archive_operation(id, Revision::new(2), None)
        );
        assert_ne!(
            persona_archive_default_operation(id, Revision::new(2), Revision::new(3),),
            persona_clear_default_operation(Revision::new(3))
        );
        assert_ne!(
            persona_archive_operation(id, Revision::new(2), None),
            persona_restore_operation(id, Revision::new(2))
        );
    }
}
