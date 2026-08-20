use lettuce_characters::{
    Character, CharacterDefaults, CharacterMedia, CharacterPresentationV1, CharacterProfile,
    CharacterProvenance, CharacterRepository, ConversationStarter, CreateCharacterPlan,
    InteractionMode, LifecycleStatus, MemoryPolicy, Persona, PersonaRepository, Scene,
    SceneDocumentV1, SceneOwner, ScenePart, SceneVariant, Selection, StarterMessage, StarterRole,
};
use lettuce_context::{
    BindingInsertionTarget, CharacterLorebookBindingRepository, DetectionPolicy,
    LorebookBehaviorVersion, LorebookBindingCreate, LorebookMetadataDraft, LorebookRepository,
    PersonaLorebookBindingRepository, PromptBehaviorVersion, PromptMetadataDraft, PromptPurpose,
    PromptRepository,
};
use lettuce_conversations::{
    ConversationKind, ConversationReader, CreateConversationPlan, DirectConversationDetails,
    IdempotencyKey, InitialMessageOrigin, MemoryModeSnapshot, MessagePart, MessageRole,
    ModelProviderKind, ParticipantRole, SnapshotSelection, SnapshotSource,
};
use lettuce_database::Database;
use lettuce_models::{
    Modality, ModelKind, ModelProfile, ModelProfileConfig, ModelProfileRepository, ProviderAccount,
    ProviderAccountRepository, ProviderConfig, ProviderProtocol,
};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{
    CharacterId, ConversationStarterId, LorebookId, ModelProfileId, PersonaId, ProviderAccountId,
    Revision, SceneId, SceneVariantId, StarterMessageId, TimestampMillis,
};

use super::planner::ConversationLaunchPlanner;
use super::policy;
use super::request::{DirectConversationLaunchRequest, DirectUserParticipant, LaunchSelection};
use crate::DirectLaunchError;

const NOW: TimestampMillis = TimestampMillis::new(1_000);

fn database() -> Database {
    Database::open_in_memory().expect("open database")
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::new(value).expect("idempotency key")
}

fn request(character_id: CharacterId, operation_key: &str) -> DirectConversationLaunchRequest {
    DirectConversationLaunchRequest {
        format_version: 1,
        title: "Session".into(),
        user: DirectUserParticipant {
            display_name: "Traveller".into(),
            authored_description: None,
        },
        character_id,
        scene: LaunchSelection::Inherit,
        starter: LaunchSelection::Inherit,
        persona: LaunchSelection::Inherit,
        operation_key: key(operation_key),
    }
}

fn request_with_starter(
    character_id: CharacterId,
    operation_key: &str,
    starter_id: ConversationStarterId,
) -> DirectConversationLaunchRequest {
    DirectConversationLaunchRequest {
        starter: LaunchSelection::Explicit(starter_id),
        ..request(character_id, operation_key)
    }
}

fn scene_with(character_id: CharacterId, ordinal: u32, parts: Vec<ScenePart>) -> Scene {
    Scene::new(
        SceneId::new(),
        SceneOwner::Character(character_id),
        ordinal,
        SceneDocumentV1::new(parts).expect("scene document"),
        TimestampMillis::new(1),
    )
    .expect("scene")
}

fn text_scene(character_id: CharacterId, ordinal: u32, text: &str) -> Scene {
    scene_with(
        character_id,
        ordinal,
        vec![ScenePart::Text { text: text.into() }],
    )
}

fn starter_with(
    character_id: CharacterId,
    ordinal: u32,
    name: &str,
    messages: Vec<StarterMessage>,
) -> ConversationStarter {
    ConversationStarter::new(
        ConversationStarterId::new(),
        character_id,
        name.into(),
        ordinal,
        messages,
        TimestampMillis::new(1),
    )
    .expect("starter")
}

fn message(role: StarterRole, content: &str) -> StarterMessage {
    StarterMessage {
        id: StarterMessageId::new(),
        role,
        content: content.into(),
    }
}

fn character_with(
    id: CharacterId,
    nickname: Option<&str>,
    defaults: CharacterDefaults,
) -> Character {
    Character::new(
        id,
        CharacterProfile {
            name: "Ada".into(),
            nickname: nickname.map(str::to_owned),
            description: Some("A meticulous engineer".into()),
            definition: None,
            design_description: None,
        },
        CharacterProvenance::default(),
        defaults,
        CharacterPresentationV1::default(),
        None,
        CharacterMedia::default(),
        TimestampMillis::new(1),
    )
    .expect("character")
}

fn seed_character(
    database: &Database,
    scenes: Vec<Scene>,
    variants: Vec<SceneVariant>,
    starters: Vec<ConversationStarter>,
    mutate: impl FnOnce(&mut CharacterDefaults),
) -> CharacterId {
    let id = CharacterId::new();
    let scenes: Vec<Scene> = scenes
        .into_iter()
        .map(|mut scene| {
            scene.owner = SceneOwner::Character(id);
            scene
        })
        .collect();
    let starters: Vec<ConversationStarter> = starters
        .into_iter()
        .map(|mut starter| {
            starter.character_id = id;
            starter
        })
        .collect();
    let mut defaults = CharacterDefaults::default();
    mutate(&mut defaults);
    let character = character_with(id, Some("Addy"), defaults);
    CharacterRepository::create(
        database,
        CreateCharacterPlan {
            character,
            scenes,
            variants,
            starters,
        },
    )
    .expect("create character");
    id
}

fn plain_character(database: &Database) -> CharacterId {
    seed_character(database, Vec::new(), Vec::new(), Vec::new(), |_| {})
}

fn seed_lorebook(database: &Database, name: &str) -> LorebookId {
    LorebookRepository::create(
        database,
        LorebookMetadataDraft {
            name: name.into(),
            detection_policy: DetectionPolicy::RecentMessageWindow,
            icon_asset_id: None,
            behavior_version: LorebookBehaviorVersion::LegacyV1,
        },
        Vec::new(),
        TimestampMillis::new(1),
    )
    .expect("lorebook")
    .book
    .id
}

fn seed_prompt(
    database: &Database,
    name: &str,
    purpose: PromptPurpose,
) -> lettuce_types::PromptDocumentId {
    PromptRepository::create_user_draft(
        database,
        PromptMetadataDraft {
            name: name.into(),
            purpose,
            condense: false,
            behavior_version: PromptBehaviorVersion::LegacyV1,
        },
        Vec::new(),
        TimestampMillis::new(1),
    )
    .expect("prompt")
    .id
}

