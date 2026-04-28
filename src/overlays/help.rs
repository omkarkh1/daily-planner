use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::{theme::Theme, views::centered_rect};

pub struct HelpState {
    pub scroll: u16,
}

impl Default for HelpState {
    fn default() -> Self {
        Self { scroll: 0 }
    }
}

impl HelpState {
    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(3);
    }
    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(3);
    }
}

pub fn render(f: &mut Frame, area: Rect, state: &HelpState, theme: &Theme) {
    let width = (area.width * 3 / 4).max(72).min(area.width);
    let height = (area.height * 4 / 5).max(30).min(area.height);
    let popup = centered_rect(width, height, area);

    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" AURORA — Keyboard Reference ")
        .title_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .border_style(Style::default().fg(theme.accent))
        .style(Style::default().bg(theme.bg_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let lines = help_lines(theme);
    let para = Paragraph::new(Text::from(lines))
        .scroll((state.scroll, 0));
    f.render_widget(para, inner);
}

fn section(title: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {} ", title),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
    ])
}

fn entry(key: &str, desc: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {:20}", key),
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc.to_string(), Style::default().fg(theme.fg)),
    ])
}

fn sep() -> Line<'static> {
    Line::from("")
}

fn help_lines(theme: &Theme) -> Vec<Line<'static>> {
    vec![
        sep(),
        section("Global", theme),
        entry("?", "This help screen", theme),
        entry("Space", "Quick capture overlay", theme),
        entry("Ctrl+P  /  :", "Command palette", theme),
        entry("Ctrl+F", "Global fuzzy search", theme),
        entry("Esc", "Close overlay / cancel", theme),
        entry("Q", "Quit AURORA", theme),
        entry("Ctrl+Z", "Undo", theme),
        entry("Ctrl+Shift+Z", "Redo", theme),
        sep(),
        section("Navigation — Views", theme),
        entry("g d", "Dashboard", theme),
        entry("g t", "Today (time grid)", theme),
        entry("g w", "Week view", theme),
        entry("g m", "Month view", theme),
        entry("g y", "Year view", theme),
        entry("g p", "Projects & Goals", theme),
        entry("g h", "Habits", theme),
        entry("g n", "Notes vault", theme),
        entry("g j", "Journal", theme),
        entry("g s", "Stats & Analytics", theme),
        entry("g r", "Reviews", theme),
        sep(),
        section("Navigation — Within View", theme),
        entry("j / k  or  ↓ / ↑", "Move down / up", theme),
        entry("h / l  or  ← / →", "Move left / right", theme),
        entry("J / K", "Page down / up", theme),
        entry("H / L", "Previous / next day", theme),
        entry("g g", "Top of list", theme),
        entry("G", "Bottom of list", theme),
        entry("Tab / Shift+Tab", "Next / previous pane", theme),
        sep(),
        section("Tasks", theme),
        entry("n", "New task", theme),
        entry("x", "Toggle complete / reopen", theme),
        entry("o", "Open task detail", theme),
        entry("D", "Delete task (confirm)", theme),
        entry("d", "Set due date", theme),
        entry("t", "Start / stop time tracker", theme),
        entry("p", "Cycle priority", theme),
        entry("e", "Cycle energy level", theme),
        entry("s", "Snooze task", theme),
        entry("v", "Multi-select mode", theme),
        sep(),
        section("Events / Calendar", theme),
        entry("N", "New event", theme),
        entry("Shift+h / l", "Move event ±15m", theme),
        entry("Shift+j / k", "Resize event ±15m", theme),
        entry("f", "Find free time slot", theme),
        entry("c", "Cycle calendars", theme),
        sep(),
        section("Focus & Pomodoro", theme),
        entry("F", "Enter / exit focus mode", theme),
        entry("Shift+F  /  P", "Start Pomodoro on task", theme),
        entry("p  (in focus)", "Pause / resume", theme),
        entry("d  (in focus)", "Log distraction", theme),
        sep(),
        section("Quick Capture Syntax", theme),
        entry("#tag", "Add tag: #work #health", theme),
        entry("!p0 .. !p4", "Set priority", theme),
        entry("~30m  ~2h", "Estimate duration", theme),
        entry("today  tomorrow", "Due date keywords", theme),
        entry("monday  fri", "Due day-of-week", theme),
        entry("4pm  14:30", "Due time", theme),
        entry("@low @medium @high", "Energy level", theme),
        sep(),
        section("Command Palette Examples", theme),
        entry(":task add \"...\"", "Add task with details", theme),
        entry(":habit log meditate", "Log habit for today", theme),
        entry(":focus start 25m", "Start 25-minute focus", theme),
        entry(":theme set nord", "Switch theme", theme),
        entry(":snapshot create", "Create a backup", theme),
        entry(":export markdown ~/out.md", "Export to Markdown", theme),
        sep(),
        Line::from(Span::styled(
            "  j/k to scroll  ·  Esc to close",
            Style::default().fg(theme.fg_dim),
        )),
        sep(),
    ]
}
