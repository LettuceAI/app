//! Gathered from inline tests in src/storage_manager/entity_transfer/mod.rs.

use lettuceai_lib::storage_manager::entity_transfer::{
    convert_export_to_uec, normalize_uec_for_read, parse_uec_character, stringify_v2_uec,
    UEC_SCHEMA_VERSION as SCHEMA_VERSION, UEC_SCHEMA_VERSION_V2 as SCHEMA_VERSION_V2,
};
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use unified_entity_card::{create_character_uec, UecKind};

#[test]
fn normalize_uec_for_read_accepts_v1_schema() {
    let card = json!({
        "schema": { "name": "UEC", "version": SCHEMA_VERSION },
        "kind": "character",
        "payload": {
            "id": "char-v1",
            "name": "Aster Vale"
        }
    });

    let parsed = normalize_uec_for_read(&card, false).expect("v1 UEC should be readable");
    assert_eq!(parsed.kind, UecKind::Character);
    assert_eq!(parsed.schema.version, SCHEMA_VERSION);
}

#[test]
fn normalize_uec_for_read_downgrades_v2_schema_for_legacy_parser() {
    let card = json!({
        "schema": {
            "name": "UEC",
            "version": SCHEMA_VERSION_V2
        },
        "kind": "character",
        "payload": {
            "id": "char-v2",
            "name": "Aster Vale",
            "scene": {
                "id": "scene-1",
                "content": "Hello there",
                "selectedVariant": 0,
                "variants": []
            }
        },
        "meta": {
            "originalCreatedAt": 1,
            "originalUpdatedAt": 2
        }
    });

    let parsed = normalize_uec_for_read(&card, false).expect("v2 UEC should be readable");
    assert_eq!(parsed.kind, UecKind::Character);
    assert_eq!(parsed.schema.version, SCHEMA_VERSION);
    let payload = parsed.payload.as_object().expect("payload object");
    assert!(payload.get("scenes").is_some());
    assert!(payload.get("scene").is_none());
}

#[test]
fn stringify_v2_uec_upgrades_v1_schema_to_v2() {
    let mut payload = JsonMap::new();
    payload.insert("id".into(), JsonValue::String("char-1".to_string()));
    payload.insert("name".into(), JsonValue::String("Aster Vale".to_string()));
    payload.insert(
        "avatar".into(),
        JsonValue::String("data:image/webp;base64,QUJD".to_string()),
    );
    payload.insert(
        "chatBackground".into(),
        JsonValue::String("https://example.com/bg.png".to_string()),
    );
    payload.insert(
        "scenes".into(),
        JsonValue::Array(vec![json!({
            "id": "scene-1",
            "content": "Hello there",
            "selectedVariantId": null,
            "variants": []
        })]),
    );
    payload.insert(
        "defaultSceneId".into(),
        JsonValue::String("scene-1".to_string()),
    );
    payload.insert("createdAt".into(), JsonValue::from(1));
    payload.insert("updatedAt".into(), JsonValue::from(2));

    let v1 = create_character_uec(
        payload,
        false,
        None,
        None,
        Some(json!({ "createdAt": 1, "updatedAt": 2, "source": "lettuceai" })),
        None,
    );
    let value: JsonValue =
        serde_json::from_str(&stringify_v2_uec(&v1).expect("v2 json")).expect("valid json");
    let schema = value
        .get("schema")
        .and_then(|schema| schema.as_object())
        .expect("schema object");

    assert_eq!(
        schema.get("version").and_then(|value| value.as_str()),
        Some(SCHEMA_VERSION_V2)
    );
    let payload = value
        .get("payload")
        .and_then(|payload| payload.as_object())
        .expect("payload object");
    assert!(payload.get("scene").is_some());
    assert!(payload.get("scenes").is_none());
    assert_eq!(
        payload
            .get("avatar")
            .and_then(|avatar| avatar.get("type"))
            .and_then(|value| value.as_str()),
        Some("inline_base64")
    );
    assert_eq!(
        payload
            .get("chatBackground")
            .and_then(|background| background.get("type"))
            .and_then(|value| value.as_str()),
        Some("remote_url")
    );
}

