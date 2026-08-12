use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use super::store::StoreError;

const DEVICE_ID_KEY: &str = "device_id";

pub fn get_or_create_device_id(conn: &Connection) -> Result<String, StoreError> {
    if let Some(device_id) = conn
        .query_row(
            "SELECT value FROM sync_v2_local_state WHERE key = ?1",
            params![DEVICE_ID_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .filter(|device_id| !device_id.is_empty())
    {
        return Ok(device_id);
    }

    let device_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO sync_v2_local_state (key, value)
         VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![DEVICE_ID_KEY, device_id],
    )?;
    Ok(device_id)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::get_or_create_device_id;
    use crate::sync::v2::create_schema;

    #[test]
    fn device_identity_is_stable() {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn).unwrap();
        let first = get_or_create_device_id(&conn).unwrap();
        assert_eq!(get_or_create_device_id(&conn).unwrap(), first);
    }
}
