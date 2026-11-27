use chrono::{Datelike, Duration, Local, NaiveDate, Timelike};

/// Authoritative 4:00 AM logical day cutoff.
/// Between 00:00 and 03:59 local time, the logical day still belongs to yesterday.
pub const LOGICAL_DAY_CUTOFF_HOUR: i64 = 4;

pub fn now_epoch_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn logical_today_date() -> NaiveDate {
    let local_now = Local::now();
    (local_now - Duration::hours(LOGICAL_DAY_CUTOFF_HOUR)).date_naive()
}

pub fn logical_today_key() -> String {
    logical_today_date().format("%Y-%m-%d").to_string()
}

/// True between 00:00 and 03:59 local time (shows "🌙 امتداد يوم [...]" badge).
pub fn is_extended_night() -> bool {
    Local::now().hour() < LOGICAL_DAY_CUTOFF_HOUR as u32
}

/// Returns 0=Sunday .. 6=Saturday to match JS Date.getDay() and V5 schedule keys.
pub fn weekday_sun0(date_str: &str) -> Result<i64, String> {
    let d = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date '{}': {}", date_str, e))?;
    Ok(d.weekday().num_days_from_sunday() as i64)
}

/// ISO-like week key anchored to Saturday/Sunday for Weekly Excuse Shield tracking.
pub fn week_key_for_date(date_str: &str) -> Result<String, String> {
    let d = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date '{}': {}", date_str, e))?;
    let iso = d.iso_week();
    Ok(format!("{}-W{:02}", iso.year(), iso.week()))
}
