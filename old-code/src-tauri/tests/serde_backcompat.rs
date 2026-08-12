//! Serde back-compat fixtures: load minimal "old" JSON shapes for major
//! persisted types and assert deserialization still succeeds. Catches
//! accidental removal of `#[serde(default)]`, renamed fields, or required
//! new fields that would silently break existing user data.

use lettuceai_lib::chat_manager::types::{Character, MemoryEmbedding, Session, Settings};
use lettuceai_lib::storage_manager::group_sessions::GroupSession;
use serde_json::json;

#[test]
fn session_minimal_shape_loads() {
    // Pre-memory, pre-companion era — only mandatory fields present.
    let v = json!({
        "id": "sess-1",
        "characterId": "char-1",
        "title": "Untitled",
        "createdAt": 1_700_000_000_000_u64,
        "updatedAt": 1_700_000_000_000_u64
    });
    let session: Session = serde_json::from_value(v).expect("legacy session must still load");
    assert_eq!(session.id, "sess-1");
    assert_eq!(session.character_id, "char-1");
    assert!(
        session.memories.is_empty(),
        "missing memories defaults to empty"
    );
    assert!(session.memory_embeddings.is_empty());
    assert_eq!(session.memory_summary_token_count, 0);
    assert!(!session.persona_disabled);
    assert!(!session.archived);
}

#[test]
fn session_pre_persona_loads_without_persona_fields() {
    let v = json!({
        "id": "sess-2",
        "characterId": "char-1",
        "title": "Pre-persona",
        "memories": ["alice loves dogs"],
        "createdAt": 1,
        "updatedAt": 2
    });
    let session: Session = serde_json::from_value(v).expect("ok");
    assert!(session.persona_id.is_none());
    assert!(!session.persona_disabled);
    assert_eq!(session.memories.len(), 1);
}

#[test]
fn session_pre_companion_state_field() {
    let v = json!({
        "id": "x",
        "characterId": "c",
        "title": "t",
        "createdAt": 0,
        "updatedAt": 0
    });
    let session: Session = serde_json::from_value(v).expect("ok");
    assert!(session.companion_state.is_none());
}

#[test]
fn memory_embedding_pre_persistence_fields_loads() {
    // Before persistence_importance / prompt_importance / volatility existed,
    // entries only had id, text, embedding, importance_score, created_at.
    let v = json!({
        "id": "mem-1",
        "text": "Alice loves dogs.",
        "embedding": [0.1, 0.2, 0.3],
        "createdAt": 1_700_000_000_000_u64,
        "importanceScore": 0.8
    });
    let mem: MemoryEmbedding = serde_json::from_value(v).expect("ok");
    assert_eq!(mem.id, "mem-1");
    assert!((mem.importance_score - 0.8).abs() < 1e-6);
    // Defaulted fields:
    assert!(!mem.is_cold);
    assert!(!mem.is_pinned);
    assert_eq!(mem.access_count, 0);
    assert!(mem.canonical_entities.is_empty());
    assert!(mem.supersedes.is_empty());
    assert!(mem.fact_signature.is_none());
    assert!(mem.superseded_by.is_none());
    // persistence_importance and prompt_importance default to 1.0
    assert!((mem.persistence_importance - 1.0).abs() < 1e-6);
    assert!((mem.prompt_importance - 1.0).abs() < 1e-6);
    assert!((mem.volatility - 0.4).abs() < 1e-6);
}

#[test]
fn memory_embedding_with_all_optional_fields_present() {
    let v = json!({
        "id": "mem-2",
        "text": "Bob hates cats.",
        "embedding": [],
        "createdAt": 1_700_000_000_000_u64,
        "importanceScore": 0.5,
        "persistenceImportance": 0.9,
        "promptImportance": 0.7,
        "volatility": 0.3,
        "isCold": true,
        "isPinned": false,
        "accessCount": 5,
        "lastAccessedAt": 1_700_000_001_000_u64,
        "category": "preference",
        "tokenCount": 12,
        "embeddingSourceVersion": "v4",
        "embeddingDimensions": 768,
        "observedAt": 1_700_000_000_000_u64,
        "observedTimePrecision": "turn",
        "canonicalEntities": [],
        "factSignature": "bob:dislikes:cats",
        "factPolarity": -1,
        "sourceRole": "user",
        "sourceMessageId": "msg-9",
        "supersededBy": null,
        "supersededAt": null,
        "supersedes": ["mem-1"]
    });
    let mem: MemoryEmbedding = serde_json::from_value(v).expect("ok");
    assert!(mem.is_cold);
    assert_eq!(mem.access_count, 5);
    assert_eq!(mem.category.as_deref(), Some("preference"));
    assert_eq!(mem.fact_polarity, Some(-1));
    assert_eq!(mem.supersedes, vec!["mem-1".to_string()]);
}

