use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails};
use lettuce_conversations::{ConversationReader, ParticipantRole};
use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaAssetRepository,
    MediaBlob, MediaBlobRepository, MediaKind, RetentionClass,
};
use lettuce_models::{CapabilityStatus, ModelProfileRepository};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{AssetId, ContentHash, MediaBlobId, Revision, TimestampMillis};

use crate::api::tests::{Harness, Reply, harness, launch};
use crate::api::{
    GenerationEventSink, conversation_launch_group, conversation_open,
    conversation_participant_update, conversation_send, conversation_settings_update,
};
use crate::launch::tests::{member, seed_group, seed_model, seed_named_character_with};

struct NoStream;

impl GenerationEventSink for NoStream {
    fn emit(&self, _event: dto::GenerationEvent) {}
}

fn asset(
    database: &lettuce_database::Database,
    seed: &str,
    kind: AssetKind,
    blob_kind: MediaKind,
) -> AssetId {
    let blob = MediaBlobRepository::register(
        database,
        MediaBlob {
            id: MediaBlobId::new(),
            content_hash: ContentHash::parse(seed.repeat(32)).expect("hash"),
            kind: blob_kind,
            mime_type: "application/octet-stream".into(),
            byte_size: 4,
            width: None,
            height: None,
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
            kind,
            AssetOrigin::Upload,
            RetentionClass::Persistent,
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

fn image(database: &lettuce_database::Database, seed: &str) -> AssetId {
    asset(database, seed, AssetKind::MessageImage, MediaKind::Image)
}

fn audio(database: &lettuce_database::Database, seed: &str) -> AssetId {
    asset(database, seed, AssetKind::MessageAudio, MediaKind::Audio)
}

fn accept(harness: &Harness, image: CapabilityStatus, audio: CapabilityStatus) {
    let database = harness.context.backend().database();
    let id = GlobalSettingsStore::load(database)
        .expect("settings")
        .default_model_profile_id
        .expect("default model");
    accept_model(database, id, image, audio);
}

fn accept_model(
    database: &lettuce_database::Database,
    id: lettuce_types::ModelProfileId,
    image: CapabilityStatus,
    audio: CapabilityStatus,
) {
    let mut model = ModelProfileRepository::get(database, id)
        .expect("model")
        .expect("model exists");
    let revision = model.revision;
    model.config.capabilities.input_modalities.image = image;
    model.config.capabilities.input_modalities.audio = audio;
    ModelProfileRepository::upsert(database, model, Some(revision)).expect("capabilities");
}

async fn send(
    harness: &Harness,
    conversation_id: &str,
    key: &str,
    text: &str,
    media: &[AssetId],
) -> Result<dto::SendAccepted, dto::ApiError> {
    send_swapped(harness, conversation_id, key, text, media, false).await
}

async fn send_swapped(
    harness: &Harness,
    conversation_id: &str,
    key: &str,
    text: &str,
    media: &[AssetId],
    swap_places: bool,
) -> Result<dto::SendAccepted, dto::ApiError> {
    conversation_send(
        &harness.context,
        dto::ConversationSendRequest {
            expected_revision: None,
            conversation_id: conversation_id.into(),
            text: text.into(),
            media: media.iter().map(ToString::to_string).collect(),
            swap_places,
            client_operation_id: key.into(),
        },
        Arc::new(NoStream),
    )
    .await
}

async fn messages(harness: &Harness, conversation_id: &str) -> Vec<dto::TimelineMessage> {
    conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation_id.into(),
        },
    )
    .await
    .expect("open")
    .messages
    .items
}

fn field(error: &dto::ApiError) -> Option<&str> {
    match &error.details {
        Some(ApiErrorDetails::InvalidField { field }) => Some(field),
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn blank_text_needs_an_attachment() {
    let harness = harness(Reply::Text("Hello."));
    accept(
        &harness,
        CapabilityStatus::Supported,
        CapabilityStatus::Unknown,
    );
    let conversation_id = launch(&harness, "blank-launch").await;
    let error = send(&harness, &conversation_id, "blank-1", "  ", &[])
        .await
        .expect_err("blank text without media");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    assert_eq!(field(&error), Some("text"));
    assert!(messages(&harness, &conversation_id).await.is_empty());
    let picture = image(harness.context.backend().database(), "a1");
    send(&harness, &conversation_id, "blank-2", "  ", &[picture])
        .await
        .expect("blank text with media");
    let sent = messages(&harness, &conversation_id).await;
    let user = sent.first().expect("user message");
    assert_eq!(user.parts.len(), 1);
    assert!(matches!(
        &user.parts[0],
        dto::MessagePartView::Media { asset, .. } if asset.asset_id == picture.to_string()
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn bad_attachment_ids_are_refused_and_nothing_is_written() {
    let harness = harness(Reply::Text("Hello."));
    accept(
        &harness,
        CapabilityStatus::Supported,
        CapabilityStatus::Supported,
    );
    let database = harness.context.backend().database();
    let conversation_id = launch(&harness, "bad-launch").await;
    let picture = image(database, "a1");
    let other = asset(database, "b2", AssetKind::OtherImage, MediaKind::Image);
    for (key, media) in [
        ("bad-1", vec![AssetId::new()]),
        ("bad-2", vec![other]),
        ("bad-3", vec![picture, picture]),
    ] {
        let error = send(&harness, &conversation_id, key, "Hi", &media)
            .await
            .expect_err("refused");
        assert_eq!(error.code, ApiErrorCode::InvalidInput, "{key}");
        assert_eq!(field(&error), Some("media"), "{key}");
    }
    let malformed = conversation_send(
        &harness.context,
        dto::ConversationSendRequest {
            expected_revision: None,
            conversation_id: conversation_id.clone(),
            text: "Hi".into(),
            media: vec!["nope".into()],
            swap_places: false,
            client_operation_id: "bad-4".into(),
        },
        Arc::new(NoStream),
    )
    .await
    .expect_err("malformed id");
    assert_eq!(field(&malformed), Some("media"));
    assert!(messages(&harness, &conversation_id).await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_modality_the_model_lacks_is_unsupported_and_writes_nothing() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let conversation_id = launch(&harness, "caps-launch").await;
    let picture = image(database, "a1");
    let clip = audio(database, "b2");
    accept(
        &harness,
        CapabilityStatus::Unknown,
        CapabilityStatus::Unknown,
    );
    let refused = send(&harness, &conversation_id, "caps-1", "Hi", &[picture])
        .await
        .expect_err("text-only model");
    assert_eq!(refused.code, ApiErrorCode::Unsupported);
    assert_eq!(field(&refused), Some("media.image"));
    accept(
        &harness,
        CapabilityStatus::Supported,
        CapabilityStatus::Unsupported,
    );
    let refused = send(&harness, &conversation_id, "caps-2", "Hi", &[picture, clip])
        .await
        .expect_err("no audio input");
    assert_eq!(refused.code, ApiErrorCode::Unsupported);
    assert_eq!(field(&refused), Some("media.audio"));
    assert!(messages(&harness, &conversation_id).await.is_empty());
    send(&harness, &conversation_id, "caps-3", "Hi", &[picture])
        .await
        .expect("image input is supported");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_send_with_media_replays_and_a_different_media_list_conflicts() {
    let harness = harness(Reply::Text("Hello."));
    accept(
        &harness,
        CapabilityStatus::Supported,
        CapabilityStatus::Supported,
    );
    let database = harness.context.backend().database();
    let conversation_id = launch(&harness, "replay-launch").await;
    let first = image(database, "a1");
    let second = audio(database, "b2");
    let accepted = send(
        &harness,
        &conversation_id,
        "replay-1",
        "Look",
        &[first, second],
    )
    .await
    .expect("send");
    accept(
        &harness,
        CapabilityStatus::Unknown,
        CapabilityStatus::Unknown,
    );
    let replayed = send(
        &harness,
        &conversation_id,
        "replay-1",
        "Look",
        &[first, second],
    )
    .await
    .expect("the same request replays");
    assert_eq!(replayed.user_message_id, accepted.user_message_id);
    assert_eq!(replayed.turn_id, accepted.turn_id);
    for media in [vec![second, first], vec![first], Vec::new()] {
        let conflict = send(&harness, &conversation_id, "replay-1", "Look", &media)
            .await
            .expect_err("different media under the key");
        assert_eq!(conflict.code, ApiErrorCode::Conflict);
    }
    let sent = messages(&harness, &conversation_id).await;
    let user = sent
        .iter()
        .find(|message| message.id == accepted.user_message_id)
        .expect("user message");
    assert_eq!(user.parts.len(), 3);
    assert!(matches!(&user.parts[0], dto::MessagePartView::Text { text } if text == "Look"));
    assert!(matches!(
        &user.parts[1],
        dto::MessagePartView::Media { asset, .. } if asset.asset_id == first.to_string()
    ));
    assert!(matches!(
        &user.parts[2],
        dto::MessagePartView::Media { asset, .. } if asset.asset_id == second.to_string()
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_swapped_send_reaches_the_turn_and_conflicts_with_an_unswapped_retry() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch(&harness, "swap-launch").await;
    let accepted = send_swapped(&harness, &conversation_id, "swap-1", "Hi", &[], true)
        .await
        .expect("swapped send");
    let turn: lettuce_types::GenerationTurnId = accepted.turn_id.parse().expect("turn id");
    let stored =
        ConversationReader::get_turn(harness.context.backend().database(), turn).expect("turn");
    assert!(stored.swap_roles);
    let replayed = send_swapped(&harness, &conversation_id, "swap-1", "Hi", &[], true)
        .await
        .expect("replay");
    assert_eq!(replayed.turn_id, accepted.turn_id);
    let conflict = send_swapped(&harness, &conversation_id, "swap-1", "Hi", &[], false)
        .await
        .expect_err("different swap under the key");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);
}

fn default_model(harness: &Harness) -> lettuce_types::ModelProfileId {
    GlobalSettingsStore::load(harness.context.backend().database())
        .expect("settings")
        .default_model_profile_id
        .expect("default model")
}

async fn launch_group(
    harness: &Harness,
    group_id: lettuce_types::GroupId,
    key: &str,
) -> dto::LaunchGroupResponse {
    conversation_launch_group(
        &harness.context,
        dto::LaunchGroupRequest {
            group_id: group_id.to_string(),
            client_operation_id: key.into(),
        },
    )
    .await
    .expect("launch group")
}

fn member_of(
    database: &lettuce_database::Database,
    name: &str,
    model: Option<lettuce_types::ModelProfileId>,
) -> lettuce_types::CharacterId {
    seed_named_character_with(database, name, |defaults| {
        defaults.model_profile_id = model;
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn attachments_report_their_kind() {
    let harness = harness(Reply::Text("Hello."));
    accept(
        &harness,
        CapabilityStatus::Supported,
        CapabilityStatus::Supported,
    );
    let database = harness.context.backend().database();
    let conversation_id = launch(&harness, "refs-launch").await;
    let picture = image(database, "a1");
    let clip = audio(database, "b2");
    send(
        &harness,
        &conversation_id,
        "refs-1",
        "Listen",
        &[picture, clip],
    )
    .await
    .expect("send");
    let sent = messages(&harness, &conversation_id).await;
    let kinds: Vec<_> = sent[0]
        .parts
        .iter()
        .filter_map(|part| match part {
            dto::MessagePartView::Media { kind, .. } => Some(*kind),
            dto::MessagePartView::Text { .. } => None,
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            Some(dto::MediaPartKind::Image),
            Some(dto::MediaPartKind::Audio)
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_conversation_model_override_decides_the_capability() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    accept(
        &harness,
        CapabilityStatus::Supported,
        CapabilityStatus::Supported,
    );
    let other = seed_model(database, lettuce_models::ProviderProtocol::Ollama, "other");
    accept_model(
        database,
        other,
        CapabilityStatus::Unknown,
        CapabilityStatus::Unknown,
    );
    let conversation_id = launch(&harness, "override-launch").await;
    conversation_settings_update(
        &harness.context,
        dto::ConversationSettingsUpdateRequest {
            conversation_id: conversation_id.clone(),
            expected_settings_revision: None,
            patch: dto::ConversationSettingsPatch {
                model: Some(dto::IdChange::Set {
                    id: other.to_string(),
                }),
                ..dto::ConversationSettingsPatch::default()
            },
        },
    )
    .await
    .expect("override");
    let picture = image(database, "a1");
    let refused = send(&harness, &conversation_id, "override-1", "Hi", &[picture])
        .await
        .expect_err("the override lacks image input");
    assert_eq!(refused.code, ApiErrorCode::Unsupported);
    assert!(messages(&harness, &conversation_id).await.is_empty());
    accept_model(
        database,
        other,
        CapabilityStatus::Supported,
        CapabilityStatus::Unknown,
    );
    send(&harness, &conversation_id, "override-2", "Hi", &[picture])
        .await
        .expect("the override accepts images");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chat_whose_model_does_not_resolve_refuses_attachments() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let conversation_id = launch(&harness, "nomodel-launch").await;
    let settings = GlobalSettingsStore::load(database).expect("settings");
    let revision = settings.revision;
    GlobalSettingsStore::save(database, settings.settings, None, revision).expect("save");
    let picture = image(database, "a1");
    let refused = send(&harness, &conversation_id, "nomodel-1", "Hi", &[picture])
        .await
        .expect_err("no model resolves");
    assert_eq!(refused.code, ApiErrorCode::Unsupported);
    assert!(messages(&harness, &conversation_id).await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_needs_every_active_member_to_accept_the_modality() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let supported = default_model(&harness);
    accept(
        &harness,
        CapabilityStatus::Supported,
        CapabilityStatus::Unknown,
    );
    let plain = seed_model(database, lettuce_models::ProviderProtocol::Ollama, "plain");
    accept_model(
        database,
        plain,
        CapabilityStatus::Unknown,
        CapabilityStatus::Unknown,
    );
    let first = member_of(database, "Ada", Some(supported));
    let second = member_of(database, "Bea", Some(plain));
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    );
    let conversation_id = launch_group(&harness, group_id, "group-launch")
        .await
        .conversation_id;
    let picture = image(database, "a1");
    let refused = send(&harness, &conversation_id, "group-1", "Hi", &[picture])
        .await
        .expect_err("one member lacks image input");
    assert_eq!(refused.code, ApiErrorCode::Unsupported);
    assert_eq!(field(&refused), Some("media.image"));
    let view = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation_id.clone(),
        },
    )
    .await
    .expect("open");
    assert!(view.messages.items.is_empty());
    let bea = view
        .participants
        .iter()
        .find(|participant| participant.character_id.as_deref() == Some(&second.to_string()))
        .expect("bea");
    conversation_participant_update(
        &harness.context,
        dto::ConversationParticipantUpdateRequest {
            conversation_id: conversation_id.clone(),
            participant_id: bea.id.clone(),
            enabled: None,
            muted: Some(true),
            model: None,
            client_operation_id: "mute-bea".into(),
        },
    )
    .await
    .expect("mute");
    send(&harness, &conversation_id, "group-2", "Hi", &[picture])
        .await
        .expect("a muted member is ignored");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_send_with_swapped_places_is_refused() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let first = member_of(database, "Ada", None);
    let second = member_of(database, "Bea", None);
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    );
    let conversation_id = launch_group(&harness, group_id, "swap-launch")
        .await
        .conversation_id;
    let swapped = send_swapped(&harness, &conversation_id, "swap-group", "Hi", &[], true)
        .await
        .expect_err("a group cannot swap places");
    assert_eq!(swapped.code, ApiErrorCode::InvalidInput);
    assert_eq!(field(&swapped), Some("generation_turn.swap_roles"));
    assert!(messages(&harness, &conversation_id).await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn attachments_need_a_speaker_to_receive_them() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    accept(
        &harness,
        CapabilityStatus::Supported,
        CapabilityStatus::Supported,
    );
    let first = member_of(database, "Ada", None);
    let second = member_of(database, "Bea", None);
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    );
    let conversation_id = launch_group(&harness, group_id, "speakers-launch")
        .await
        .conversation_id;
    let mut conversation = ConversationReader::get(database, conversation_id.parse().expect("id"))
        .expect("conversation")
        .conversation;
    conversation
        .participants
        .retain(|participant| participant.role == ParticipantRole::User);
    let refused = super::send_media::require_input_modalities(
        database,
        &conversation,
        &[super::send_media::AttachmentKind::Image],
    )
    .expect_err("no speaker can receive the attachment");
    assert_eq!(refused.code, ApiErrorCode::Unsupported);
    assert_eq!(field(&refused), Some("media.speakers"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_member_without_a_live_model_uses_the_conversations_model() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    accept(
        &harness,
        CapabilityStatus::Supported,
        CapabilityStatus::Unknown,
    );
    let first = member_of(database, "Ada", None);
    let second = member_of(database, "Bea", None);
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    );
    let conversation_id = launch_group(&harness, group_id, "effective-launch")
        .await
        .conversation_id;
    let settings = GlobalSettingsStore::load(database).expect("settings");
    let revision = settings.revision;
    GlobalSettingsStore::save(database, settings.settings, None, revision).expect("save");
    let picture = image(database, "a1");
    send(&harness, &conversation_id, "effective-1", "Hi", &[picture])
        .await
        .expect("the conversation's own model accepts images");
}

#[tokio::test(flavor = "multi_thread")]
async fn system_messages_keep_media_without_requiring_a_generation_model() {
    let harness = harness(Reply::Text("Hello."));
    let chat = launch(&harness, "system-media-launch").await;
    let database = harness.context.backend().database();
    let image_id = image(database, "af");
    let audio_id = audio(database, "bf");
    let revision = ConversationReader::get(database, chat.parse().expect("chat")).expect("conversation").conversation.revision.get();
    let request = dto::ConversationAddSystemMessageRequest {
        conversation_id: chat.clone(),
        text: String::new(),
        media: vec![image_id.to_string(), audio_id.to_string()],
        expected_revision: revision,
        client_operation_id: "system-media".into(),
    };
    let added = crate::api::conversation_add_system_message(&harness.context, request.clone()).await.expect("media-only system message");
    assert_eq!(added.message.role, dto::MessageRole::System);
    assert_eq!(added.message.parts.len(), 2);
    for (part, id) in added.message.parts.iter().zip([image_id, audio_id]) {
        let dto::MessagePartView::Media { asset, .. } = part else { panic!("media part") };
        assert_eq!(asset.asset_id, id.to_string());
    }
    let replay = crate::api::conversation_add_system_message(&harness.context, request).await.expect("replay");
    assert_eq!(replay.message.id, added.message.id);
    let mut invalid = dto::ConversationAddSystemMessageRequest {
        conversation_id: chat,
        text: "Note".into(),
        media: vec![image_id.to_string(), image_id.to_string()],
        expected_revision: added.revision,
        client_operation_id: "system-invalid-media".into(),
    };
    assert_eq!(crate::api::conversation_add_system_message(&harness.context, invalid.clone()).await.expect_err("duplicate").code, ApiErrorCode::InvalidInput);
    invalid.media = vec![lettuce_types::AssetId::new().to_string()];
    assert_eq!(crate::api::conversation_add_system_message(&harness.context, invalid).await.expect_err("missing").code, ApiErrorCode::InvalidInput);
}
