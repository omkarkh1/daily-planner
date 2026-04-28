use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

// ── Task ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
    Open,
    InProgress,
    Blocked,
    Waiting,
    Done,
    Cancelled,
    Archived,
}

impl TaskStatus {
    pub fn label(&self) -> &'static str {
        match self {
            TaskStatus::Open => "open",
            TaskStatus::InProgress => "in-progress",
            TaskStatus::Blocked => "blocked",
            TaskStatus::Waiting => "waiting",
            TaskStatus::Done => "done",
            TaskStatus::Cancelled => "cancelled",
            TaskStatus::Archived => "archived",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "in-progress" => TaskStatus::InProgress,
            "blocked" => TaskStatus::Blocked,
            "waiting" => TaskStatus::Waiting,
            "done" => TaskStatus::Done,
            "cancelled" => TaskStatus::Cancelled,
            "archived" => TaskStatus::Archived,
            _ => TaskStatus::Open,
        }
    }

    pub fn icon(&self, nerd: bool) -> &'static str {
        if nerd {
            match self {
                TaskStatus::Open => "󰄱",
                TaskStatus::InProgress => "󰦗",
                TaskStatus::Done => "󰄲",
                TaskStatus::Blocked => "󰅙",
                TaskStatus::Cancelled => "󰅙",
                _ => "󰄱",
            }
        } else {
            match self {
                TaskStatus::Open => "[ ]",
                TaskStatus::InProgress => "[~]",
                TaskStatus::Done => "[x]",
                TaskStatus::Blocked => "[!]",
                TaskStatus::Cancelled => "[-]",
                _ => "[ ]",
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Priority {
    P0,
    P1,
    P2,
    P3,
    P4,
}

impl Priority {
    pub fn from_i64(n: i64) -> Self {
        match n {
            0 => Priority::P0,
            1 => Priority::P1,
            2 => Priority::P2,
            4 => Priority::P4,
            _ => Priority::P3,
        }
    }

    pub fn to_i64(&self) -> i64 {
        match self {
            Priority::P0 => 0,
            Priority::P1 => 1,
            Priority::P2 => 2,
            Priority::P3 => 3,
            Priority::P4 => 4,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Priority::P0 => "!p0",
            Priority::P1 => "!p1",
            Priority::P2 => "!p2",
            Priority::P3 => "!p3",
            Priority::P4 => "!p4",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Energy {
    Low,
    Medium,
    High,
}

impl Energy {
    pub fn from_str(s: &str) -> Self {
        match s {
            "low" => Energy::Low,
            "high" => Energy::High,
            _ => Energy::Medium,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Energy::Low => "low",
            Energy::Medium => "medium",
            Energy::High => "high",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: Priority,
    pub energy: Energy,
    pub estimate_min: Option<i64>,
    pub due_at: Option<DateTime<Local>>,
    pub start_at: Option<DateTime<Local>>,
    pub parent_id: Option<i64>,
    pub project_id: Option<i64>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub completed_at: Option<DateTime<Local>>,
}

impl Task {
    pub fn is_done(&self) -> bool {
        matches!(self.status, TaskStatus::Done | TaskStatus::Cancelled | TaskStatus::Archived)
    }

    pub fn priority_marker(&self) -> &'static str {
        match self.priority {
            Priority::P0 => "!!!",
            Priority::P1 => "!! ",
            Priority::P2 => "!  ",
            Priority::P3 => "   ",
            Priority::P4 => "·  ",
        }
    }
}

#[derive(Debug, Default)]
pub struct NewTask {
    pub title: String,
    pub description: Option<String>,
    pub priority: Priority,
    pub energy: Energy,
    pub estimate_min: Option<i64>,
    pub due_at: Option<DateTime<Local>>,
    pub tags: Vec<String>,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::P3
    }
}

impl Default for Energy {
    fn default() -> Self {
        Energy::Medium
    }
}

// ── Event ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: i64,
    pub title: String,
    pub starts_at: DateTime<Local>,
    pub ends_at: DateTime<Local>,
    pub all_day: bool,
    pub calendar: String,
    pub recurrence: Option<String>,
    pub notes: Option<String>,
}

impl Event {
    pub fn duration_min(&self) -> i64 {
        (self.ends_at - self.starts_at).num_minutes()
    }
}

// ── Habit ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HabitType {
    Bool,
    Counted,
    Timed,
}

impl HabitType {
    pub fn from_str(s: &str) -> Self {
        match s {
            "counted" => HabitType::Counted,
            "timed" => HabitType::Timed,
            _ => HabitType::Bool,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            HabitType::Bool => "bool",
            HabitType::Counted => "counted",
            HabitType::Timed => "timed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Habit {
    pub id: i64,
    pub name: String,
    pub cadence: String,
    pub habit_type: HabitType,
    pub target: Option<f64>,
    pub domain: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HabitLogStatus {
    Done,
    Skip,
    Missed,
    Partial,
}

impl HabitLogStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "skip" => HabitLogStatus::Skip,
            "missed" => HabitLogStatus::Missed,
            "partial" => HabitLogStatus::Partial,
            _ => HabitLogStatus::Done,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            HabitLogStatus::Done => "done",
            HabitLogStatus::Skip => "skip",
            HabitLogStatus::Missed => "missed",
            HabitLogStatus::Partial => "partial",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HabitLog {
    pub habit_id: i64,
    pub date: NaiveDate,
    pub value: f64,
    pub status: HabitLogStatus,
}

#[derive(Debug, Clone)]
pub struct HabitWithStreak {
    pub habit: Habit,
    pub streak: i64,
    pub personal_best: i64,
    pub today_log: Option<HabitLog>,
}

// ── Goal ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: i64,
    pub title: String,
    pub horizon: String,
    pub period: Option<String>,
    pub progress: f64,
    pub notes: Option<String>,
    pub parent_id: Option<i64>,
}

// ── Journal ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub date: NaiveDate,
    pub mood: Option<i64>,
    pub energy: Option<i64>,
    pub focus: Option<i64>,
    pub sleep_h: Option<f64>,
    pub body_md: Option<String>,
}

// ── Focus Session ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusSession {
    pub id: i64,
    pub task_id: Option<i64>,
    pub started_at: DateTime<Local>,
    pub ended_at: Option<DateTime<Local>>,
    pub kind: String,
    pub distractions: i64,
}

// ── Note ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub vault_path: String,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

// ── Inbox ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxItem {
    pub id: i64,
    pub text: String,
    pub created_at: DateTime<Local>,
}