#[test]
fn character_minimal_shape_loads() {
    let v = json!({
        "id": "char-1",
        "name": "Alice",
        "createdAt": 0,
        "updatedAt": 0
    });
    let character: Character = serde_json::from_value(v).expect("legacy character must load");
    assert_eq!(character.name, "Alice");
    assert!(character.scenes.is_empty());
    assert!(character.definition.is_none());
    assert!(character.default_scene_id.is_none());
}

#[test]
fn settings_minimal_required_fields_load() {
    // Settings has no #[serde(default)] on its top-level Vec/Option fields,
    // so an empty `{}` fails. This pins the contract: if you remove a required
    // field from a user's saved settings JSON, the app must still load.
    let v = json!({
        "defaultProviderCredentialId": null,
        "defaultModelId": null,
        "providerCredentials": [],
        "models": []
    });
    let s: Settings = serde_json::from_value(v).expect("minimal settings must deserialize");
    assert!(s.provider_credentials.is_empty());
    assert!(s.models.is_empty());
    assert!(s.advanced_settings.is_none());
    assert_eq!(s.migration_version, 0);
}

#[test]
fn settings_unknown_field_is_ignored() {
    // serde without #[serde(deny_unknown_fields)] should accept extras.
    // If anyone adds `deny_unknown_fields`, every user with a newer settings
    // file would have their app break on downgrade.
    let v = json!({
        "defaultProviderCredentialId": null,
        "defaultModelId": null,
        "providerCredentials": [],
        "models": [],
        "futureFieldThatDoesNotExist": "ignore-me",
        "anotherUnknown": { "nested": true }
    });
    let _settings: Settings = serde_json::from_value(v).expect("unknown fields must not fail");
}

#[test]
fn group_session_minimal_loads() {
    let v = json!({
        "id": "g-1",
        "name": "Group",
        "memoryType": "manual",
        "characterIds": ["c1", "c2"],
        "createdAt": 0,
        "updatedAt": 0,
        "archived": false,
        "chatType": "conversation"
    });
    let group: GroupSession = serde_json::from_value(v).expect("group session must load");
    assert_eq!(group.character_ids.len(), 2);
    assert!(group.muted_character_ids.is_empty());
}

#[test]
fn session_roundtrip_preserves_fields() {
    let v = json!({
        "id": "rt-1",
        "characterId": "c",
        "title": "Round trip",
        "memories": ["a", "b"],
        "memorySummaryTokenCount": 42,
        "createdAt": 100,
        "updatedAt": 200
    });
    let session: Session = serde_json::from_value(v.clone()).expect("load");
    let serialized = serde_json::to_value(&session).expect("serialize");
    let reloaded: Session = serde_json::from_value(serialized).expect("reload");
    assert_eq!(reloaded.memories, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(reloaded.memory_summary_token_count, 42);
}

#[test]
fn memory_embedding_roundtrip_strips_default_optionals() {
    // Defaults for Option fields with skip_serializing_if should not reappear as nulls.
    let mem = MemoryEmbedding {
        id: "rt".into(),
        text: "x".into(),
        embedding: vec![0.1],
        created_at: 1,
        token_count: 1,
        is_cold: false,
        last_accessed_at: 0,
        importance_score: 1.0,
        persistence_importance: 1.0,
        prompt_importance: 1.0,
        volatility: 0.4,
        is_pinned: false,
        access_count: 0,
        embedding_source_version: None,
        embedding_dimensions: None,
        match_score: None,
        category: None,
        observed_at: None,
        observed_time_precision: None,
        canonical_entities: vec![],
        fact_signature: None,
        fact_polarity: None,
        source_role: None,
        source_message_id: None,
        superseded_by: None,
        superseded_at: None,
        supersedes: vec![],
    };
    let v = serde_json::to_value(&mem).expect("serialize");
    let obj = v.as_object().expect("object");
    // skip_serializing_if for None Options
    assert!(!obj.contains_key("embeddingSourceVersion"));
    assert!(!obj.contains_key("matchScore"));
    assert!(!obj.contains_key("category"));
    assert!(!obj.contains_key("factSignature"));
    assert!(!obj.contains_key("supersededBy"));
    // Vec::is_empty fields also skipped
    assert!(!obj.contains_key("canonicalEntities"));
    assert!(!obj.contains_key("supersedes"));
}