#[test]
fn stringify_v2_uec_preserves_scene_variants_and_selected_id() {
    let mut payload = JsonMap::new();
    payload.insert("id".into(), JsonValue::String("char-1".to_string()));
    payload.insert("name".into(), JsonValue::String("Aster Vale".to_string()));
    payload.insert(
        "scenes".into(),
        JsonValue::Array(vec![json!({
            "id": "scene-1",
            "content": "Hello there",
            "selectedVariantId": "variant-2",
            "variants": [
                {
                    "id": "variant-1",
                    "content": "Variant one",
                    "createdAt": 10
                },
                {
                    "id": "variant-2",
                    "content": "Variant two",
                    "direction": "Second",
                    "createdAt": 20
                }
            ]
        })]),
    );
    payload.insert(
        "defaultSceneId".into(),
        JsonValue::String("scene-1".to_string()),
    );
    payload.insert("createdAt".into(), JsonValue::from(1));
    payload.insert("updatedAt".into(), JsonValue::from(2));

    let v1 = create_character_uec(
        payload,
        false,
        None,
        None,
        Some(json!({ "createdAt": 1, "updatedAt": 2, "source": "lettuceai" })),
        None,
    );

    let value: JsonValue =
        serde_json::from_str(&stringify_v2_uec(&v1).expect("v2 json")).expect("valid json");
    let scene = value
        .get("payload")
        .and_then(|payload| payload.get("scene"))
        .and_then(JsonValue::as_object)
        .expect("scene object");

    assert_eq!(
        scene.get("selectedVariant").and_then(JsonValue::as_str),
        Some("variant-2")
    );
    let variants = scene
        .get("variants")
        .and_then(JsonValue::as_array)
        .expect("variants array");
    assert_eq!(variants.len(), 2);
    assert_eq!(
        variants[1].get("id").and_then(JsonValue::as_str),
        Some("variant-2")
    );
    assert_eq!(
        variants[1].get("direction").and_then(JsonValue::as_str),
        Some("Second")
    );
}

#[test]
fn stringify_v2_uec_flattens_additional_scenes_into_variants() {
    let mut payload = JsonMap::new();
    payload.insert("id".into(), JsonValue::String("char-1".to_string()));
    payload.insert("name".into(), JsonValue::String("Aster Vale".to_string()));
    payload.insert(
        "scenes".into(),
        JsonValue::Array(vec![
            json!({
                "id": "scene-1",
                "content": "Primary scene",
                "selectedVariantId": null,
                "variants": []
            }),
            json!({
                "id": "scene-2",
                "content": "Second scene",
                "direction": "alt",
                "createdAt": 20,
                "selectedVariantId": null,
                "variants": []
            }),
            json!({
                "id": "scene-3",
                "content": "Third scene",
                "createdAt": 30,
                "selectedVariantId": null,
                "variants": []
            }),
        ]),
    );
    payload.insert(
        "defaultSceneId".into(),
        JsonValue::String("scene-1".to_string()),
    );
    payload.insert("createdAt".into(), JsonValue::from(1));
    payload.insert("updatedAt".into(), JsonValue::from(2));

    let v1 = create_character_uec(
        payload,
        false,
        None,
        None,
        Some(json!({ "createdAt": 1, "updatedAt": 2, "source": "lettuceai" })),
        None,
    );

    let value: JsonValue =
        serde_json::from_str(&stringify_v2_uec(&v1).expect("v2 json")).expect("valid json");
    let scene = value
        .get("payload")
        .and_then(|payload| payload.get("scene"))
        .and_then(JsonValue::as_object)
        .expect("scene object");
    let variants = scene
        .get("variants")
        .and_then(JsonValue::as_array)
        .expect("variants array");

    assert_eq!(variants.len(), 2);
    assert_eq!(
        variants[0].get("id").and_then(JsonValue::as_str),
        Some("scene-2")
    );
    assert_eq!(
        variants[1].get("id").and_then(JsonValue::as_str),
        Some("scene-3")
    );
}

