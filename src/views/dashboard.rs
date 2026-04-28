use chrono::{Datelike, Local, Timelike};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

use crate::theme::{mood_dots, progress_bar, sparkline, Theme};
use crate::{
    db::Database,
    models::{HabitWithStreak, InboxItem, Task, TaskStatus},
};

use super::styled_block;

pub struct DashboardData {
    pub today_tasks: Vec<Task>,
    pub habits: Vec<HabitWithStreak>,
    pub inbox: Vec<InboxItem>,
    pub goals_quarter: Vec<crate::models::Goal>,
    pub focus_week: Vec<i64>,
    pub focus_today: i64,
    pub yesterday_mood: Option<i64>,
    pub sleep_h: Option<f64>,
}

impl DashboardData {
    pub fn load(db: &Database) -> anyhow::Result<Self> {
        let today = Local::now().date_naive();
        let yesterday = today - chrono::Duration::days(1);
        let yesterday_entry = db.journal_for_date(yesterday)?;
        Ok(DashboardData {
            today_tasks: db.tasks_due_today()?,
            habits: db.habits_with_streaks(today)?,
            inbox: db.inbox_items()?,
            goals_quarter: db.goals_for_horizon("quarter")?,
            focus_week: db.focus_minutes_for_week()?,
            focus_today: db.focus_minutes_today()?,
            yesterday_mood: yesterday_entry.as_ref().and_then(|e| e.mood),
            sleep_h: yesterday_entry.as_ref().and_then(|e| e.sleep_h),
        })
    }
}

pub fn render(f: &mut Frame, area: ratatui::layout::Rect, data: &DashboardData, theme: &Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // Row 1: summary + focus week
            Constraint::Min(8),     // Row 2: today plan + goals
            Constraint::Length(8),  // Row 3: habits + inbox + quote
        ])
        .split(area);

    render_row1(f, rows[0], data, theme);
    render_row2(f, rows[1], data, theme);
    render_row3(f, rows[2], data, theme);
}

fn render_row1(f: &mut Frame, area: ratatui::layout::Rect, data: &DashboardData, theme: &Theme) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    render_today_summary(f, cols[0], data, theme);
    render_focus_week(f, cols[1], data, theme);
}

