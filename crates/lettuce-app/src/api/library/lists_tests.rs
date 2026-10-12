use lettuce_characters::{
    Character, CharacterDefaults, CharacterMedia, CharacterPresentationV1, CharacterProfile,
    CharacterProvenance, CharacterRepository, CreateCharacterPlan, Persona, PersonaRepository,
};
use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_types::{CharacterId, PersonaId, TimestampMillis};

use crate::api::tests::{Reply, harness};
use crate::api::*;

#[tokio::test]
async fn character_library_lists_page_by_creation_and_offer_name_updated_and_nickname_options() {
    let harness = harness(Reply::Text("reply"));
    let database = harness.context.backend().database();
    let mut ids = Vec::new();
    for (name, created, updated) in [
        ("Library-Zed", 1, 10),
        ("Library-Alpha", 2, 5),
        ("Library-Middle", 3, 9),
    ] {
        let mut character = Character::new(
            CharacterId::new(),
            CharacterProfile {
                name: name.into(),
                nickname: Some("alias-only".into()),
                description: None,
                definition: None,
                design_description: None,
                scenario: None,
            },
            CharacterProvenance::default(),
            CharacterDefaults::default(),
            CharacterPresentationV1::default(),
            None,
            CharacterMedia::default(),
            TimestampMillis::new(created),
        )
        .expect("character");
        character.updated_at = TimestampMillis::new(updated);
        ids.push(character.id.to_string());
        CharacterRepository::create(
            database,
            CreateCharacterPlan {
                character,
                scenes: vec![],
                variants: vec![],
                starters: vec![],
            },
        )
        .expect("create");
    }
    let request = dto::CharactersListRequest {
        query: Some("Library-".into()),
        limit: Some(1),
        ..Default::default()
    };
    let first = characters_list(&harness.context, request.clone())
        .await
        .expect("page");
    assert_eq!(first.items[0].id, ids[0]);
    let mut cursor = first.next_cursor.clone();
    let mut found = vec![first.items[0].id.clone()];
    while let Some(value) = cursor {
        let page = characters_list(
            &harness.context,
            dto::CharactersListRequest {
                cursor: Some(value),
                ..request.clone()
            },
        )
        .await
        .expect("next");
        found.extend(page.items.into_iter().map(|item| item.id));
        cursor = page.next_cursor;
    }
    assert_eq!(found, ids);
    for (sort, expected) in [
        (
            dto::LibrarySort::Name,
            vec![ids[1].clone(), ids[2].clone(), ids[0].clone()],
        ),
        (
            dto::LibrarySort::UpdatedDesc,
            vec![ids[0].clone(), ids[2].clone(), ids[1].clone()],
        ),
    ] {
        let page = characters_list(
            &harness.context,
            dto::CharactersListRequest {
                sort: Some(sort),
                limit: Some(20),
                ..request.clone()
            },
        )
        .await
        .expect("sort");
        assert_eq!(
            page.items
                .into_iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            expected
        );
        let mut cursor = None;
        let mut paged = Vec::new();
        loop {
            let page = characters_list(
                &harness.context,
                dto::CharactersListRequest {
                    cursor,
                    sort: Some(sort),
                    limit: Some(1),
                    ..request.clone()
                },
            )
            .await
            .expect("sorted page");
            paged.extend(page.items.into_iter().map(|item| item.id));
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(paged, expected);
    }
    let nickname = dto::CharactersListRequest {
        query: Some("alias-only".into()),
        ..Default::default()
    };
    assert!(
        characters_list(&harness.context, nickname.clone())
            .await
            .expect("name only")
            .items
            .is_empty()
    );
    assert_eq!(
        characters_list(
            &harness.context,
            dto::CharactersListRequest {
                search_nickname: true,
                ..nickname
            }
        )
        .await
        .expect("nickname")
        .items
        .len(),
        3
    );
    assert_eq!(
        characters_list(
            &harness.context,
            dto::CharactersListRequest {
                cursor: first.next_cursor,
                query: Some("changed".into()),
                ..request
            }
        )
        .await
        .expect_err("cursor filter changed")
        .code,
        ApiErrorCode::InvalidInput
    );
}

#[tokio::test]
async fn persona_library_lists_use_creation_order_and_return_the_default_singleton() {
    let harness = harness(Reply::Text("reply"));
    let database = harness.context.backend().database();
    let mut first = Persona::new(
        PersonaId::new(),
        "Zed".into(),
        "First".into(),
        TimestampMillis::new(1),
    )
    .expect("first");
    first.nickname = Some("alias-only".into());
    first.updated_at = TimestampMillis::new(10);
    let first = PersonaRepository::create(database, first).expect("create first");
    let second = PersonaRepository::create(
        database,
        Persona::new(
            PersonaId::new(),
            "Alpha".into(),
            "Second".into(),
            TimestampMillis::new(2),
        )
        .expect("second"),
    )
    .expect("create second");
    let current = PersonaRepository::get_default_snapshot(database)
        .expect("default")
        .state;
    let selected = PersonaRepository::set_default(
        database,
        first.id,
        current.revision,
        TimestampMillis::new(11),
    )
    .expect("select");
    let page = personas_list(
        &harness.context,
        dto::PersonasListRequest {
            limit: Some(1),
            ..Default::default()
        },
    )
    .await
    .expect("first page");
    assert_eq!(page.items[0].id, first.id.to_string());
    assert_eq!(page.default_persona.persona_id, Some(first.id.to_string()));
    assert_eq!(page.default_persona.revision, selected.revision.get());
    let next = personas_list(
        &harness.context,
        dto::PersonasListRequest {
            cursor: page.next_cursor,
            ..Default::default()
        },
    )
    .await
    .expect("next page");
    assert_eq!(next.items[0].id, second.id.to_string());
    let sorted = personas_list(
        &harness.context,
        dto::PersonasListRequest {
            sort: Some(dto::LibrarySort::Name),
            ..Default::default()
        },
    )
    .await
    .expect("name order");
    assert_eq!(sorted.items[0].id, second.id.to_string());
    assert!(
        personas_list(
            &harness.context,
            dto::PersonasListRequest {
                query: Some("alias-only".into()),
                ..Default::default()
            }
        )
        .await
        .expect("name only")
        .items
        .is_empty()
    );
    assert_eq!(
        personas_list(
            &harness.context,
            dto::PersonasListRequest {
                query: Some("alias-only".into()),
                search_nickname: true,
                ..Default::default()
            }
        )
        .await
        .expect("nickname")
        .items[0]
            .id,
        first.id.to_string()
    );
}

#[test]
fn existing_character_picker_requests_keep_their_wire_shape() {
    let request: dto::CharactersListRequest =
        serde_json::from_str(r#"{"cursor":null,"limit":20}"#).expect("existing request");
    assert_eq!(request.query, None);
    assert_eq!(request.sort, None);
    assert!(!request.search_nickname);
}

#[tokio::test]
async fn group_library_pages_follow_activity_and_bind_cursor_to_filter_and_sort() {
    use crate::launch::tests::{member, seed_group, seed_named_character};
    let harness = harness(Reply::Text("reply"));
    let database = harness.context.backend().database();
    let second = seed_named_character(database, "Second");
    let group = |name: &str| {
        seed_group(
            database,
            vec![member(harness.character_id, 0), member(second, 1)],
            None,
            |group| group.name = name.into(),
        )
    };
    let zed = group("Library-Zed");
    let alpha = group("Library-Alpha");
    conversation_launch_group(
        &harness.context,
        dto::LaunchGroupRequest {
            group_id: zed.to_string(),
            client_operation_id: "list-activity-launch".into(),
        },
    )
    .await
    .expect("launch");
    let request = dto::GroupsListRequest {
        query: Some("Library-".into()),
        sort: None,
        cursor: None,
        limit: 1,
    };
    let page = groups_list(&harness.context, request.clone())
        .await
        .expect("page");
    assert_eq!(page.items[0].group.id, zed.to_string());
    let cursor = page.next_cursor.expect("cursor");
    let next = groups_list(
        &harness.context,
        dto::GroupsListRequest {
            cursor: Some(cursor.clone()),
            ..request.clone()
        },
    )
    .await
    .expect("next");
    assert_eq!(next.items[0].group.id, alpha.to_string());
    assert!(next.next_cursor.is_none());
    assert_eq!(
        groups_list(
            &harness.context,
            dto::GroupsListRequest {
                cursor: Some(cursor),
                sort: Some(dto::LibrarySort::Name),
                ..request.clone()
            }
        )
        .await
        .expect_err("sort")
        .code,
        ApiErrorCode::InvalidInput
    );
    let names = groups_list(
        &harness.context,
        dto::GroupsListRequest {
            sort: Some(dto::LibrarySort::Name),
            limit: 100,
            ..request
        },
    )
    .await
    .expect("names");
    assert_eq!(names.items[0].group.id, alpha.to_string());
    assert_eq!(names.items[1].group.id, zed.to_string());
}
