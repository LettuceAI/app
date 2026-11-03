use std::sync::Arc;

use lettuce_characters::{CharacterDefaults, InteractionMode};
use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails};
use lettuce_conversations::{
    ConversationClockSettings, ConversationReader, ConversationTimeOverride,
};
use lettuce_jobs::FakeClock;
use lettuce_types::TimestampMillis;

use crate::api::tests::{Harness, Reply, create_character, harness_in};
use crate::api::{
    NoModels, conversation_launch_direct, conversation_launch_group, conversation_settings_get,
    conversation_settings_update,
};

fn harness() -> (Harness, Arc<FakeClock>) {
    let clock = Arc::new(FakeClock::new(TimestampMillis::new(1_700_000_000_000)));
    let harness = harness_in(
        Reply::Text("Hello."),
        clock.clone(),
        None,
        None,
        Arc::new(NoModels),
    );
    (harness, clock)
}

async fn launch(harness: &Harness, time_awareness: bool) -> String {
    let character = create_character(
        harness.context.backend().database(),
        "Mira",
        CharacterDefaults {
            interaction_mode: InteractionMode::Companion,
            companion_soul: Some(lettuce_companions::CompanionSoulConfig::default()),
            time_awareness,
            ..Default::default()
        },
    );
    conversation_launch_direct(
        &harness.context,
        dto::LaunchDirectRequest {
            character_id: character.to_string(),
            title: None,
            scene_id: None,
            starter_id: None,
            client_operation_id: format!("time-launch-{character}"),
        },
    )
    .await
    .expect("launch companion")
    .conversation_id
}

async fn view(harness: &Harness, chat: &str) -> dto::ConversationSettingsView {
    conversation_settings_get(
        &harness.context,
        dto::ConversationSettingsGetRequest {
            conversation_id: chat.into(),
        },
    )
    .await
    .expect("settings")
}

async fn update(
    harness: &Harness,
    chat: &str,
    expected: Option<u64>,
    change: dto::TimeChange,
) -> Result<dto::ConversationSettingsView, dto::ApiError> {
    conversation_settings_update(
        &harness.context,
        dto::ConversationSettingsUpdateRequest {
            conversation_id: chat.into(),
            expected_settings_revision: expected,
            patch: dto::ConversationSettingsPatch {
                time: Some(change),
                ..Default::default()
            },
        },
    )
    .await
}

fn set(mode: dto::TimeMode, anchor_at: Option<i64>) -> dto::TimeChange {
    dto::TimeChange::Set { mode, anchor_at }
}

fn stored_clock(harness: &Harness, chat: &str) -> ConversationClockSettings {
    ConversationReader::get(
        harness.context.backend().database(),
        chat.parse().expect("id"),
    )
    .expect("stored conversation")
    .conversation
    .current_settings
    .expect("settings")
    .companion_clock
    .expect("clock")
}