fn render_today_summary(f: &mut Frame, area: ratatui::layout::Rect, data: &DashboardData, theme: &Theme) {
    let now = Local::now();
    let hour = now.hour();
    let greeting = if hour < 12 {
        "Good morning."
    } else if hour < 17 {
        "Good afternoon."
    } else {
        "Good evening."
    };

    let sleep_str = data
        .sleep_h
        .map(|h| format!(" You slept {:.0}h.", h))
        .unwrap_or_default();
    let mood_str = data
        .yesterday_mood
        .map(|m| format!(" Yesterday: {}", mood_dots(m, 5)))
        .unwrap_or_default();

    let today = now.date_naive();
    let day_of_year = today.ordinal();
    let days_in_year = if today.leap_year() { 366u32 } else { 365u32 };
    let pct = (day_of_year as f64 / days_in_year as f64 * 100.0) as u32;
    let remaining = days_in_year - day_of_year;

    let done = data.today_tasks.iter().filter(|t| t.is_done()).count();
    let open = data.today_tasks.len() - done;

    let block = styled_block(" Today ", theme, false);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines = vec![
        Line::from(vec![
            Span::styled(greeting, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
            Span::styled(&sleep_str, Style::default().fg(theme.fg_dim)),
            Span::styled(&mood_str, Style::default().fg(theme.fg_dim)),
        ]),
        Line::from(vec![
            Span::styled("  Year theme · ", Style::default().fg(theme.fg_dim)),
            Span::styled(
                "\"relentless craftsmanship\"",
                Style::default().fg(theme.accent_alt).add_modifier(Modifier::ITALIC),
            ),
        ]),
        Line::from(Span::styled(
            format!(
                "  Day {} of {} · {}% of year · {} days remain",
                day_of_year, days_in_year, pct, remaining
            ),
            Style::default().fg(theme.fg_dim),
        )),
        Line::from(Span::styled(
            format!("  {} done · {} open", done, open),
            Style::default().fg(theme.fg_dim),
        )),
    ];
    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn render_focus_week(f: &mut Frame, area: ratatui::layout::Rect, data: &DashboardData, theme: &Theme) {
    let block = styled_block(" Focus this week ", theme, false);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let total_goal = 20 * 60i64; // 20 hours goal
    let total_actual: i64 = data.focus_week.iter().sum();
    let max = data.focus_week.iter().cloned().max().unwrap_or(1).max(1);

    let spark = sparkline(&data.focus_week, max);
    let pct = (total_actual * 100 / total_goal).min(100);
    let bar = progress_bar(total_actual as f64 / total_goal as f64, 20);

    let days = ["M", "T", "W", "T", "F", "S", "S"];
    let day_str = days.join("  ");

    let total_h = total_actual / 60;
    let total_m = total_actual % 60;
    let goal_h = total_goal / 60;

    let lines = vec![
        Line::from(Span::styled(&day_str, Style::default().fg(theme.fg_dim))),
        Line::from(Span::styled(&spark, Style::default().fg(theme.accent))),
        Line::from(Span::styled(
            format!("{}h {:02}m / {}h", total_h, total_m, goal_h),
            Style::default().fg(theme.fg),
        )),
        Line::from(Span::styled(
            format!("{} {}%", bar, pct),
            Style::default().fg(theme.accent_alt),
        )),
    ];
    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn render_row2(f: &mut Frame, area: ratatui::layout::Rect, data: &DashboardData, theme: &Theme) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_today_plan(f, cols[0], data, theme);
    render_goals(f, cols[1], data, theme);
}

fn render_today_plan(f: &mut Frame, area: ratatui::layout::Rect, data: &DashboardData, theme: &Theme) {
    let block = styled_block(" Today's plan ", theme, true);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = Vec::new();
    let available = inner.height as usize;

    for task in data.today_tasks.iter().take(available.saturating_sub(1)) {
        let (icon, color) = match task.status {
            TaskStatus::Done => ("✔", theme.ok),
            TaskStatus::InProgress => ("◐", theme.warn),
            TaskStatus::Blocked => ("⊗", theme.err),
            _ => ("○", theme.fg),
        };

        let priority_color = match task.priority {
            crate::models::Priority::P0 => theme.err,
            crate::models::Priority::P1 => theme.warn,
            _ => theme.fg_dim,
        };

        let due_str = task
            .due_at
            .map(|d| format!("  {}", d.format("%H:%M")))
            .unwrap_or_else(|| "      ".to_string());

        lines.push(Line::from(vec![
            Span::styled(due_str, Style::default().fg(theme.fg_dim)),
            Span::raw("  "),
            Span::styled(icon, Style::default().fg(color)),
            Span::raw(" "),
            Span::styled(
                truncate(&task.title, (inner.width as usize).saturating_sub(14)),
                Style::default().fg(if task.is_done() { theme.fg_dim } else { theme.fg }),
            ),
        ]));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No tasks for today  ·  press n to add",
            Style::default().fg(theme.fg_dim),
        )));
    }

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn render_goals(f: &mut Frame, area: ratatui::layout::Rect, data: &DashboardData, theme: &Theme) {
    let block = styled_block(" Goals this quarter ", theme, false);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = Vec::new();
    let bar_width = (inner.width as usize).saturating_sub(10).min(20);

    for goal in data.goals_quarter.iter().take(4) {
        let bar = progress_bar(goal.progress, bar_width);
        let pct = (goal.progress * 100.0) as u32;
        lines.push(Line::from(Span::styled(
            truncate(&goal.title, (inner.width as usize).saturating_sub(2)),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(bar, Style::default().fg(theme.accent)),
            Span::styled(
                format!("  {}%", pct),
                Style::default().fg(theme.fg_dim),
            ),
        ]));
        lines.push(Line::from(""));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No quarterly goals yet",
            Style::default().fg(theme.fg_dim),
        )));
        lines.push(Line::from(Span::styled(
            "  :goal add \"...\" to create one",
            Style::default().fg(theme.fg_dim),
        )));
    }

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn render_row3(f: &mut Frame, area: ratatui::layout::Rect, data: &DashboardData, theme: &Theme) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(42),
            Constraint::Percentage(30),
            Constraint::Percentage(28),
        ])
        .split(area);

    render_habits(f, cols[0], data, theme);
    render_inbox(f, cols[1], data, theme);
    render_quote(f, cols[2], theme);
}

