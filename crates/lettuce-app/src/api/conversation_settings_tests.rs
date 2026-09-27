use std::sync::Arc;

use lettuce_characters::{ChatMode, GroupRepository, MemoryPolicy, Selection, SpeakerSelection};
use lettuce_context::{
    BindingInsertionTarget, GroupLorebookBindingRepository, LorebookBindingCreate, PromptPurpose,
};
use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails};
use lettuce_conversations::{ConversationReader, ParticipantSource};
use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaAssetRepository,
    MediaBlob, MediaBlobRepository, MediaKind, RetentionClass,
};
use lettuce_models::ProviderProtocol;
use lettuce_types::{
    AssetId, CharacterId, ContentHash, ConversationId, GroupId, MediaBlobId, Revision,
    TimestampMillis,
};

use super::tests::{Harness, RecordingStream, Reply, harness, send};
use super::*;
use crate::launch::tests::{
    group_starting_scene, member, seed_group, seed_lorebook, seed_model, seed_named_character,
    seed_persona, seed_prompt,
};

const NOW: TimestampMillis = TimestampMillis::new(50);

fn image_asset(database: &lettuce_database::Database) -> AssetId {
    let blob = MediaBlobRepository::register(
        database,
        MediaBlob {
            id: MediaBlobId::new(),
            content_hash: ContentHash::parse("cd".repeat(32)).expect("hash"),
            kind: MediaKind::Image,
            mime_type: "image/webp".into(),
            byte_size: 4,
            width: Some(2),
            height: Some(2),
            duration_ms: None,
            validation_version: 1,
            state: BlobState::Staged,
            created_at: TimestampMillis::new(3),
            updated_at: TimestampMillis::new(3),
        },
    )
    .expect("blob");
    let blob =
        MediaBlobRepository::finalize_staged_to_ready(database, blob.id, TimestampMillis::new(3))
            .expect("ready blob");
    MediaAssetRepository::create(
        database,
        MediaAsset::new(
            AssetId::new(),
            blob.id,
            AssetKind::BackgroundImage,
            AssetOrigin::Upload,
            RetentionClass::Library,
            AssetProvenanceV1::default(),
            Revision::INITIAL,
            TimestampMillis::new(4),
            TimestampMillis::new(4),
        )
        .expect("asset"),
    )
    .expect("create asset")
    .id
}

fn group_revision(harness: &Harness, group_id: GroupId) -> Revision {
    GroupRepository::get(harness.context.backend().database(), group_id)
        .expect("group")
        .expect("group exists")
        .group
        .revision
}

async fn launch_group(harness: &Harness, group_id: GroupId, key: &str) -> String {
    conversation_launch_group(
        &harness.context,
        dto::LaunchGroupRequest {
            group_id: group_id.to_string(),
            client_operation_id: key.into(),
        },
    )
    .await
    .expect("launch group")
    .conversation_id
}

async fn view(harness: &Harness, conversation_id: &str) -> dto::ConversationSettingsView {
    conversation_settings_get(
        &harness.context,
        dto::ConversationSettingsGetRequest {
            conversation_id: conversation_id.into(),
        },
    )
    .await
    .expect("settings")
}

async fn update(
    harness: &Harness,
    conversation_id: &str,
    expected: Option<u64>,
    patch: dto::ConversationSettingsPatch,
) -> Result<dto::ConversationSettingsView, dto::ApiError> {
    conversation_settings_update(
        &harness.context,
        dto::ConversationSettingsUpdateRequest {
            conversation_id: conversation_id.into(),
            expected_settings_revision: expected,
            patch,
        },
    )
    .await
}

async fn participant_update(
    harness: &Harness,
    conversation_id: &str,
    participant_id: &str,
    enabled: Option<bool>,
    muted: Option<bool>,
) -> Result<dto::ConversationRevisions, dto::ApiError> {
    conversation_participant_update(
        &harness.context,
        dto::ConversationParticipantUpdateRequest {
            conversation_id: conversation_id.into(),
            participant_id: participant_id.into(),
            enabled,
            muted,
            model: None,
        },
    )
    .await
}

fn member_view(
    view: &dto::ConversationSettingsView,
    character_id: CharacterId,
) -> Option<&dto::ParticipantSettings> {
    view.members.as_ref().and_then(|members| {
        members
            .participants
            .iter()
            .find(|participant| participant.character_id == Some(character_id.to_string()))
    })
}

