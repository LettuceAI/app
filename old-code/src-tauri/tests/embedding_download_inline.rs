//! Gathered from inline tests in src/embedding/download.rs.

use lettuceai_lib::embedding::download::apply_embedding_version_preference;

#[test]
fn sets_embedding_version_preference_in_advanced_settings() {
    let mut advanced = serde_json::json!({
        "embeddingMaxTokens": 2048
    });

    apply_embedding_version_preference(&mut advanced, "v4").expect("should update settings");

    assert_eq!(
        advanced.get("embeddingModelVersion"),
        Some(&serde_json::json!("v4"))
    );
    assert_eq!(
        advanced.get("embeddingMaxTokens"),
        Some(&serde_json::json!(2048))
    );
}

#[test]
fn rejects_non_object_advanced_settings() {
    let mut advanced = serde_json::json!(null);

    let err = apply_embedding_version_preference(&mut advanced, "v4")
        .expect_err("non-object settings should fail");

    assert!(err.contains("Advanced settings payload is not an object"));
}
