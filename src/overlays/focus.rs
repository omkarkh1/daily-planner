use std::time::{Duration, Instant};

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::{models::Task, theme::Theme, views::centered_rect};

pub struct FocusState {
    pub task: Option<Task>,
    pub start: Instant,
    pub paused: bool,
    pub paused_at: Option<Instant>,
    pub paused_total: Duration,
    pub is_pomodoro: bool,
    pub pomodoro_work_m: u64,
    pub pomodoro_break_m: u64,
    pub pomodoro_num: u32,
    pub in_break: bool,
    pub distractions: u32,
    pub session_id: Option<i64>,
}

impl FocusState {
    pub fn new_deep(task: Option<Task>) -> Self {
        Self {
            task,
            start: Instant::now(),
            paused: false,
            paused_at: None,
            paused_total: Duration::ZERO,
            is_pomodoro: false,
            pomodoro_work_m: 25,
            pomodoro_break_m: 5,
            pomodoro_num: 1,
            in_break: false,
            distractions: 0,
            session_id: None,
        }
    }

    pub fn new_pomodoro(task: Option<Task>, work_m: u64, break_m: u64) -> Self {
        Self {
            task,
            start: Instant::now(),
            paused: false,
            paused_at: None,
            paused_total: Duration::ZERO,
            is_pomodoro: true,
            pomodoro_work_m: work_m,
            pomodoro_break_m: break_m,
            pomodoro_num: 1,
            in_break: false,
            distractions: 0,
            session_id: None,
        }
    }

    pub fn elapsed(&self) -> Duration {
        let raw = self.start.elapsed();
        raw.saturating_sub(self.paused_total)
            - self.paused_at.map(|t| t.elapsed()).unwrap_or(Duration::ZERO)
    }

    pub fn pause(&mut self) {
        if !self.paused {
            self.paused = true;
            self.paused_at = Some(Instant::now());
        }
    }

    pub fn resume(&mut self) {
        if self.paused {
            if let Some(at) = self.paused_at.take() {
                self.paused_total += at.elapsed();
            }
            self.paused = false;
        }
    }

    pub fn toggle_pause(&mut self) {
        if self.paused {
            self.resume();
        } else {
            self.pause();
        }
    }

    pub fn log_distraction(&mut self) {
        self.distractions += 1;
    }

    pub fn pomodoro_remaining(&self) -> Duration {
        let target_m = if self.in_break {
            self.pomodoro_break_m
        } else {
            self.pomodoro_work_m
        };
        let target = Duration::from_secs(target_m * 60);
        target.saturating_sub(self.elapsed())
    }

    pub fn pomodoro_expired(&self) -> bool {
        self.is_pomodoro && self.pomodoro_remaining() == Duration::ZERO
    }

    pub fn pomodoro_progress(&self) -> f64 {
        let target_m = if self.in_break {
            self.pomodoro_break_m
        } else {
            self.pomodoro_work_m
        };
        let target_secs = (target_m * 60) as f64;
        (self.elapsed().as_secs_f64() / target_secs).min(1.0)
    }
}

pub fn render(f: &mut Frame, area: Rect, state: &FocusState, theme: &Theme) {
    let popup = centered_rect(40, 18, area);
    f.render_widget(Clear, popup);

    let border_color = if state.paused { theme.warn } else { theme.accent };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(if state.in_break { " BREAK " } else { " DEEP WORK " })
        .title_style(
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        )
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(theme.bg_inset));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let elapsed = state.elapsed();
    let elapsed_secs = elapsed.as_secs();

    let (timer_str, phase_str, progress) = if state.is_pomodoro {
        let remaining = state.pomodoro_remaining();
        let rem_secs = remaining.as_secs();
        let t = format!("{:>2}:{:02}", rem_secs / 60, rem_secs % 60);
        let phase = if state.in_break {
            format!("break  ·  pomo {} of 4", state.pomodoro_num)
        } else {
            format!("pomodoro {} of 4", state.pomodoro_num)
        };
        (t, phase, state.pomodoro_progress())
    } else {
        let t = format!("{:>2}:{:02}:{:02}", elapsed_secs / 3600, (elapsed_secs % 3600) / 60, elapsed_secs % 60);
        ("".to_string(), t, (elapsed_secs as f64 / 5400.0).min(1.0)) // arbitrary 90m baseline
    };

    // Progress bar
    let bar_width = (inner.width as usize).saturating_sub(4).min(28);
    let filled = (progress * bar_width as f64).round() as usize;
    let empty = bar_width.saturating_sub(filled);
    let bar = format!(
        "{}{}",
        "▰".repeat(filled),
        "▱".repeat(empty)
    );

    let task_title = state
        .task
        .as_ref()
        .map(|t| t.title.clone())
        .unwrap_or_else(|| "free focus".to_string());
    let task_display = if task_title.len() > inner.width.saturating_sub(4) as usize {
        format!("{}…", &task_title[..inner.width.saturating_sub(5) as usize])
    } else {
        task_title
    };

    let paused_str = if state.paused { "  ⏸ PAUSED" } else { "" };

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            &task_display,
            Style::default().fg(theme.fg_dim),
        )),
        Line::from(""),
    ];

    if state.is_pomodoro {
        lines.push(Line::from(vec![
            Span::styled("  ┌", Style::default().fg(theme.fg_dim)),
            Span::styled(
                "─".repeat(inner.width.saturating_sub(4) as usize),
                Style::default().fg(theme.fg_dim),
            ),
            Span::styled("┐", Style::default().fg(theme.fg_dim)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  │ ", Style::default().fg(theme.fg_dim)),
            Span::styled(
                format_timer_big(&timer_str),
                Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}", paused_str),
                Style::default().fg(theme.warn),
            ),
            Span::styled(" │", Style::default().fg(theme.fg_dim)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  └", Style::default().fg(theme.fg_dim)),
            Span::styled(
                "─".repeat(inner.width.saturating_sub(4) as usize),
                Style::default().fg(theme.fg_dim),
            ),
            Span::styled("┘", Style::default().fg(theme.fg_dim)),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            format!("  {}{}", phase_str, paused_str),
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(bar, Style::default().fg(border_color)),
    ]));

    if state.is_pomodoro {
        let phase_label = format!("  {}", phase_str);
        lines.push(Line::from(Span::styled(
            phase_label,
            Style::default().fg(theme.fg_dim),
        )));
    }

    lines.push(Line::from(""));

    if state.distractions > 0 {
        lines.push(Line::from(Span::styled(
            format!("  {} distraction{}", state.distractions, if state.distractions == 1 { "" } else { "s" }),
            Style::default().fg(theme.warn),
        )));
    }

    lines.push(Line::from(Span::styled(
        "  p pause  ·  d distraction  ·  Esc exit",
        Style::default().fg(theme.fg_dim),
    )));

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn format_timer_big(t: &str) -> String {
    // Space out digits for a big display feel
    t.chars()
        .map(|c| format!("{} ", c))
        .collect::<String>()
        .trim_end()
        .to_string()
}
