use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;
use uuid::Uuid;

use crate::db::models::*;
use crate::db::{repo, v5_importer};
use crate::logical_time::{logical_today_key, now_epoch_ms};
use crate::sync::{gist_client, merge_engine};

pub struct AppDbState {
    pub conn: Mutex<Connection>,
    pub device_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskInput {
    pub logical_date: String,
    pub title: String,
    pub category: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub note: Option<String>,
    pub priority: Option<String>,
    pub reminder_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SaveNoteInput {
    pub id: Option<String>,
    pub logical_date: Option<String>,
    pub title: String,
    pub body_md: String,
    pub tags_json: Option<String>,
    pub pinned: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct SyncStatusResult {
    pub status: String,
    pub etag: Option<String>,
    pub merged_tasks: usize,
}

#[tauri::command]
pub fn get_logical_today() -> String {
    logical_today_key()
}

#[tauri::command]
pub fn get_day_bundle(state: State<'_, AppDbState>, logical_date: String) -> Result<DayBundle, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    repo::get_day_bundle(&conn, &logical_date, &state.device_id)
}

#[tauri::command]
pub fn create_task(state: State<'_, AppDbState>, input: CreateTaskInput) -> Result<String, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO tasks
         (id, logical_date, original_logical_date, rule_id, title, category, start_time, end_time, note, priority, status, reschedule_count, reminder_at, reminder_fired, updated_at, device_id, deleted_at)
         VALUES (?1, ?2, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', 0, ?9, 0, ?10, ?11, NULL)",
        params![
            id,
            input.logical_date,
            input.title.trim(),
            input.category,
            input.start_time,
            input.end_time,
            input.note,
            input.priority.unwrap_or_else(|| "normal".to_string()),
            input.reminder_at,
            now,
            &state.device_id
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn toggle_task_done(state: State<'_, AppDbState>, task_id: String, done: bool) -> Result<i64, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    repo::toggle_task_done(&conn, &task_id, done, &state.device_id)
}

#[tauri::command]
pub fn reschedule_task(
    state: State<'_, AppDbState>,
    task_id: String,
    to_date: String,
    reason: String,
) -> Result<(), String> {
    let mut conn = state.conn.lock().map_err(|e| e.to_string())?;
    repo::reschedule_task(&mut conn, &task_id, &to_date, &reason, &state.device_id)
}

#[tauri::command]
pub fn cancel_task_with_excuse(
    state: State<'_, AppDbState>,
    task_id: String,
    excuse_category: String,
    excuse_text: String,
) -> Result<(), String> {
    let mut conn = state.conn.lock().map_err(|e| e.to_string())?;
    repo::cancel_task_with_excuse(&mut conn, &task_id, &excuse_category, &excuse_text, &state.device_id)
}

#[tauri::command]
pub fn toggle_prayer(
    state: State<'_, AppDbState>,
    logical_date: String,
    prayer_key: String,
    completed: bool,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    repo::toggle_prayer(&conn, &logical_date, &prayer_key, completed, &state.device_id)
}

#[tauri::command]
pub fn toggle_weekly_shield(
    state: State<'_, AppDbState>,
    logical_date: String,
    freeze: bool,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    repo::toggle_weekly_shield(&conn, &logical_date, freeze, &state.device_id)
}

#[tauri::command]
pub fn log_focus_session(
    state: State<'_, AppDbState>,
    logical_date: String,
    planned_mins: i64,
    actual_mins: i64,
    completed: bool,
) -> Result<String, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO focus_sessions
         (id, logical_date, started_at, ended_at, planned_mins, actual_mins, completed, checkpoint_json, updated_at, device_id, deleted_at)
         VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, NULL, ?3, ?7, NULL)",
        params![id, logical_date, now, planned_mins, actual_mins, if completed { 1 } else { 0 }, &state.device_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn add_distraction(
    state: State<'_, AppDbState>,
    logical_date: String,
    text: String,
) -> Result<String, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO distractions (id, logical_date, session_id, text, resolved, created_at, updated_at, device_id, deleted_at)
         VALUES (?1, ?2, NULL, ?3, 0, ?4, ?4, ?5, NULL)",
        params![id, logical_date, text.trim(), now, &state.device_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn list_notes(state: State<'_, AppDbState>) -> Result<Vec<NoteItem>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, logical_date, title, body_md, tags_json, pinned, created_at, updated_at, device_id, deleted_at
             FROM notes WHERE deleted_at IS NULL ORDER BY pinned DESC, updated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let items = stmt
        .query_map([], |r| {
            Ok(NoteItem {
                id: r.get(0)?,
                logical_date: r.get(1)?,
                title: r.get(2)?,
                body_md: r.get(3)?,
                tags_json: r.get(4)?,
                pinned: r.get::<_, i64>(5)? != 0,
                created_at: r.get(6)?,
                updated_at: r.get(7)?,
                device_id: r.get(8)?,
                deleted_at: r.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(items)
}

#[tauri::command]
pub fn save_note(state: State<'_, AppDbState>, input: SaveNoteInput) -> Result<String, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO notes (id, logical_date, title, body_md, tags_json, pinned, created_at, updated_at, device_id, deleted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8, NULL)
         ON CONFLICT(id) DO UPDATE SET
           logical_date = excluded.logical_date,
           title = excluded.title,
           body_md = excluded.body_md,
           tags_json = excluded.tags_json,
           pinned = excluded.pinned,
           updated_at = excluded.updated_at,
           device_id = excluded.device_id",
        params![
            id,
            input.logical_date,
            input.title.trim(),
            input.body_md,
            input.tags_json.unwrap_or_else(|| "[]".to_string()),
            if input.pinned.unwrap_or(false) { 1 } else { 0 },
            now,
            &state.device_id
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn delete_note(state: State<'_, AppDbState>, note_id: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let now = now_epoch_ms();
    conn.execute(
        "UPDATE notes SET deleted_at = ?1, updated_at = ?1, device_id = ?2 WHERE id = ?3",
        params![now, &state.device_id, note_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_monthly_telemetry(
    state: State<'_, AppDbState>,
    month_prefix: String,
) -> Result<MonthlyTelemetry, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    repo::get_monthly_telemetry(&conn, &month_prefix, &state.device_id)
}

#[tauri::command]
pub fn import_v5_backup(
    state: State<'_, AppDbState>,
    raw_json: String,
) -> Result<v5_importer::V5ImportReport, String> {
    let mut conn = state.conn.lock().map_err(|e| e.to_string())?;
    v5_importer::import_v5_json(&mut conn, &raw_json, &state.device_id)
}

#[tauri::command]
pub async fn run_gist_sync(
    state: State<'_, AppDbState>,
    pat: String,
    gist_id: String,
) -> Result<SyncStatusResult, String> {
    let (local_snap, last_etag, dev_id) = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        let snap = repo::export_sync_snapshot(&conn, &state.device_id)?;
        let etag: Option<String> = conn
            .query_row(
                "SELECT value_json FROM app_meta WHERE key = 'last_sync_etag'",
                [],
                |r| r.get(0),
            )
            .ok()
            .and_then(|s: String| serde_json::from_str(&s).ok());
        (snap, etag, state.device_id.clone())
    };

    let pull = gist_client::pull_gist(&pat, &gist_id, last_etag.as_deref()).await?;
    let merged = match pull.snapshot {
        Some(cloud_snap) => merge_engine::merge_snapshots(&local_snap, &cloud_snap, &dev_id),
        None => local_snap.clone(),
    };

    let new_etag = gist_client::push_gist(&pat, &gist_id, &merged).await?;
    if let Some(ref et) = new_etag {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        let now = now_epoch_ms();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO app_meta (key, value_json, updated_at, device_id) VALUES ('last_sync_etag', ?1, ?2, ?3)",
            params![serde_json::to_string(et).unwrap_or_default(), now, &dev_id],
        );
    }

    Ok(SyncStatusResult {
        status: "synced".to_string(),
        etag: new_etag,
        merged_tasks: merged.tasks.len(),
    })
}
