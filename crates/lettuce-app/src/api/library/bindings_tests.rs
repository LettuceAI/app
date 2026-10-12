use lettuce_contracts::{self as dto, ApiErrorCode};

use crate::api::tests::{Reply, harness};
use crate::api::*;

#[tokio::test]
async fn persona_binding_commands_replay_compare_revisions_and_preserve_order() {
    let harness = harness(Reply::Text("reply"));
    let persona =
        crate::launch::tests::seed_persona(harness.context.backend().database(), "Persona")
            .to_string();
    let first = crate::api::lorebooks::tests::create(&harness, "first", "First", Vec::new())
        .await
        .lorebook
        .id;
    let second = crate::api::lorebooks::tests::create(&harness, "second", "Second", Vec::new())
        .await
        .lorebook
        .id;
    let request = dto::PersonaLorebookBindRequest {
        persona_id: persona.clone(),
        lorebook_id: first.clone(),
        expected_revision: 1,
        target_ordinal: None,
        client_operation_id: "bind".into(),
    };
    let bound = persona_lorebooks_bind(&harness.context, request.clone())
        .await
        .expect("bind");
    assert_eq!(
        persona_lorebooks_bind(&harness.context, request.clone())
            .await
            .expect("replay"),
        bound
    );
    let mut changed = request.clone();
    changed.lorebook_id = second.clone();
    assert_eq!(
        persona_lorebooks_bind(&harness.context, changed)
            .await
            .expect_err("changed digest")
            .code,
        ApiErrorCode::Conflict
    );
    assert_eq!(
        persona_lorebooks_enable(
            &harness.context,
            dto::PersonaLorebookEnableRequest {
                persona_id: persona.clone(),
                lorebook_id: first.clone(),
                expected_revision: 1,
                enabled: false,
                client_operation_id: "stale".into()
            }
        )
        .await
        .expect_err("stale")
        .code,
        ApiErrorCode::Conflict
    );
    let second_result = persona_lorebooks_bind(
        &harness.context,
        dto::PersonaLorebookBindRequest {
            persona_id: persona.clone(),
            lorebook_id: second.clone(),
            expected_revision: bound.owner_revision,
            target_ordinal: None,
            client_operation_id: "bind-second".into(),
        },
    )
    .await
    .expect("second");
    let reordered = persona_lorebooks_reorder(
        &harness.context,
        dto::PersonaLorebookReorderRequest {
            persona_id: persona.clone(),
            lorebook_id: second.clone(),
            expected_revision: second_result.owner_revision,
            target_ordinal: 0,
            client_operation_id: "reorder".into(),
        },
    )
    .await
    .expect("reorder");
    assert_eq!(reordered.bindings[0].lorebook_id, second);
    let disabled = persona_lorebooks_enable(
        &harness.context,
        dto::PersonaLorebookEnableRequest {
            persona_id: persona.clone(),
            lorebook_id: second.clone(),
            expected_revision: reordered.owner_revision,
            enabled: false,
            client_operation_id: "disable".into(),
        },
    )
    .await
    .expect("disable");
    persona_lorebooks_unbind(
        &harness.context,
        dto::PersonaLorebookUnbindRequest {
            persona_id: persona.clone(),
            lorebook_id: first,
            expected_revision: disabled.owner_revision,
            client_operation_id: "unbind".into(),
        },
    )
    .await
    .expect("unbind");
    let bindings = persona_lorebooks_list(
        &harness.context,
        dto::PersonaLorebooksListRequest {
            persona_id: persona,
        },
    )
    .await
    .expect("list");
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].lorebook_id, second);
    assert!(!bindings[0].enabled);
}

