use lettuce_contracts::{self as dto, ApiErrorCode};

use super::tests::{Reply, harness};
use super::*;

async fn draft(context: &ApiContext, character_id: String) -> dto::GroupDraft {
    let character = character_get(
        context,
        dto::CharacterGetRequest {
            character_id: character_id.clone(),
        },
    )
    .await
    .expect("character");
    let copy = character_duplicate(
        context,
        dto::CharacterDuplicateRequest {
            character_id: character_id.clone(),
            client_operation_id: "second-member".into(),
        },
    )
    .await
    .expect("copy");
    dto::GroupDraft {
        name: "Library group".into(),
        chat_mode: dto::GroupChatMode::Conversation,
        persona: dto::LibraryPersonaSelection::Inherit,
        speaker_selection: dto::SpeakerSelectionMethod::Heuristic,
        memory_policy: dto::LibraryMemoryPolicy::Manual,
        disable_character_lorebooks: false,
        group_conversation_prompt_id: None,
        group_roleplay_prompt_id: None,
        presentation: character.draft.presentation.chat_appearance,
        members: vec![
            dto::LibraryGroupMember {
                character_id,
                ordinal: 0,
                muted: false,
                model_profile_override: None,
            },
            dto::LibraryGroupMember {
                character_id: copy.id,
                ordinal: 1,
                muted: true,
                model_profile_override: None,
            },
        ],
        background_asset_id: None,
        starting_scene: None,
    }
}

#[tokio::test]
async fn group_editor_get_save_replays_and_rejects_changed_or_stale_writes() {
    let harness = harness(Reply::Text("reply"));
    let draft = draft(&harness.context, harness.character_id.to_string()).await;
    let request = dto::GroupSaveRequest {
        group_id: None,
        draft,
        expected_revision: None,
        client_operation_id: "new-group".into(),
    };
    let before = group_save(&harness.context, request.clone())
        .await
        .expect("create");
    assert_eq!(before.revision, 1);
    assert_eq!(before.draft.persona, dto::LibraryPersonaSelection::Disabled);
    assert_eq!(
        group_save(&harness.context, request.clone())
            .await
            .expect("replay"),
        before
    );
    assert_eq!(
        group_get(
            &harness.context,
            dto::GroupGetRequest {
                group_id: before.id.clone()
            }
        )
        .await
        .expect("get"),
        before
    );
    let mut changed = request;
    changed.draft.name = "Changed".into();
    assert_eq!(
        group_save(&harness.context, changed)
            .await
            .expect_err("changed")
            .code,
        ApiErrorCode::Conflict
    );
    let mut draft = before.draft.clone();
    draft.name = "Edited".into();
    draft.members[1].muted = false;
    let request = dto::GroupSaveRequest {
        group_id: Some(before.id.clone()),
        draft: draft.clone(),
        expected_revision: Some(before.revision),
        client_operation_id: "edit-group".into(),
    };
    let after = group_save(&harness.context, request.clone())
        .await
        .expect("edit");
    assert_eq!(after.draft, draft);
    assert_eq!(after.created_at, before.created_at);
    assert_eq!(after.revision, 2);
    assert_eq!(
        group_save(&harness.context, request.clone())
            .await
            .expect("replay"),
        after
    );
    let mut stale = request;
    stale.client_operation_id = "stale-group".into();
    assert_eq!(
        group_save(&harness.context, stale)
            .await
            .expect_err("stale")
            .code,
        ApiErrorCode::Conflict
    );
}

#[tokio::test]
async fn group_editor_shutdown_does_not_write_a_graph_or_receipt() {
    let harness = harness(Reply::Text("reply"));
    let draft = draft(&harness.context, harness.character_id.to_string()).await;
    harness.context.begin_shutdown();
    assert_eq!(
        group_save(
            &harness.context,
            dto::GroupSaveRequest {
                group_id: None,
                draft,
                expected_revision: None,
                client_operation_id: "shutdown-group".into()
            }
        )
        .await
        .expect_err("shutdown")
        .code,
        ApiErrorCode::Cancelled
    );
    assert!(
        harness
            .context
            .backend()
            .database()
            .lookup_api_operation("group_save", "shutdown-group")
            .expect("receipt")
            .is_none()
    );
}

