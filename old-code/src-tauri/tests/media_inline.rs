//! Gathered from inline tests in src/storage_manager/media.rs.

use lettuceai_lib::storage_manager::media::{
    validate_avatar_filename, validate_simple_id, validate_single_component,
};

#[test]
fn validate_simple_id_rejects_traversal() {
    assert!(validate_simple_id("../escape", "id").is_err());
    assert!(validate_simple_id("nested/path", "id").is_err());
    assert!(validate_simple_id("", "id").is_err());
}

#[test]
fn validate_avatar_filename_requires_single_supported_file() {
    assert!(validate_avatar_filename("avatar_base.webp").is_ok());
    assert!(validate_avatar_filename("../avatar_base.webp").is_err());
    assert!(validate_avatar_filename("nested/avatar_base.webp").is_err());
    assert!(validate_avatar_filename("avatar_base.txt").is_err());
}

#[test]
fn validate_single_component_rejects_path_segments() {
    assert!(validate_single_component("../../x", "file", true).is_err());
    assert!(validate_single_component("/tmp/x", "file", true).is_err());
    assert!(validate_single_component("ok-name.png", "file", true).is_ok());
}
