use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::{theme::Theme, views::centered_rect};

#[derive(Clone)]
pub struct CommandEntry {
    pub name: String,
    pub description: String,
    pub shortcut: Option<String>,
}

pub struct CommandPaletteState {
    pub input: String,
    pub cursor: usize,
    pub all_commands: Vec<CommandEntry>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub list_state: ListState,
}

impl CommandPaletteState {
    pub fn new() -> Self {
        let commands = builtin_commands();
        let filtered: Vec<usize> = (0..commands.len()).collect();
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            input: String::new(),
            cursor: 0,
            all_commands: commands,
            filtered,
            selected: 0,
            list_state,
        }
    }

    pub fn push_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.refilter();
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
            self.refilter();
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.list_state.select(Some(self.selected));
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.filtered.len() {
            self.selected += 1;
            self.list_state.select(Some(self.selected));
        }
    }

    pub fn selected_command(&self) -> Option<&CommandEntry> {
        self.filtered
            .get(self.selected)
            .and_then(|&i| self.all_commands.get(i))
    }

    fn refilter(&mut self) {
        let matcher = SkimMatcherV2::default();
        if self.input.trim().is_empty() {
            self.filtered = (0..self.all_commands.len()).collect();
        } else {
            let mut scored: Vec<(usize, i64)> = self
                .all_commands
                .iter()
                .enumerate()
                .filter_map(|(i, cmd)| {
                    let hay = format!("{} {}", cmd.name, cmd.description);
                    matcher.fuzzy_match(&hay, &self.input).map(|s| (i, s))
                })
                .collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        }
        self.selected = 0;
        self.list_state.select(if self.filtered.is_empty() { None } else { Some(0) });
    }

    pub fn reset(&mut self) {
        self.input.clear();
        self.cursor = 0;
        self.refilter();
    }
}

pub fn render(f: &mut Frame, area: Rect, state: &mut CommandPaletteState, theme: &Theme) {
    let width = (area.width * 2 / 3).max(56).min(area.width);
    let list_h = (state.filtered.len() as u16 + 2).min(12);
    let height = 3 + list_h;
    let popup = centered_rect(width, height, area);

    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" command palette ")
        .title_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .border_style(Style::default().fg(theme.accent))
        .style(Style::default().bg(theme.bg_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    use ratatui::layout::{Constraint, Direction, Layout};
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    // Input line
    let before = &state.input[..state.cursor];
    let at = state.input[state.cursor..].chars().next().unwrap_or(' ');
    let after: String = state.input[state.cursor..].chars().skip(1).collect();

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("> ", Style::default().fg(theme.accent)),
            Span::styled(before, Style::default().fg(theme.fg)),
            Span::styled(
                at.to_string(),
                Style::default().fg(theme.bg).bg(theme.accent),
            ),
            Span::styled(&after, Style::default().fg(theme.fg)),
        ])),
        sections[0],
    );

    // Divider
    f.render_widget(
        Paragraph::new(Span::styled(
            "─".repeat(inner.width as usize),
            Style::default().fg(theme.fg_dim),
        )),
        sections[1],
    );

    // Results list
    let items: Vec<ListItem> = state
        .filtered
        .iter()
        .filter_map(|&i| state.all_commands.get(i))
        .map(|cmd| {
            let shortcut = cmd
                .shortcut
                .as_deref()
                .map(|s| format!(" {}", s))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(&cmd.name, Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  {}", cmd.description), Style::default().fg(theme.fg)),
                Span::styled(shortcut, Style::default().fg(theme.fg_dim)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(theme.bg_inset)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, sections[2], &mut state.list_state);
}

fn builtin_commands() -> Vec<CommandEntry> {
    vec![
        cmd("task add", "Add a new task", None),
        cmd("task complete", "Mark task as done", Some("x")),
        cmd("task delete", "Delete a task", Some("D")),
        cmd("task list", "Show all tasks", Some("gp")),
        cmd("event add", "Add a new event", Some("N")),
        cmd("habit add", "Add a new habit", None),
        cmd("habit log", "Log habit for today", None),
        cmd("habit skip", "Skip habit today (vacation)", None),
        cmd("goal add", "Add a new goal", None),
        cmd("goal progress", "Update goal progress", None),
        cmd("note new", "Create a new note", None),
        cmd("journal append", "Append to today's journal", Some("J")),
        cmd("focus start", "Start a focus session", Some("F")),
        cmd("focus pomodoro", "Start Pomodoro timer", Some("Shift+F")),
        cmd("review daily", "Start daily review", None),
        cmd("review weekly", "Start weekly review", None),
        cmd("theme set", "Change the theme", None),
        cmd("layout save", "Save current layout", None),
        cmd("snapshot create", "Create a snapshot", None),
        cmd("snapshot restore", "Restore a snapshot", None),
        cmd("export markdown", "Export to Markdown", None),
        cmd("vault encrypt", "Enable vault encryption", None),
        cmd("settings", "Open settings", Some("Ctrl+,")),
        cmd("help", "Show keyboard help", Some("?")),
        cmd("quit", "Quit AURORA", Some("Q")),
        cmd("dashboard", "Go to Dashboard", Some("gd")),
        cmd("today", "Go to Today view", Some("gt")),
        cmd("week", "Go to Week view", Some("gw")),
        cmd("month", "Go to Month view", Some("gm")),
        cmd("habits", "Go to Habits view", Some("gh")),
        cmd("notes", "Go to Notes view", Some("gn")),
        cmd("journal", "Go to Journal view", Some("gj")),
        cmd("stats", "Go to Stats view", Some("gs")),
    ]
}

fn cmd(name: &str, description: &str, shortcut: Option<&str>) -> CommandEntry {
    CommandEntry {
        name: name.to_string(),
        description: description.to_string(),
        shortcut: shortcut.map(|s| s.to_string()),
    }
}
