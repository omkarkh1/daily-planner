pub mod dashboard;
pub mod month;
pub mod today;
pub mod week;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders},
    Frame,
};

use crate::theme::Theme;

pub fn styled_block<'a>(title: &'a str, theme: &Theme, focused: bool) -> Block<'a> {
    let border_color = if focused { theme.accent } else { theme.fg_dim };
    Block::default()
        .borders(Borders::ALL)
        .border_type(theme.border_type)
        .title(title)
        .title_style(
            Style::default()
                .fg(if focused { theme.accent } else { theme.fg })
                .add_modifier(Modifier::BOLD),
        )
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(theme.bg))
}

/// Center a rect of given dimensions inside a larger rect
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

/// Render the bottom status ribbon
pub fn render_statusbar(
    f: &mut Frame,
    area: Rect,
    mode: &str,
    view_name: &str,
    done: i64,
    open: i64,
    focus_mins: i64,
    battery: Option<u8>,
    theme: &Theme,
) {
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    let time_str = chrono::Local::now().format("%H:%M").to_string();
    let battery_str = battery
        .map(|b| format!(" {}% ·", b))
        .unwrap_or_default();
    let focus_str = if focus_mins > 0 {
        format!(" {}h {:02}m focused ·", focus_mins / 60, focus_mins % 60)
    } else {
        String::new()
    };

    let left = Span::styled(
        format!(" {} ", mode),
        Style::default()
            .fg(theme.bg)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD),
    );
    let mid = Span::styled(
        format!("  {} · {}/{} done ·{} {}", view_name, done, done + open, focus_str, battery_str),
        Style::default().fg(theme.fg_dim).bg(theme.bg_alt),
    );
    let right = Span::styled(
        format!("  {}  ?help  Space:capture  :cmd ", time_str),
        Style::default().fg(theme.fg_dim).bg(theme.bg_alt),
    );

    let line = Line::from(vec![left, mid, right]);
    let para = Paragraph::new(line)
        .style(Style::default().bg(theme.bg_alt));
    f.render_widget(para, area);
}

/// Splash screen
pub fn render_splash(f: &mut Frame, area: Rect, theme: &Theme) {
    use ratatui::text::{Line, Text};
    use ratatui::widgets::Paragraph;

    let lines = vec![
        Line::from(""),
        Line::from(""),
        Line::from(""),
        Line::from(ratatui::text::Span::styled(
            "  ░█████╗░██╗░░░██╗██████╗░░█████╗░██████╗░░█████╗░  ",
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(ratatui::text::Span::styled(
            "  ██╔══██╗██║░░░██║██╔══██╗██╔══██╗██╔══██╗██╔══██╗  ",
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(ratatui::text::Span::styled(
            "  ███████║██║░░░██║██████╔╝██║░░██║██████╔╝███████║  ",
            Style::default().fg(theme.accent_alt).add_modifier(Modifier::BOLD),
        )),
        Line::from(ratatui::text::Span::styled(
            "  ██╔══██║██║░░░██║██╔══██╗██║░░██║██╔══██╗██╔══██║  ",
            Style::default().fg(theme.accent_alt).add_modifier(Modifier::BOLD),
        )),
        Line::from(ratatui::text::Span::styled(
            "  ██║░░██║╚██████╔╝██║░░██║╚█████╔╝██║░░██║██║░░██║  ",
            Style::default().fg(theme.fg_dim),
        )),
        Line::from(""),
        Line::from(ratatui::text::Span::styled(
            "            « your day, composed »",
            Style::default().fg(theme.fg_dim).add_modifier(Modifier::ITALIC),
        )),
        Line::from(""),
        Line::from(ratatui::text::Span::styled(
            "     ── 2026 theme: relentless craftsmanship ──",
            Style::default().fg(theme.fg_dim),
        )),
        Line::from(""),
        Line::from(""),
        Line::from(ratatui::text::Span::styled(
            "              press  Enter  to begin",
            Style::default().fg(theme.fg),
        )),
        Line::from(ratatui::text::Span::styled(
            "              press  ?  for the keymap",
            Style::default().fg(theme.fg_dim),
        )),
        Line::from(ratatui::text::Span::styled(
            "              press  :  for the command line",
            Style::default().fg(theme.fg_dim),
        )),
    ];

    let text = Text::from(lines);
    let para = Paragraph::new(text)
        .style(Style::default().bg(theme.bg_inset).fg(theme.fg))
        .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(para, area);
}
