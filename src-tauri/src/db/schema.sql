-- Abdullah Masar V6 — Normalized Offline-First SQLite Schema
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS app_meta (
  key         TEXT PRIMARY KEY,
  value_json  TEXT NOT NULL,
  updated_at  INTEGER NOT NULL,
  device_id   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS recurrence_rules (
  id            TEXT PRIMARY KEY,
  weekday       INTEGER NOT NULL CHECK (weekday BETWEEN 0 AND 6),
  title         TEXT NOT NULL,
  category      TEXT NOT NULL,
  start_time    TEXT NOT NULL,
  end_time      TEXT,
  prayer_slot   TEXT,
  note          TEXT,
  is_work_shift INTEGER NOT NULL DEFAULT 0,
  sort_order    INTEGER NOT NULL DEFAULT 0,
  active        INTEGER NOT NULL DEFAULT 1,
  updated_at    INTEGER NOT NULL,
  device_id     TEXT NOT NULL,
  deleted_at    INTEGER
);
CREATE INDEX IF NOT EXISTS idx_rules_weekday ON recurrence_rules(weekday, active, deleted_at);

CREATE TABLE IF NOT EXISTS tasks (
  id                    TEXT PRIMARY KEY,
  logical_date          TEXT NOT NULL,
  original_logical_date TEXT NOT NULL,
  rule_id               TEXT REFERENCES recurrence_rules(id) ON DELETE SET NULL,
  title                 TEXT NOT NULL,
  category              TEXT NOT NULL,
  start_time            TEXT,
  end_time              TEXT,
  note                  TEXT,
  priority              TEXT NOT NULL DEFAULT 'normal',
  status                TEXT NOT NULL DEFAULT 'pending'
                        CHECK (status IN ('pending','done','rescheduled','cancelled')),
  reschedule_count      INTEGER NOT NULL DEFAULT 0,
  reminder_at           TEXT,
  reminder_fired        INTEGER NOT NULL DEFAULT 0,
  updated_at            INTEGER NOT NULL,
  device_id             TEXT NOT NULL,
  deleted_at            INTEGER
);
CREATE INDEX IF NOT EXISTS idx_tasks_date ON tasks(logical_date, deleted_at);
CREATE INDEX IF NOT EXISTS idx_tasks_rule_date ON tasks(rule_id, logical_date);

CREATE TABLE IF NOT EXISTS reschedule_events (
  id          TEXT PRIMARY KEY,
  task_id     TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  from_date   TEXT NOT NULL,
  to_date     TEXT NOT NULL,
  reason      TEXT NOT NULL CHECK (length(trim(reason)) >= 3),
  created_at  INTEGER NOT NULL,
  device_id   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_reschedule_task ON reschedule_events(task_id);
CREATE INDEX IF NOT EXISTS idx_reschedule_dates ON reschedule_events(from_date, to_date);

CREATE TABLE IF NOT EXISTS cancellation_excuses (
  id              TEXT PRIMARY KEY,
  task_id         TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  logical_date    TEXT NOT NULL,
  excuse_category TEXT NOT NULL,
  excuse_text     TEXT NOT NULL CHECK (length(trim(excuse_text)) >= 5),
  created_at      INTEGER NOT NULL,
  device_id       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_excuses_date ON cancellation_excuses(logical_date);
CREATE INDEX IF NOT EXISTS idx_excuses_cat ON cancellation_excuses(excuse_category);

CREATE TABLE IF NOT EXISTS prayers_log (
  logical_date TEXT NOT NULL,
  prayer_key   TEXT NOT NULL CHECK (prayer_key IN ('fajr','dhuhr','asr','maghrib','isha')),
  completed    INTEGER NOT NULL DEFAULT 0,
  updated_at   INTEGER NOT NULL,
  device_id    TEXT NOT NULL,
  PRIMARY KEY (logical_date, prayer_key)
);

CREATE TABLE IF NOT EXISTS day_summaries (
  logical_date    TEXT PRIMARY KEY,
  is_shift_day    INTEGER NOT NULL DEFAULT 0,
  frozen          INTEGER NOT NULL DEFAULT 0,
  shield_week_key TEXT,
  celebrated_json TEXT NOT NULL DEFAULT '{}',
  updated_at      INTEGER NOT NULL,
  device_id       TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS focus_sessions (
  id              TEXT PRIMARY KEY,
  logical_date    TEXT NOT NULL,
  started_at      INTEGER NOT NULL,
  ended_at        INTEGER,
  planned_mins    INTEGER NOT NULL,
  actual_mins     INTEGER NOT NULL DEFAULT 0,
  completed       INTEGER NOT NULL DEFAULT 0,
  checkpoint_json TEXT,
  updated_at      INTEGER NOT NULL,
  device_id       TEXT NOT NULL,
  deleted_at      INTEGER
);
CREATE INDEX IF NOT EXISTS idx_focus_date ON focus_sessions(logical_date, deleted_at);

CREATE TABLE IF NOT EXISTS distractions (
  id           TEXT PRIMARY KEY,
  logical_date TEXT NOT NULL,
  session_id   TEXT REFERENCES focus_sessions(id) ON DELETE SET NULL,
  text         TEXT NOT NULL,
  resolved     INTEGER NOT NULL DEFAULT 0,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL,
  device_id    TEXT NOT NULL,
  deleted_at   INTEGER
);
CREATE INDEX IF NOT EXISTS idx_distractions_date ON distractions(logical_date, deleted_at);

CREATE TABLE IF NOT EXISTS notes (
  id           TEXT PRIMARY KEY,
  logical_date TEXT,
  title        TEXT NOT NULL,
  body_md      TEXT NOT NULL DEFAULT '',
  tags_json    TEXT NOT NULL DEFAULT '[]',
  pinned       INTEGER NOT NULL DEFAULT 0,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL,
  device_id    TEXT NOT NULL,
  deleted_at   INTEGER
);
CREATE INDEX IF NOT EXISTS idx_notes_pinned ON notes(pinned DESC, updated_at DESC);

CREATE TABLE IF NOT EXISTS quotes (
  id          TEXT PRIMARY KEY,
  category    TEXT NOT NULL,
  text        TEXT NOT NULL,
  source      TEXT NOT NULL DEFAULT '',
  is_builtin  INTEGER NOT NULL DEFAULT 0,
  is_favorite INTEGER NOT NULL DEFAULT 0,
  updated_at  INTEGER NOT NULL,
  device_id   TEXT NOT NULL,
  deleted_at  INTEGER
);
CREATE INDEX IF NOT EXISTS idx_quotes_cat ON quotes(category, deleted_at);