fn id<T: ToString>(value: T) -> Option<String> {
    Some(value.to_string())
}

fn field(error: &dto::ApiError) -> Option<&str> {
    match &error.details {
        Some(ApiErrorDetails::InvalidField { field }) => Some(field.as_str()),
        _ => None,
    }
}

/// Each of the group's settings reaches an existing chat that does not set
/// it; a key the chat set stays; a reset follows the group again.
#[tokio::test(flavor = "multi_thread")]
async fn a_group_chat_follows_each_group_setting_until_it_sets_the_key_itself() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let ada = seed_named_character(database, "Ada");
    let bea = seed_named_character(database, "Bea");
    let cleo = seed_named_character(database, "Cleo");
    let group_id = seed_group(
        database,
        vec![member(ada, 0), member(bea, 1)],
        Some(group_starting_scene("The hall.")),
        |group| group.speaker_selection = SpeakerSelection::RoundRobin,
    );
    let chat = launch_group(&harness, group_id, "follow-launch").await;
    let launched = view(&harness, &chat).await;
    assert_eq!(
        launched.chat_mode.expect("mode").source,
        dto::SettingSource::Group
    );
    assert_eq!(
        launched.speaker_selection.expect("speaker").method,
        dto::SpeakerSelectionMethod::RoundRobin
    );
    assert_eq!(launched.memory.mode, dto::MemoryMode::Manual);

    let persona = seed_persona(database, "Wren");
    let talk = seed_prompt(database, "Talk", PromptPurpose::GroupChatConversational);
    let play = seed_prompt(database, "Play", PromptPurpose::GroupChatRoleplay);
    let book = seed_lorebook(database, "Lore");
    let model = seed_model(database, ProviderProtocol::Ollama, "member-model");
    let background = image_asset(database);
    GroupRepository::set_persona(
        database,
        group_id,
        group_revision(&harness, group_id),
        Selection::Explicit(persona),
        NOW,
    )
    .expect("persona");
    GroupRepository::set_chat_mode(
        database,
        group_id,
        group_revision(&harness, group_id),
        ChatMode::Roleplay,
        NOW,
    )
    .expect("chat mode");
    GroupRepository::set_speaker_selection(
        database,
        group_id,
        group_revision(&harness, group_id),
        SpeakerSelection::Heuristic,
        NOW,
    )
    .expect("speaker selection");
    GroupRepository::set_memory_policy(
        database,
        group_id,
        group_revision(&harness, group_id),
        MemoryPolicy::Dynamic,
        NOW,
    )
    .expect("memory");
    GroupRepository::set_disable_character_lorebooks(
        database,
        group_id,
        group_revision(&harness, group_id),
        true,
        NOW,
    )
    .expect("character lorebooks");
    GroupRepository::set_prompt_defaults(
        database,
        group_id,
        group_revision(&harness, group_id),
        Some(talk),
        Some(play),
        NOW,
    )
    .expect("prompts");
    GroupRepository::set_background(
        database,
        group_id,
        group_revision(&harness, group_id),
        Some(background),
        NOW,
    )
    .expect("background");
    let mut scene = group_starting_scene("The garden.");
    scene.scene.owner = lettuce_characters::SceneOwner::Group(group_id);
    let scene_id = scene.scene.id;
    GroupRepository::set_starting_scene(
        database,
        group_id,
        group_revision(&harness, group_id),
        Some(scene),
        NOW,
    )
    .expect("scene");
    GroupRepository::replace_members(
        database,
        group_id,
        group_revision(&harness, group_id),
        vec![member(ada, 0), member(bea, 1), member(cleo, 2)],
        NOW,
    )
    .expect("members");
    GroupRepository::set_member_muted(
        database,
        group_id,
        group_revision(&harness, group_id),
        bea,
        true,
        NOW,
    )
    .expect("mute");
    GroupRepository::set_member_model_override(
        database,
        group_id,
        group_revision(&harness, group_id),
        ada,
        Some(model),
        NOW,
    )
    .expect("member model");
    GroupLorebookBindingRepository::bind_group_lorebook(
        database,
        group_id,
        group_revision(&harness, group_id),
        LorebookBindingCreate {
            lorebook_id: book,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("group lorebook");

    let live = view(&harness, &chat).await;
    let group = dto::SettingSource::Group;
    assert_eq!(
        live.persona,
        dto::SettingChoice {
            id: id(persona),
            source: group
        }
    );
    assert_eq!(
        live.chat_mode.map(|mode| (mode.mode, mode.source)),
        Some((dto::GroupChatMode::Roleplay, group))
    );
    assert_eq!(
        live.speaker_selection
            .map(|speaker| (speaker.method, speaker.source)),
        Some((dto::SpeakerSelectionMethod::Heuristic, group))
    );
    assert_eq!(
        (live.memory.mode, live.memory.source),
        (dto::MemoryMode::Dynamic, group)
    );
    assert_eq!(
        live.disable_character_lorebooks
            .map(|flag| (flag.value, flag.source)),
        Some((true, group))
    );
    assert_eq!(
        live.prompt,
        dto::SettingChoice {
            id: id(talk),
            source: group
        }
    );
    assert_eq!(
        live.roleplay_prompt,
        Some(dto::SettingChoice {
            id: id(play),
            source: group
        })
    );
    assert_eq!(
        live.background
            .asset
            .as_ref()
            .map(|asset| asset.asset_id.clone()),
        id(background)
    );
    assert_eq!(live.background.source, group);
    assert_eq!(
        live.scene,
        dto::SettingChoice {
            id: id(scene_id),
            source: group
        }
    );
    assert_eq!(live.lorebooks.ids, vec![book.to_string()]);
    assert_eq!(live.lorebooks.source, group);
    let members = live.members.as_ref().expect("members");
    assert_eq!(
        (
            members.members_source,
            members.muted_source,
            members.models_source
        ),
        (group, group, group)
    );
    assert!(member_view(&live, cleo).is_some_and(|cleo| cleo.enabled && !cleo.muted));
    assert!(member_view(&live, bea).is_some_and(|bea| bea.muted));
    assert_eq!(
        member_view(&live, ada).expect("ada").model,
        dto::SettingChoice {
            id: id(model),
            source: group
        }
    );
    let conversation_id: ConversationId = chat.parse().expect("id");
    let stored = ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation;
    let turn_group = crate::generation::live_sources::live_group(database, &stored)
        .expect("live group")
        .expect("group chat");
    assert_eq!(
        turn_group.chat_mode,
        lettuce_conversations::GroupChatModeSnapshot::Roleplay
    );
    assert_eq!(
        turn_group.speaker_selection,
        lettuce_conversations::GroupSpeakerSelectionSnapshot::Heuristic
    );
    assert!(turn_group.disable_character_lorebooks);

    let other_persona = seed_persona(database, "Oak");
    let other_talk = seed_prompt(database, "Talk two", PromptPurpose::GroupChatConversational);
    let own = update(
        &harness,
        &chat,
        live.settings_revision,
        dto::ConversationSettingsPatch {
            persona: Some(dto::ChoiceChange::Set {
                id: other_persona.to_string(),
            }),
            prompt: Some(dto::ChoiceChange::Set {
                id: other_talk.to_string(),
            }),
            roleplay_prompt: Some(dto::ChoiceChange::None),
            lorebooks: Some(dto::LorebooksChange::Set { ids: Vec::new() }),
            background: Some(dto::BackgroundChange::Hidden),
            scene: Some(dto::IdChange::Set {
                id: scene_id.to_string(),
            }),
            speaker_selection: Some(dto::SpeakerSelectionChange::Set {
                method: dto::SpeakerSelectionMethod::RoundRobin,
            }),
            memory: Some(dto::MemoryModeChange::Set {
                mode: dto::MemoryMode::Manual,
            }),
            chat_mode: Some(dto::ChatModeChange::Set {
                mode: dto::GroupChatMode::Roleplay,
            }),
            disable_character_lorebooks: Some(dto::FlagChange::Set { value: false }),
            ..dto::ConversationSettingsPatch::default()
        },
    )
    .await
    .expect("own settings");
    assert!(
        own.revision > live.revision,
        "a settings change bumps the conversation"
    );
    let bea_participant = member_view(&own, bea).expect("bea").participant_id.clone();
    participant_update(&harness, &chat, &bea_participant, None, Some(false))
        .await
        .expect("unmute bea in the chat");

    GroupRepository::set_persona(
        database,
        group_id,
        group_revision(&harness, group_id),
        Selection::Disabled,
        NOW,
    )
    .expect("persona again");
    GroupRepository::set_chat_mode(
        database,
        group_id,
        group_revision(&harness, group_id),
        ChatMode::Conversation,
        NOW,
    )
    .expect("mode again");
    GroupRepository::set_speaker_selection(
        database,
        group_id,
        group_revision(&harness, group_id),
        SpeakerSelection::Llm,
        NOW,
    )
    .expect("speaker again");
    GroupRepository::set_memory_policy(
        database,
        group_id,
        group_revision(&harness, group_id),
        MemoryPolicy::Dynamic,
        NOW,
    )
    .expect("memory again");
    GroupRepository::set_member_muted(
        database,
        group_id,
        group_revision(&harness, group_id),
        bea,
        true,
        NOW,
    )
    .expect("mute again");
    let kept = view(&harness, &chat).await;
    let conversation = dto::SettingSource::Conversation;
    assert_eq!(
        kept.persona,
        dto::SettingChoice {
            id: id(other_persona),
            source: conversation
        }
    );
    assert_eq!(
        kept.prompt,
        dto::SettingChoice {
            id: id(other_talk),
            source: conversation
        }
    );
    assert_eq!(
        kept.roleplay_prompt,
        Some(dto::SettingChoice {
            id: None,
            source: conversation
        })
    );
    assert_eq!(
        (kept.lorebooks.ids.len(), kept.lorebooks.source),
        (0, conversation)
    );
    assert!(kept.background.hidden);
    assert_eq!(kept.scene.source, conversation);
    assert_eq!(
        kept.chat_mode.map(|mode| (mode.mode, mode.source)),
        Some((dto::GroupChatMode::Roleplay, conversation))
    );
    assert_eq!(
        kept.speaker_selection
            .map(|speaker| (speaker.method, speaker.source)),
        Some((dto::SpeakerSelectionMethod::RoundRobin, conversation))
    );
    assert_eq!(
        (kept.memory.mode, kept.memory.source),
        (dto::MemoryMode::Manual, conversation)
    );
    assert_eq!(
        kept.disable_character_lorebooks
            .map(|flag| (flag.value, flag.source)),
        Some((false, conversation))
    );
    assert_eq!(
        kept.members.as_ref().expect("members").muted_source,
        conversation
    );
    assert!(
        member_view(&kept, bea).is_some_and(|bea| !bea.muted),
        "the chat's own mute stays"
    );

    let reset = update(
        &harness,
        &chat,
        kept.settings_revision,
        dto::ConversationSettingsPatch {
            persona: Some(dto::ChoiceChange::Reset),
            prompt: Some(dto::ChoiceChange::Reset),
            roleplay_prompt: Some(dto::ChoiceChange::Reset),
            lorebooks: Some(dto::LorebooksChange::Reset),
            background: Some(dto::BackgroundChange::Reset),
            scene: Some(dto::IdChange::Reset),
            speaker_selection: Some(dto::SpeakerSelectionChange::Reset),
            memory: Some(dto::MemoryModeChange::Reset),
            chat_mode: Some(dto::ChatModeChange::Reset),
            disable_character_lorebooks: Some(dto::FlagChange::Reset),
            reset_members: true,
            reset_member_models: true,
            ..dto::ConversationSettingsPatch::default()
        },
    )
    .await
    .expect("reset");
    assert_eq!(
        reset.persona,
        dto::SettingChoice {
            id: None,
            source: group
        }
    );
    assert_eq!(
        reset.prompt,
        dto::SettingChoice {
            id: id(talk),
            source: group
        }
    );
    assert_eq!(
        reset.roleplay_prompt,
        Some(dto::SettingChoice {
            id: id(play),
            source: group
        })
    );
    assert_eq!(reset.lorebooks.ids, vec![book.to_string()]);
    assert_eq!(reset.background.source, group);
    assert_eq!(reset.scene.source, group);
    assert_eq!(
        reset.chat_mode.map(|mode| (mode.mode, mode.source)),
        Some((dto::GroupChatMode::Conversation, group))
    );
    assert_eq!(
        reset
            .speaker_selection
            .map(|speaker| (speaker.method, speaker.source)),
        Some((dto::SpeakerSelectionMethod::Llm, group))
    );
    assert_eq!(
        (reset.memory.mode, reset.memory.source),
        (dto::MemoryMode::Dynamic, group)
    );
    let members = reset.members.as_ref().expect("members");
    assert_eq!(
        (members.members_source, members.muted_source),
        (group, group)
    );
    assert!(
        member_view(&reset, bea).is_some_and(|bea| bea.muted),
        "the group's mute again"
    );
}

/// A member the group gained after the chat launched gets its row when a
/// send needs it, once, however often the send is retried or raced, and
/// speaks in the old chat.
#[tokio::test(flavor = "multi_thread")]
async fn a_member_the_group_gained_after_launch_speaks_in_an_old_chat() {
    let harness = harness(Reply::Text("Cleo here."));
    let database = harness.context.backend().database();
    let ada = seed_named_character(database, "Ada");
    let bea = seed_named_character(database, "Bea");
    let cleo = seed_named_character(database, "Cleo");
    let group_id = seed_group(
        database,
        vec![member(ada, 0), member(bea, 1)],
        None,
        |group| {
            group.speaker_selection = SpeakerSelection::RoundRobin;
        },
    );
    let chat = launch_group(&harness, group_id, "gained-launch").await;
    let conversation_id: ConversationId = chat.parse().expect("id");
    let mut muted_ada = member(ada, 0);
    muted_ada.muted = true;
    let mut muted_bea = member(bea, 1);
    muted_bea.muted = true;
    GroupRepository::replace_members(
        database,
        group_id,
        group_revision(&harness, group_id),
        vec![muted_ada, muted_bea, member(cleo, 2)],
        NOW,
    )
    .expect("cleo joins the group");

    let (first, second) = tokio::join!(
        harness.context.blocking(move |context| {
            crate::conversation::ensure_group_members(
                context.backend().database(),
                conversation_id,
                context.now(),
            )
            .map_err(super::conversation_settings::edit_error)
        }),
        harness.context.blocking(move |context| {
            crate::conversation::ensure_group_members(
                context.backend().database(),
                conversation_id,
                context.now(),
            )
            .map_err(super::conversation_settings::edit_error)
        }),
    );
    first.expect("first");
    second.expect("second");
    let rows = |database: &lettuce_database::Database| {
        ConversationReader::get(database, conversation_id)
            .expect("conversation")
            .conversation
            .participants
            .iter()
            .filter(|participant| participant.source == ParticipantSource::Character(cleo))
            .count()
    };
    assert_eq!(rows(database), 1, "racing paths create one row");

    let stream = Arc::new(RecordingStream::default());
    let accepted = send(
        &harness,
        &chat,
        "gained-send",
        "Who is new?",
        stream.clone(),
    )
    .await
    .expect("send");
    let again = send(
        &harness,
        &chat,
        "gained-send",
        "Who is new?",
        stream.clone(),
    )
    .await
    .expect("retried send");
    assert_eq!(again.turn_id, accepted.turn_id);
    assert_eq!(rows(database), 1);
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    assert!(worker.run_once().await.expect("worker ran"));
    let turn = ConversationReader::get_turn(database, accepted.turn_id.parse().expect("turn"))
        .expect("turn");
    assert_eq!(
        turn.selected_speaker.map(|speaker| speaker.participant_id),
        Some(crate::conversation::member_participant_id(
            conversation_id,
            cleo
        )),
        "the only unmuted member is the one the group gained"
    );
    assert_eq!(
        turn.status,
        lettuce_conversations::GenerationTurnStatus::Succeeded
    );
}

/// A chat whose group no longer exists keeps its launch values.
#[tokio::test(flavor = "multi_thread")]
async fn a_chat_whose_group_is_gone_keeps_its_launch_values() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let ada = seed_named_character(database, "Ada");
    let bea = seed_named_character(database, "Bea");
    let cleo = seed_named_character(database, "Cleo");
    let group_id = seed_group(
        database,
        vec![member(ada, 0), member(bea, 1), member(cleo, 2)],
        None,
        |group| {
            group.speaker_selection = SpeakerSelection::RoundRobin;
        },
    );
    let chat = launch_group(&harness, group_id, "gone-launch").await;
    GroupRepository::set_speaker_selection(
        database,
        group_id,
        group_revision(&harness, group_id),
        SpeakerSelection::Heuristic,
        NOW,
    )
    .expect("the group changes");
    GroupRepository::replace_members(
        database,
        group_id,
        group_revision(&harness, group_id),
        vec![member(ada, 0), member(cleo, 1)],
        NOW,
    )
    .expect("bea leaves the group");
    let mut conversation = ConversationReader::get(database, chat.parse().expect("id"))
        .expect("conversation")
        .conversation;
    let live = crate::generation::live_sources::live_group(database, &conversation)
        .expect("live")
        .expect("group chat");
    assert_eq!(
        live.speaker_selection,
        lettuce_conversations::GroupSpeakerSelectionSnapshot::Heuristic
    );
    let following = crate::generation::live_sources::effective_participants(
        &conversation,
        live.profile.as_ref(),
    );
    assert!(following.iter().any(|participant| {
        participant.source == ParticipantSource::Character(bea) && !participant.enabled
    }));
    let lettuce_conversations::ConversationKind::Group(details) = &mut conversation.kind else {
        panic!("a group chat");
    };
    details.group.source_id = GroupId::new();
    let gone = crate::generation::live_sources::live_group(database, &conversation)
        .expect("live")
        .expect("group chat");
    assert!(gone.profile.is_none());
    assert_eq!(
        gone.speaker_selection,
        lettuce_conversations::GroupSpeakerSelectionSnapshot::RoundRobin
    );
    let launched = crate::generation::live_sources::effective_participants(&conversation, None);
    assert!(
        launched
            .iter()
            .filter(|participant| participant.source != ParticipantSource::User)
            .all(|participant| participant.enabled && !participant.muted)
    );
    let settings = lettuce_settings::GlobalSettingsStore::load(database)
        .expect("settings")
        .settings;
    assert_eq!(
        crate::generation::live_sources::live_memory(database, &conversation, &settings)
            .expect("memory")
            .map(|memory| memory.mode),
        Some(lettuce_conversations::MemoryModeSnapshot::Manual)
    );
}

