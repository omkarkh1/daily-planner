use chrono::{Datelike, Duration, Local, NaiveDate};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

use crate::{db::Database, theme::Theme};

use super::styled_block;

pub struct MonthData {
    pub year: i32,
    pub month: u32,
    pub day_task_counts: Vec<(NaiveDate, usize, usize)>, // (date, done, total)
}

impl MonthData {
    pub fn load(db: &Database, month_offset: i32) -> anyhow::Result<Self> {
        let today = Local::now().date_naive();
        let mut year = today.year();
        let mut month = today.month() as i32 + month_offset;
        while month > 12 {
            month -= 12;
            year += 1;
        }
        while month < 1 {
            month += 12;
            year -= 1;
        }
        let month = month as u32;

        let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(today);
        let last = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap_or(first) - Duration::days(1)
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap_or(first) - Duration::days(1)
        };

        let mut day_task_counts = Vec::new();
        let mut cur = first;
        while cur <= last {
            let tasks = db.tasks_for_date(cur)?;
            let done = tasks.iter().filter(|t| t.is_done()).count();
            day_task_counts.push((cur, done, tasks.len()));
            cur += Duration::days(1);
        }

        Ok(MonthData {
            year,
            month,
            day_task_counts,
        })
    }
}

pub fn render(f: &mut Frame, area: ratatui::layout::Rect, data: &MonthData, theme: &Theme) {
    use chrono::Weekday;

    let month_name = month_name(data.month);
    let title = format!(" {} {} ", month_name, data.year);
    let block = styled_block(&title, theme, false);
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Header row: day names
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // day name header
            Constraint::Min(0),    // weeks
        ])
        .split(inner);

    let day_names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let col_w = inner.width / 7;

    let mut header = String::new();
    for name in &day_names {
        header.push_str(&format!("{:<width$}", name, width = col_w as usize));
    }
    f.render_widget(
        Paragraph::new(Span::styled(
            header,
            Style::default().fg(theme.fg_dim).add_modifier(Modifier::BOLD),
        )),
        rows[0],
    );

    // Calendar grid
    let first_day = NaiveDate::from_ymd_opt(data.year, data.month, 1).unwrap();
    let first_weekday = first_day.weekday().num_days_from_monday() as usize; // 0=Mon
    let today = Local::now().date_naive();

    // Build week rows (max 6 weeks)
    let week_constraints: Vec<Constraint> = (0..6).map(|_| Constraint::Ratio(1, 6)).collect();
    let week_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(week_constraints)
        .split(rows[1]);

    let col_constraints: Vec<Constraint> = (0..7).map(|_| Constraint::Ratio(1, 7)).collect();

    let mut day_idx: i32 = -(first_weekday as i32);
    for week_row in week_rows.iter() {
        let day_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(col_constraints.clone())
            .split(*week_row);

        for (col_i, col) in day_cols.iter().enumerate() {
            let date = first_day + Duration::days((day_idx + col_i as i32) as i64);
            if date.month() != data.month {
                // Render empty or dim date from adjacent month
                f.render_widget(
                    Paragraph::new(Span::styled(
                        format!("{:>2}", date.day()),
                        Style::default().fg(theme.bg_alt),
                    )),
                    *col,
                );
            } else {
                let task_info = data
                    .day_task_counts
                    .iter()
                    .find(|(d, _, _)| *d == date)
                    .cloned();

                let is_today = date == today;
                let day_color = if is_today {
                    theme.accent
                } else if date.weekday() == Weekday::Sat || date.weekday() == Weekday::Sun {
                    theme.fg_dim
                } else {
                    theme.fg
                };

                let mut lines = vec![];
                let day_label = if is_today {
                    format!("{:>2}◀", date.day())
                } else {
                    format!("{:>2} ", date.day())
                };
                lines.push(Line::from(Span::styled(
                    day_label,
                    Style::default()
                        .fg(day_color)
                        .add_modifier(if is_today { Modifier::BOLD } else { Modifier::empty() }),
                )));

                if let Some((_, done, total)) = task_info {
                    if total > 0 {
                        let dot_color = if done == total { theme.ok } else { theme.warn };
                        lines.push(Line::from(Span::styled(
                            format!("  {}/{}", done, total),
                            Style::default().fg(dot_color),
                        )));
                    }
                }

                f.render_widget(Paragraph::new(Text::from(lines)), *col);
            }
        }
        day_idx += 7;
    }
}

fn month_name(m: u32) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "?",
    }
}
