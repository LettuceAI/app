use lettuce_characters::{LibrarySort, RepositoryError, ValidationError};
use rusqlite::types::Value;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum LibraryCursorKey {
    Time(i64),
    Name(String),
}

impl LibraryCursorKey {
    pub(crate) fn sql_value(&self) -> Value {
        match self {
            Self::Time(value) => Value::Integer(*value),
            Self::Name(value) => Value::Text(value.clone()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LibraryCursor {
    pub entity: String,
    pub sort: LibrarySort,
    pub query: String,
    pub search_nickname: bool,
    pub include_archived: bool,
    pub key: LibraryCursorKey,
    pub id: String,
}

fn invalid() -> RepositoryError {
    RepositoryError::Invalid(ValidationError::InvalidValue {
        field: "library.cursor",
    })
}

impl LibraryCursor {
    pub(crate) fn encode(&self) -> Result<String, RepositoryError> {
        serde_json::to_vec(self)
            .map(|bytes| crate::hex_encode(&bytes))
            .map_err(|_| invalid())
    }

    pub(crate) fn decode(
        cursor: Option<&str>,
        entity: &str,
        sort: LibrarySort,
        query: &str,
        search_nickname: bool,
        include_archived: bool,
    ) -> Result<Option<Self>, RepositoryError> {
        let Some(cursor) = cursor else {
            return Ok(None);
        };
        let bytes = crate::hex_decode(cursor).map_err(|_| invalid())?;
        let decoded: Self = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        let key_matches = matches!(
            (&decoded.key, sort),
            (LibraryCursorKey::Name(_), LibrarySort::Name)
                | (
                    LibraryCursorKey::Time(_),
                    LibrarySort::CreatedAsc | LibrarySort::UpdatedDesc
                )
        );
        if decoded.entity != entity
            || decoded.sort != sort
            || decoded.query != query
            || decoded.search_nickname != search_nickname
            || decoded.include_archived != include_archived
            || !key_matches
        {
            return Err(invalid());
        }
        Ok(Some(decoded))
    }
}