#[tokio::test]
async fn group_editor_captures_the_default_and_round_trips_its_starting_scene() {
    use lettuce_characters::PersonaRepository;
    let harness = harness(Reply::Text("reply"));
    let persona = persona_save(
        &harness.context,
        dto::PersonaSaveRequest {
            persona_id: None,
            expected_revision: None,
            client_operation_id: "narrator".into(),
            draft: dto::PersonaDraft {
                title: "Narrator".into(),
                description: "Narrator description".into(),
                nickname: None,
                design_description: None,
                avatar_crop: None,
                image_recommendation: None,
                media: vec![],
            },
        },
    )
    .await
    .expect("persona");
    let state = PersonaRepository::get_default_snapshot(harness.context.backend().database())
        .expect("default")
        .state;
    persona_default_set(
        &harness.context,
        dto::PersonaDefaultSetRequest {
            persona_id: persona.id.clone(),
            expected_revision: state.revision.get(),
            client_operation_id: "default-narrator".into(),
        },
    )
    .await
    .expect("default");
    let mut draft = draft(&harness.context, harness.character_id.to_string()).await;
    let scene_id = lettuce_types::SceneId::new().to_string();
    let variant_id = lettuce_types::SceneVariantId::new().to_string();
    let document = dto::LibrarySceneDocumentV1 {
        format_version: 1,
        parts: vec![dto::LibraryScenePart::Text {
            text: "Opening scene".into(),
        }],
    };
    draft.starting_scene = Some(dto::LibraryGroupStartingSceneDraft {
        scene: dto::LibrarySceneDraft {
            id: scene_id.clone(),
            ordinal: 0,
            content: document.clone(),
            direction: Some("Opening direction".into()),
            selected_variant_id: Some(variant_id.clone()),
            assets: vec![],
        },
        variants: vec![dto::LibrarySceneVariantDraft {
            id: variant_id,
            scene_id,
            ordinal: 0,
            content: document,
            direction: Some("Variant direction".into()),
        }],
    });
    let created = group_save(
        &harness.context,
        dto::GroupSaveRequest {
            group_id: None,
            expected_revision: None,
            client_operation_id: "scene-group".into(),
            draft: draft.clone(),
        },
    )
    .await
    .expect("group");
    draft.persona = dto::LibraryPersonaSelection::Explicit(persona.id);
    assert_eq!(created.draft, draft);
    let mut partial = serde_json::to_value(&draft).expect("serialize");
    partial
        .as_object_mut()
        .expect("object")
        .remove("starting_scene");
    assert!(serde_json::from_value::<dto::GroupDraft>(partial).is_err());
    draft.starting_scene.as_mut().expect("scene").variants[0].direction =
        Some("Edited variant".into());
    let after = group_save(
        &harness.context,
        dto::GroupSaveRequest {
            group_id: Some(created.id.clone()),
            expected_revision: Some(created.revision),
            client_operation_id: "scene-group-edit".into(),
            draft: draft.clone(),
        },
    )
    .await
    .expect("edit");
    assert_eq!(after.draft, draft);
    assert_eq!(
        group_get(
            &harness.context,
            dto::GroupGetRequest {
                group_id: created.id
            }
        )
        .await
        .expect("get"),
        after
    );
}

#[derive(Debug, Default)]
struct ShutdownClock {
    remaining: std::sync::Mutex<Option<(usize, lettuce_jobs::handle::CancellationToken)>>,
}

impl ShutdownClock {
    fn arm(&self, calls: usize, token: lettuce_jobs::handle::CancellationToken) {
        *self.remaining.lock().expect("clock lock") = Some((calls, token));
    }
}

