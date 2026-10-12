use lettuce_characters::{
    Crop, LifecycleStatus, Persona, PersonaMedia, PersonaRepository, RepositoryError,
};
use lettuce_context::{
    BindingRepositoryError, LifecycleStatus as LorebookStatus, LorebookRepository,
    PersonaLorebookBindingRepository,
};
use lettuce_transfer::{
    EntityPackageError, LegacyImportSkip, LegacyImportSkipKind, LegacyImportSkipReason,
    PersonaFileImport, PersonaFileRepository, PersonaPackage,
};
use lettuce_types::{LorebookId, PersonaId, Revision, TimestampMillis};

#[derive(Debug, thiserror::Error)]
pub enum PersonaFileError {
    #[error(transparent)]
    Package(#[from] EntityPackageError),
    #[error("persona storage failed: {0}")]
    Repository(#[from] RepositoryError),
    #[error("persona lorebook storage failed: {0}")]
    Bindings(#[from] BindingRepositoryError),
    #[error("Persona not found")]
    NotFound,
}

/// Compatibility persona packages read into new personas.
#[derive(Debug)]
pub struct PersonaFileCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: ?Sized> PersonaFileCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }
}

impl<R> PersonaFileCoordinator<'_, R>
where
    R: PersonaRepository
        + PersonaFileRepository
        + PersonaLorebookBindingRepository
        + LorebookRepository
        + ?Sized,
{
    /// The package in a persona UEC or the pre-UEC persona package; the host
    /// may replace a remote `avatar_data` with the downloaded image's data URL
    /// before importing it.
    pub fn read(
        &self,
        json: &str,
        now: TimestampMillis,
    ) -> Result<PersonaPackage, PersonaFileError> {
        let value = serde_json::from_str(json)
            .map_err(|error| EntityPackageError::InvalidImport(error.to_string()))?;
        Ok(lettuce_transfer::parse_persona_import(&value, now.get())?)
    }

    pub fn prepare(
        &self,
        package: &PersonaPackage,
        now: TimestampMillis,
    ) -> Result<(PersonaFileImport, Vec<LegacyImportSkip>), PersonaFileError> {
        let data = &package.persona;
        let persona = Persona {
            id: PersonaId::new(),
            status: LifecycleStatus::Active,
            title: data.title.clone(),
            description: data.description.clone(),
            nickname: data.nickname.clone(),
            design_description: None,
            avatar_crop: data
                .avatar_crop
                .as_ref()
                .and_then(|crop| Crop::new(crop.x as f32, crop.y as f32, crop.scale as f32).ok()),
            image_recommendation: None,
            media: PersonaMedia::default(),
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        };
        persona.validate().map_err(RepositoryError::from)?;
        let mut skipped = Vec::new();
        let mut bound = Vec::new();
        for source_id in &data.active_lorebook_ids {
            let lorebook_id = source_id.parse::<LorebookId>().ok();
            if lorebook_id.is_some_and(|id| bound.contains(&id)) {
                continue;
            }
            let active = lorebook_id
                .map(|id| LorebookRepository::get(self.repository, id))
                .transpose()
                .map_err(|error| {
                    PersonaFileError::Bindings(BindingRepositoryError::Failure(error.to_string()))
                })?
                .flatten()
                .filter(|details| details.book.status == LorebookStatus::Active);
            let Some(lorebook) = active else {
                skipped.push(LegacyImportSkip {
                    kind: LegacyImportSkipKind::LorebookReference,
                    source_key: format!("personas.active_lorebook_ids:{source_id}"),
                    reason: LegacyImportSkipReason::MissingLorebook,
                });
                continue;
            };
            bound.push(lorebook.book.id);
        }
        Ok((
            PersonaFileImport {
                persona,
                lorebook_ids: bound,
                disabled_lorebook_ids: Vec::new(),
                make_default: data.is_default == Some(true),
            },
            skipped,
        ))
    }
}

#[cfg(test)]
mod tests {
    use lettuce_context::{DetectionPolicy, LorebookBehaviorVersion, LorebookMetadataDraft};
    use lettuce_types::OperationId;

    use super::*;

    #[test]
    fn a_persona_uec_without_a_description_imports_like_legacy_parse_uec_persona() {
        let root = std::env::temp_dir().join(format!("persona-blank-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("root");
        let path = root.join("state.sqlite3");
        let backend = crate::AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
        let mut uec: serde_json::Value = serde_json::from_str(include_str!(
            "../../../lettuce-transfer/tests/fixtures/uec2-persona-blank.json"
        ))
        .expect("fixture");
        uec["payload"]
            .as_object_mut()
            .expect("payload")
            .remove("description");
        let files = backend.persona_files();
        let package = files
            .read(&uec.to_string(), TimestampMillis::new(5))
            .expect("read");
        let (import, _) = files
            .prepare(&package, TimestampMillis::new(5))
            .expect("prepare");
        let persona = backend
            .database()
            .import_persona_file(&import)
            .expect("a title-only persona imports");
        assert_eq!(persona.title, "Wanderer");
        assert_eq!(persona.description, "");
        assert_eq!(
            PersonaRepository::get_default_snapshot(backend.database())
                .expect("default")
                .state
                .persona_id,
            Some(persona.id)
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_persona_file_round_trips_through_a_new_default_persona() {
        let root = std::env::temp_dir().join(format!("persona-file-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("root");
        let path = root.join("state.sqlite3");
        let backend = crate::AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
        let lorebook = LorebookRepository::create(
            backend.database(),
            LorebookMetadataDraft {
                name: "Harbour".to_owned(),
                detection_policy: DetectionPolicy::RecentMessageWindow,
                icon_asset_id: None,
                behavior_version: LorebookBehaviorVersion::LegacyV1,
            },
            Vec::new(),
            TimestampMillis::new(2),
        )
        .expect("lorebook");
        let files = backend.persona_files();
        let package = files
            .read(
                &serde_json::json!({
                    "version": 1,
                    "exportedAt": 5,
                    "persona": {
                        "title": "Mira",
                        "description": "A traveller",
                        "nickname": "Mi",
                        "isDefault": true,
                        "avatarCrop": {"x": 0.3, "y": 0.1, "scale": 1.5},
                        "activeLorebookIds": [lorebook.book.id.to_string(), "missing"]
                    }
                })
                .to_string(),
                TimestampMillis::new(10),
            )
            .expect("read");
        let (import, skipped) = files
            .prepare(&package, TimestampMillis::new(10))
            .expect("prepare");
        let persona = &backend
            .database()
            .import_persona_file(&import)
            .expect("import");
        assert_eq!(persona.title, "Mira");
        assert_eq!(skipped.len(), 1);
        assert_eq!(
            backend
                .database()
                .list_persona_bindings(persona.id)
                .expect("bindings")
                .len(),
            1
        );
        assert_eq!(
            PersonaRepository::get_default_snapshot(backend.database())
                .expect("default")
                .state
                .persona_id,
            Some(persona.id)
        );
        let again = PersonaRepository::get(backend.database(), persona.id)
            .expect("read")
            .expect("persona");
        assert_eq!(again.title, "Mira");
        assert_eq!(again.nickname.as_deref(), Some("Mi"));
        assert_eq!(again.avatar_crop.map(|crop| crop.x), Some(0.3f32));
        let bindings = backend
            .database()
            .list_persona_bindings(again.id)
            .expect("bindings");
        assert_eq!(
            bindings
                .iter()
                .map(|binding| binding.lorebook_id.to_string())
                .collect::<Vec<_>>(),
            vec![lorebook.book.id.to_string()]
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
