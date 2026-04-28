use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDate, TimeZone};
use rusqlite::{params, Connection};
use std::path::PathBuf;

use crate::models::{
    Energy, Event, FocusSession, Goal, Habit, HabitLog, HabitLogStatus, HabitType,
    HabitWithStreak, InboxItem, JournalEntry, NewTask, Note, Priority, Task, TaskStatus,
};

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open() -> Result<Self> {
        let path = db_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)
            .with_context(|| format!("Failed to open database at {:?}", path))?;
        let db = Database { conn };
        db.initialize()?;
        Ok(db)
    }

    fn initialize(&self) -> Result<()> {
        self.conn.execute_batch(SCHEMA)?;
        // WAL mode for crash safety
        self.conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(())
    }

    // ── Tasks ──────────────────────────────────────────────────────────────────

    pub fn create_task(&self, t: &NewTask) -> Result<i64> {
        let now = Local::now();
        let due = t.due_at.map(|d| d.timestamp());
        self.conn.execute(
            "INSERT INTO task (title, description, status, priority, energy, estimate_min,
                               due_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                t.title,
                t.description,
                "open",
                t.priority.to_i64(),
                t.energy.label(),
                t.estimate_min,
                due,
                now.timestamp(),
                now.timestamp(),
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        for tag in &t.tags {
            self.conn.execute(
                "INSERT OR IGNORE INTO task_tag (task_id, tag) VALUES (?1, ?2)",
                params![id, tag],
            )?;
        }
        Ok(id)
    }

    pub fn all_tasks(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, status, priority, energy, estimate_min,
                    due_at, start_at, parent_id, project_id, created_at, updated_at, completed_at
             FROM task WHERE status NOT IN ('archived','cancelled')
             ORDER BY COALESCE(due_at, 9999999999), priority, created_at",
        )?;
        let tasks: Vec<Task> = stmt
            .query_map([], |row| row_to_task(row))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(self.attach_tags(tasks)?)
    }

    pub fn tasks_due_today(&self) -> Result<Vec<Task>> {
        let today_start = today_start_ts();
        let today_end = today_end_ts();
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, status, priority, energy, estimate_min,
                    due_at, start_at, parent_id, project_id, created_at, updated_at, completed_at
             FROM task
             WHERE status NOT IN ('archived','cancelled','done')
               AND (due_at BETWEEN ?1 AND ?2 OR due_at IS NULL)
             ORDER BY priority, COALESCE(due_at, 9999999999)",
        )?;
        let tasks: Vec<Task> = stmt
            .query_map(params![today_start, today_end], |row| row_to_task(row))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(self.attach_tags(tasks)?)
    }

    pub fn tasks_for_date(&self, date: NaiveDate) -> Result<Vec<Task>> {
        let start = date.and_hms_opt(0, 0, 0).unwrap();
        let end = date.and_hms_opt(23, 59, 59).unwrap();
        let start_ts = Local
            .from_local_datetime(&start)
            .single()
            .map(|d| d.timestamp())
            .unwrap_or(0);
        let end_ts = Local
            .from_local_datetime(&end)
            .single()
            .map(|d| d.timestamp())
            .unwrap_or(i64::MAX);
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, status, priority, energy, estimate_min,
                    due_at, start_at, parent_id, project_id, created_at, updated_at, completed_at
             FROM task
             WHERE status NOT IN ('archived','cancelled')
               AND due_at BETWEEN ?1 AND ?2
             ORDER BY priority, due_at",
        )?;
        let tasks: Vec<Task> = stmt
            .query_map(params![start_ts, end_ts], |row| row_to_task(row))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(self.attach_tags(tasks)?)
    }

    pub fn complete_task(&self, id: i64) -> Result<()> {
        let now = Local::now().timestamp();
        self.conn.execute(
            "UPDATE task SET status='done', completed_at=?1, updated_at=?1 WHERE id=?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn reopen_task(&self, id: i64) -> Result<()> {
        let now = Local::now().timestamp();
        self.conn.execute(
            "UPDATE task SET status='open', completed_at=NULL, updated_at=?1 WHERE id=?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn delete_task(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM task_tag WHERE task_id=?1", params![id])?;
        self.conn.execute("DELETE FROM task WHERE id=?1", params![id])?;
        Ok(())
    }

    fn attach_tags(&self, tasks: Vec<Task>) -> Result<Vec<Task>> {
        let mut result = tasks;
        for task in &mut result {
            let mut stmt = self.conn.prepare(
                "SELECT tag FROM task_tag WHERE task_id=?1 ORDER BY tag",
            )?;
            task.tags = stmt
                .query_map(params![task.id], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
        }
        Ok(result)
    }

    // ── Events ─────────────────────────────────────────────────────────────────

    pub fn events_for_date(&self, date: NaiveDate) -> Result<Vec<Event>> {
        let start = date.and_hms_opt(0, 0, 0).unwrap();
        let end = date.and_hms_opt(23, 59, 59).unwrap();
        let start_ts = Local.from_local_datetime(&start).single().map(|d| d.timestamp()).unwrap_or(0);
        let end_ts = Local.from_local_datetime(&end).single().map(|d| d.timestamp()).unwrap_or(i64::MAX);
        let mut stmt = self.conn.prepare(
            "SELECT id, title, starts_at, ends_at, all_day, calendar, recurrence, notes
             FROM event WHERE starts_at BETWEEN ?1 AND ?2 ORDER BY starts_at",
        )?;
        let result: Vec<Event> = stmt
            .query_map(params![start_ts, end_ts], |row| row_to_event(row))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(result)
    }

    pub fn create_event(&self, title: &str, starts_at: DateTime<Local>, ends_at: DateTime<Local>, calendar: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO event (title, starts_at, ends_at, all_day, calendar)
             VALUES (?1, ?2, ?3, 0, ?4)",
            params![title, starts_at.timestamp(), ends_at.timestamp(), calendar],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    // ── Habits ─────────────────────────────────────────────────────────────────

    pub fn all_habits(&self) -> Result<Vec<Habit>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, cadence, type, target, domain, color FROM habit ORDER BY name",
        )?;
        let result: Vec<Habit> = stmt
            .query_map([], |row| {
                Ok(Habit {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    cadence: row.get(2)?,
                    habit_type: HabitType::from_str(&row.get::<_, String>(3)?),
                    target: row.get(4)?,
                    domain: row.get(5)?,
                    color: row.get(6)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(result)
    }

    pub fn habit_log_for_date(&self, habit_id: i64, date: NaiveDate) -> Result<Option<HabitLog>> {
        let date_str = date.format("%Y-%m-%d").to_string();
        self.conn
            .query_row(
                "SELECT habit_id, date, value, status FROM habit_log WHERE habit_id=?1 AND date=?2",
                params![habit_id, date_str],
                |row| {
                    Ok(HabitLog {
                        habit_id: row.get(0)?,
                        date,
                        value: row.get(2)?,
                        status: HabitLogStatus::from_str(&row.get::<_, String>(3)?),
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn log_habit(&self, habit_id: i64, date: NaiveDate, value: f64) -> Result<()> {
        let date_str = date.format("%Y-%m-%d").to_string();
        self.conn.execute(
            "INSERT OR REPLACE INTO habit_log (habit_id, date, value, status)
             VALUES (?1, ?2, ?3, 'done')",
            params![habit_id, date_str, value],
        )?;
        Ok(())
    }

    pub fn habit_streak(&self, habit_id: i64) -> Result<(i64, i64)> {
        // Returns (current_streak, personal_best)
        let date_str_col: Vec<String> = {
            let mut stmt = self.conn.prepare(
                "SELECT date FROM habit_log WHERE habit_id=?1 AND status='done' ORDER BY date DESC",
            )?;
            let rows: Vec<String> = stmt.query_map(params![habit_id], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            rows
        };

        let dates: Vec<NaiveDate> = date_str_col
            .iter()
            .filter_map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .collect();

        let streak = compute_streak(&dates);
        let pb = compute_personal_best(&dates);
        Ok((streak, pb))
    }

    pub fn habits_with_streaks(&self, date: NaiveDate) -> Result<Vec<HabitWithStreak>> {
        let habits = self.all_habits()?;
        let mut result = Vec::new();
        for habit in habits {
            let (streak, pb) = self.habit_streak(habit.id)?;
            let today_log = self.habit_log_for_date(habit.id, date)?;
            result.push(HabitWithStreak {
                habit,
                streak,
                personal_best: pb,
                today_log,
            });
        }
        Ok(result)
    }

    pub fn create_habit(&self, name: &str, cadence: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO habit (name, cadence, type) VALUES (?1, ?2, 'bool')",
            params![name, cadence],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    // ── Goals ──────────────────────────────────────────────────────────────────

    pub fn goals_for_horizon(&self, horizon: &str) -> Result<Vec<Goal>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, horizon, period, progress, notes, parent_id
             FROM goal WHERE horizon=?1 ORDER BY progress DESC",
        )?;
        let result: Vec<Goal> = stmt
            .query_map(params![horizon], |row| {
                Ok(Goal {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    horizon: row.get(2)?,
                    period: row.get(3)?,
                    progress: row.get(4)?,
                    notes: row.get(5)?,
                    parent_id: row.get(6)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(result)
    }

    pub fn create_goal(&self, title: &str, horizon: &str, period: Option<&str>) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO goal (title, horizon, period, progress) VALUES (?1, ?2, ?3, 0)",
            params![title, horizon, period],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_goal_progress(&self, id: i64, progress: f64) -> Result<()> {
        self.conn.execute(
            "UPDATE goal SET progress=?1 WHERE id=?2",
            params![progress, id],
        )?;
        Ok(())
    }

    // ── Journal ────────────────────────────────────────────────────────────────

    pub fn journal_for_date(&self, date: NaiveDate) -> Result<Option<JournalEntry>> {
        let date_str = date.format("%Y-%m-%d").to_string();
        self.conn
            .query_row(
                "SELECT date, mood, energy, focus, sleep_h, body_md FROM journal WHERE date=?1",
                params![date_str],
                |row| {
                    Ok(JournalEntry {
                        date,
                        mood: row.get(1)?,
                        energy: row.get(2)?,
                        focus: row.get(3)?,
                        sleep_h: row.get(4)?,
                        body_md: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn upsert_journal(&self, entry: &JournalEntry) -> Result<()> {
        let date_str = entry.date.format("%Y-%m-%d").to_string();
        self.conn.execute(
            "INSERT OR REPLACE INTO journal (date, mood, energy, focus, sleep_h, body_md)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![date_str, entry.mood, entry.energy, entry.focus, entry.sleep_h, entry.body_md],
        )?;
        Ok(())
    }

    // ── Focus Sessions ─────────────────────────────────────────────────────────

    pub fn start_focus_session(&self, task_id: Option<i64>, kind: &str) -> Result<i64> {
        let now = Local::now().timestamp();
        self.conn.execute(
            "INSERT INTO focus_session (task_id, started_at, kind, distractions)
             VALUES (?1, ?2, ?3, 0)",
            params![task_id, now, kind],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn end_focus_session(&self, id: i64, distractions: i64) -> Result<()> {
        let now = Local::now().timestamp();
        self.conn.execute(
            "UPDATE focus_session SET ended_at=?1, distractions=?2 WHERE id=?3",
            params![now, distractions, id],
        )?;
        Ok(())
    }

    pub fn focus_minutes_today(&self) -> Result<i64> {
        let start = today_start_ts();
        let end = today_end_ts();
        let mins: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM((COALESCE(ended_at, strftime('%s','now')) - started_at) / 60), 0)
             FROM focus_session WHERE started_at BETWEEN ?1 AND ?2",
            params![start, end],
            |row| row.get(0),
        )?;
        Ok(mins)
    }

    pub fn focus_minutes_for_week(&self) -> Result<Vec<i64>> {
        use chrono::{Datelike, Duration};
        let today = Local::now().date_naive();
        let weekday_num = today.weekday().num_days_from_monday() as i64;
        let monday = today - Duration::days(weekday_num);
        let mut result = Vec::new();
        for i in 0..7 {
            let day = monday + Duration::days(i);
            let start = Local
                .from_local_datetime(&day.and_hms_opt(0, 0, 0).unwrap())
                .single()
                .map(|d| d.timestamp())
                .unwrap_or(0);
            let end = Local
                .from_local_datetime(&day.and_hms_opt(23, 59, 59).unwrap())
                .single()
                .map(|d| d.timestamp())
                .unwrap_or(i64::MAX);
            let mins: i64 = self.conn.query_row(
                "SELECT COALESCE(SUM((COALESCE(ended_at, started_at) - started_at) / 60), 0)
                 FROM focus_session WHERE started_at BETWEEN ?1 AND ?2",
                params![start, end],
                |row| row.get(0),
            )?;
            result.push(mins);
        }
        Ok(result)
    }

    // ── Notes ──────────────────────────────────────────────────────────────────

    pub fn all_notes(&self) -> Result<Vec<Note>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, slug, title, vault_path, created_at, updated_at FROM note ORDER BY updated_at DESC",
        )?;
        let result: Vec<Note> = stmt
            .query_map([], |row| {
                let crt: i64 = row.get(4)?;
                let upd: i64 = row.get(5)?;
                Ok(Note {
                    id: row.get(0)?,
                    slug: row.get(1)?,
                    title: row.get(2)?,
                    vault_path: row.get(3)?,
                    created_at: Local.timestamp_opt(crt, 0).single().unwrap_or_else(Local::now),
                    updated_at: Local.timestamp_opt(upd, 0).single().unwrap_or_else(Local::now),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(result)
    }

    pub fn create_note(&self, slug: &str, title: &str, vault_path: &str) -> Result<i64> {
        let now = Local::now().timestamp();
        self.conn.execute(
            "INSERT INTO note (slug, title, vault_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![slug, title, vault_path, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    // ── Inbox ──────────────────────────────────────────────────────────────────

    pub fn inbox_items(&self) -> Result<Vec<InboxItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, text, created_at FROM inbox ORDER BY created_at DESC",
        )?;
        let result: Vec<InboxItem> = stmt
            .query_map([], |row| {
                let ts: i64 = row.get(2)?;
                Ok(InboxItem {
                    id: row.get(0)?,
                    text: row.get(1)?,
                    created_at: Local.timestamp_opt(ts, 0).single().unwrap_or_else(Local::now),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(result)
    }

    pub fn add_inbox_item(&self, text: &str) -> Result<i64> {
        let now = Local::now().timestamp();
        self.conn.execute(
            "INSERT INTO inbox (text, created_at) VALUES (?1, ?2)",
            params![text, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_inbox_item(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM inbox WHERE id=?1", params![id])?;
        Ok(())
    }

    // ── Stats ──────────────────────────────────────────────────────────────────

    pub fn tasks_completed_today(&self) -> Result<i64> {
        let start = today_start_ts();
        let end = today_end_ts();
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM task WHERE status='done' AND completed_at BETWEEN ?1 AND ?2",
            params![start, end],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn tasks_open_count(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM task WHERE status IN ('open','in-progress','blocked','waiting')",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}

// ── Row mappers ───────────────────────────────────────────────────────────────

fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<Task> {
    let due_ts: Option<i64> = row.get(7)?;
    let start_ts: Option<i64> = row.get(8)?;
    let crt_ts: i64 = row.get(11)?;
    let upd_ts: i64 = row.get(12)?;
    let cmp_ts: Option<i64> = row.get(13)?;

    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        status: TaskStatus::from_str(&row.get::<_, String>(3)?),
        priority: Priority::from_i64(row.get(4)?),
        energy: Energy::from_str(&row.get::<_, String>(5)?),
        estimate_min: row.get(6)?,
        due_at: due_ts.and_then(|ts| Local.timestamp_opt(ts, 0).single()),
        start_at: start_ts.and_then(|ts| Local.timestamp_opt(ts, 0).single()),
        parent_id: row.get(9)?,
        project_id: row.get(10)?,
        tags: vec![],
        created_at: Local.timestamp_opt(crt_ts, 0).single().unwrap_or_else(Local::now),
        updated_at: Local.timestamp_opt(upd_ts, 0).single().unwrap_or_else(Local::now),
        completed_at: cmp_ts.and_then(|ts| Local.timestamp_opt(ts, 0).single()),
    })
}

fn row_to_event(row: &rusqlite::Row) -> rusqlite::Result<Event> {
    let start_ts: i64 = row.get(2)?;
    let end_ts: i64 = row.get(3)?;
    Ok(Event {
        id: row.get(0)?,
        title: row.get(1)?,
        starts_at: Local.timestamp_opt(start_ts, 0).single().unwrap_or_else(Local::now),
        ends_at: Local.timestamp_opt(end_ts, 0).single().unwrap_or_else(Local::now),
        all_day: row.get::<_, i64>(4)? != 0,
        calendar: row.get(5)?,
        recurrence: row.get(6)?,
        notes: row.get(7)?,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn today_start_ts() -> i64 {
    let today = Local::now().date_naive();
    Local
        .from_local_datetime(&today.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .map(|d| d.timestamp())
        .unwrap_or(0)
}

fn today_end_ts() -> i64 {
    let today = Local::now().date_naive();
    Local
        .from_local_datetime(&today.and_hms_opt(23, 59, 59).unwrap())
        .single()
        .map(|d| d.timestamp())
        .unwrap_or(i64::MAX)
}

fn compute_streak(sorted_desc_dates: &[NaiveDate]) -> i64 {
    if sorted_desc_dates.is_empty() {
        return 0;
    }
    let today = Local::now().date_naive();
    let mut streak = 0i64;
    let mut expected = today;
    for &date in sorted_desc_dates {
        if date == expected || date == expected - chrono::Duration::days(1) {
            streak += 1;
            expected = date - chrono::Duration::days(1);
        } else {
            break;
        }
    }
    streak
}

fn compute_personal_best(sorted_desc_dates: &[NaiveDate]) -> i64 {
    if sorted_desc_dates.is_empty() {
        return 0;
    }
    let mut pb = 1i64;
    let mut current = 1i64;
    for i in 1..sorted_desc_dates.len() {
        let prev = sorted_desc_dates[i - 1];
        let curr = sorted_desc_dates[i];
        if prev - curr == chrono::Duration::days(1) {
            current += 1;
            if current > pb {
                pb = current;
            }
        } else {
            current = 1;
        }
    }
    pb
}

pub fn db_path() -> Result<PathBuf> {
    let base = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
    Ok(base.join(".aurora").join("aurora.db"))
}

// ── Schema ────────────────────────────────────────────────────────────────────

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS task (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    title         TEXT NOT NULL,
    description   TEXT,
    status        TEXT NOT NULL DEFAULT 'open',
    priority      INTEGER NOT NULL DEFAULT 3,
    energy        TEXT NOT NULL DEFAULT 'medium',
    estimate_min  INTEGER,
    due_at        INTEGER,
    start_at      INTEGER,
    parent_id     INTEGER REFERENCES task(id),
    project_id    INTEGER REFERENCES project(id),
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    completed_at  INTEGER,
    archived_at   INTEGER
);

CREATE TABLE IF NOT EXISTS task_tag (
    task_id INTEGER REFERENCES task(id) ON DELETE CASCADE,
    tag     TEXT NOT NULL,
    PRIMARY KEY (task_id, tag)
);

CREATE TABLE IF NOT EXISTS task_dep (
    blocker_id INTEGER REFERENCES task(id) ON DELETE CASCADE,
    blocked_id INTEGER REFERENCES task(id) ON DELETE CASCADE,
    PRIMARY KEY (blocker_id, blocked_id)
);

CREATE TABLE IF NOT EXISTS project (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    description TEXT,
    color       TEXT,
    created_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS event (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    title       TEXT NOT NULL,
    starts_at   INTEGER NOT NULL,
    ends_at     INTEGER NOT NULL,
    all_day     INTEGER NOT NULL DEFAULT 0,
    calendar    TEXT NOT NULL DEFAULT 'personal',
    recurrence  TEXT,
    notes       TEXT
);

CREATE TABLE IF NOT EXISTS habit (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    cadence     TEXT NOT NULL DEFAULT 'daily',
    type        TEXT NOT NULL DEFAULT 'bool',
    target      REAL,
    domain      TEXT,
    color       TEXT,
    cue         TEXT,
    routine     TEXT,
    reward      TEXT,
    goal_id     INTEGER REFERENCES goal(id)
);

CREATE TABLE IF NOT EXISTS habit_log (
    habit_id INTEGER NOT NULL REFERENCES habit(id) ON DELETE CASCADE,
    date     TEXT NOT NULL,
    value    REAL NOT NULL DEFAULT 1,
    status   TEXT NOT NULL DEFAULT 'done',
    PRIMARY KEY (habit_id, date)
);

CREATE TABLE IF NOT EXISTS goal (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    title       TEXT NOT NULL,
    horizon     TEXT NOT NULL DEFAULT 'quarter',
    period      TEXT,
    progress    REAL NOT NULL DEFAULT 0,
    notes       TEXT,
    parent_id   INTEGER REFERENCES goal(id)
);

CREATE TABLE IF NOT EXISTS note (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    slug        TEXT UNIQUE NOT NULL,
    title       TEXT NOT NULL,
    vault_path  TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS note_link (
    src TEXT NOT NULL,
    dst TEXT NOT NULL,
    PRIMARY KEY (src, dst)
);

CREATE TABLE IF NOT EXISTS journal (
    date     TEXT PRIMARY KEY,
    mood     INTEGER,
    energy   INTEGER,
    focus    INTEGER,
    sleep_h  REAL,
    body_md  TEXT
);

CREATE TABLE IF NOT EXISTS focus_session (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id      INTEGER REFERENCES task(id),
    started_at   INTEGER NOT NULL,
    ended_at     INTEGER,
    kind         TEXT NOT NULL DEFAULT 'deep',
    distractions INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS time_log (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id    INTEGER REFERENCES task(id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL,
    ended_at   INTEGER
);

CREATE TABLE IF NOT EXISTS inbox (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    text       TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS task_fts USING fts5(title, description, content='task', content_rowid='id');
";

// Extension trait for optional query
trait OptionalExt<T> {
    fn optional(self) -> rusqlite::Result<Option<T>>;
}

impl<T> OptionalExt<T> for rusqlite::Result<T> {
    fn optional(self) -> rusqlite::Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
