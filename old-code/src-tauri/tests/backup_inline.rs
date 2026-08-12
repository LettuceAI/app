//! Gathered from inline tests in src/storage_manager/backup.rs.

use lettuceai_lib::storage_manager::backup::{
    disable_dynamic_memory_in_advanced_settings, require_encrypted_backup,
    sanitize_media_archive_name, BackupManifest,
};
use std::path::PathBuf;

#[test]
fn sanitize_media_archive_name_accepts_safe_paths() {
    assert_eq!(
        sanitize_media_archive_name("images/example.png", false).unwrap(),
        PathBuf::from("images/example.png")
    );
    assert_eq!(
        sanitize_media_archive_name("images/example.png.enc", true).unwrap(),
        PathBuf::from("images/example.png")
    );
    assert_eq!(
        sanitize_media_archive_name("avatars/character-1/", false).unwrap(),
        PathBuf::from("avatars/character-1")
    );
}

#[test]
fn sanitize_media_archive_name_rejects_traversal() {
    assert!(sanitize_media_archive_name("images/../../tmp/pwned", false).is_err());
    assert!(sanitize_media_archive_name("images\\..\\..\\tmp\\pwned", false).is_err());
    assert!(sanitize_media_archive_name("/tmp/pwned", false).is_err());
}

#[test]
fn require_encrypted_backup_rejects_plaintext_archives() {
    let manifest = BackupManifest {
        version: lettuceai_lib::storage_manager::backup::BACKUP_VERSION,
        created_at: 0,
        app_version: "test".to_string(),
        encrypted: false,
        salt: None,
        nonce: None,
    };

    assert!(require_encrypted_backup(&manifest).is_err());
}

#[test]
fn disable_dynamic_memory_preserves_imported_settings() {
    let mut settings = serde_json::json!({
        "dynamicMemory": {
            "enabled": true,
            "summaryMessageInterval": 42,
            "maxEntries": 99,
            "retrievalLimit": 7,
            "contextEnrichmentEnabled": false
        },
        "groupDynamicMemory": {
            "enabled": true,
            "summaryMessageInterval": 11
        }
    });

    disable_dynamic_memory_in_advanced_settings(&mut settings);

    assert_eq!(
        settings["dynamicMemory"]["enabled"],
        serde_json::Value::Bool(false)
    );
    assert_eq!(settings["dynamicMemory"]["summaryMessageInterval"], 42);
    assert_eq!(settings["dynamicMemory"]["maxEntries"], 99);
    assert_eq!(settings["dynamicMemory"]["retrievalLimit"], 7);
    assert_eq!(settings["dynamicMemory"]["contextEnrichmentEnabled"], false);
    assert_eq!(settings["groupDynamicMemory"]["enabled"], true);
}