fn seed_model(database: &Database, protocol: ProviderProtocol, kind_label: &str) -> ModelProfileId {
    seed_model_with(database, protocol, kind_label, ModelKind::Chat, true)
}

fn seed_model_with(
    database: &Database,
    protocol: ProviderProtocol,
    kind_label: &str,
    kind: ModelKind,
    enabled: bool,
) -> ModelProfileId {
    let account = ProviderAccountRepository::upsert(
        database,
        ProviderAccount {
            id: ProviderAccountId::new(),
            provider_kind: kind_label.into(),
            protocol,
            label: "Account".into(),
            endpoint: None,
            enabled,
            api_key_ref: None,
            secret_headers: Vec::new(),
            config: ProviderConfig::Standard,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        },
        None,
    )
    .expect("account");
    ModelProfileRepository::upsert(
        database,
        ModelProfile {
            id: ModelProfileId::new(),
            provider_account_id: account.id,
            external_model_id: "vendor/model".into(),
            display_name: "Vendor Model".into(),
            kind,
            config: ModelProfileConfig {
                input_modalities: vec![Modality::Text],
                output_modalities: vec![Modality::Text],
                temperature: Some(0.7),
                context_length: Some(8192),
                max_output_tokens: Some(1024),
            },
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        },
        None,
    )
    .expect("profile")
    .id
}

fn set_application_default_model(database: &Database, id: ModelProfileId) {
    let stored = GlobalSettingsStore::load(database).expect("settings");
    GlobalSettingsStore::save(database, stored.settings, Some(id), stored.revision)
        .expect("save settings");
}

fn seed_persona(database: &Database, title: &str) -> PersonaId {
    let persona = Persona::new(
        PersonaId::new(),
        title.into(),
        "A wandering cartographer".into(),
        TimestampMillis::new(1),
    )
    .expect("persona");
    PersonaRepository::create(database, persona)
        .expect("create persona")
        .id
}

fn plan_for(
    database: &Database,
    request: &DirectConversationLaunchRequest,
) -> CreateConversationPlan {
    ConversationLaunchPlanner::new(database)
        .prepare_direct(request)
        .expect("prepare launch")
        .into_parts()
        .0
}

fn direct_details(plan: &CreateConversationPlan) -> &DirectConversationDetails {
    match &plan.kind {
        ConversationKind::Direct(details) => details,
        ConversationKind::Group(_) => panic!("direct launch produced a group conversation"),
    }
}

fn message_text(part: &MessagePart) -> &str {
    match part {
        MessagePart::Text { text } => text,
        _ => panic!("expected a text part"),
    }
}

fn lorebook_names(
    selection: &SnapshotSelection<Vec<lettuce_conversations::LorebookLaunchSnapshot>>,
) -> Option<Vec<String>> {
    match selection {
        SnapshotSelection::Inherited(books) | SnapshotSelection::Explicit(books) => {
            Some(books.iter().map(|book| book.name.clone()).collect())
        }
        SnapshotSelection::Disabled => None,
    }
}

#[test]
fn happy_path_direct_launch_has_two_participants_and_an_empty_timeline() {
    let database = database();
    let character_id = plain_character(&database);
    let result = ConversationLaunchPlanner::new(&database)
        .launch_direct(&request(character_id, "happy-path"), NOW)
        .expect("launch");
    result.value.validate().expect("aggregate validates");
    assert_eq!(result.value.conversation.participants.len(), 2);
    assert!(
        ConversationReader::timeline_page(
            &database,
            result.value.conversation.id,
            result.value.conversation.active_branch_id,
            &lettuce_types::PageRequest::default(),
        )
        .expect("timeline")
        .items
        .is_empty()
    );
    let plan = plan_for(&database, &request(character_id, "happy-path"));
    let details = direct_details(&plan);
    assert_eq!(details.scene, SnapshotSelection::Disabled);
    assert_eq!(details.starter, SnapshotSelection::Disabled);
    assert_eq!(details.voice, SnapshotSelection::Disabled);
}

#[test]
fn character_participant_uses_nickname_and_mirrors_the_model_selection() {
    let database = database();
    let model_id = seed_model(&database, ProviderProtocol::Anthropic, "anthropic");
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.model_profile_id = Some(model_id);
    });
    let plan = plan_for(&database, &request(character_id, "participant-shape"));
    let details = direct_details(&plan);
    let participant = plan
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::Character)
        .expect("character participant");
    assert_eq!(participant.display_name, "Addy");
    assert_eq!(participant.model_selection, details.model);
}

#[test]
fn archived_character_is_rejected() {
    let database = database();
    let character_id = plain_character(&database);
    CharacterRepository::archive(&database, character_id, Revision::INITIAL, NOW).expect("archive");
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request(character_id, "archived"))
            .expect_err("archived character"),
        DirectLaunchError::CharacterArchived { character_id }
    );
}

#[test]
fn companion_character_is_unsupported() {
    let database = database();
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.interaction_mode = InteractionMode::Companion;
    });
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request(character_id, "companion"))
            .expect_err("companion"),
        DirectLaunchError::CompanionUnsupported { character_id }
    );
}

#[test]
fn scene_only_launch_materializes_one_trimmed_scene_message() {
    let database = database();
    let scene = text_scene(CharacterId::new(), 0, "  A quiet harbour at dawn.  ");
    let scene_id = scene.id;
    let character_id = seed_character(&database, vec![scene], Vec::new(), Vec::new(), |defaults| {
        defaults.default_scene_id = Some(scene_id);
    });
    let plan = plan_for(&database, &request(character_id, "scene-only"));
    let details = direct_details(&plan);
    assert!(matches!(details.scene, SnapshotSelection::Inherited(_)));
    assert_eq!(plan.initial_timeline.entries.len(), 1);
    let entry = &plan.initial_timeline.entries[0];
    assert_eq!(entry.role, MessageRole::Scene);
    assert_eq!(entry.author_participant_id, None);
    assert_eq!(message_text(&entry.parts[0]), "A quiet harbour at dawn.");
    assert!(matches!(
        entry.origin,
        InitialMessageOrigin::SelectedScene { .. }
    ));
}

