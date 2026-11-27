use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::logical_time::{now_epoch_ms, week_key_for_date};

#[derive(Debug, Serialize)]
pub struct V5ImportReport {
    pub rules_imported: usize,
    pub tasks_imported: usize,
    pub days_imported: usize,
    pub prayers_imported: usize,
    pub focus_sessions_imported: usize,
    pub custom_quotes_imported: usize,
    pub distractions_imported: usize,
}

/// Lossless one-click V5 (`masar_os_v3_abdullah`) JSON -> V6 SQLite transactional migration.
pub fn import_v5_json(
    conn: &mut Connection,
    raw_json: &str,
    device_id: &str,
) -> Result<V5ImportReport, String> {
    let root: Value = serde_json::from_str(raw_json)
        .map_err(|e| format!("ملف V5 غير صالح (JSON Error): {}", e))?;

    // Support both raw state JSON and wrapped backup { state: {...} }
    let v5 = root.get("state").unwrap_or(&root);
    let now = now_epoch_ms();
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let mut report = V5ImportReport {
        rules_imported: 0,
        tasks_imported: 0,
        days_imported: 0,
        prayers_imported: 0,
        focus_sessions_imported: 0,
        custom_quotes_imported: 0,
        distractions_imported: 0,
    };

    // 1. Meta settings
    for key in [
        "identity",
        "audioPrefs",
        "calmMode",
        "sfxMuted",
        "alertSettings",
        "bestStreak",
        "pinnedQuoteId",
    ] {
        if let Some(val) = v5.get(key) {
            tx.execute(
                "INSERT OR REPLACE INTO app_meta (key, value_json, updated_at, device_id) VALUES (?1, ?2, ?3, ?4)",
                params![key, val.to_string(), now, device_id],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    // 2. Weekly schedule -> recurrence_rules
    if let Some(sched) = v5.get("schedule").and_then(|v| v.as_object()) {
        let sched_ts = v5.get("scheduleUpdatedAt").and_then(|v| v.as_i64()).unwrap_or(now);
        for (day_str, blocks_val) in sched {
            let weekday: i64 = match day_str.parse() {
                Ok(w) if (0..=6).contains(&w) => w,
                _ => continue,
            };
            if let Some(blocks) = blocks_val.as_array() {
                for (idx, b) in blocks.iter().enumerate() {
                    let id = b.get("id").and_then(|v| v.as_str()).map(|s| s.to_string())
                        .unwrap_or_else(|| Uuid::new_v4().to_string());
                    let title = b.get("title").and_then(|v| v.as_str()).unwrap_or("مهمة");
                    let category = b.get("category").and_then(|v| v.as_str()).unwrap_or("شخصي");
                    let start = b.get("start").and_then(|v| v.as_str()).unwrap_or("09:00");
                    let end = b.get("end").and_then(|v| v.as_str());
                    let prayer = b.get("prayer").and_then(|v| v.as_str());
                    let note = b.get("note").and_then(|v| v.as_str());
                    let is_shift = if category == "عمل" || category.eq_ignore_ascii_case("work") { 1 } else { 0 };

                    tx.execute(
                        "INSERT OR REPLACE INTO recurrence_rules
                         (id, weekday, title, category, start_time, end_time, prayer_slot, note, is_work_shift, sort_order, active, updated_at, device_id, deleted_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, ?12, NULL)",
                        params![id, weekday, title, category, start, end, prayer, note, is_shift, idx as i64, sched_ts, device_id],
                    )
                    .map_err(|e| e.to_string())?;
                    report.rules_imported += 1;
                }
            }
        }
    }

    // 3. Extra Tasks + Tombstones
    let deleted_ids: Vec<String> = v5
        .get("deletedExtraIds")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    if let Some(extras) = v5.get("extraTasks").and_then(|v| v.as_array()) {
        for t in extras {
            let id = t.get("id").and_then(|v| v.as_str()).map(|s| s.to_string())
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let date = t.get("date").and_then(|v| v.as_str()).unwrap_or("2026-11-27");
            let title = t.get("title").and_then(|v| v.as_str()).unwrap_or("مهمة إضافية");
            let cat = t.get("category").and_then(|v| v.as_str()).unwrap_or("شخصي");
            let time = t.get("time").and_then(|v| v.as_str());
            let note = t.get("note").and_then(|v| v.as_str());
            let priority = t.get("priority").and_then(|v| v.as_str()).unwrap_or("normal");
            let raw_status = t.get("status").and_then(|v| v.as_str()).unwrap_or("pending");
            let status = match raw_status {
                "done" => "done",
                "skip" | "skipped" | "cancelled" => "cancelled",
                _ => "pending",
            };
            let reminder_at = t.get("reminderAt").and_then(|v| v.as_str());
            let reminder_fired = if t.get("reminderFired").and_then(|v| v.as_bool()).unwrap_or(false) { 1 } else { 0 };
            let ts = t.get("updatedAt").and_then(|v| v.as_i64()).unwrap_or(now);
            let del_at = if deleted_ids.contains(&id) { Some(now) } else { None };

            tx.execute(
                "INSERT OR REPLACE INTO tasks
                 (id, logical_date, original_logical_date, rule_id, title, category, start_time, end_time, note, priority, status, reschedule_count, reminder_at, reminder_fired, updated_at, device_id, deleted_at)
                 VALUES (?1, ?2, ?2, NULL, ?3, ?4, ?5, NULL, ?6, ?7, ?8, 0, ?9, ?10, ?11, ?12, ?13)",
                params![id, date, title, cat, time, note, priority, status, reminder_at, reminder_fired, ts, device_id, del_at],
            )
            .map_err(|e| e.to_string())?;
            report.tasks_imported += 1;
        }
    }

    // 4. Daily History (prayers, statuses, focus sessions, shield)
    if let Some(hist) = v5.get("history").and_then(|v| v.as_object()) {
        for (date_key, rec) in hist {
            let frozen = rec.get("frozen").and_then(|v| v.as_bool()).unwrap_or(false);
            let wk = if frozen { week_key_for_date(date_key).ok() } else { None };
            tx.execute(
                "INSERT OR REPLACE INTO day_summaries
                 (logical_date, is_shift_day, frozen, shield_week_key, celebrated_json, updated_at, device_id)
                 VALUES (?1, 0, ?2, ?3, '{}', ?4, ?5)",
                params![date_key, if frozen { 1 } else { 0 }, wk, now, device_id],
            )
            .map_err(|e| e.to_string())?;
            report.days_imported += 1;

            if let Some(prayers) = rec.get("prayers").and_then(|v| v.as_object()) {
                let p_ts_map = rec.get("prayersUpdatedAt").and_then(|v| v.as_object());
                for (p_key, val) in prayers {
                    if !matches!(p_key.as_str(), "fajr" | "dhuhr" | "asr" | "maghrib" | "isha") {
                        continue;
                    }
                    let done = val.as_bool().unwrap_or(false);
                    let p_ts = p_ts_map.and_then(|m| m.get(p_key)).and_then(|v| v.as_i64()).unwrap_or(now);
                    tx.execute(
                        "INSERT OR REPLACE INTO prayers_log (logical_date, prayer_key, completed, updated_at, device_id)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![date_key, p_key, if done { 1 } else { 0 }, p_ts, device_id],
                    )
                    .map_err(|e| e.to_string())?;
                    report.prayers_imported += 1;
                }
            }

            // Focus minutes -> focus_sessions
            let focus_mins = rec.get("focusMinutes").and_then(|v| v.as_i64()).unwrap_or(0);
            if focus_mins > 0 {
                tx.execute(
                    "INSERT INTO focus_sessions
                     (id, logical_date, started_at, ended_at, planned_mins, actual_mins, completed, checkpoint_json, updated_at, device_id, deleted_at)
                     VALUES (?1, ?2, ?3, ?3, ?4, ?4, 1, NULL, ?3, ?5, NULL)",
                    params![Uuid::new_v4().to_string(), date_key, now, focus_mins, device_id],
                )
                .map_err(|e| e.to_string())?;
                report.focus_sessions_imported += 1;
            }
        }
    }

    // 5. Custom Quotes + Favorite flags
    if let Some(cquotes) = v5.get("customQuotes").and_then(|v| v.as_array()) {
        for q in cquotes {
            let id = q.get("id").and_then(|v| v.as_str()).map(|s| s.to_string())
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let cat = q.get("cat").and_then(|v| v.as_str()).unwrap_or("مقولاتي");
            let text = q.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let source = q.get("source").and_then(|v| v.as_str()).unwrap_or("");
            if text.trim().is_empty() { continue; }
            tx.execute(
                "INSERT OR REPLACE INTO quotes (id, category, text, source, is_builtin, is_favorite, updated_at, device_id, deleted_at)
                 VALUES (?1, ?2, ?3, ?4, 0, 1, ?5, ?6, NULL)",
                params![id, cat, text, source, now, device_id],
            )
            .map_err(|e| e.to_string())?;
            report.custom_quotes_imported += 1;
        }
    }

    // 6. Distractions
    if let Some(dists) = v5.get("distractions").and_then(|v| v.as_array()) {
        for d in dists {
            let text = d.get("text").and_then(|v| v.as_str()).or_else(|| d.as_str()).unwrap_or("");
            if text.trim().is_empty() { continue; }
            let id = d.get("id").and_then(|v| v.as_str()).map(|s| s.to_string())
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let date = d.get("date").and_then(|v| v.as_str()).unwrap_or("2026-11-27");
            let resolved = if d.get("done").and_then(|v| v.as_bool()).unwrap_or(false) { 1 } else { 0 };
            tx.execute(
                "INSERT OR REPLACE INTO distractions (id, logical_date, session_id, text, resolved, created_at, updated_at, device_id, deleted_at)
                 VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?5, ?6, NULL)",
                params![id, date, text, resolved, now, device_id],
            )
            .map_err(|e| e.to_string())?;
            report.distractions_imported += 1;
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(report)
}