/// Membership edits: an add is idempotent by its key, the chat then owns its
/// member list, the last active member cannot be muted or removed, and a
/// reset follows the group again.
#[tokio::test(flavor = "multi_thread")]
async fn participants_keep_one_active_member_and_adds_replay_by_key() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let ada = seed_named_character(database, "Ada");
    let bea = seed_named_character(database, "Bea");
    let cleo = seed_named_character(database, "Cleo");
    let dan = seed_named_character(database, "Dan");
    let group_id = seed_group(database, vec![member(ada, 0), member(bea, 1)], None, |_| {});
    let chat = launch_group(&harness, group_id, "members-launch").await;
    let add = |character: CharacterId, key: &str| dto::ConversationParticipantAddRequest {
        conversation_id: chat.clone(),
        character_id: character.to_string(),
        client_operation_id: key.into(),
    };
    let added = conversation_participant_add(&harness.context, add(cleo, "add-cleo"))
        .await
        .expect("add cleo");
    let replayed = conversation_participant_add(&harness.context, add(cleo, "add-cleo"))
        .await
        .expect("retried add");
    assert_eq!(replayed, added);
    let conflict = conversation_participant_add(&harness.context, add(dan, "add-cleo"))
        .await
        .expect_err("the key was used for another character");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);
    let members = view(&harness, &chat).await;
    assert_eq!(
        members.members.as_ref().expect("members").members_source,
        dto::SettingSource::Conversation
    );
    assert!(member_view(&members, cleo).is_some_and(|cleo| cleo.enabled));

    let participant = |view: &dto::ConversationSettingsView, character| {
        member_view(view, character)
            .expect("member")
            .participant_id
            .clone()
    };
    participant_update(
        &harness,
        &chat,
        &participant(&members, ada),
        None,
        Some(true),
    )
    .await
    .expect("mute ada");
    participant_update(
        &harness,
        &chat,
        &participant(&members, bea),
        Some(false),
        None,
    )
    .await
    .expect("remove bea");
    let last = participant_update(
        &harness,
        &chat,
        &participant(&members, cleo),
        None,
        Some(true),
    )
    .await
    .expect_err("cleo is the last active member");
    assert_eq!(last.code, ApiErrorCode::InvalidInput);
    assert_eq!(field(&last), Some("conversation.group.active_member"));
    let removed = participant_update(
        &harness,
        &chat,
        &participant(&members, cleo),
        Some(false),
        None,
    )
    .await
    .expect_err("removing the last active member");
    assert_eq!(field(&removed), Some("conversation.group.active_member"));

    GroupRepository::replace_members(
        database,
        group_id,
        group_revision(&harness, group_id),
        vec![member(ada, 0), member(bea, 1), member(dan, 2)],
        NOW,
    )
    .expect("dan joins the group");
    let owned = view(&harness, &chat).await;
    assert!(
        member_view(&owned, dan).is_none(),
        "the chat owns its member list"
    );
    let followed = update(
        &harness,
        &chat,
        owned.settings_revision,
        dto::ConversationSettingsPatch {
            reset_members: true,
            ..dto::ConversationSettingsPatch::default()
        },
    )
    .await
    .expect("follow the group's members again");
    assert!(member_view(&followed, dan).is_some_and(|dan| dan.enabled));
    assert!(member_view(&followed, bea).is_some_and(|bea| bea.enabled));
    assert!(member_view(&followed, ada).is_some_and(|ada| !ada.muted));
    assert!(
        member_view(&followed, cleo).is_some_and(|cleo| !cleo.enabled),
        "cleo is not a group member"
    );
}