#[test]
fn starter_only_launch_maps_roles_authors_and_origins() {
    let database = database();
    let starter = starter_with(
        CharacterId::new(),
        0,
        "Greeting",
        vec![
            message(StarterRole::User, "Hello"),
            message(StarterRole::Assistant, "Welcome."),
        ],
    );
    let starter_id = starter.id;
    let user_message_id = starter.messages[0].id;
    let character_id = seed_character(&database, Vec::new(), Vec::new(), vec![starter], |_| {});
    let plan = plan_for(
        &database,
        &request_with_starter(character_id, "starter-only", starter_id),
    );
    let user = plan
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::User)
        .expect("user participant")
        .id;
    let character = plan
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::Character)
        .expect("character participant")
        .id;
    assert_eq!(plan.initial_timeline.entries.len(), 2);
    assert_eq!(plan.initial_timeline.entries[0].role, MessageRole::User);
    assert_eq!(
        plan.initial_timeline.entries[0].author_participant_id,
        Some(user)
    );
    assert!(matches!(
        plan.initial_timeline.entries[0].origin,
        InitialMessageOrigin::StarterMessage { starter_message_id, .. }
            if starter_message_id == user_message_id
    ));
    assert_eq!(
        plan.initial_timeline.entries[1].role,
        MessageRole::Assistant
    );
    assert_eq!(
        plan.initial_timeline.entries[1].author_participant_id,
        Some(character)
    );
}

#[test]
fn scene_and_starter_launch_puts_the_scene_first() {
    let database = database();
    let scene = text_scene(CharacterId::new(), 0, "The room is quiet.");
    let scene_id = scene.id;
    let mut starter = starter_with(
        CharacterId::new(),
        0,
        "Greeting",
        vec![message(StarterRole::Assistant, "Welcome.")],
    );
    starter.scene_id = Some(scene_id);
    let starter_id = starter.id;
    let character_id = seed_character(&database, vec![scene], Vec::new(), vec![starter], |_| {});
    let plan = plan_for(
        &database,
        &request_with_starter(character_id, "scene-and-starter", starter_id),
    );
    assert_eq!(plan.initial_timeline.entries.len(), 2);
    assert_eq!(plan.initial_timeline.entries[0].role, MessageRole::Scene);
    assert_eq!(
        plan.initial_timeline.entries[1].role,
        MessageRole::Assistant
    );
}

#[test]
fn a_starter_scene_replaces_the_requested_scene() {
    let database = database();
    let requested = text_scene(CharacterId::new(), 0, "Requested scene.");
    let starter_scene = text_scene(CharacterId::new(), 1, "Starter scene.");
    let requested_id = requested.id;
    let starter_scene_id = starter_scene.id;
    let mut starter = starter_with(CharacterId::new(), 0, "Greeting", Vec::new());
    starter.scene_id = Some(starter_scene_id);
    let starter_id = starter.id;
    let character_id = seed_character(
        &database,
        vec![requested, starter_scene],
        Vec::new(),
        vec![starter],
        |_| {},
    );
    let mut launch = request(character_id, "starter-scene-override");
    launch.scene = LaunchSelection::Explicit(requested_id);
    launch.starter = LaunchSelection::Explicit(starter_id);
    let plan = plan_for(&database, &launch);
    let details = direct_details(&plan);
    match &details.scene {
        SnapshotSelection::Explicit(scene) => assert_eq!(scene.source_id, starter_scene_id),
        other => panic!("expected the starter scene, got {other:?}"),
    }
}

#[test]
fn a_starter_without_a_scene_clears_the_requested_scene() {
    let database = database();
    let requested = text_scene(CharacterId::new(), 0, "Requested scene.");
    let requested_id = requested.id;
    let starter = starter_with(CharacterId::new(), 0, "Greeting", Vec::new());
    let starter_id = starter.id;
    let character_id = seed_character(
        &database,
        vec![requested],
        Vec::new(),
        vec![starter],
        |defaults| {
            defaults.default_scene_id = Some(requested_id);
        },
    );
    let mut launch = request(character_id, "starter-clears-scene");
    launch.scene = LaunchSelection::Explicit(requested_id);
    launch.starter = LaunchSelection::Explicit(starter_id);
    let plan = plan_for(&database, &launch);
    assert_eq!(direct_details(&plan).scene, SnapshotSelection::Disabled);
    assert!(plan.initial_timeline.entries.is_empty());
}

#[test]
fn scene_text_prefers_variant_then_base_then_direction() {
    let character_id = CharacterId::new();
    let mut scene = text_scene(character_id, 0, "  Base body.  ");
    let variant_id = SceneVariantId::new();
    let variant = SceneVariant {
        id: variant_id,
        scene_id: scene.id,
        ordinal: 0,
        content: SceneDocumentV1::new(vec![ScenePart::Text {
            text: " Variant body. ".into(),
        }])
        .expect("variant document"),
        direction: None,
        revision: Revision::INITIAL,
        created_at: TimestampMillis::new(1),
        updated_at: TimestampMillis::new(1),
    };
    scene.selected_variant_id = Some(variant_id);
    assert_eq!(
        policy::resolve_scene_text(&scene, std::slice::from_ref(&variant)),
        Some("Variant body.".to_owned())
    );

    let blank_variant = SceneVariant {
        content: SceneDocumentV1::new(vec![ScenePart::Text { text: "   ".into() }])
            .expect("variant document"),
        ..variant.clone()
    };
    assert_eq!(
        policy::resolve_scene_text(&scene, &[blank_variant]),
        Some("Base body.".to_owned())
    );

    assert_eq!(
        policy::resolve_scene_text(&scene, &[]),
        Some("Base body.".to_owned())
    );

    let mut blank_scene = text_scene(character_id, 0, "   ");
    blank_scene.direction = Some("  Keep it slow.  ".into());
    assert_eq!(
        policy::resolve_scene_text(&blank_scene, &[]),
        Some("Keep it slow.".to_owned())
    );

    let empty_scene = text_scene(character_id, 0, "  ");
    assert_eq!(policy::resolve_scene_text(&empty_scene, &[]), None);
}