#[test]
fn parse_uec_character_reads_v2_asset_locators() {
    let card = json!({
        "schema": { "name": "UEC", "version": SCHEMA_VERSION_V2 },
        "kind": "character",
        "payload": {
            "id": "char-v2",
            "name": "Aster Vale",
            "avatar": {
                "type": "inline_base64",
                "mimeType": "image/webp",
                "data": "QUJD"
            },
            "chatBackground": {
                "type": "remote_url",
                "url": "https://example.com/bg.png"
            },
            "scene": {
                "id": "scene-1",
                "content": "Hello there",
                "selectedVariant": 0,
                "variants": []
            }
        },
        "meta": {
            "createdAt": 1,
            "updatedAt": 2,
            "originalCreatedAt": 1,
            "originalUpdatedAt": 2
        }
    });

    let package = parse_uec_character(&card).expect("v2 character should parse");
    assert_eq!(
        package.avatar_data.as_deref(),
        Some("data:image/webp;base64,QUJD")
    );
    assert_eq!(
        package.background_image_data.as_deref(),
        Some("https://example.com/bg.png")
    );
}

#[test]
fn parse_uec_character_expands_v2_scene_variants_into_scenes() {
    let card = json!({
        "schema": { "name": "UEC", "version": SCHEMA_VERSION_V2 },
        "kind": "character",
        "payload": {
            "id": "char-v2",
            "name": "Aster Vale",
            "scene": {
                "id": "scene-1",
                "content": "Primary scene",
                "selectedVariant": "scene-3",
                "variants": [
                    {
                        "id": "scene-2",
                        "content": "Second scene",
                        "direction": "Alt two",
                        "createdAt": 20
                    },
                    {
                        "id": "scene-3",
                        "content": "Third scene",
                        "createdAt": 30
                    }
                ]
            }
        },
        "meta": {
            "createdAt": 1,
            "updatedAt": 2,
            "originalCreatedAt": 1,
            "originalUpdatedAt": 2
        }
    });

    let package = parse_uec_character(&card).expect("v2 character should parse");
    assert_eq!(package.character.scenes.len(), 3);
    assert_eq!(package.character.scenes[0].id, "scene-1");
    assert_eq!(package.character.scenes[1].id, "scene-2");
    assert_eq!(package.character.scenes[2].id, "scene-3");
    assert_eq!(
        package.character.default_scene_id.as_deref(),
        Some("scene-3")
    );
}

