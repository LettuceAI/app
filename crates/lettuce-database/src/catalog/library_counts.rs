use lettuce_characters::RepositoryError;
use crate::Database;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LibraryCounts {
    pub characters: u32,
    pub personas: u32,
    pub lorebooks: u32,
    pub images: u32,
    pub audio: u32,
}

impl Database {
    pub fn library_counts(&self) -> Result<LibraryCounts, RepositoryError> {
        let connection = self.connection().map_err(|_| RepositoryError::Storage)?;
        connection.query_row(
            "SELECT (SELECT count(*) FROM characters WHERE status='active'), (SELECT count(*) FROM personas WHERE status='active'), (SELECT count(*) FROM lorebooks WHERE status='active'), (SELECT count(*) FROM media_assets WHERE blob_kind='image'), (SELECT count(*) FROM media_assets WHERE blob_kind='audio')",
            [],
            |row| Ok(LibraryCounts { characters: row.get(0)?, personas: row.get(1)?, lorebooks: row.get(2)?, images: row.get(3)?, audio: row.get(4)? }),
        ).map_err(|_| RepositoryError::Storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_library_counts_every_category() {
        let counts = Database::open_in_memory().expect("database").library_counts().expect("counts");
        assert_eq!([counts.characters, counts.personas, counts.lorebooks, counts.images, counts.audio], [0; 5]);
    }
}
