use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use uuid::Uuid;

use crate::db::models::*;
use crate::logical_time::{is_extended_night, now_epoch_ms, week_key_for_date, weekday_sun0};

const PRAYERS: [&str; 5] = ["fajr", "dhuhr", "asr", "maghrib", "isha"];

/// Ensures all active recurrence_rules for `logical_date`'s weekday are materialized
/// as rows in `tasks` (idempotent; never overwrites existing status/reschedules).
pub fn materialize_day_rules(
    conn: &Connection,
    logical_date: &str,
    device_id: &str,
) -> Result<(), String> {
    let weekday = weekday_sun0(logical_date)?;
    let now = now_epoch_ms();

    let mut stmt = conn
        .prepare(
            "SELECT id, title, category, start_time, end_time, note, is_work_shift
             FROM recurrence_rules
             WHERE weekday = ?1 AND active = 1 AND deleted_at IS NULL
             ORDER BY sort_order ASC, start_time ASC",
        )
        .map_err(|e| e.to_string())?;

    let rules: Vec<(String, String, String, String, Option<String>, Option<String>, i64)> = stmt
        .query_map([weekday], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut has_shift = false;
    for (rule_id, title, category, start_time, end_time, note, is_work_shift) in rules {
        if is_work_shift == 1 || category == "عمل" || category.eq_ignore_ascii_case("work") {
            has_shift = true;
        }
        let exists: Option<String> = conn
            .query_row(
                "SELECT id FROM tasks WHERE rule_id = ?1 AND original_logical_date = ?2 LIMIT 1",
                params![&rule_id, logical_date],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        if exists.is_none() {
            let task_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO tasks
                 (id, logical_date, original_logical_date, rule_id, title, category, start_time, end_time, note, priority, status, reschedule_count, reminder_at, reminder_fired, updated_at, device_id, deleted_at)
                 VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'normal', 'pending', 0, NULL, 0, ?9, ?10, NULL)",
                params![task_id, logical_date, rule_id, title, category, start_time, end_time, note, now, device_id],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    // Ensure day_summaries row exists with accurate shift-day flag
    conn.execute(
        "INSERT OR IGNORE INTO day_summaries
         (logical_date, is_shift_day, frozen, shield_week_key, celebrated_json, updated_at, device_id)
         VALUES (?1, ?2, 0, NULL, '{}', ?3, ?4)",
        params![logical_date, if has_shift { 1 } else { 0 }, now, device_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn compute_day_score(
    tasks: &[TaskItem],
    prayers: &HashMap<String, bool>,
    focus_mins: i64,
    is_shift_day: bool,
    frozen: bool,
) -> DayScore {
    let prayers_done = PRAYERS
        .iter()
        .filter(|p| *prayers.get(**p).unwrap_or(&false))
        .count() as i64;
    let active_tasks: Vec<&TaskItem> = tasks
        .iter()
        .filter(|t| t.deleted_at.is_none() && t.status != "rescheduled")
        .collect();
    let tasks_total = active_tasks.len() as i64;
    let tasks_done = active_tasks.iter().filter(|t| t.status == "done").count() as i64;

    let prayer_score = (prayers_done as f64 / 5.0) * 45.0;
    let task_ratio = if tasks_total > 0 {
        tasks_done as f64 / tasks_total as f64
    } else {
        1.0
    };

    let total_pct = if is_shift_day {
        // V5/V6 Fair Shift-Day Balance: Prayers 45% + Tasks 55% + Focus Bonus up to +10% (capped at 100%)
        let task_score = task_ratio * 55.0;
        let focus_bonus = ((focus_mins as f64 / 90.0).min(1.0)) * 10.0;
        (prayer_score + task_score + focus_bonus).min(100.0)
    } else {
        // Off-day: Prayers 45% + Tasks 40% + Focus 15% (target 120 mins)
        let task_score = task_ratio * 40.0;
        let focus_score = ((focus_mins as f64 / 120.0).min(1.0)) * 15.0;
        (prayer_score + task_score + focus_score).min(100.0)
    };

    let qualified = frozen || total_pct >= 60.0;
    DayScore {
        total_pct: (total_pct * 10.0).round() / 10.0,
        prayers_done,
        tasks_done,
        tasks_total,
        focus_mins,
        is_shift_day,
        qualified,
    }
}

pub fn get_day_bundle(
    conn: &Connection,
    logical_date: &str,
    device_id: &str,
) -> Result<DayBundle, String> {
    materialize_day_rules(conn, logical_date, device_id)?;

    let mut t_stmt = conn
        .prepare(
            "SELECT id, logical_date, original_logical_date, rule_id, title, category, start_time, end_time,
                    note, priority, status, reschedule_count, reminder_at, reminder_fired, updated_at, device_id, deleted_at
             FROM tasks
             WHERE logical_date = ?1 AND deleted_at IS NULL
             ORDER BY COALESCE(start_time, '23:59') ASC, updated_at ASC",
        )
        .map_err(|e| e.to_string())?;

    let tasks: Vec<TaskItem> = t_stmt
        .query_map([logical_date], |r| {
            Ok(TaskItem {
                id: r.get(0)?,
                logical_date: r.get(1)?,
                original_logical_date: r.get(2)?,
                rule_id: r.get(3)?,
                title: r.get(4)?,
                category: r.get(5)?,
                start_time: r.get(6)?,
                end_time: r.get(7)?,
                note: r.get(8)?,
                priority: r.get(9)?,
                status: r.get(10)?,
                reschedule_count: r.get(11)?,
                reminder_at: r.get(12)?,
                reminder_fired: r.get::<_, i64>(13)? != 0,
                updated_at: r.get(14)?,
                device_id: r.get(15)?,
                deleted_at: r.get(16)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut prayers: HashMap<String, bool> = PRAYERS.iter().map(|p| (p.to_string(), false)).collect();
    let mut p_stmt = conn
        .prepare("SELECT prayer_key, completed FROM prayers_log WHERE logical_date = ?1")
        .map_err(|e| e.to_string())?;
    let p_rows = p_stmt
        .query_map([logical_date], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? != 0)))
        .map_err(|e| e.to_string())?;
    for row in p_rows {
        let (k, v) = row.map_err(|e| e.to_string())?;
        prayers.insert(k, v);
    }

    let summary = conn
        .query_row(
            "SELECT logical_date, is_shift_day, frozen, shield_week_key, celebrated_json, updated_at, device_id
             FROM day_summaries WHERE logical_date = ?1",
            [logical_date],
            |r| {
                Ok(DaySummary {
                    logical_date: r.get(0)?,
                    is_shift_day: r.get::<_, i64>(1)? != 0,
                    frozen: r.get::<_, i64>(2)? != 0,
                    shield_week_key: r.get(3)?,
                    celebrated_json: r.get(4)?,
                    updated_at: r.get(5)?,
                    device_id: r.get(6)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    let mut f_stmt = conn
        .prepare(
            "SELECT id, logical_date, started_at, ended_at, planned_mins, actual_mins, completed, checkpoint_json, updated_at, device_id, deleted_at
             FROM focus_sessions WHERE logical_date = ?1 AND deleted_at IS NULL ORDER BY started_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let focus_sessions: Vec<FocusSession> = f_stmt
        .query_map([logical_date], |r| {
            Ok(FocusSession {
                id: r.get(0)?,
                logical_date: r.get(1)?,
                started_at: r.get(2)?,
                ended_at: r.get(3)?,
                planned_mins: r.get(4)?,
                actual_mins: r.get(5)?,
                completed: r.get::<_, i64>(6)? != 0,
                checkpoint_json: r.get(7)?,
                updated_at: r.get(8)?,
                device_id: r.get(9)?,
                deleted_at: r.get(10)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let focus_mins: i64 = focus_sessions.iter().map(|s| s.actual_mins).sum();

    let mut d_stmt = conn
        .prepare(
            "SELECT id, logical_date, session_id, text, resolved, created_at, updated_at, device_id, deleted_at
             FROM distractions WHERE logical_date = ?1 AND deleted_at IS NULL ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let distractions: Vec<DistractionItem> = d_stmt
        .query_map([logical_date], |r| {
            Ok(DistractionItem {
                id: r.get(0)?,
                logical_date: r.get(1)?,
                session_id: r.get(2)?,
                text: r.get(3)?,
                resolved: r.get::<_, i64>(4)? != 0,
                created_at: r.get(5)?,
                updated_at: r.get(6)?,
                device_id: r.get(7)?,
                deleted_at: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let score = compute_day_score(&tasks, &prayers, focus_mins, summary.is_shift_day, summary.frozen);

    Ok(DayBundle {
        logical_date: logical_date.to_string(),
        is_extended_night: is_extended_night(),
        tasks,
        prayers,
        summary,
        score,
        focus_sessions,
        distractions,
    })
}

pub fn toggle_task_done(
    conn: &Connection,
    task_id: &str,
    done: bool,
    device_id: &str,
) -> Result<i64, String> {
    let now = now_epoch_ms();
    let status = if done { "done" } else { "pending" };
    conn.execute(
        "UPDATE tasks SET status = ?1, updated_at = ?2, device_id = ?3 WHERE id = ?4",
        params![status, now, device_id, task_id],
    )
    .map_err(|e| e.to_string())?;

    // Return count of completed tasks for that logical_date to drive 4-layer ring pitch
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks
             WHERE logical_date = (SELECT logical_date FROM tasks WHERE id = ?1)
               AND status = 'done' AND deleted_at IS NULL",
            [task_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count)
}

/// V6 Anti-Skip Rule: Rescheduling requires a target date and a non-trivial reason (>=3 chars).
pub fn reschedule_task(
    conn: &mut Connection,
    task_id: &str,
    to_date: &str,
    reason: &str,
    device_id: &str,
) -> Result<(), String> {
    if reason.trim().chars().count() < 3 {
        return Err("يجب كتابة سبب واضح للتأجيل (3 أحرف على الأقل)".into());
    }
    let now = now_epoch_ms();
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let from_date: String = tx
        .query_row("SELECT logical_date FROM tasks WHERE id = ?1", [task_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE tasks
         SET logical_date = ?1, status = 'pending', reschedule_count = reschedule_count + 1,
             updated_at = ?2, device_id = ?3
         WHERE id = ?4",
        params![to_date, now, device_id, task_id],
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO reschedule_events (id, task_id, from_date, to_date, reason, created_at, device_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![Uuid::new_v4().to_string(), task_id, from_date, to_date, reason.trim(), now, device_id],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())
}

/// V6 Anti-Skip Rule: Cancelling a task requires a structured excuse category + text (>=5 chars).
pub fn cancel_task_with_excuse(
    conn: &mut Connection,
    task_id: &str,
    excuse_category: &str,
    excuse_text: &str,
    device_id: &str,
) -> Result<(), String> {
    if excuse_text.trim().chars().count() < 5 {
        return Err("لا يمكن الإلغاء بدون عذر صريح (5 أحرف على الأقل)".into());
    }
    let now = now_epoch_ms();
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let logical_date: String = tx
        .query_row("SELECT logical_date FROM tasks WHERE id = ?1", [task_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE tasks SET status = 'cancelled', updated_at = ?1, device_id = ?2 WHERE id = ?3",
        params![now, device_id, task_id],
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO cancellation_excuses (id, task_id, logical_date, excuse_category, excuse_text, created_at, device_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![Uuid::new_v4().to_string(), task_id, logical_date, excuse_category, excuse_text.trim(), now, device_id],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())
}

pub fn toggle_prayer(
    conn: &Connection,
    logical_date: &str,
    prayer_key: &str,
    completed: bool,
    device_id: &str,
) -> Result<(), String> {
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO prayers_log (logical_date, prayer_key, completed, updated_at, device_id)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(logical_date, prayer_key) DO UPDATE SET
           completed = excluded.completed,
           updated_at = excluded.updated_at,
           device_id = excluded.device_id",
        params![logical_date, prayer_key, if completed { 1 } else { 0 }, now, device_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn toggle_weekly_shield(
    conn: &Connection,
    logical_date: &str,
    freeze: bool,
    device_id: &str,
) -> Result<(), String> {
    let wk = week_key_for_date(logical_date)?;
    if freeze {
        let already: Option<String> = conn
            .query_row(
                "SELECT logical_date FROM day_summaries WHERE shield_week_key = ?1 AND frozen = 1 AND logical_date != ?2 LIMIT 1",
                params![&wk, logical_date],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(used_on) = already {
            return Err(format!("درع الإجازة الأسبوعي مستخدم بالفعل في {}", used_on));
        }
    }
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO day_summaries (logical_date, is_shift_day, frozen, shield_week_key, celebrated_json, updated_at, device_id)
         VALUES (?1, 0, ?2, ?3, '{}', ?4, ?5)
         ON CONFLICT(logical_date) DO UPDATE SET
           frozen = excluded.frozen,
           shield_week_key = excluded.shield_week_key,
           updated_at = excluded.updated_at,
           device_id = excluded.device_id",
        params![logical_date, if freeze { 1 } else { 0 }, if freeze { Some(wk) } else { None }, now, device_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_monthly_telemetry(
    conn: &Connection,
    month_prefix: &str,
    device_id: &str,
) -> Result<MonthlyTelemetry, String> {
    let like_pat = format!("{}%", month_prefix);

    let mut d_stmt = conn
        .prepare("SELECT logical_date FROM day_summaries WHERE logical_date LIKE ?1 ORDER BY logical_date ASC")
        .map_err(|e| e.to_string())?;
    let dates: Vec<String> = d_stmt
        .query_map([&like_pat], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut daily_scores = HashMap::new();
    let mut sum_all = 0.0;
    let mut sum_shift = 0.0;
    let mut cnt_shift = 0;
    let mut sum_off = 0.0;
    let mut cnt_off = 0;
    let mut prayers_completed = 0;
    let mut tasks_done = 0;
    let mut tasks_cancelled = 0;
    let mut total_focus_mins = 0;

    for d in &dates {
        if let Ok(bundle) = get_day_bundle(conn, d, device_id) {
            daily_scores.insert(d.clone(), bundle.score.total_pct);
            sum_all += bundle.score.total_pct;
            if bundle.summary.is_shift_day {
                sum_shift += bundle.score.total_pct;
                cnt_shift += 1;
            } else {
                sum_off += bundle.score.total_pct;
                cnt_off += 1;
            }
            prayers_completed += bundle.score.prayers_done;
            tasks_done += bundle.score.tasks_done;
            tasks_cancelled += bundle.tasks.iter().filter(|t| t.status == "cancelled").count() as i64;
            total_focus_mins += bundle.score.focus_mins;
        }
    }

    let tasks_rescheduled: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM reschedule_events WHERE from_date LIKE ?1",
            [&like_pat],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let mut excuse_breakdown = HashMap::new();
    let mut e_stmt = conn
        .prepare(
            "SELECT excuse_category, COUNT(*) FROM cancellation_excuses
             WHERE logical_date LIKE ?1 GROUP BY excuse_category",
        )
        .map_err(|e| e.to_string())?;
    let e_rows = e_stmt
        .query_map([&like_pat], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(|e| e.to_string())?;
    for row in e_rows {
        let (cat, cnt) = row.map_err(|e| e.to_string())?;
        excuse_breakdown.insert(cat, cnt);
    }

    let days_logged = dates.len() as i64;
    Ok(MonthlyTelemetry {
        month_prefix: month_prefix.to_string(),
        days_logged,
        avg_score_pct: if days_logged > 0 { (sum_all / days_logged as f64 * 10.0).round() / 10.0 } else { 0.0 },
        shift_days_avg_pct: if cnt_shift > 0 { (sum_shift / cnt_shift as f64 * 10.0).round() / 10.0 } else { 0.0 },
        off_days_avg_pct: if cnt_off > 0 { (sum_off / cnt_off as f64 * 10.0).round() / 10.0 } else { 0.0 },
        prayers_completed,
        prayers_possible: days_logged * 5,
        tasks_done,
        tasks_rescheduled,
        tasks_cancelled,
        total_focus_mins,
        excuse_breakdown,
        daily_scores,
    })
}

pub fn export_sync_snapshot(conn: &Connection, device_id: &str) -> Result<SyncSnapshot, String> {
    let app_meta = conn
        .prepare("SELECT key, value_json, updated_at, device_id FROM app_meta WHERE key NOT IN ('device_id','last_sync_etag')")
        .map_err(|e| e.to_string())?
        .query_map([], |r| {
            Ok(AppMetaRow {
                key: r.get(0)?,
                value_json: r.get(1)?,
                updated_at: r.get(2)?,
                device_id: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let recurrence_rules = conn
        .prepare("SELECT id, weekday, title, category, start_time, end_time, prayer_slot, note, is_work_shift, sort_order, active, updated_at, device_id, deleted_at FROM recurrence_rules")
        .map_err(|e| e.to_string())?
        .query_map([], |r| {
            Ok(RecurrenceRule {
                id: r.get(0)?, weekday: r.get(1)?, title: r.get(2)?, category: r.get(3)?,
                start_time: r.get(4)?, end_time: r.get(5)?, prayer_slot: r.get(6)?, note: r.get(7)?,
                is_work_shift: r.get::<_, i64>(8)? != 0, sort_order: r.get(9)?, active: r.get::<_, i64>(10)? != 0,
                updated_at: r.get(11)?, device_id: r.get(12)?, deleted_at: r.get(13)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let tasks = conn
        .prepare("SELECT id, logical_date, original_logical_date, rule_id, title, category, start_time, end_time, note, priority, status, reschedule_count, reminder_at, reminder_fired, updated_at, device_id, deleted_at FROM tasks")
        .map_err(|e| e.to_string())?
        .query_map([], |r| {
            Ok(TaskItem {
                id: r.get(0)?, logical_date: r.get(1)?, original_logical_date: r.get(2)?, rule_id: r.get(3)?,
                title: r.get(4)?, category: r.get(5)?, start_time: r.get(6)?, end_time: r.get(7)?,
                note: r.get(8)?, priority: r.get(9)?, status: r.get(10)?, reschedule_count: r.get(11)?,
                reminder_at: r.get(12)?, reminder_fired: r.get::<_, i64>(13)? != 0,
                updated_at: r.get(14)?, device_id: r.get(15)?, deleted_at: r.get(16)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let reschedule_events = conn
        .prepare("SELECT id, task_id, from_date, to_date, reason, created_at, device_id FROM reschedule_events")
        .map_err(|e| e.to_string())?
        .query_map([], |r| {
            Ok(RescheduleEvent {
                id: r.get(0)?, task_id: r.get(1)?, from_date: r.get(2)?, to_date: r.get(3)?,
                reason: r.get(4)?, created_at: r.get(5)?, device_id: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let cancellation_excuses = conn
        .prepare("SELECT id, task_id, logical_date, excuse_category, excuse_text, created_at, device_id FROM cancellation_excuses")
        .map_err(|e| e.to_string())?
        .query_map([], |r| {
            Ok(CancellationExcuse {
                id: r.get(0)?, task_id: r.get(1)?, logical_date: r.get(2)?,
                excuse_category: r.get(3)?, excuse_text: r.get(4)?, created_at: r.get(5)?, device_id: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let prayers_log = conn
        .prepare("SELECT logical_date, prayer_key, completed, updated_at, device_id FROM prayers_log")
        .map_err(|e| e.to_string())?
        .query_map([], |r| {
            Ok(PrayerLogRow {
                logical_date: r.get(0)?, prayer_key: r.get(1)?, completed: r.get::<_, i64>(2)? != 0,
                updated_at: r.get(3)?, device_id: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let day_summaries = conn
        .prepare("SELECT logical_date, is_shift_day, frozen, shield_week_key, celebrated_json, updated_at, device_id FROM day_summaries")
        .map_err(|e| e.to_string())?
        .query_map([], |r| {
            Ok(DaySummary {
                logical_date: r.get(0)?, is_shift_day: r.get::<_, i64>(1)? != 0, frozen: r.get::<_, i64>(2)? != 0,
                shield_week_key: r.get(3)?, celebrated_json: r.get(4)?, updated_at: r.get(5)?, device_id: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let focus_sessions = conn
        .prepare("SELECT id, logical_date, started_at, ended_at, planned_mins, actual_mins, completed, checkpoint_json, updated_at, device_id, deleted_at FROM focus_sessions")
        .map_err(|e| e.to_string())?
        .query_map([], |r| {
            Ok(FocusSession {
                id: r.get(0)?, logical_date: r.get(1)?, started_at: r.get(2)?, ended_at: r.get(3)?,
                planned_mins: r.get(4)?, actual_mins: r.get(5)?, completed: r.get::<_, i64>(6)? != 0,
                checkpoint_json: r.get(7)?, updated_at: r.get(8)?, device_id: r.get(9)?, deleted_at: r.get(10)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let distractions = conn
        .prepare("SELECT id, logical_date, session_id, text, resolved, created_at, updated_at, device_id, deleted_at FROM distractions")
        .map_err(|e| e.to_string())?
        .query_map([], |r| {
            Ok(DistractionItem {
                id: r.get(0)?, logical_date: r.get(1)?, session_id: r.get(2)?, text: r.get(3)?,
                resolved: r.get::<_, i64>(4)? != 0, created_at: r.get(5)?, updated_at: r.get(6)?,
                device_id: r.get(7)?, deleted_at: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let notes = conn
        .prepare("SELECT id, logical_date, title, body_md, tags_json, pinned, created_at, updated_at, device_id, deleted_at FROM notes")
        .map_err(|e| e.to_string())?
        .query_map([], |r| {
            Ok(NoteItem {
                id: r.get(0)?, logical_date: r.get(1)?, title: r.get(2)?, body_md: r.get(3)?,
                tags_json: r.get(4)?, pinned: r.get::<_, i64>(5)? != 0, created_at: r.get(6)?,
                updated_at: r.get(7)?, device_id: r.get(8)?, deleted_at: r.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let quotes = conn
        .prepare("SELECT id, category, text, source, is_builtin, is_favorite, updated_at, device_id, deleted_at FROM quotes")
        .map_err(|e| e.to_string())?
        .query_map([], |r| {
            Ok(QuoteItem {
                id: r.get(0)?, category: r.get(1)?, text: r.get(2)?, source: r.get(3)?,
                is_builtin: r.get::<_, i64>(4)? != 0, is_favorite: r.get::<_, i64>(5)? != 0,
                updated_at: r.get(6)?, device_id: r.get(7)?, deleted_at: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(SyncSnapshot {
        schema_version: 6,
        exported_at: now_epoch_ms(),
        device_id: device_id.to_string(),
        app_meta,
        recurrence_rules,
        tasks,
        reschedule_events,
        cancellation_excuses,
        prayers_log,
        day_summaries,
        focus_sessions,
        distractions,
        notes,
        quotes,
    })
}