#[test]
fn convert_export_to_uec_keeps_companion_data_in_app_specific_settings() {
    let legacy = json!({
        "version": 1,
        "exportedAt": 1234,
        "character": {
            "name": "Aster Vale",
            "description": "Long-running companion.",
            "definition": "Aster keeps continuity across chats.",
            "rules": [],
            "scenes": [],
            "defaultSceneId": null,
            "defaultModelId": null,
            "mode": "companion",
            "companion": {
                "soul": {
                    "essence": "Wry, loyal, hard to impress.",
                    "traits": "Observant and careful.",
                    "baselineAffect": {
                        "warmth": 0.6,
                        "trust": 0.4,
                        "calm": 0.5,
                        "vulnerability": 0.2,
                        "longing": 0.1,
                        "hurt": 0.0,
                        "tension": 0.05,
                        "irritation": 0.0,
                        "affectionIntensity": 0.3,
                        "reassuranceNeed": 0.15
                    },
                    "regulationStyle": {
                        "suppression": 0.3,
                        "volatility": 0.2,
                        "recoverySpeed": 0.6,
                        "conflictAvoidance": 0.4,
                        "reassuranceSeeking": 0.2,
                        "protestBehavior": 0.1,
                        "emotionalTransparency": 0.5,
                        "attachmentActivation": 0.4,
                        "pride": 0.3
                    }
                },
                "relationshipDefaults": {
                    "closeness": 0.2,
                    "trust": 0.3,
                    "affection": 0.15,
                    "tension": 0.0
                },
                "memory": {
                    "enabled": true,
                    "retrievalLimit": 8,
                    "maxEntries": 120,
                    "prioritizeRelationship": true,
                    "prioritizeEpisodic": true,
                    "useEmotionalSnapshots": true,
                    "sharedAcrossSessions": true
                },
                "prompting": {
                    "promptTemplateId": "prompt-companion",
                    "styleNotes": "Keep the edge."
                },
                "timeAwareness": true
            },
            "companionScheduledNotes": [
                {
                    "id": "note-old",
                    "characterId": "char-old",
                    "label": "Anniversary",
                    "content": "Remember the bridge conversation.",
                    "availableAt": 2000,
                    "recurrence": "yearly",
                    "enabled": true,
                    "createdAt": 1000,
                    "updatedAt": 1500
                }
            ],
            "companionSharedMemory": {
                "memories": [
                    { "id": "mem-1", "text": "Aster and the user chose a meeting place." }
                ],
                "memorySummary": "They have one shared plan.",
                "memorySummaryTokenCount": 7,
                "memoryToolEvents": [],
                "createdAt": 1000,
                "updatedAt": 1500
            },
            "memoryType": "dynamic",
            "activeLorebookIds": [],
            "lorebooks": [],
            "promptTemplateId": null,
            "systemPrompt": null,
            "voiceConfig": null,
            "voiceAutoplay": false,
            "disableAvatarGradient": false,
            "avatarCrop": null,
            "bannerCrop": null,
            "customGradientEnabled": false,
            "customGradientColors": null,
            "customTextColor": null,
            "customTextSecondary": null,
            "chatTemplates": [],
            "defaultChatTemplateId": null
        },
        "avatarData": null,
        "backgroundImageData": null
    });

    let uec: JsonValue = serde_json::from_str(
        &convert_export_to_uec(legacy.to_string()).expect("legacy export converts to UEC"),
    )
    .expect("UEC JSON");

    let payload = uec
        .get("payload")
        .and_then(JsonValue::as_object)
        .expect("payload object");
    assert!(payload.get("companion").is_none());
    assert!(payload.get("companionScheduledNotes").is_none());
    assert!(payload.get("companionSharedMemory").is_none());

    let app_specific = uec
        .get("app_specific_settings")
        .and_then(JsonValue::as_object)
        .expect("app-specific settings");
    assert_eq!(
        app_specific
            .get("interaction_mode")
            .and_then(JsonValue::as_str),
        Some("companion")
    );
    assert_eq!(
        app_specific
            .get("companion")
            .and_then(|value| value.get("soul"))
            .and_then(|value| value.get("essence"))
            .and_then(JsonValue::as_str),
        Some("Wry, loyal, hard to impress.")
    );
    assert_eq!(
        app_specific
            .get("companion")
            .and_then(|value| value.get("prompting"))
            .and_then(|value| value.get("promptTemplateId"))
            .and_then(JsonValue::as_str),
        Some("prompt-companion")
    );
    assert_eq!(
        app_specific
            .get("companionScheduledNotes")
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        app_specific
            .get("companionSharedMemory")
            .and_then(|value| value.get("memorySummary"))
            .and_then(JsonValue::as_str),
        Some("They have one shared plan.")
    );
}

#[test]
fn parse_uec_character_prefers_interaction_mode_app_setting() {
    let card = json!({
        "schema": { "name": "UEC", "version": SCHEMA_VERSION },
        "kind": "character",
        "payload": {
            "id": "char-v1",
            "name": "Aster Vale"
        },
        "app_specific_settings": {
            "interaction_mode": "companion",
            "mode": "roleplay"
        }
    });

    let package = parse_uec_character(&card).expect("character should parse");
    assert_eq!(package.character.mode.as_deref(), Some("companion"));
}
