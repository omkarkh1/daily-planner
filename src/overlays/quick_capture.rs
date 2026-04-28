use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::{
    parser::{parse, ParsedCapture},
    theme::Theme,
    views::centered_rect,
};

pub struct QuickCaptureState {
    pub input: String,
    pub cursor: usize,
    pub parsed: Option<ParsedCapture>,
}

impl Default for QuickCaptureState {
    fn default() -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            parsed: None,
        }
    }
}

impl QuickCaptureState {
    pub fn push_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.reparse();
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev = self.input[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input.remove(prev);
            self.cursor = prev;
            self.reparse();
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.input.len() {
            self.input.remove(self.cursor);
            self.reparse();
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor += 1;
        }
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end_key(&mut self) {
        self.cursor = self.input.len();
    }

    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor = 0;
        self.parsed = None;
    }

    fn reparse(&mut self) {
        if self.input.trim().is_empty() {
            self.parsed = None;
        } else {
            self.parsed = Some(parse(&self.input));
        }
    }
}

pub fn render(f: &mut Frame, area: Rect, state: &QuickCaptureState, theme: &Theme) {
    let width = (area.width * 2 / 3).max(60).min(area.width);
    let height = if state.parsed.is_some() { 14 } else { 5 };
    let popup = centered_rect(width, height, area);

    // Clear background
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" quick capture ")
        .title_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .border_style(Style::default().fg(theme.accent))
        .style(Style::default().bg(theme.bg_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let mut lines = Vec::new();

    // Input line with cursor
    let before_cursor = &state.input[..state.cursor];
    let at_cursor = state.input[state.cursor..].chars().next().unwrap_or(' ');
    let after_cursor: String = state.input[state.cursor..]
        .chars()
        .skip(1)
        .collect();

    lines.push(Line::from(vec![
        Span::raw(" > "),
        Span::styled(before_cursor, Style::default().fg(theme.fg)),
        Span::styled(
            at_cursor.to_string(),
            Style::default()
                .fg(theme.bg)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(&after_cursor, Style::default().fg(theme.fg)),
    ]));

    // Parse preview
    if let Some(parsed) = &state.parsed {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  parsed:",
            Style::default().fg(theme.fg_dim),
        )));

        if !parsed.title.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("   title  · ", Style::default().fg(theme.fg_dim)),
                Span::styled(&parsed.title, Style::default().fg(theme.fg)),
            ]));
        }

        if let Some(due) = &parsed.due_at {
            lines.push(Line::from(vec![
                Span::styled("   when   · ", Style::default().fg(theme.fg_dim)),
                Span::styled(
                    due.format("%a %b %-d · %H:%M").to_string(),
                    Style::default().fg(theme.accent),
                ),
            ]));
        }

        if !parsed.tags.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("   tag    · ", Style::default().fg(theme.fg_dim)),
                Span::styled(
                    parsed.tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" "),
                    Style::default().fg(theme.accent_alt),
                ),
            ]));
        }

        let priority_label = match parsed.priority {
            crate::models::Priority::P0 => "P0 — drop everything",
            crate::models::Priority::P1 => "P1 — high",
            crate::models::Priority::P2 => "P2 — medium-high",
            crate::models::Priority::P3 => "P3 — normal",
            crate::models::Priority::P4 => "P4 — someday",
        };
        lines.push(Line::from(vec![
            Span::styled("   prio   · ", Style::default().fg(theme.fg_dim)),
            Span::styled(priority_label, Style::default().fg(theme.warn)),
        ]));

        if let Some(mins) = parsed.estimate_min {
            let est_str = if mins >= 60 {
                format!("{}h {:02}m", mins / 60, mins % 60)
            } else {
                format!("{}m", mins)
            };
            lines.push(Line::from(vec![
                Span::styled("   est    · ", Style::default().fg(theme.fg_dim)),
                Span::styled(est_str, Style::default().fg(theme.info)),
            ]));
        }

        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![
        Span::styled("  Enter ", Style::default().fg(theme.fg_dim)),
        Span::styled("capture", Style::default().fg(theme.accent)),
        Span::styled("  ·  Esc ", Style::default().fg(theme.fg_dim)),
        Span::styled("cancel", Style::default().fg(theme.fg_dim)),
        Span::styled("  ·  Tab ", Style::default().fg(theme.fg_dim)),
        Span::styled("edit fields", Style::default().fg(theme.fg_dim)),
    ]));

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}