/// Settings take ids, normalise text, refuse unknown sources and stale
/// revisions, and repeat a change made against the same revision.
#[tokio::test(flavor = "multi_thread")]
async fn settings_updates_resolve_ids_and_refuse_stale_revisions() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let chat = super::tests::launch(&harness, "settings-launch").await;
    let first = view(&harness, &chat).await;
    assert_eq!(first.settings_revision, None);
    assert_eq!(first.persona.source, dto::SettingSource::AppDefault);
    let book = seed_lorebook(database, "Lore");
    let patch = dto::ConversationSettingsPatch {
        author_note: Some("  keep it short  ".into()),
        lorebooks: Some(dto::LorebooksChange::Set {
            ids: vec![book.to_string()],
        }),
        ..dto::ConversationSettingsPatch::default()
    };
    let noted = update(&harness, &chat, None, patch.clone())
        .await
        .expect("note");
    assert_eq!(noted.author_note.as_deref(), Some("keep it short"));
    assert_eq!(noted.lorebooks.ids, vec![book.to_string()]);
    assert_eq!(noted.settings_revision, Some(1));
    let repeated = update(&harness, &chat, None, patch)
        .await
        .expect("same change again");
    assert_eq!(repeated.settings_revision, Some(1));
    let stale = update(
        &harness,
        &chat,
        None,
        dto::ConversationSettingsPatch {
            author_note: Some("other".into()),
            ..dto::ConversationSettingsPatch::default()
        },
    )
    .await
    .expect_err("the chat has settings now");
    assert_eq!(stale.code, ApiErrorCode::Conflict);
    let cleared = update(
        &harness,
        &chat,
        Some(1),
        dto::ConversationSettingsPatch {
            author_note: Some("   ".into()),
            lorebooks: Some(dto::LorebooksChange::Set { ids: Vec::new() }),
            ..dto::ConversationSettingsPatch::default()
        },
    )
    .await
    .expect("clear");
    assert_eq!(cleared.author_note, None);
    assert!(cleared.lorebooks.ids.is_empty());
    assert_eq!(cleared.lorebooks.source, dto::SettingSource::Conversation);
    let behind = update(
        &harness,
        &chat,
        Some(1),
        dto::ConversationSettingsPatch {
            author_note: Some("late".into()),
            ..dto::ConversationSettingsPatch::default()
        },
    )
    .await
    .expect_err("a stale settings revision");
    assert_eq!(behind.code, ApiErrorCode::Conflict);
    for (patch, name) in [
        (
            dto::ConversationSettingsPatch {
                persona: Some(dto::ChoiceChange::Set {
                    id: lettuce_types::PersonaId::new().to_string(),
                }),
                ..dto::ConversationSettingsPatch::default()
            },
            "persona_id",
        ),
        (
            dto::ConversationSettingsPatch {
                prompt: Some(dto::ChoiceChange::Set {
                    id: seed_prompt(database, "Feature", PromptPurpose::DynamicMemorySummarizer)
                        .to_string(),
                }),
                ..dto::ConversationSettingsPatch::default()
            },
            "prompt_id",
        ),
        (
            dto::ConversationSettingsPatch {
                chat_mode: Some(dto::ChatModeChange::Set {
                    mode: dto::GroupChatMode::Roleplay,
                }),
                ..dto::ConversationSettingsPatch::default()
            },
            "chat_mode",
        ),
    ] {
        let error = update(&harness, &chat, cleared.settings_revision, patch)
            .await
            .expect_err("refused");
        assert_eq!(error.code, ApiErrorCode::InvalidInput);
        assert_eq!(field(&error), Some(name));
    }

    let blank = conversation_rename(
        &harness.context,
        dto::ConversationRenameRequest {
            conversation_id: chat.clone(),
            expected_revision: cleared.revision,
            title: "   ".into(),
        },
    )
    .await
    .expect_err("blank title");
    assert_eq!(field(&blank), Some("title"));
    let renamed = conversation_rename(
        &harness.context,
        dto::ConversationRenameRequest {
            conversation_id: chat.clone(),
            expected_revision: cleared.revision,
            title: "  Tea time ".into(),
        },
    )
    .await
    .expect("rename");
    assert_eq!(view(&harness, &chat).await.title, "Tea time");
    assert!(renamed.revision > cleared.revision);
    let archived = conversation_archive(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: chat.clone(),
        },
    )
    .await
    .expect("archive");
    let again = conversation_archive(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: chat.clone(),
        },
    )
    .await
    .expect("archive again");
    assert_eq!(again, archived);
    let restored = conversation_restore(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: chat.clone(),
        },
    )
    .await
    .expect("restore");
    assert!(restored.revision > archived.revision);
}