#[tokio::test(flavor = "multi_thread")]
async fn companion_time_defaults_to_live_and_round_trips_frozen_ticking_and_reset() {
    let (harness, clock) = harness();
    let chat = launch(&harness, true).await;
    let first = view(&harness, &chat).await;
    assert!(first.companion);
    assert_eq!(
        first.time,
        dto::TimeView {
            enabled: true,
            mode: dto::TimeMode::Live,
            anchor_at: None,
            set_at: None,
            effective_at: harness.context.now().get(),
        }
    );
    let anchor = -2_519_820_000_000;
    let frozen = update(
        &harness,
        &chat,
        first.settings_revision,
        set(dto::TimeMode::Frozen, Some(anchor)),
    )
    .await
    .expect("freeze");
    clock.set(TimestampMillis::new(1_700_000_060_000));
    let frozen_get = view(&harness, &chat).await;
    assert_eq!(frozen_get.time, frozen.time);
    assert_eq!(
        frozen_get.time,
        dto::TimeView {
            enabled: true,
            mode: dto::TimeMode::Frozen,
            anchor_at: Some(anchor),
            set_at: None,
            effective_at: anchor,
        }
    );
    let set_at = harness.context.now().get();
    let ticking = update(
        &harness,
        &chat,
        frozen.settings_revision,
        set(dto::TimeMode::Ticking, Some(anchor)),
    )
    .await
    .expect("tick");
    clock.set(TimestampMillis::new(set_at + 90_000));
    let tick_get = view(&harness, &chat).await;
    assert_eq!(
        tick_get.time,
        dto::TimeView {
            enabled: true,
            mode: dto::TimeMode::Ticking,
            anchor_at: Some(anchor),
            set_at: Some(set_at),
            effective_at: anchor + 90_000,
        }
    );
    let replay = update(
        &harness,
        &chat,
        frozen.settings_revision,
        set(dto::TimeMode::Ticking, Some(anchor)),
    )
    .await
    .expect("same implicit settings key replays at a later server time");
    assert_eq!(replay, tick_get);
    assert_eq!(replay.settings_revision, ticking.settings_revision);
    clock.set(TimestampMillis::new(set_at - 1));
    assert_eq!(view(&harness, &chat).await.time.effective_at, anchor);
    clock.set(TimestampMillis::new(set_at + 90_000));
    let conflict = update(
        &harness,
        &chat,
        frozen.settings_revision,
        set(dto::TimeMode::Frozen, Some(anchor + 1)),
    )
    .await
    .expect_err("a changed patch under the used revision key conflicts");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);
    let reset = update(
        &harness,
        &chat,
        ticking.settings_revision,
        dto::TimeChange::Reset {},
    )
    .await
    .expect("reset");
    assert_eq!(
        reset.time,
        dto::TimeView {
            effective_at: harness.context.now().get(),
            ..first.time
        }
    );
    assert_eq!(view(&harness, &chat).await, reset);
    assert!(stored_clock(&harness, &chat).time_awareness_enabled);
    assert_eq!(
        stored_clock(&harness, &chat).time_override,
        ConversationTimeOverride::Live
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stale_companion_time_revision_conflicts_without_a_previous_operation() {
    let (harness, _) = harness();
    let chat = launch(&harness, true).await;
    let first = view(&harness, &chat).await;
    crate::conversation::apply_settings_change(
        harness.context.backend().database(),
        chat.parse().expect("id"),
        first.settings_revision.map(lettuce_types::Revision::new),
        crate::conversation::edit_operation("external-note".into(), &[b"external-note"])
            .expect("operation"),
        &crate::conversation::ConversationSettingsChange {
            author_note: Some("External update".into()),
            ..Default::default()
        },
        harness.context.now(),
    )
    .expect("advance the revision without consuming the API settings key");
    let advanced = view(&harness, &chat).await;
    let error = update(
        &harness,
        &chat,
        first.settings_revision,
        dto::TimeChange::Reset {},
    )
    .await
    .expect_err("revision mismatch without an operation receipt");
    assert_eq!(error.code, ApiErrorCode::Conflict);
    assert_eq!(view(&harness, &chat).await, advanced);
}

#[tokio::test(flavor = "multi_thread")]
async fn disabled_time_awareness_keeps_the_override_and_reset_preserves_other_settings() {
    let (harness, clock) = harness();
    let chat = launch(&harness, false).await;
    let first = view(&harness, &chat).await;
    assert_eq!(first.settings_revision, None);
    assert_eq!(first.time.mode, dto::TimeMode::Live);
    assert!(!first.time.enabled);
    for mode in [dto::TimeMode::Frozen, dto::TimeMode::Ticking] {
        let before = view(&harness, &chat).await;
        update(
            &harness,
            &chat,
            before.settings_revision,
            set(mode, Some(100)),
        )
        .await
        .expect("store override while awareness is disabled");
        clock.set(TimestampMillis::new(harness.context.now().get() + 10_000));
        let after = view(&harness, &chat).await.time;
        assert_eq!(after.mode, mode);
        assert_eq!(after.anchor_at, Some(100));
        assert_eq!(after.effective_at, harness.context.now().get());
        assert!(!stored_clock(&harness, &chat).time_awareness_enabled);
    }
    let before = view(&harness, &chat).await;
    let noted = conversation_settings_update(
        &harness.context,
        dto::ConversationSettingsUpdateRequest {
            conversation_id: chat.clone(),
            expected_settings_revision: before.settings_revision,
            patch: dto::ConversationSettingsPatch {
                author_note: Some("Keep this note".into()),
                ..Default::default()
            },
        },
    )
    .await
    .expect("other patch keeps the clock");
    assert_eq!(noted.time, before.time);
    let reset = update(
        &harness,
        &chat,
        noted.settings_revision,
        dto::TimeChange::Reset {},
    )
    .await
    .expect("reset");
    assert_eq!(reset.author_note.as_deref(), Some("Keep this note"));
    assert_eq!(
        stored_clock(&harness, &chat),
        ConversationClockSettings::default()
    );
}

async fn patch(
    harness: &Harness,
    chat: &str,
    patch: dto::ConversationSettingsPatch,
) -> dto::ConversationSettingsView {
    let before = view(harness, chat).await;
    conversation_settings_update(
        &harness.context,
        dto::ConversationSettingsUpdateRequest {
            conversation_id: chat.into(),
            expected_settings_revision: before.settings_revision,
            patch,
        },
    )
    .await
    .expect("settings update")
}

#[tokio::test(flavor = "multi_thread")]
async fn every_direct_and_group_chat_has_a_time_block_that_awareness_turns_on() {
    let (harness, _) = harness();
    let direct = crate::api::tests::launch(&harness, "time-roleplay").await;
    let database = harness.context.backend().database();
    let group = crate::launch::tests::two_member_group(database);
    let group = conversation_launch_group(
        &harness.context,
        dto::LaunchGroupRequest {
            group_id: group.to_string(),
            client_operation_id: "time-group".into(),
        },
    )
    .await
    .expect("launch group")
    .conversation_id;
    for chat in [direct, group] {
        let before = view(&harness, &chat).await;
        assert!(!before.companion);
        assert_eq!(
            before.time,
            dto::TimeView {
                enabled: false,
                mode: dto::TimeMode::Live,
                anchor_at: None,
                set_at: None,
                effective_at: harness.context.now().get(),
            }
        );
        let on = patch(
            &harness,
            &chat,
            dto::ConversationSettingsPatch {
                time_awareness: Some(true),
                ..Default::default()
            },
        )
        .await;
        assert!(on.time.enabled);
        assert!(stored_clock(&harness, &chat).time_awareness_enabled);
        let frozen = update(
            &harness,
            &chat,
            on.settings_revision,
            set(dto::TimeMode::Frozen, Some(-2_519_820_000_000)),
        )
        .await
        .expect("freeze");
        assert!(frozen.time.enabled);
        assert_eq!(frozen.time.effective_at, -2_519_820_000_000);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn turning_awareness_off_keeps_the_override_and_turning_it_on_restores_it() {
    let (harness, _) = harness();
    let chat = launch(&harness, true).await;
    let anchor = -2_519_820_000_000;
    let first = view(&harness, &chat).await;
    let frozen = update(
        &harness,
        &chat,
        first.settings_revision,
        set(dto::TimeMode::Frozen, Some(anchor)),
    )
    .await
    .expect("freeze");
    assert_eq!(frozen.time.effective_at, anchor);
    let off = patch(
        &harness,
        &chat,
        dto::ConversationSettingsPatch {
            time_awareness: Some(false),
            ..Default::default()
        },
    )
    .await;
    assert!(!off.time.enabled);
    assert_eq!(off.time.mode, dto::TimeMode::Frozen);
    assert_eq!(off.time.anchor_at, Some(anchor));
    assert_eq!(off.time.effective_at, harness.context.now().get());
    assert_eq!(
        stored_clock(&harness, &chat).time_override,
        ConversationTimeOverride::Frozen {
            anchor_at: TimestampMillis::new(anchor)
        }
    );
    let on = patch(
        &harness,
        &chat,
        dto::ConversationSettingsPatch {
            time_awareness: Some(true),
            ..Default::default()
        },
    )
    .await;
    assert!(on.time.enabled);
    assert_eq!(on.time.mode, dto::TimeMode::Frozen);
    assert_eq!(on.time.effective_at, anchor);
}

#[tokio::test(flavor = "multi_thread")]
async fn companion_time_requires_an_anchor_only_for_frozen_and_ticking() {
    let (harness, clock) = harness();
    let chat = launch(&harness, true).await;
    let before = view(&harness, &chat).await;
    for change in [
        set(dto::TimeMode::Frozen, None),
        set(dto::TimeMode::Ticking, None),
        set(dto::TimeMode::Live, Some(100)),
    ] {
        let error = update(&harness, &chat, before.settings_revision, change)
            .await
            .expect_err("invalid anchor");
        assert_eq!(error.code, ApiErrorCode::InvalidInput);
        assert_eq!(
            error.details,
            Some(ApiErrorDetails::InvalidField {
                field: "time.anchor_at".into()
            })
        );
    }
    assert_eq!(view(&harness, &chat).await, before);
    let live = update(
        &harness,
        &chat,
        before.settings_revision,
        set(dto::TimeMode::Live, None),
    )
    .await
    .expect("explicit live");
    assert_eq!(live.time, before.time);
    clock.set(TimestampMillis::new(-1));
    let error = update(
        &harness,
        &chat,
        live.settings_revision,
        set(dto::TimeMode::Ticking, Some(100)),
    )
    .await
    .expect_err("clock validation rejects a negative real set_at");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    assert_eq!(
        error.details,
        Some(ApiErrorDetails::InvalidField {
            field: "companion_clock.time_override".into()
        })
    );
}
