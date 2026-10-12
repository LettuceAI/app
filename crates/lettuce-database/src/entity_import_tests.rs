use lettuce_characters::{CharacterMedia, LifecycleStatus, Persona, PersonaMedia};
use lettuce_transfer::{CharacterFileAssets, PersonaFileImport, ProviderBackupSource};
use lettuce_types::{PersonaId, Revision, TimestampMillis};

use crate::{ApiOperationError, Database};

#[test]
fn character_bundles_and_persona_default_roll_back_on_a_late_receipt_failure() {
    let database = Database::open_in_memory().expect("database");
    let now = TimestampMillis::new(1);
    let (package, _) = lettuce_transfer::parse_character_import(&serde_json::json!({"version":1,"character":{"name":"Ada","disableAvatarGradient":false,"rules":[],"scenes":[],"lorebooks":[{"lorebook":{"id":"bundled","name":"Harbour","createdAt":1,"updatedAt":1},"entries":[]}]}}),1,||uuid::Uuid::new_v4().to_string()).expect("package");
    let references =
        lettuce_transfer::CharacterFileRepository::character_file_references(&database)
            .expect("references");
    let plan = lettuce_transfer::plan_character_file(&package, &references, 1, uuid::Uuid::new_v4)
        .expect("plan");
    let import = plan
        .import(
            &references,
            &CharacterFileAssets::default(),
            1,
            uuid::Uuid::new_v4,
        )
        .expect("import");
    assert_eq!(import.character.character.media, CharacterMedia::default());
    let persona = PersonaFileImport {
        persona: Persona {
            id: PersonaId::new(),
            status: LifecycleStatus::Active,
            title: "Writer".into(),
            description: "Authored".into(),
            nickname: None,
            design_description: None,
            avatar_crop: None,
            image_recommendation: None,
            media: PersonaMedia::default(),
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        },
        lorebook_ids: vec![],
        disabled_lorebook_ids: Vec::new(),
        make_default: true,
    };
    let before = database.read_provider_backup_graph().expect("before");
    database.connection().expect("connection").execute_batch("CREATE TEMP TRIGGER refuse_import_receipt BEFORE INSERT ON api_operation_receipts BEGIN SELECT RAISE(ABORT,'late failure'); END;").expect("fault");
    for case in 0..2 {
        let result: Result<String, ApiOperationError> = database.commit_api_operation(
            "import_commit",
            &case.to_string(),
            "digest",
            now,
            |scope| {
                if case == 0 {
                    scope
                        .import_character_file(&import)
                        .map_err(|_| ApiOperationError::Storage)?;
                } else {
                    scope
                        .import_persona_file(&persona)
                        .map_err(|_| ApiOperationError::Storage)?;
                }
                Ok("result".into())
            },
        );
        assert_eq!(result, Err(ApiOperationError::Storage));
        assert_eq!(
            database.read_provider_backup_graph().expect("after"),
            before
        );
    }
    database
        .connection()
        .expect("connection")
        .execute_batch("DROP TRIGGER refuse_import_receipt")
        .expect("remove fault");
    let result = database
        .commit_api_operation("import_commit", "0", "digest", now, |scope| {
            scope
                .import_character_file(&import)
                .map_err(|_| ApiOperationError::Storage)?;
            Ok::<_, ApiOperationError>("character".to_owned())
        })
        .expect("retry");
    assert_eq!(
        database
            .commit_api_operation(
                "import_commit",
                "0",
                "digest",
                now,
                |_| -> Result<String, ApiOperationError> { panic!("replay") }
            )
            .expect("replay"),
        result
    );
    assert_eq!(
        database.commit_api_operation("import_commit", "0", "different", now, |_| Ok::<
            _,
            ApiOperationError,
        >(
            "other".to_owned()
        )),
        Err(ApiOperationError::Conflict)
    );
}