/// A one-to-one chat follows its character's memory mode every turn unless
/// it chose one itself.
#[tokio::test(flavor = "multi_thread")]
async fn a_direct_chat_follows_its_characters_memory_mode() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let chat = super::tests::launch(&harness, "memory-launch").await;
    assert_eq!(
        view(&harness, &chat).await.memory.mode,
        dto::MemoryMode::Manual
    );
    let character = lettuce_characters::CharacterRepository::get(database, harness.character_id)
        .expect("character")
        .expect("exists")
        .character;
    let mut defaults = character.defaults.clone();
    defaults.memory_policy = MemoryPolicy::Dynamic;
    lettuce_characters::CharacterRepository::update_defaults(
        database,
        harness.character_id,
        character.revision,
        defaults,
        NOW,
    )
    .expect("dynamic character");
    let live = view(&harness, &chat).await;
    assert_eq!(
        (live.memory.mode, live.memory.source),
        (dto::MemoryMode::Dynamic, dto::SettingSource::Character)
    );
    let stored = lettuce_settings::GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    let conversation = ConversationReader::get(database, chat.parse().expect("id"))
        .expect("conversation")
        .conversation;
    assert!(
        crate::companion::companion_memory_host::dynamic_memory_on(
            database,
            &conversation,
            &settings
        )
        .expect("memory gate")
    );
    let own = update(
        &harness,
        &chat,
        live.settings_revision,
        dto::ConversationSettingsPatch {
            memory: Some(dto::MemoryModeChange::Set {
                mode: dto::MemoryMode::Manual,
            }),
            ..dto::ConversationSettingsPatch::default()
        },
    )
    .await
    .expect("own memory mode");
    assert_eq!(
        (own.memory.mode, own.memory.source),
        (dto::MemoryMode::Manual, dto::SettingSource::Conversation)
    );
    let reset = update(
        &harness,
        &chat,
        own.settings_revision,
        dto::ConversationSettingsPatch {
            memory: Some(dto::MemoryModeChange::Reset),
            ..dto::ConversationSettingsPatch::default()
        },
    )
    .await
    .expect("reset");
    assert_eq!(reset.memory.mode, dto::MemoryMode::Dynamic);
}
