use chrono::{Local, Timelike};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::{
    db::Database,
    models::{Event, Task, TaskStatus},
    theme::Theme,
};

use super::styled_block;

pub struct TodayData {
    pub tasks: Vec<Task>,
    pub events: Vec<Event>,
    pub focus_mins: i64,
}

impl TodayData {
    pub fn load(db: &Database) -> anyhow::Result<Self> {
        let today = Local::now().date_naive();
        Ok(TodayData {
            tasks: db.tasks_due_today()?,
            events: db.events_for_date(today)?,
            focus_mins: db.focus_minutes_today()?,
        })
    }
}

pub fn render(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    data: &TodayData,
    task_state: &mut ListState,
    theme: &Theme,
) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(5)])
        .split(cols[0]);

    render_time_grid(f, left_rows[0], data, theme);
    render_today_notes(f, left_rows[1], theme);
    render_task_list(f, cols[1], data, task_state, theme);
}

fn render_time_grid(f: &mut Frame, area: ratatui::layout::Rect, data: &TodayData, theme: &Theme) {
    let now = Local::now();
    let today_str = now.format("Today · %a %d %b").to_string();
    let title = format!(" {} ", today_str);
    let block = styled_block(&title, theme, false);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let inner_width = inner.width as usize;
    let bar_width = inner_width.saturating_sub(6);
    let current_hour = now.hour() as usize;

    let mut lines = Vec::new();

    // Show hours from 6 to 23
    for hour in 6usize..=23 {
        let events_in_hour: Vec<&Event> = data
            .events
            .iter()
            .filter(|e| e.starts_at.hour() as usize == hour)
            .collect();

        let is_current = hour == current_hour;
        let hour_color = if is_current { theme.accent } else { theme.fg_dim };

        if events_in_hour.is_empty() {
            let fill: String = if is_current {
                format!("{:>2} ▶", hour)
            } else {
                format!("{:>2}  ", hour)
            };
            let dots = "░".repeat(bar_width.min(50));
            lines.push(Line::from(vec![
                Span::styled(fill, Style::default().fg(hour_color)),
                Span::styled(dots, Style::default().fg(theme.bg_alt)),
            ]));
        } else {
            for (i, event) in events_in_hour.iter().enumerate() {
                let cal_color = calendar_color(event.calendar.as_str(), theme);
                let event_width = bar_width.saturating_sub(2);
                let title_padded = pad_or_truncate(&event.title, event_width);
                let hour_label = if i == 0 {
                    if is_current {
                        format!("{:>2} ▶", hour)
                    } else {
                        format!("{:>2}  ", hour)
                    }
                } else {
                    "     ".to_string()
                };
                lines.push(Line::from(vec![
                    Span::styled(hour_label, Style::default().fg(hour_color)),
                    Span::styled(
                        format!("│{}│", title_padded),
                        Style::default().fg(cal_color).add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
        }

        if lines.len() >= inner.height as usize {
            break;
        }
    }

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn render_task_list(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    data: &TodayData,
    state: &mut ListState,
    theme: &Theme,
) {
    let done = data.tasks.iter().filter(|t| t.is_done()).count();
    let total = data.tasks.len();
    let title = format!(" Tasks  {}/{} ", done, total);
    let block = styled_block(&title, theme, true);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let items: Vec<ListItem> = data
        .tasks
        .iter()
        .map(|task| {
            let (icon, fg) = match task.status {
                TaskStatus::Done => ("✔", theme.ok),
                TaskStatus::InProgress => ("◐", theme.warn),
                TaskStatus::Blocked => ("⊗", theme.err),
                _ => ("○", theme.fg),
            };
            let priority = match task.priority {
                crate::models::Priority::P0 => "!!!",
                crate::models::Priority::P1 => "!! ",
                crate::models::Priority::P2 => "!  ",
                _ => "   ",
            };
            let est = task
                .estimate_min
                .map(|m| {
                    if m >= 60 {
                        format!("~{}h", m / 60)
                    } else {
                        format!("~{}m", m)
                    }
                })
                .unwrap_or_default();
            let tags = if task.tags.is_empty() {
                String::new()
            } else {
                format!(" #{}", task.tags.join(" #"))
            };

            let max_title = (inner.width as usize).saturating_sub(14);
            let title_str = if task.title.len() > max_title {
                format!("{}…", &task.title[..max_title.saturating_sub(1)])
            } else {
                task.title.clone()
            };

            ListItem::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(icon, Style::default().fg(fg)),
                Span::raw(" "),
                Span::styled(
                    title_str,
                    Style::default().fg(if task.is_done() { theme.fg_dim } else { theme.fg }),
                ),
                Span::styled(
                    format!(" {}{}{}", priority, est, tags),
                    Style::default().fg(theme.fg_dim),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(theme.bg_alt)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, inner, state);

    // Footer hint
    if inner.height > 2 {
        let hint_area = ratatui::layout::Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(Span::styled(
                " n:add  x:done  D:del  t:timer ",
                Style::default().fg(theme.fg_dim),
            )),
            hint_area,
        );
    }
}

fn render_today_notes(f: &mut Frame, area: ratatui::layout::Rect, theme: &Theme) {
    let block = styled_block(" Notes for today ", theme, false);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines = vec![
        Line::from(Span::styled(
            " Quick notes appear here · press J to append",
            Style::default().fg(theme.fg_dim),
        )),
    ];
    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn calendar_color(calendar: &str, theme: &Theme) -> ratatui::style::Color {
    match calendar {
        "work" => theme.accent,
        "personal" => theme.accent_alt,
        "health" => theme.ok,
        "learning" => theme.info,
        _ => theme.fg,
    }
}

fn pad_or_truncate(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s)
    }
}
