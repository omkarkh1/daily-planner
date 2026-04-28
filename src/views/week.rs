use chrono::{Datelike, Duration, Local, NaiveDate, Timelike};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

use crate::{db::Database, models::Event, theme::Theme};

use super::styled_block;

pub struct WeekData {
    pub week_start: NaiveDate,
    pub day_tasks: Vec<Vec<crate::models::Task>>,
    pub day_events: Vec<Vec<Event>>,
    pub day_focus: Vec<i64>,
}

impl WeekData {
    pub fn load(db: &Database, week_offset: i32) -> anyhow::Result<Self> {
        let today = Local::now().date_naive();
        let weekday_num = today.weekday().num_days_from_monday() as i64;
        let monday = today - Duration::days(weekday_num) + Duration::days(week_offset as i64 * 7);

        let mut day_tasks = Vec::new();
        let mut day_events = Vec::new();
        let day_focus = db.focus_minutes_for_week()?;

        for i in 0..7i64 {
            let day = monday + Duration::days(i);
            day_tasks.push(db.tasks_for_date(day)?);
            day_events.push(db.events_for_date(day)?);
        }

        Ok(WeekData {
            week_start: monday,
            day_tasks,
            day_events,
            day_focus,
        })
    }
}

pub fn render(f: &mut Frame, area: ratatui::layout::Rect, data: &WeekData, theme: &Theme) {
    let week_num = data.week_start.iso_week().week();
    let end_date = data.week_start + Duration::days(6);
    let title = format!(
        " Week {} · {} — {} ",
        week_num,
        data.week_start.format("%b %d"),
        end_date.format("%b %d")
    );
    let block = styled_block(&title, theme, false);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let day_names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let today = Local::now().date_naive();

    // Split into header row and content
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // day headers
            Constraint::Min(0),    // time grid
            Constraint::Length(2), // footer stats
        ])
        .split(inner);

    // Day header with equal-width columns
    let day_constraints: Vec<Constraint> = (0..7).map(|_| Constraint::Ratio(1, 7)).collect();
    let header_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(day_constraints.clone())
        .split(rows[0]);

    for (i, col) in header_cols.iter().enumerate() {
        let day = data.week_start + Duration::days(i as i64);
        let is_today = day == today;
        let label = format!("{} {}", day_names[i], day.format("%d"));
        let style = if is_today {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(theme.fg_dim)
        };
        f.render_widget(Paragraph::new(Span::styled(label, style)), *col);
    }

    // Time grid
    let content_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(day_constraints.clone())
        .split(rows[1]);

    let hours_visible = rows[1].height as usize;
    let start_hour = 8usize;

    for (col_idx, col) in content_cols.iter().enumerate() {
        let day = data.week_start + Duration::days(col_idx as i64);
        let events = &data.day_events[col_idx];
        let tasks = &data.day_tasks[col_idx];

        let mut lines = Vec::new();
        for h in start_hour..(start_hour + hours_visible) {
            let evts: Vec<&Event> = events
                .iter()
                .filter(|e| e.starts_at.hour() as usize == h)
                .collect();

            if evts.is_empty() {
                let done = tasks.iter().filter(|t| t.is_done()).count();
                let open = tasks.len() - done;
                if h == start_hour && (done > 0 || open > 0) {
                    let col_width = col.width as usize;
                    let s = format!("{}/{}", done, done + open);
                    lines.push(Line::from(Span::styled(
                        pad_right(&s, col_width),
                        Style::default().fg(theme.fg_dim),
                    )));
                } else {
                    lines.push(Line::from(Span::styled(
                        "░".repeat(col.width as usize),
                        Style::default().fg(theme.bg_alt),
                    )));
                }
            } else {
                for evt in evts.iter().take(1) {
                    let cal_color = match evt.calendar.as_str() {
                        "work" => theme.accent,
                        "personal" => theme.accent_alt,
                        "health" => theme.ok,
                        _ => theme.info,
                    };
                    let w = col.width as usize;
                    let title = if evt.title.len() > w.saturating_sub(2) {
                        evt.title[..w.saturating_sub(2)].to_string()
                    } else {
                        evt.title.clone()
                    };
                    lines.push(Line::from(Span::styled(
                        format!("│{:<width$}", title, width = w.saturating_sub(1)),
                        Style::default().fg(cal_color),
                    )));
                }
            }

            if lines.len() >= hours_visible {
                break;
            }
        }

        f.render_widget(Paragraph::new(Text::from(lines)), *col);
    }

    // Footer: focus minutes per day
    let footer_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(day_constraints)
        .split(rows[2]);

    for (i, col) in footer_cols.iter().enumerate() {
        let mins = data.day_focus.get(i).cloned().unwrap_or(0);
        let label = if mins >= 60 {
            format!("{}h{:02}m", mins / 60, mins % 60)
        } else if mins > 0 {
            format!("{}m", mins)
        } else {
            "—".to_string()
        };
        f.render_widget(
            Paragraph::new(Span::styled(label, Style::default().fg(theme.fg_dim))),
            *col,
        );
    }
}

fn pad_right(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s)
    }
}