#[test]
fn a_blank_scene_is_selected_without_a_timeline_entry() {
    let database = database();
    let scene = text_scene(CharacterId::new(), 0, "   ");
    let scene_id = scene.id;
    let character_id = seed_character(&database, vec![scene], Vec::new(), Vec::new(), |defaults| {
        defaults.default_scene_id = Some(scene_id);
    });
    let plan = plan_for(&database, &request(character_id, "blank-scene"));
    let details = direct_details(&plan);
    match &details.scene {
        SnapshotSelection::Inherited(scene) => assert_eq!(scene.title, "Scene 1"),
        other => panic!("expected an inherited scene, got {other:?}"),
    }
    assert!(plan.initial_timeline.entries.is_empty());
}

#[test]
fn scene_titles_are_bounded_and_fall_back_to_the_ordinal() {
    assert_eq!(policy::scene_title(None, 0), "Scene 1");
    assert_eq!(policy::scene_title(Some("   "), 3), "Scene 4");
    assert_eq!(policy::scene_title(Some(" Harbour "), 0), "Harbour");
    let long = "x".repeat(400);
    assert!(policy::scene_title(Some(&long), 0).chars().count() < 256);
}

#[test]
fn starter_message_content_is_copied_verbatim() {
    let database = database();
    let starter = starter_with(
        CharacterId::new(),
        0,
        "Greeting",
        vec![message(StarterRole::Assistant, "  spaced out  ")],
    );
    let starter_id = starter.id;
    let character_id = seed_character(&database, Vec::new(), Vec::new(), vec![starter], |_| {});
    let plan = plan_for(
        &database,
        &request_with_starter(character_id, "verbatim-starter-message", starter_id),
    );
    assert_eq!(plan.initial_timeline.entries.len(), 1);
    assert_eq!(
        message_text(&plan.initial_timeline.entries[0].parts[0]),
        "  spaced out  "
    );
}

#[test]
fn a_blank_starter_message_is_preserved() {
    let database = database();
    let starter = starter_with(
        CharacterId::new(),
        0,
        "Greeting",
        vec![message(StarterRole::Assistant, "")],
    );
    let starter_id = starter.id;
    let character_id = seed_character(&database, Vec::new(), Vec::new(), vec![starter], |_| {});
    let plan = plan_for(
        &database,
        &request_with_starter(character_id, "blank-starter-message", starter_id),
    );
    assert_eq!(plan.initial_timeline.entries.len(), 1);
    assert_eq!(message_text(&plan.initial_timeline.entries[0].parts[0]), "");
}

