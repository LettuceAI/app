use crate::Database;

#[test]
fn an_open_database_cannot_be_reserved_for_deletion_until_every_handle_closes() {
    let path =
        std::env::temp_dir().join(format!("lettuce-file-use-{}.sqlite3", uuid::Uuid::new_v4()));
    let first = Database::open(&path).expect("first database");
    let second = Database::open(&path).expect("second database");
    assert!(
        Database::try_reserve_file_deletion(&path)
            .expect("deletion reservation")
            .is_none()
    );
    drop(first);
    assert!(
        Database::try_reserve_file_deletion(&path)
            .expect("deletion reservation")
            .is_none()
    );
    drop(second);
    let permit = Database::try_reserve_file_deletion(&path)
        .expect("deletion reservation")
        .expect("unused file");
    std::fs::remove_file(&path).expect("explicit test deletion");
    drop(permit);
}

#[cfg(unix)]
#[test]
fn database_file_deletion_refuses_a_symlink_and_preserves_its_target() {
    let root = std::env::temp_dir().join(format!("lettuce-db-file-link-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("root");
    let target = root.join("target.sqlite3");
    drop(Database::open(&target).expect("database"));
    let alias = root.join("alias.sqlite3");
    std::os::unix::fs::symlink(&target, &alias).expect("alias");
    assert!(Database::try_reserve_file_deletion(&alias).is_err());
    assert!(target.exists());
    std::fs::remove_dir_all(root).expect("cleanup");
}
