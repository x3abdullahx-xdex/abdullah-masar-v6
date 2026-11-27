use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMetaRow {
    pub key: String,
    pub value_json: String,
    pub updated_at: i64,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurrenceRule {
    pub id: String,
    pub weekday: i64,
    pub title: String,
    pub category: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub prayer_slot: Option<String>,
    pub note: Option<String>,
    pub is_work_shift: bool,
    pub sort_order: i64,
    pub active: bool,
    pub updated_at: i64,
    pub device_id: String,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub logical_date: String,
    pub original_logical_date: String,
    pub rule_id: Option<String>,
    pub title: String,
    pub category: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub note: Option<String>,
    pub priority: String,
    pub status: String, // pending | done | rescheduled | cancelled
    pub reschedule_count: i64,
    pub reminder_at: Option<String>,
    pub reminder_fired: bool,
    pub updated_at: i64,
    pub device_id: String,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RescheduleEvent {
    pub id: String,
    pub task_id: String,
    pub from_date: String,
    pub to_date: String,
    pub reason: String,
    pub created_at: i64,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancellationExcuse {
    pub id: String,
    pub task_id: String,
    pub logical_date: String,
    pub excuse_category: String,
    pub excuse_text: String,
    pub created_at: i64,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrayerLogRow {
    pub logical_date: String,
    pub prayer_key: String,
    pub completed: bool,
    pub updated_at: i64,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaySummary {
    pub logical_date: String,
    pub is_shift_day: bool,
    pub frozen: bool,
    pub shield_week_key: Option<String>,
    pub celebrated_json: String,
    pub updated_at: i64,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusSession {
    pub id: String,
    pub logical_date: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub planned_mins: i64,
    pub actual_mins: i64,
    pub completed: bool,
    pub checkpoint_json: Option<String>,
    pub updated_at: i64,
    pub device_id: String,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistractionItem {
    pub id: String,
    pub logical_date: String,
    pub session_id: Option<String>,
    pub text: String,
    pub resolved: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub device_id: String,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteItem {
    pub id: String,
    pub logical_date: Option<String>,
    pub title: String,
    pub body_md: String,
    pub tags_json: String,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub device_id: String,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteItem {
    pub id: String,
    pub category: String,
    pub text: String,
    pub source: String,
    pub is_builtin: bool,
    pub is_favorite: bool,
    pub updated_at: i64,
    pub device_id: String,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayScore {
    pub total_pct: f64,
    pub prayers_done: i64,
    pub tasks_done: i64,
    pub tasks_total: i64,
    pub focus_mins: i64,
    pub is_shift_day: bool,
    pub qualified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayBundle {
    pub logical_date: String,
    pub is_extended_night: bool,
    pub tasks: Vec<TaskItem>,
    pub prayers: HashMap<String, bool>,
    pub summary: DaySummary,
    pub score: DayScore,
    pub focus_sessions: Vec<FocusSession>,
    pub distractions: Vec<DistractionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyTelemetry {
    pub month_prefix: String, // 'YYYY-MM'
    pub days_logged: i64,
    pub avg_score_pct: f64,
    pub shift_days_avg_pct: f64,
    pub off_days_avg_pct: f64,
    pub prayers_completed: i64,
    pub prayers_possible: i64,
    pub tasks_done: i64,
    pub tasks_rescheduled: i64,
    pub tasks_cancelled: i64,
    pub total_focus_mins: i64,
    pub excuse_breakdown: HashMap<String, i64>,
    pub daily_scores: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSnapshot {
    pub schema_version: i64,
    pub exported_at: i64,
    pub device_id: String,
    pub app_meta: Vec<AppMetaRow>,
    pub recurrence_rules: Vec<RecurrenceRule>,
    pub tasks: Vec<TaskItem>,
    pub reschedule_events: Vec<RescheduleEvent>,
    pub cancellation_excuses: Vec<CancellationExcuse>,
    pub prayers_log: Vec<PrayerLogRow>,
    pub day_summaries: Vec<DaySummary>,
    pub focus_sessions: Vec<FocusSession>,
    pub distractions: Vec<DistractionItem>,
    pub notes: Vec<NoteItem>,
    pub quotes: Vec<QuoteItem>,
}