#[test]
fn starter_lorebooks_distinguish_explicit_empty_inherit_and_disabled() {
    let database = database();
    let book = seed_lorebook(&database, "Atlas");
    let starter = starter_with(CharacterId::new(), 0, "Greeting", Vec::new());
    let starter_id = starter.id;
    let character_id = seed_character(&database, Vec::new(), Vec::new(), vec![starter], |_| {});
    let bound = CharacterLorebookBindingRepository::bind_character_lorebook(
        &database,
        character_id,
        Revision::INITIAL,
        LorebookBindingCreate {
            lorebook_id: book,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind");

    let inherit = plan_for(
        &database,
        &request_with_starter(character_id, "starter-books-inherit", starter_id),
    );
    assert!(matches!(
        direct_details(&inherit).lorebooks,
        SnapshotSelection::Inherited(_)
    ));
    assert_eq!(
        lorebook_names(&direct_details(&inherit).lorebooks),
        Some(vec!["Atlas".to_owned()])
    );

    let details = CharacterRepository::get(&database, character_id)
        .expect("character")
        .expect("character exists");
    let mut starter = details.starters[0].clone();
    starter.lorebooks = Selection::Explicit(Vec::new());
    let update = lettuce_characters::ConversationStarterDraftUpdate {
        name: starter.name.clone(),
        scene_id: starter.scene_id,
        prompt_id: starter.prompt_id,
        lorebooks: Selection::Explicit(Vec::new()),
    };
    lettuce_characters::StarterRepository::update_starter(
        &database,
        character_id,
        bound.owner_revision,
        starter_id,
        update,
        NOW,
    )
    .expect("explicit empty");
    let explicit_empty = plan_for(
        &database,
        &request_with_starter(character_id, "starter-books-empty", starter_id),
    );
    assert_eq!(
        direct_details(&explicit_empty).lorebooks,
        SnapshotSelection::Explicit(Vec::new())
    );

    let revision = CharacterRepository::get(&database, character_id)
        .expect("character")
        .expect("character exists")
        .character
        .revision;
    lettuce_characters::StarterRepository::update_starter(
        &database,
        character_id,
        revision,
        starter_id,
        lettuce_characters::ConversationStarterDraftUpdate {
            name: "Greeting".into(),
            scene_id: None,
            prompt_id: None,
            lorebooks: Selection::Disabled,
        },
        NOW,
    )
    .expect("disabled");
    let disabled = plan_for(
        &database,
        &request_with_starter(character_id, "starter-books-disabled", starter_id),
    );
    assert_eq!(
        direct_details(&disabled).lorebooks,
        SnapshotSelection::Disabled
    );
}

#[test]
fn inherited_lorebooks_keep_binding_order_and_deduplicate_persona_books() {
    let database = database();
    let shared = seed_lorebook(&database, "Shared");
    let character_book = seed_lorebook(&database, "Character");
    let persona_book = seed_lorebook(&database, "Persona");
    let character_id = plain_character(&database);
    let first = CharacterLorebookBindingRepository::bind_character_lorebook(
        &database,
        character_id,
        Revision::INITIAL,
        LorebookBindingCreate {
            lorebook_id: character_book,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind character book");
    CharacterLorebookBindingRepository::bind_character_lorebook(
        &database,
        character_id,
        first.owner_revision,
        LorebookBindingCreate {
            lorebook_id: shared,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind shared book");
    let persona_id = seed_persona(&database, "Traveller");
    let bound = PersonaLorebookBindingRepository::bind_persona_lorebook(
        &database,
        persona_id,
        Revision::INITIAL,
        LorebookBindingCreate {
            lorebook_id: shared,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind persona shared");
    PersonaLorebookBindingRepository::bind_persona_lorebook(
        &database,
        persona_id,
        bound.owner_revision,
        LorebookBindingCreate {
            lorebook_id: persona_book,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind persona book");

    let mut launch = request(character_id, "inherited-books");
    launch.persona = LaunchSelection::Explicit(persona_id);
    let plan = plan_for(&database, &launch);
    let details = direct_details(&plan);
    assert_eq!(
        lorebook_names(&details.lorebooks),
        Some(vec![
            "Character".to_owned(),
            "Shared".to_owned(),
            "Persona".to_owned(),
        ])
    );
    match &details.persona {
        SnapshotSelection::Explicit(persona) => assert_eq!(
            lorebook_names(&persona.lorebooks),
            Some(vec!["Shared".to_owned(), "Persona".to_owned()])
        ),
        other => panic!("expected an explicit persona, got {other:?}"),
    }
    let sources: Vec<SnapshotSource> = ConversationLaunchPlanner::new(&database)
        .prepare_direct(&launch)
        .expect("prepare")
        .into_parts()
        .1
        .iter()
        .map(|draft| draft.source)
        .collect();
    assert_eq!(
        sources
            .iter()
            .filter(|source| matches!(source, SnapshotSource::Lorebook(_)))
            .count(),
        3
    );
}

#[test]
fn persona_resolution_covers_explicit_inherited_and_disabled() {
    let database = database();
    let character_id = plain_character(&database);
    let persona_id = seed_persona(&database, "Traveller");

    let mut explicit = request(character_id, "persona-explicit");
    explicit.persona = LaunchSelection::Explicit(persona_id);
    assert!(matches!(
        direct_details(&plan_for(&database, &explicit)).persona,
        SnapshotSelection::Explicit(_)
    ));

    let inherit = request(character_id, "persona-inherit-none");
    assert_eq!(
        direct_details(&plan_for(&database, &inherit)).persona,
        SnapshotSelection::Disabled
    );

    let default_revision = PersonaRepository::get_default_snapshot(&database)
        .expect("default snapshot")
        .state
        .revision;
    PersonaRepository::set_default(&database, persona_id, default_revision, NOW)
        .expect("set default");
    let inherited = request(character_id, "persona-inherited");
    assert!(matches!(
        direct_details(&plan_for(&database, &inherited)).persona,
        SnapshotSelection::Inherited(_)
    ));

    let mut disabled = request(character_id, "persona-disabled");
    disabled.persona = LaunchSelection::Disabled;
    assert_eq!(
        direct_details(&plan_for(&database, &disabled)).persona,
        SnapshotSelection::Disabled
    );

    let mut missing = request(character_id, "persona-missing");
    let persona_id = PersonaId::new();
    missing.persona = LaunchSelection::Explicit(persona_id);
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&missing)
            .expect_err("missing persona"),
        DirectLaunchError::PersonaNotFound { persona_id }
    );
}

#[test]
fn prompt_precedence_prefers_the_starter_and_rejects_a_wrong_purpose() {
    let database = database();
    let character_prompt = seed_prompt(&database, "Character prompt", PromptPurpose::DirectChat);
    let starter_prompt = seed_prompt(&database, "Starter prompt", PromptPurpose::DirectChat);
    let mut starter = starter_with(CharacterId::new(), 0, "Greeting", Vec::new());
    starter.prompt_id = Some(starter_prompt);
    let starter_id = starter.id;
    let character_id = seed_character(
        &database,
        Vec::new(),
        Vec::new(),
        vec![starter],
        |defaults| {
            defaults.direct_prompt_id = Some(character_prompt);
        },
    );

    let inherited = plan_for(&database, &request(character_id, "prompt-inherited"));
    match &direct_details(&inherited).prompt {
        SnapshotSelection::Inherited(prompt) => assert_eq!(prompt.source_id, character_prompt),
        other => panic!("expected an inherited prompt, got {other:?}"),
    }

    let mut explicit = request(character_id, "prompt-explicit");
    explicit.starter = LaunchSelection::Explicit(starter_id);
    match &direct_details(&plan_for(&database, &explicit)).prompt {
        SnapshotSelection::Explicit(prompt) => assert_eq!(prompt.source_id, starter_prompt),
        other => panic!("expected an explicit prompt, got {other:?}"),
    }

    let wrong = seed_prompt(&database, "Group prompt", PromptPurpose::GroupChatRoleplay);
    let wrong_character =
        seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
            defaults.direct_prompt_id = Some(wrong);
        });
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request(wrong_character, "prompt-wrong-purpose"))
            .expect_err("wrong purpose"),
        DirectLaunchError::PromptWrongPurpose { prompt_id: wrong }
    );
}

#[test]
fn model_prefers_the_character_default_over_the_application_default() {
    let database = database();
    let application = seed_model(&database, ProviderProtocol::OpenAiCompatible, "openrouter");
    let character_model = seed_model(&database, ProviderProtocol::Gemini, "gemini");
    set_application_default_model(&database, application);
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.model_profile_id = Some(character_model);
    });
    match &direct_details(&plan_for(
        &database,
        &request(character_id, "model-character"),
    ))
    .model
    {
        SnapshotSelection::Inherited(model) => {
            assert_eq!(model.source_id, character_model);
            assert_eq!(model.provider_kind, ModelProviderKind::Gemini);
            assert_eq!(model.context_length, Some(8192));
            assert_eq!(model.max_output_tokens, Some(1024));
        }
        other => panic!("expected an inherited model, got {other:?}"),
    }

    let plain = plain_character(&database);
    match &direct_details(&plan_for(&database, &request(plain, "model-application"))).model {
        SnapshotSelection::Inherited(model) => assert_eq!(model.source_id, application),
        other => panic!("expected the application default model, got {other:?}"),
    }
}

#[test]
fn a_launch_without_any_model_default_is_disabled() {
    let database = database();
    let character_id = plain_character(&database);
    let plan = plan_for(&database, &request(character_id, "model-disabled"));
    assert_eq!(direct_details(&plan).model, SnapshotSelection::Disabled);
    let participant = plan
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::Character)
        .expect("character participant");
    assert_eq!(participant.model_selection, SnapshotSelection::Disabled);
}

#[test]
fn an_unmapped_provider_protocol_becomes_other() {
    let database = database();
    let model = seed_model(
        &database,
        ProviderProtocol::StableDiffusion,
        "mystery-vendor",
    );
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.model_profile_id = Some(model);
    });
    match &direct_details(&plan_for(
        &database,
        &request(character_id, "provider-other"),
    ))
    .model
    {
        SnapshotSelection::Inherited(model) => {
            assert_eq!(model.provider_kind, ModelProviderKind::Other);
        }
        other => panic!("expected an inherited model, got {other:?}"),
    }
}