fn render_habits(f: &mut Frame, area: ratatui::layout::Rect, data: &DashboardData, theme: &Theme) {
    let block = styled_block(" Habits today ", theme, false);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = Vec::new();
    for hw in data.habits.iter().take(inner.height as usize) {
        let (icon, color) = if let Some(log) = &hw.today_log {
            match log.status {
                crate::models::HabitLogStatus::Done => ("✔", theme.ok),
                crate::models::HabitLogStatus::Skip => ("~", theme.fg_dim),
                crate::models::HabitLogStatus::Missed => ("✗", theme.err),
                crate::models::HabitLogStatus::Partial => ("◐", theme.warn),
            }
        } else {
            ("○", theme.fg_dim)
        };

        let streak_str = if hw.streak > 0 {
            format!("  ·  {}-day streak", hw.streak)
        } else {
            String::new()
        };

        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(icon, Style::default().fg(color)),
            Span::raw(" "),
            Span::styled(
                truncate(&hw.habit.name, (inner.width as usize).saturating_sub(20)),
                Style::default().fg(theme.fg),
            ),
            Span::styled(streak_str, Style::default().fg(theme.fg_dim)),
        ]));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No habits yet  ·  :habit add",
            Style::default().fg(theme.fg_dim),
        )));
    }

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn render_inbox(f: &mut Frame, area: ratatui::layout::Rect, data: &DashboardData, theme: &Theme) {
    let title = format!(" Inbox · {} ", data.inbox.len());
    let block = styled_block(&title, theme, false);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = Vec::new();
    for item in data.inbox.iter().take((inner.height as usize).saturating_sub(1)) {
        lines.push(Line::from(vec![
            Span::raw(" • "),
            Span::styled(
                truncate(&item.text, (inner.width as usize).saturating_sub(4)),
                Style::default().fg(theme.fg),
            ),
        ]));
    }
    if !data.inbox.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Space to add  ·  i to triage",
            Style::default().fg(theme.fg_dim),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "  Inbox is clear",
            Style::default().fg(theme.ok),
        )));
    }

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn render_quote(f: &mut Frame, area: ratatui::layout::Rect, theme: &Theme) {
    let block = styled_block(" Quote ", theme, false);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let quotes = [
        ("We are what we\nrepeatedly do.\nExcellence, then,\nis not an act,\nbut a habit.", "Aristotle"),
        ("The secret of getting\nahead is getting\nstarted.", "Mark Twain"),
        ("Focus is a matter\nof deciding what\nthings you're not\ngoing to do.", "John Carmack"),
        ("Do the hard jobs\nfirst. The easy\njobs will take care\nof themselves.", "Dale Carnegie"),
    ];

    let day = Local::now().day() as usize;
    let (text, author) = quotes[day % quotes.len()];

    let mut lines: Vec<Line> = text
        .lines()
        .map(|l| {
            Line::from(Span::styled(
                format!(" \"{}\"", l),
                Style::default().fg(theme.fg_dim).add_modifier(Modifier::ITALIC),
            ))
        })
        .collect();
    lines.push(Line::from(Span::styled(
        format!("           — {}", author),
        Style::default().fg(theme.fg_dim),
    )));

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max > 3 {
        format!("{}…", &s[..max - 1])
    } else {
        s[..max].to_string()
    }
}