impl lettuce_jobs::Clock for ShutdownClock {
    fn now(&self) -> lettuce_types::TimestampMillis {
        let mut remaining = self.remaining.lock().expect("clock lock");
        if let Some((calls, token)) = remaining.as_mut() {
            *calls -= 1;
            if *calls == 0 {
                token.cancel();
                *remaining = None;
            }
        }
        lettuce_types::TimestampMillis::new(1_800_000_000_000)
    }
}

#[tokio::test]
async fn admitted_library_saves_finish_atomically_during_shutdown() {
    for kind in 0..3 {
        let clock = std::sync::Arc::new(ShutdownClock::default());
        let harness = super::tests::harness_in(
            Reply::Text("reply"),
            clock.clone(),
            None,
            None,
            std::sync::Arc::new(NoModels),
        );
        if kind == 0 {
            let draft = draft(&harness.context, harness.character_id.to_string()).await;
            clock.arm(3, harness.context.shutdown_token().clone());
            let saved = group_save(
                &harness.context,
                dto::GroupSaveRequest {
                    group_id: None,
                    draft,
                    expected_revision: None,
                    client_operation_id: "admitted-group".into(),
                },
            )
            .await
            .expect("admitted group commit");
            assert!(harness.context.shutdown_token().is_cancelled());
            assert_eq!(
                group_get(
                    &harness.context,
                    dto::GroupGetRequest {
                        group_id: saved.id.clone()
                    }
                )
                .await
                .expect("get"),
                saved
            );
            assert!(
                harness
                    .context
                    .backend()
                    .database()
                    .lookup_api_operation("group_save", "admitted-group")
                    .expect("receipt")
                    .is_some()
            );
        } else if kind == 1 {
            let before = character_get(
                &harness.context,
                dto::CharacterGetRequest {
                    character_id: harness.character_id.to_string(),
                },
            )
            .await
            .expect("get");
            let mut draft = before.draft;
            draft.profile.name = "Admitted character".into();
            clock.arm(3, harness.context.shutdown_token().clone());
            let saved = character_save(
                &harness.context,
                dto::CharacterSaveRequest {
                    character_id: Some(before.id),
                    draft,
                    expected_revision: Some(before.revision),
                    client_operation_id: "admitted-character".into(),
                },
            )
            .await
            .expect("admitted character commit");
            assert!(harness.context.shutdown_token().is_cancelled());
            assert_eq!(
                character_get(
                    &harness.context,
                    dto::CharacterGetRequest {
                        character_id: saved.id.clone()
                    }
                )
                .await
                .expect("get"),
                saved
            );
            assert!(
                harness
                    .context
                    .backend()
                    .database()
                    .lookup_api_operation("character_save", "admitted-character")
                    .expect("receipt")
                    .is_some()
            );
        } else {
            clock.arm(2, harness.context.shutdown_token().clone());
            let saved = persona_save(
                &harness.context,
                dto::PersonaSaveRequest {
                    persona_id: None,
                    expected_revision: None,
                    client_operation_id: "admitted-persona".into(),
                    draft: dto::PersonaDraft {
                        title: "Admitted persona".into(),
                        description: "Description".into(),
                        nickname: None,
                        design_description: None,
                        avatar_crop: None,
                        image_recommendation: None,
                        media: vec![],
                    },
                },
            )
            .await
            .expect("admitted persona commit");
            assert!(harness.context.shutdown_token().is_cancelled());
            assert_eq!(
                persona_get(
                    &harness.context,
                    dto::PersonaGetRequest {
                        persona_id: saved.id.clone()
                    }
                )
                .await
                .expect("get"),
                saved
            );
            assert!(
                harness
                    .context
                    .backend()
                    .database()
                    .lookup_api_operation("persona_save", "admitted-persona")
                    .expect("receipt")
                    .is_some()
            );
        }
    }
}