#[test]
fn memory_is_inherited_from_the_character_policy() {
    let database = database();
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.memory_policy = MemoryPolicy::Dynamic;
    });
    match &direct_details(&plan_for(&database, &request(character_id, "memory"))).memory {
        SnapshotSelection::Inherited(memory) => {
            assert_eq!(memory.mode, MemoryModeSnapshot::Dynamic);
            assert!(memory.selected_revision_ids.is_empty());
            assert!(memory.policy_ref.is_none());
        }
        other => panic!("expected inherited memory, got {other:?}"),
    }
}

#[test]
fn a_foreign_scene_or_starter_is_rejected() {
    let database = database();
    let foreign_scene = text_scene(CharacterId::new(), 0, "Elsewhere.");
    let foreign_scene_id = foreign_scene.id;
    let foreign_starter = starter_with(CharacterId::new(), 0, "Elsewhere", Vec::new());
    let foreign_starter_id = foreign_starter.id;
    seed_character(
        &database,
        vec![foreign_scene],
        Vec::new(),
        vec![foreign_starter],
        |_| {},
    );
    let character_id = plain_character(&database);

    let mut scene_request = request(character_id, "foreign-scene");
    scene_request.scene = LaunchSelection::Explicit(foreign_scene_id);
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&scene_request)
            .expect_err("foreign scene"),
        DirectLaunchError::SceneNotOwned {
            scene_id: foreign_scene_id,
            character_id,
        }
    );

    let mut starter_request = request(character_id, "foreign-starter");
    starter_request.starter = LaunchSelection::Explicit(foreign_starter_id);
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&starter_request)
            .expect_err("foreign starter"),
        DirectLaunchError::StarterNotOwned {
            starter_id: foreign_starter_id,
            character_id,
        }
    );
}

#[test]
fn two_planner_runs_on_identical_state_are_byte_identical() {
    let database = database();
    let scene = text_scene(CharacterId::new(), 0, "A quiet harbour.");
    let scene_id = scene.id;
    let character_id = seed_character(&database, vec![scene], Vec::new(), Vec::new(), |defaults| {
        defaults.default_scene_id = Some(scene_id);
    });
    let launch = request(character_id, "determinism");
    let first = plan_for(&database, &launch);
    let second = plan_for(&database, &launch);
    assert_eq!(first, second);
}

#[test]
fn an_identical_retry_replays_a_single_conversation() {
    let database = database();
    let character_id = plain_character(&database);
    let launch = request(character_id, "idempotent");
    let planner = ConversationLaunchPlanner::new(&database);
    let first = planner.launch_direct(&launch, NOW).expect("first launch");
    let second = planner
        .launch_direct(&launch, TimestampMillis::new(2_000))
        .expect("replayed launch");
    assert_eq!(first.value.conversation.id, second.value.conversation.id);
    assert_eq!(first.operation.id, second.operation.id);
    assert_eq!(
        ConversationReader::page(
            &database,
            &lettuce_conversations::ConversationQuery::default()
        )
        .expect("page")
        .items
        .len(),
        1
    );
}

#[test]
fn reusing_a_key_after_a_source_edit_conflicts() {
    let database = database();
    let character_id = plain_character(&database);
    let launch = request(character_id, "conflicting");
    let planner = ConversationLaunchPlanner::new(&database);
    planner.launch_direct(&launch, NOW).expect("first launch");
    CharacterRepository::revise_profile(
        &database,
        character_id,
        Revision::INITIAL,
        CharacterProfile {
            name: "Ada Lovelace".into(),
            nickname: Some("Addy".into()),
            description: Some("A meticulous engineer".into()),
            definition: None,
            design_description: None,
        },
        NOW,
    )
    .expect("revise character");
    assert_eq!(
        planner
            .launch_direct(&launch, TimestampMillis::new(2_000))
            .expect_err("conflict"),
        DirectLaunchError::CreateConflict
    );
}

#[test]
fn a_different_operation_key_creates_a_second_conversation() {
    let database = database();
    let character_id = plain_character(&database);
    let planner = ConversationLaunchPlanner::new(&database);
    let first = planner
        .launch_direct(&request(character_id, "first-key"), NOW)
        .expect("first launch");
    let second = planner
        .launch_direct(&request(character_id, "second-key"), NOW)
        .expect("second launch");
    assert_ne!(first.value.conversation.id, second.value.conversation.id);
    assert_eq!(
        ConversationReader::page(
            &database,
            &lettuce_conversations::ConversationQuery::default()
        )
        .expect("page")
        .items
        .len(),
        2
    );
}

#[test]
fn the_app_backend_exposes_the_direct_launch() {
    let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let character_id = plain_character(backend.database());
    let result = backend
        .launch_direct_conversation(&request(character_id, "backend-launch"), NOW)
        .expect("launch");
    result.value.validate().expect("aggregate validates");
    assert_eq!(
        result.value.conversation.id,
        super::identity::launch_conversation_id(&key("backend-launch"))
    );
}

#[test]
fn scene_inherit_uses_the_default_then_the_lowest_ordinal_active_scene() {
    let database = database();
    let first = text_scene(CharacterId::new(), 0, "First scene.");
    let second = text_scene(CharacterId::new(), 1, "Second scene.");
    let second_id = second.id;
    let chosen = seed_character(
        &database,
        vec![first, second],
        Vec::new(),
        Vec::new(),
        |defaults| {
            defaults.default_scene_id = Some(second_id);
        },
    );
    match &direct_details(&plan_for(&database, &request(chosen, "scene-tier-default"))).scene {
        SnapshotSelection::Inherited(scene) => assert_eq!(scene.source_id, second_id),
        other => panic!("expected the character default scene, got {other:?}"),
    }

    let third = text_scene(CharacterId::new(), 0, "First scene.");
    let fourth = text_scene(CharacterId::new(), 1, "Second scene.");
    let third_id = third.id;
    let fallback = seed_character(
        &database,
        vec![third, fourth],
        Vec::new(),
        Vec::new(),
        |_| {},
    );
    match &direct_details(&plan_for(
        &database,
        &request(fallback, "scene-tier-lowest"),
    ))
    .scene
    {
        SnapshotSelection::Inherited(scene) => assert_eq!(scene.source_id, third_id),
        other => panic!("expected the lowest-ordinal scene, got {other:?}"),
    }
}