#[tokio::test]
async fn character_binding_commands_replay_compare_revisions_and_preserve_order() {
    let harness = harness(Reply::Text("reply"));
    let character = harness.character_id.to_string();
    let first = crate::api::lorebooks::tests::create(&harness, "first", "First", Vec::new())
        .await
        .lorebook
        .id;
    let second = crate::api::lorebooks::tests::create(&harness, "second", "Second", Vec::new())
        .await
        .lorebook
        .id;
    let request = dto::CharacterLorebookBindRequest {
        character_id: character.clone(),
        lorebook_id: first.clone(),
        expected_revision: 1,
        target_ordinal: None,
        client_operation_id: "bind".into(),
    };
    let bound = character_lorebooks_bind(&harness.context, request.clone())
        .await
        .expect("bind");
    assert_eq!(
        character_lorebooks_bind(&harness.context, request.clone())
            .await
            .expect("replay"),
        bound
    );
    let mut changed = request.clone();
    changed.lorebook_id = second.clone();
    assert_eq!(
        character_lorebooks_bind(&harness.context, changed)
            .await
            .expect_err("changed digest")
            .code,
        ApiErrorCode::Conflict
    );
    assert_eq!(
        character_lorebooks_enable(
            &harness.context,
            dto::CharacterLorebookEnableRequest {
                character_id: character.clone(),
                lorebook_id: first.clone(),
                expected_revision: 1,
                enabled: false,
                client_operation_id: "stale".into()
            }
        )
        .await
        .expect_err("stale")
        .code,
        ApiErrorCode::Conflict
    );
    let second_result = character_lorebooks_bind(
        &harness.context,
        dto::CharacterLorebookBindRequest {
            character_id: character.clone(),
            lorebook_id: second.clone(),
            expected_revision: bound.owner_revision,
            target_ordinal: None,
            client_operation_id: "bind-second".into(),
        },
    )
    .await
    .expect("second");
    let reordered = character_lorebooks_reorder(
        &harness.context,
        dto::CharacterLorebookReorderRequest {
            character_id: character.clone(),
            lorebook_id: second.clone(),
            expected_revision: second_result.owner_revision,
            target_ordinal: 0,
            client_operation_id: "reorder".into(),
        },
    )
    .await
    .expect("reorder");
    assert_eq!(reordered.bindings[0].lorebook_id, second);
    let disabled = character_lorebooks_enable(
        &harness.context,
        dto::CharacterLorebookEnableRequest {
            character_id: character.clone(),
            lorebook_id: second.clone(),
            expected_revision: reordered.owner_revision,
            enabled: false,
            client_operation_id: "disable".into(),
        },
    )
    .await
    .expect("disable");
    character_lorebooks_unbind(
        &harness.context,
        dto::CharacterLorebookUnbindRequest {
            character_id: character.clone(),
            lorebook_id: first,
            expected_revision: disabled.owner_revision,
            client_operation_id: "unbind".into(),
        },
    )
    .await
    .expect("unbind");
    let bindings = character_lorebooks_list(
        &harness.context,
        dto::CharacterLorebooksListRequest {
            character_id: character,
        },
    )
    .await
    .expect("list");
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].lorebook_id, second);
    assert!(!bindings[0].enabled);
}

#[tokio::test]
async fn group_binding_commands_replay_compare_revisions_and_preserve_order() {
    let harness = harness(Reply::Text("reply"));
    let group =
        crate::launch::tests::two_member_group(harness.context.backend().database()).to_string();
    let first = crate::api::lorebooks::tests::create(&harness, "first", "First", Vec::new())
        .await
        .lorebook
        .id;
    let second = crate::api::lorebooks::tests::create(&harness, "second", "Second", Vec::new())
        .await
        .lorebook
        .id;
    let request = dto::GroupLorebookBindRequest {
        group_id: group.clone(),
        lorebook_id: first.clone(),
        expected_revision: 1,
        target_ordinal: None,
        client_operation_id: "bind".into(),
    };
    let bound = group_lorebooks_bind(&harness.context, request.clone())
        .await
        .expect("bind");
    assert_eq!(
        group_lorebooks_bind(&harness.context, request.clone())
            .await
            .expect("replay"),
        bound
    );
    let mut changed = request.clone();
    changed.lorebook_id = second.clone();
    assert_eq!(
        group_lorebooks_bind(&harness.context, changed)
            .await
            .expect_err("changed digest")
            .code,
        ApiErrorCode::Conflict
    );
    assert_eq!(
        group_lorebooks_enable(
            &harness.context,
            dto::GroupLorebookEnableRequest {
                group_id: group.clone(),
                lorebook_id: first.clone(),
                expected_revision: 1,
                enabled: false,
                client_operation_id: "stale".into()
            }
        )
        .await
        .expect_err("stale")
        .code,
        ApiErrorCode::Conflict
    );
    let second_result = group_lorebooks_bind(
        &harness.context,
        dto::GroupLorebookBindRequest {
            group_id: group.clone(),
            lorebook_id: second.clone(),
            expected_revision: bound.owner_revision,
            target_ordinal: None,
            client_operation_id: "bind-second".into(),
        },
    )
    .await
    .expect("second");
    let reordered = group_lorebooks_reorder(
        &harness.context,
        dto::GroupLorebookReorderRequest {
            group_id: group.clone(),
            lorebook_id: second.clone(),
            expected_revision: second_result.owner_revision,
            target_ordinal: 0,
            client_operation_id: "reorder".into(),
        },
    )
    .await
    .expect("reorder");
    assert_eq!(reordered.bindings[0].lorebook_id, second);
    let disabled = group_lorebooks_enable(
        &harness.context,
        dto::GroupLorebookEnableRequest {
            group_id: group.clone(),
            lorebook_id: second.clone(),
            expected_revision: reordered.owner_revision,
            enabled: false,
            client_operation_id: "disable".into(),
        },
    )
    .await
    .expect("disable");
    group_lorebooks_unbind(
        &harness.context,
        dto::GroupLorebookUnbindRequest {
            group_id: group.clone(),
            lorebook_id: first,
            expected_revision: disabled.owner_revision,
            client_operation_id: "unbind".into(),
        },
    )
    .await
    .expect("unbind");
    let bindings = group_lorebooks_list(
        &harness.context,
        dto::GroupLorebooksListRequest { group_id: group },
    )
    .await
    .expect("list");
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].lorebook_id, second);
    assert!(!bindings[0].enabled);
}
