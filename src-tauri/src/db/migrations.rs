use rusqlite::{Connection, Result};
use uuid::Uuid;

use crate::logical_time::now_epoch_ms;

const SCHEMA_SQL: &str = include_str!("schema.sql");
pub const CURRENT_SCHEMA_VERSION: i64 = 6;

pub fn init_database(conn: &Connection) -> Result<String> {
    conn.execute_batch(SCHEMA_SQL)?;

    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version < CURRENT_SCHEMA_VERSION {
        conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;
    }

    // Ensure stable device_id exists in app_meta
    let existing: Option<String> = conn
        .query_row(
            "SELECT value_json FROM app_meta WHERE key = 'device_id'",
            [],
            |r| r.get(0),
        )
        .ok();

    if let Some(raw) = existing {
        if let Ok(parsed) = serde_json::from_str::<String>(&raw) {
            return Ok(parsed);
        }
    }

    let device_id = format!("dev_{}", Uuid::new_v4().simple());
    let now = now_epoch_ms();
    conn.execute(
        "INSERT OR REPLACE INTO app_meta (key, value_json, updated_at, device_id) VALUES ('device_id', ?1, ?2, ?3)",
        rusqlite::params![serde_json::to_string(&device_id).unwrap_or_default(), now, &device_id],
    )?;

    Ok(device_id)
}