#[test]
fn an_inherited_scene_pointer_degrades_when_archived_and_errors_when_dangling() {
    let character_id = CharacterId::new();
    let mut archived = text_scene(character_id, 0, "A quiet harbour.");
    archived.status = LifecycleStatus::Archived;
    let archived_id = archived.id;
    let active = text_scene(character_id, 1, "A busy market.");
    let scenes = vec![archived, active];

    assert_eq!(
        policy::inherited_scene(&scenes, Some(archived_id)),
        policy::InheritedScene::None
    );
    match policy::inherited_scene(&scenes, None) {
        policy::InheritedScene::Resolved(scene) => assert_eq!(scene.ordinal, 1),
        other => panic!("expected the lowest-ordinal active scene, got {other:?}"),
    }

    let dangling = SceneId::new();
    assert_eq!(
        policy::inherited_scene(&scenes, Some(dangling)),
        policy::InheritedScene::Dangling(dangling)
    );
    assert_eq!(
        policy::inherited_scene(&[], None),
        policy::InheritedScene::None
    );
}

#[test]
fn a_default_starter_is_never_applied_without_an_explicit_request() {
    let database = database();
    let starter = starter_with(
        CharacterId::new(),
        0,
        "Greeting",
        vec![message(StarterRole::Assistant, "Welcome.")],
    );
    let starter_id = starter.id;
    let character_id = seed_character(
        &database,
        Vec::new(),
        Vec::new(),
        vec![starter],
        |defaults| {
            defaults.default_starter_id = Some(starter_id);
        },
    );
    let plan = plan_for(&database, &request(character_id, "default-starter-ignored"));
    assert_eq!(direct_details(&plan).starter, SnapshotSelection::Disabled);
    assert!(plan.initial_timeline.entries.is_empty());

    let explicit = plan_for(
        &database,
        &request_with_starter(character_id, "default-starter-explicit", starter_id),
    );
    assert!(matches!(
        direct_details(&explicit).starter,
        SnapshotSelection::Explicit(_)
    ));
}

#[test]
fn launching_without_a_default_persona_freezes_the_absence() {
    let database = database();
    let character_id = plain_character(&database);
    let persona_id = seed_persona(&database, "Traveller");
    let launch = request(character_id, "persona-absence-frozen");
    let first = plan_for(&database, &launch);
    assert_eq!(direct_details(&first).persona, SnapshotSelection::Disabled);

    let default_revision = PersonaRepository::get_default_snapshot(&database)
        .expect("default snapshot")
        .state
        .revision;
    PersonaRepository::set_default(&database, persona_id, default_revision, NOW)
        .expect("set default");
    let second = plan_for(&database, &request(character_id, "persona-absence-later"));
    assert!(matches!(
        direct_details(&second).persona,
        SnapshotSelection::Inherited(_)
    ));
}

#[test]
fn an_inherited_archived_prompt_degrades_while_an_authored_one_errors() {
    let database = database();
    let prompt_id = seed_prompt(&database, "Character prompt", PromptPurpose::DirectChat);
    let mut starter = starter_with(CharacterId::new(), 0, "Greeting", Vec::new());
    starter.prompt_id = Some(prompt_id);
    let starter_id = starter.id;
    let character_id = seed_character(
        &database,
        Vec::new(),
        Vec::new(),
        vec![starter],
        |defaults| {
            defaults.direct_prompt_id = Some(prompt_id);
        },
    );
    PromptRepository::archive(&database, prompt_id, Revision::INITIAL, NOW).expect("archive");

    let inherited = plan_for(
        &database,
        &request(character_id, "prompt-archived-inherited"),
    );
    assert_eq!(
        direct_details(&inherited).prompt,
        SnapshotSelection::Disabled
    );

    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request_with_starter(
                character_id,
                "prompt-archived-authored",
                starter_id
            ))
            .expect_err("archived starter prompt"),
        DirectLaunchError::PromptArchived { prompt_id }
    );
}

#[test]
fn an_archived_bound_lorebook_is_skipped_but_an_authored_one_errors() {
    let database = database();
    let kept = seed_lorebook(&database, "Kept");
    let archived = seed_lorebook(&database, "Archived");
    let mut starter = starter_with(CharacterId::new(), 0, "Greeting", Vec::new());
    starter.lorebooks = Selection::Explicit(vec![archived]);
    let starter_id = starter.id;
    let character_id = seed_character(&database, Vec::new(), Vec::new(), vec![starter], |_| {});
    let bound = CharacterLorebookBindingRepository::bind_character_lorebook(
        &database,
        character_id,
        Revision::INITIAL,
        LorebookBindingCreate {
            lorebook_id: kept,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind kept");
    CharacterLorebookBindingRepository::bind_character_lorebook(
        &database,
        character_id,
        bound.owner_revision,
        LorebookBindingCreate {
            lorebook_id: archived,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind archived");
    LorebookRepository::archive(&database, archived, Revision::INITIAL, NOW).expect("archive book");

    let inherited = plan_for(
        &database,
        &request(character_id, "lorebook-archived-skipped"),
    );
    assert_eq!(
        lorebook_names(&direct_details(&inherited).lorebooks),
        Some(vec!["Kept".to_owned()])
    );

    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request_with_starter(
                character_id,
                "lorebook-archived-authored",
                starter_id
            ))
            .expect_err("authored archived lorebook"),
        DirectLaunchError::LorebookArchived {
            lorebook_id: archived
        }
    );
}

#[test]
fn a_non_chat_model_or_disabled_provider_is_rejected() {
    let database = database();
    let image = seed_model_with(
        &database,
        ProviderProtocol::OpenAiCompatible,
        "openai",
        ModelKind::Image,
        true,
    );
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.model_profile_id = Some(image);
    });
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request(character_id, "model-non-chat"))
            .expect_err("non chat model"),
        DirectLaunchError::NonChatModel {
            model_profile_id: image
        }
    );

    let disabled = seed_model_with(
        &database,
        ProviderProtocol::Anthropic,
        "anthropic",
        ModelKind::Chat,
        false,
    );
    let disabled_character =
        seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
            defaults.model_profile_id = Some(disabled);
        });
    let account = ModelProfileRepository::get(&database, disabled)
        .expect("profile")
        .expect("profile exists")
        .provider_account_id;
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request(disabled_character, "model-disabled-provider"))
            .expect_err("disabled provider"),
        DirectLaunchError::ProviderDisabled {
            provider_account_id: account
        }
    );
}

