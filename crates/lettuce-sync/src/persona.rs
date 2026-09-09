use lettuce_characters::Persona;
use lettuce_types::{OperationId, PersonaId, Revision};
use uuid::Uuid;

use crate::{CanonicalPayload, SyncChangeError, SyncEntity};

pub const PERSONA_SYNC_SCHEMA: &str = "persona.snapshot";
pub const PERSONA_SYNC_VERSION: u32 = 1;

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

#[must_use]
pub fn persona_create_operation(id: PersonaId) -> OperationId {
    persona_operation(id, "create", Revision::INITIAL)
}

#[must_use]
pub fn persona_revise_operation(id: PersonaId, expected_revision: Revision) -> OperationId {
    persona_operation(id, "revise", expected_revision)
}

fn persona_operation(id: PersonaId, action: &str, revision: Revision) -> OperationId {
    let name = format!("{action}\0{id}\0{}", revision.get());
    OperationId::from_uuid(Uuid::new_v5(&PERSONA_OPERATION_NAMESPACE, name.as_bytes()))
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
    }
}