#[test]
fn an_image_application_default_model_is_rejected() {
    let database = database();
    let image = seed_model_with(
        &database,
        ProviderProtocol::OpenAiCompatible,
        "openai",
        ModelKind::Image,
        true,
    );
    set_application_default_model(&database, image);
    let character_id = plain_character(&database);
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request(character_id, "application-default-image"))
            .expect_err("image application default"),
        DirectLaunchError::NonChatModel {
            model_profile_id: image
        }
    );
}

#[test]
fn the_resolved_lorebook_set_is_bounded_before_loading() {
    assert!(!policy::lorebook_bound_exceeded(
        policy::MAX_LAUNCH_LOREBOOKS
    ));
    assert!(policy::lorebook_bound_exceeded(
        policy::MAX_LAUNCH_LOREBOOKS + 1
    ));

    let database = database();
    let character_id = plain_character(&database);
    let mut revision = Revision::INITIAL;
    for index in 0..policy::MAX_LAUNCH_LOREBOOKS {
        let book = seed_lorebook(&database, &format!("Book {index}"));
        revision = CharacterLorebookBindingRepository::bind_character_lorebook(
            &database,
            character_id,
            revision,
            LorebookBindingCreate {
                lorebook_id: book,
                target: BindingInsertionTarget::Append,
            },
            NOW,
        )
        .expect("bind")
        .owner_revision;
    }
    assert!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request(character_id, "lorebooks-at-bound"))
            .is_ok()
    );

    let overflow = seed_lorebook(&database, "Book overflow");
    CharacterLorebookBindingRepository::bind_character_lorebook(
        &database,
        character_id,
        revision,
        LorebookBindingCreate {
            lorebook_id: overflow,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind overflow");
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request(character_id, "lorebooks-over-bound"))
            .expect_err("lorebook bound"),
        DirectLaunchError::TooManyLorebooks {
            max: policy::MAX_LAUNCH_LOREBOOKS
        }
    );
}

#[test]
fn the_initial_timeline_is_bounded_before_building() {
    assert!(!policy::timeline_bound_exceeded(
        policy::MAX_LAUNCH_TIMELINE_ENTRIES
    ));
    assert!(policy::timeline_bound_exceeded(
        policy::MAX_LAUNCH_TIMELINE_ENTRIES + 1
    ));

    let database = database();
    let at_bound = starter_with(
        CharacterId::new(),
        0,
        "At bound",
        (0..policy::MAX_LAUNCH_TIMELINE_ENTRIES)
            .map(|_| message(StarterRole::Assistant, "Line."))
            .collect(),
    );
    let over_bound = starter_with(
        CharacterId::new(),
        1,
        "Over bound",
        (0..=policy::MAX_LAUNCH_TIMELINE_ENTRIES)
            .map(|_| message(StarterRole::Assistant, "Line."))
            .collect(),
    );
    let at_bound_id = at_bound.id;
    let over_bound_id = over_bound.id;
    let character_id = seed_character(
        &database,
        Vec::new(),
        Vec::new(),
        vec![at_bound, over_bound],
        |_| {},
    );
    let plan = plan_for(
        &database,
        &request_with_starter(character_id, "timeline-at-bound", at_bound_id),
    );
    assert_eq!(
        plan.initial_timeline.entries.len(),
        policy::MAX_LAUNCH_TIMELINE_ENTRIES
    );
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request_with_starter(
                character_id,
                "timeline-over-bound",
                over_bound_id
            ))
            .expect_err("timeline bound"),
        DirectLaunchError::TooManyInitialMessages {
            max: policy::MAX_LAUNCH_TIMELINE_ENTRIES
        }
    );
}

#[test]
fn retrying_after_a_source_is_archived_reports_the_existing_conversation() {
    let database = database();
    let character_id = plain_character(&database);
    let launch = request(character_id, "already-launched");
    let planner = ConversationLaunchPlanner::new(&database);
    let created = planner.launch_direct(&launch, NOW).expect("first launch");
    CharacterRepository::archive(&database, character_id, Revision::INITIAL, NOW)
        .expect("archive character");
    assert_eq!(
        planner
            .launch_direct(&launch, TimestampMillis::new(2_000))
            .expect_err("already launched"),
        DirectLaunchError::AlreadyLaunched {
            conversation_id: created.value.conversation.id
        }
    );
}

#[test]
fn source_drift_between_the_two_reads_is_detected() {
    let character_id = CharacterId::new();
    let persona_id = PersonaId::new();
    assert_eq!(
        policy::detect_source_drift(
            (character_id, Revision::INITIAL, Revision::INITIAL),
            Some((persona_id, Revision::INITIAL, Revision::INITIAL)),
        ),
        None
    );
    assert_eq!(
        policy::detect_source_drift((character_id, Revision::INITIAL, Revision::new(2)), None,),
        Some(SnapshotSource::Character(character_id))
    );
    assert_eq!(
        policy::detect_source_drift(
            (character_id, Revision::INITIAL, Revision::INITIAL),
            Some((persona_id, Revision::INITIAL, Revision::new(3))),
        ),
        Some(SnapshotSource::Persona(persona_id))
    );
}

#[test]
fn a_variant_belonging_to_another_scene_is_ignored() {
    let character_id = CharacterId::new();
    let mut scene = text_scene(character_id, 0, "Base body.");
    let variant_id = SceneVariantId::new();
    scene.selected_variant_id = Some(variant_id);
    let foreign = SceneVariant {
        id: variant_id,
        scene_id: SceneId::new(),
        ordinal: 0,
        content: SceneDocumentV1::new(vec![ScenePart::Text {
            text: "Foreign body.".into(),
        }])
        .expect("variant document"),
        direction: None,
        revision: Revision::INITIAL,
        created_at: TimestampMillis::new(1),
        updated_at: TimestampMillis::new(1),
    };
    assert_eq!(
        policy::resolve_scene_text(&scene, std::slice::from_ref(&foreign)),
        Some("Base body.".to_owned())
    );
}

#[test]
fn generated_operation_keys_are_unique() {
    let first = DirectConversationLaunchRequest::new_operation_key();
    let second = DirectConversationLaunchRequest::new_operation_key();
    assert_ne!(first, second);
    assert_ne!(
        super::identity::launch_conversation_id(&first),
        super::identity::launch_conversation_id(&second)
    );
}
