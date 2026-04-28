use anyhow::Result;
use chrono::Local;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::ListState,
    Frame,
};
use std::time::Instant;

use crate::{
    config::{self, Config},
    db::Database,
    keymap::{Action, Keymap, NavTarget},
    macos,
    models::{NewTask, TaskStatus},
    overlays::{
        command_palette::CommandPaletteState,
        focus::FocusState,
        help::HelpState,
        quick_capture::QuickCaptureState,
    },
    snapshot,
    theme::Theme,
    vault::Vault,
    views::{
        dashboard::{self, DashboardData},
        month::{self, MonthData},
        today::{self, TodayData},
        week::{self, WeekData},
    },
};

// ── View ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ActiveView {
    Dashboard,
    Today,
    Week,
    Month,
    Year,
    Projects,
    Habits,
    Notes,
    Journal,
    Stats,
    Reviews,
}

impl ActiveView {
    pub fn name(&self) -> &'static str {
        match self {
            ActiveView::Dashboard => "dashboard",
            ActiveView::Today => "today",
            ActiveView::Week => "week",
            ActiveView::Month => "month",
            ActiveView::Year => "year",
            ActiveView::Projects => "projects",
            ActiveView::Habits => "habits",
            ActiveView::Notes => "notes",
            ActiveView::Journal => "journal",
            ActiveView::Stats => "stats",
            ActiveView::Reviews => "reviews",
        }
    }
}

// ── Overlay ───────────────────────────────────────────────────────────────────

pub enum ActiveOverlay {
    QuickCapture,
    CommandPalette,
    Help,
    Focus,
    NewTaskInput,
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub config: Config,
    pub theme: Theme,
    pub keymap: Keymap,
    pub db: Database,
    pub vault: Vault,

    // View state
    pub view: ActiveView,
    pub overlay: Option<ActiveOverlay>,
    pub g_pressed: bool,
    pub should_quit: bool,

    // Splash
    pub show_splash: bool,

    // Status message (temporary feedback)
    pub status_msg: Option<(String, Instant)>,

    // Battery (cached, refreshed every N ticks)
    pub battery: Option<u8>,
    pub tick_count: u64,

    // View-specific states
    pub week_offset: i32,
    pub month_offset: i32,
    pub task_list_state: ListState,

    // Overlay states
    pub quick_capture: QuickCaptureState,
    pub command_palette: CommandPaletteState,
    pub help_state: HelpState,
    pub focus_state: Option<FocusState>,

    // New task inline input
    pub new_task_input: String,
    pub new_task_cursor: usize,
}

impl App {
    pub fn new() -> Result<Self> {
        let config = config::load()?;
        let theme = Theme::by_name(&config.appearance.theme);
        let keymap = Keymap::new(&config.keymap.preset);
        let db = Database::open()?;
        let vault = Vault::new(&config);
        vault.ensure_dirs().ok();

        // Auto-snapshot on startup
        snapshot::maybe_auto_snapshot(&config);

        let mut list_state = ListState::default();
        list_state.select(Some(0));

        let start_view = match config.startup.view.as_str() {
            "today" => ActiveView::Today,
            "week" => ActiveView::Week,
            _ => ActiveView::Dashboard,
        };

        let battery = macos::battery_percent();

        Ok(App {
            config: config.clone(),
            theme,
            keymap,
            db,
            vault,
            view: start_view,
            overlay: None,
            g_pressed: false,
            should_quit: false,
            show_splash: config.startup.splash,
            status_msg: None,
            battery,
            tick_count: 0,
            week_offset: 0,
            month_offset: 0,
            task_list_state: list_state,
            quick_capture: QuickCaptureState::default(),
            command_palette: CommandPaletteState::new(),
            help_state: HelpState::default(),
            focus_state: None,
            new_task_input: String::new(),
            new_task_cursor: 0,
        })
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_msg = Some((msg.into(), Instant::now()));
    }

    pub fn tick(&mut self) {
        self.tick_count += 1;

        // Clear old status messages (3 second timeout)
        if let Some((_, t)) = &self.status_msg {
            if t.elapsed().as_secs() > 3 {
                self.status_msg = None;
            }
        }

        // Refresh battery every ~10s (200 ticks at 50ms each)
        if self.tick_count % 200 == 0 {
            self.battery = macos::battery_percent();
        }

        // Check pomodoro expiry
        if let Some(focus) = &mut self.focus_state {
            if focus.is_pomodoro && focus.pomodoro_expired() && !focus.in_break {
                focus.in_break = true;
                focus.start = std::time::Instant::now();
                focus.paused_total = std::time::Duration::ZERO;
                macos::notify("AURORA", "Pomodoro complete! Time for a break.");
            } else if focus.is_pomodoro && focus.pomodoro_expired() && focus.in_break {
                focus.in_break = false;
                focus.pomodoro_num += 1;
                focus.start = std::time::Instant::now();
                focus.paused_total = std::time::Duration::ZERO;
                macos::notify("AURORA", "Break over. Back to work!");
            }
        }
    }

    /// Returns true if the app should quit
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        // Only handle key press events, not release
        if key.kind != KeyEventKind::Press {
            return false;
        }

        // Dismiss splash on any key
        if self.show_splash {
            self.show_splash = false;
            return false;
        }

        let action = self.keymap.resolve(&key, self.g_pressed);
        self.g_pressed = false;

        // Handle overlay input first
        if let Some(ref overlay) = self.overlay {
            match overlay {
                ActiveOverlay::QuickCapture => {
                    return self.handle_quick_capture_key(key, action);
                }
                ActiveOverlay::CommandPalette => {
                    return self.handle_command_palette_key(key, action);
                }
                ActiveOverlay::Help => {
                    return self.handle_help_key(action);
                }
                ActiveOverlay::Focus => {
                    return self.handle_focus_key(action);
                }
                ActiveOverlay::NewTaskInput => {
                    return self.handle_new_task_key(key, action);
                }
            }
        }

        // Global actions
        match action {
            Action::Quit => return true,
            Action::QuitPane => {
                if self.view != ActiveView::Dashboard {
                    self.view = ActiveView::Dashboard;
                } else {
                    return true;
                }
            }
            Action::GPrefix => self.g_pressed = true,
            Action::NavigateTo(target) => self.navigate_to(target),
            Action::ToggleHelp => {
                self.help_state = HelpState::default();
                self.overlay = Some(ActiveOverlay::Help);
            }
            Action::OpenQuickCapture => {
                self.quick_capture = QuickCaptureState::default();
                self.overlay = Some(ActiveOverlay::QuickCapture);
            }
            Action::OpenCommandPalette => {
                self.command_palette.reset();
                self.overlay = Some(ActiveOverlay::CommandPalette);
            }
            Action::ToggleFocus => {
                if self.focus_state.is_some() {
                    self.end_focus();
                } else {
                    let task = self.selected_task();
                    let fs = FocusState::new_deep(task);
                    if let Ok(id) = self.db.start_focus_session(
                        self.focus_state.as_ref().and_then(|f| f.task.as_ref().map(|t| t.id)),
                        "deep",
                    ) {
                        let fs_mut = self.focus_state.get_or_insert(FocusState::new_deep(None));
                        fs_mut.session_id = Some(id);
                    }
                    self.focus_state = Some(fs);
                    self.overlay = Some(ActiveOverlay::Focus);
                }
            }
            Action::StartPomodoro => {
                let task = self.selected_task();
                let work_m = self.config.focus.pomodoro_work_m;
                let break_m = self.config.focus.pomodoro_break_m;
                let mut fs = FocusState::new_pomodoro(task, work_m, break_m);
                if let Ok(id) = self.db.start_focus_session(None, "pomodoro") {
                    fs.session_id = Some(id);
                }
                self.focus_state = Some(fs);
                self.overlay = Some(ActiveOverlay::Focus);
            }

            // View-specific actions
            Action::Down => self.handle_down(),
            Action::Up => self.handle_up(),
            Action::Left => self.handle_left(),
            Action::Right => self.handle_right(),
            Action::NextDay => self.handle_next(),
            Action::PrevDay => self.handle_prev(),
            Action::GotoTop => self.goto_top(),
            Action::GotoBottom => self.goto_bottom(),
            Action::NewTask => self.start_new_task(),
            Action::ToggleComplete => self.toggle_complete(),
            Action::DeleteItem => self.delete_selected(),
            Action::Escape => {}
            _ => {}
        }

        false
    }

    fn navigate_to(&mut self, target: NavTarget) {
        self.view = match target {
            NavTarget::Dashboard => ActiveView::Dashboard,
            NavTarget::Today => ActiveView::Today,
            NavTarget::Week => ActiveView::Week,
            NavTarget::Month => ActiveView::Month,
            NavTarget::Year => ActiveView::Year,
            NavTarget::Projects => ActiveView::Projects,
            NavTarget::Habits => ActiveView::Habits,
            NavTarget::Notes => ActiveView::Notes,
            NavTarget::Journal => ActiveView::Journal,
            NavTarget::Stats => ActiveView::Stats,
            NavTarget::Reviews => ActiveView::Reviews,
        };
        // Reset selection
        self.task_list_state.select(Some(0));
    }

    fn handle_down(&mut self) {
        match self.view {
            ActiveView::Today | ActiveView::Dashboard => {
                let len = self.today_task_count();
                if let Some(sel) = self.task_list_state.selected() {
                    if sel + 1 < len {
                        self.task_list_state.select(Some(sel + 1));
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_up(&mut self) {
        match self.view {
            ActiveView::Today | ActiveView::Dashboard => {
                if let Some(sel) = self.task_list_state.selected() {
                    if sel > 0 {
                        self.task_list_state.select(Some(sel - 1));
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_left(&mut self) {
        match self.view {
            ActiveView::Week => {
                self.week_offset -= 1;
            }
            ActiveView::Month => {
                self.month_offset -= 1;
            }
            _ => {}
        }
    }

    fn handle_right(&mut self) {
        match self.view {
            ActiveView::Week => {
                self.week_offset += 1;
            }
            ActiveView::Month => {
                self.month_offset += 1;
            }
            _ => {}
        }
    }

    fn handle_next(&mut self) {
        self.handle_right();
    }

    fn handle_prev(&mut self) {
        self.handle_left();
    }

    fn goto_top(&mut self) {
        self.task_list_state.select(Some(0));
    }

    fn goto_bottom(&mut self) {
        let len = self.today_task_count();
        if len > 0 {
            self.task_list_state.select(Some(len - 1));
        }
    }

    fn today_task_count(&self) -> usize {
        self.db.tasks_due_today().map(|t| t.len()).unwrap_or(0)
    }

    fn selected_task(&self) -> Option<crate::models::Task> {
        let idx = self.task_list_state.selected()?;
        let tasks = self.db.tasks_due_today().ok()?;
        tasks.into_iter().nth(idx)
    }

    fn toggle_complete(&mut self) {
        if let Some(task) = self.selected_task() {
            if task.is_done() {
                let _ = self.db.reopen_task(task.id);
                self.set_status("Task reopened");
            } else {
                let _ = self.db.complete_task(task.id);
                self.set_status(format!("✔ \"{}\"", task.title));
            }
        }
    }

    fn delete_selected(&mut self) {
        if let Some(task) = self.selected_task() {
            let _ = self.db.delete_task(task.id);
            let sel = self.task_list_state.selected().unwrap_or(0);
            let new_len = self.today_task_count();
            if sel >= new_len && new_len > 0 {
                self.task_list_state.select(Some(new_len - 1));
            }
            self.set_status(format!("Deleted \"{}\"", task.title));
        }
    }

    fn start_new_task(&mut self) {
        self.new_task_input.clear();
        self.new_task_cursor = 0;
        self.overlay = Some(ActiveOverlay::NewTaskInput);
    }

    fn end_focus(&mut self) {
        if let Some(fs) = &self.focus_state {
            if let Some(id) = fs.session_id {
                let _ = self.db.end_focus_session(id, fs.distractions as i64);
            }
        }
        self.focus_state = None;
        self.overlay = None;
    }

    // ── Overlay key handlers ───────────────────────────────────────────────────

    fn handle_quick_capture_key(&mut self, key: KeyEvent, action: Action) -> bool {
        match action {
            Action::Escape => {
                self.overlay = None;
                self.quick_capture.clear();
            }
            Action::Enter => {
                let input = self.quick_capture.input.trim().to_string();
                if !input.is_empty() {
                    let parsed = crate::parser::parse(&input);
                    let new_task: NewTask = parsed.into();
                    match self.db.create_task(&new_task) {
                        Ok(_) => self.set_status(format!("Captured: {}", input.chars().take(40).collect::<String>())),
                        Err(e) => self.set_status(format!("Error: {}", e)),
                    }
                }
                self.overlay = None;
                self.quick_capture.clear();
            }
            Action::Backspace => self.quick_capture.backspace(),
            Action::Delete => self.quick_capture.delete(),
            Action::Left => self.quick_capture.move_left(),
            Action::Right => self.quick_capture.move_right(),
            Action::Home => self.quick_capture.home(),
            Action::End => self.quick_capture.end_key(),
            Action::Char(c) => self.quick_capture.push_char(c),
            _ => {
                // Handle raw char input for keys not mapped to Action::Char
                if let KeyCode::Char(c) = key.code {
                    self.quick_capture.push_char(c);
                }
            }
        }
        false
    }

    fn handle_command_palette_key(&mut self, key: KeyEvent, action: Action) -> bool {
        match action {
            Action::Escape => {
                self.overlay = None;
            }
            Action::Enter => {
                if let Some(cmd) = self.command_palette.selected_command() {
                    let name = cmd.name.clone();
                    self.overlay = None;
                    self.execute_command(&name);
                } else {
                    self.overlay = None;
                }
            }
            Action::Up => self.command_palette.move_up(),
            Action::Down => self.command_palette.move_down(),
            Action::Backspace => self.command_palette.backspace(),
            Action::Char(c) => self.command_palette.push_char(c),
            _ => {
                if let KeyCode::Char(c) = key.code {
                    self.command_palette.push_char(c);
                }
            }
        }
        false
    }

    fn handle_help_key(&mut self, action: Action) -> bool {
        match action {
            Action::Escape | Action::ToggleHelp => {
                self.overlay = None;
            }
            Action::Down | Action::PageDown => self.help_state.scroll_down(),
            Action::Up | Action::PageUp => self.help_state.scroll_up(),
            Action::Quit => return true,
            _ => {}
        }
        false
    }

    fn handle_focus_key(&mut self, action: Action) -> bool {
        match action {
            Action::Escape | Action::ToggleFocus => {
                self.end_focus();
            }
            Action::PauseFocus | Action::Char('p') => {
                if let Some(fs) = &mut self.focus_state {
                    fs.toggle_pause();
                }
            }
            Action::LogDistraction | Action::Char('d') => {
                if let Some(fs) = &mut self.focus_state {
                    fs.log_distraction();
                    self.set_status("Distraction logged");
                }
            }
            Action::Quit => return true,
            _ => {}
        }
        false
    }

    fn handle_new_task_key(&mut self, key: KeyEvent, action: Action) -> bool {
        match action {
            Action::Escape => {
                self.overlay = None;
                self.new_task_input.clear();
            }
            Action::Enter => {
                let input = self.new_task_input.trim().to_string();
                if !input.is_empty() {
                    let parsed = crate::parser::parse(&input);
                    let new_task: NewTask = parsed.into();
                    match self.db.create_task(&new_task) {
                        Ok(_) => self.set_status(format!("Added: {}", input.chars().take(40).collect::<String>())),
                        Err(e) => self.set_status(format!("Error: {}", e)),
                    }
                }
                self.overlay = None;
                self.new_task_input.clear();
            }
            Action::Backspace => {
                if self.new_task_cursor > 0 {
                    self.new_task_cursor -= 1;
                    self.new_task_input.remove(self.new_task_cursor);
                }
            }
            Action::Left => {
                if self.new_task_cursor > 0 {
                    self.new_task_cursor -= 1;
                }
            }
            Action::Right => {
                if self.new_task_cursor < self.new_task_input.len() {
                    self.new_task_cursor += 1;
                }
            }
            Action::Char(c) => {
                self.new_task_input.insert(self.new_task_cursor, c);
                self.new_task_cursor += c.len_utf8();
            }
            _ => {
                if let KeyCode::Char(c) = key.code {
                    self.new_task_input.insert(self.new_task_cursor, c);
                    self.new_task_cursor += c.len_utf8();
                }
            }
        }
        false
    }

    fn execute_command(&mut self, cmd: &str) {
        match cmd {
            "dashboard" => self.view = ActiveView::Dashboard,
            "today" => self.view = ActiveView::Today,
            "week" => self.view = ActiveView::Week,
            "month" => self.view = ActiveView::Month,
            "habits" => self.view = ActiveView::Habits,
            "notes" => self.view = ActiveView::Notes,
            "journal" => self.view = ActiveView::Journal,
            "stats" => self.view = ActiveView::Stats,
            "help" => {
                self.help_state = HelpState::default();
                self.overlay = Some(ActiveOverlay::Help);
            }
            "quit" => self.should_quit = true,
            "task add" => self.start_new_task(),
            "snapshot create" => {
                let snaps = snapshot::Snapshots::new(&self.config);
                match snaps.create(None) {
                    Ok(p) => self.set_status(format!("Snapshot: {}", p.display())),
                    Err(e) => self.set_status(format!("Snapshot failed: {}", e)),
                }
            }
            _ => {
                self.set_status(format!("Command: {}", cmd));
            }
        }
    }

    // ── Rendering ──────────────────────────────────────────────────────────────

    pub fn render(&mut self, f: &mut Frame) {
        if self.show_splash {
            crate::views::render_splash(f, f.area(), &self.theme);
            return;
        }

        let area = f.area();

        // Main layout: content + status bar
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        let content_area = chunks[0];
        let status_area = chunks[1];

        // Render current view
        self.render_view(f, content_area);

        // Status bar
        let done = self.db.tasks_completed_today().unwrap_or(0);
        let open = self.db.tasks_open_count().unwrap_or(0);
        let focus_mins = self.db.focus_minutes_today().unwrap_or(0);
        let mode = if self.overlay.is_some() { "INSERT" } else { "NORMAL" };

        let status_text = if let Some((msg, _)) = &self.status_msg {
            msg.clone()
        } else {
            String::new()
        };

        if !status_text.is_empty() {
            use ratatui::{style::Style, text::Span, widgets::Paragraph};
            f.render_widget(
                Paragraph::new(Span::styled(
                    format!(" ✓ {}", status_text),
                    Style::default().fg(self.theme.ok).bg(self.theme.bg_alt),
                )),
                status_area,
            );
        } else {
            crate::views::render_statusbar(
                f,
                status_area,
                mode,
                self.view.name(),
                done,
                open,
                focus_mins,
                self.battery,
                &self.theme,
            );
        }

        // Render overlay on top
        self.render_overlay(f, area);
    }

    fn render_view(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        match self.view {
            ActiveView::Dashboard => {
                match DashboardData::load(&self.db) {
                    Ok(data) => dashboard::render(f, area, &data, &self.theme),
                    Err(e) => self.render_error(f, area, &e.to_string()),
                }
            }
            ActiveView::Today => {
                match TodayData::load(&self.db) {
                    Ok(data) => {
                        let mut state = self.task_list_state.clone();
                        today::render(f, area, &data, &mut state, &self.theme);
                        self.task_list_state = state;
                    }
                    Err(e) => self.render_error(f, area, &e.to_string()),
                }
            }
            ActiveView::Week => {
                match WeekData::load(&self.db, self.week_offset) {
                    Ok(data) => week::render(f, area, &data, &self.theme),
                    Err(e) => self.render_error(f, area, &e.to_string()),
                }
            }
            ActiveView::Month => {
                match MonthData::load(&self.db, self.month_offset) {
                    Ok(data) => month::render(f, area, &data, &self.theme),
                    Err(e) => self.render_error(f, area, &e.to_string()),
                }
            }
            _ => {
                self.render_placeholder(f, area);
            }
        }
    }

    fn render_overlay(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        if let Some(overlay) = &self.overlay {
            match overlay {
                ActiveOverlay::QuickCapture => {
                    crate::overlays::quick_capture::render(f, area, &self.quick_capture, &self.theme);
                }
                ActiveOverlay::CommandPalette => {
                    let mut state = CommandPaletteState::new();
                    std::mem::swap(&mut state, &mut self.command_palette);
                    crate::overlays::command_palette::render(f, area, &mut state, &self.theme);
                    std::mem::swap(&mut state, &mut self.command_palette);
                }
                ActiveOverlay::Help => {
                    crate::overlays::help::render(f, area, &self.help_state, &self.theme);
                }
                ActiveOverlay::Focus => {
                    if let Some(ref fs) = self.focus_state {
                        crate::overlays::focus::render(f, area, fs, &self.theme);
                    }
                }
                ActiveOverlay::NewTaskInput => {
                    self.render_new_task_input(f, area);
                }
            }
        }
    }

    fn render_new_task_input(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        use ratatui::{
            style::{Modifier, Style},
            text::{Line, Span},
            widgets::{Block, BorderType, Borders, Clear, Paragraph},
        };

        let width = (area.width * 2 / 3).max(50).min(area.width);
        let popup = crate::views::centered_rect(width, 5, area);
        f.render_widget(Clear, popup);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" new task ")
            .title_style(Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD))
            .border_style(Style::default().fg(self.theme.accent))
            .style(Style::default().bg(self.theme.bg_alt));

        let inner = block.inner(popup);
        f.render_widget(block, popup);

        let before = &self.new_task_input[..self.new_task_cursor];
        let at = self.new_task_input[self.new_task_cursor..].chars().next().unwrap_or(' ');
        let after: String = self.new_task_input[self.new_task_cursor..].chars().skip(1).collect();

        let lines = vec![
            Line::from(Span::styled(
                "  title · #tag !p1 ~30m friday 4pm",
                Style::default().fg(self.theme.fg_dim),
            )),
            Line::from(vec![
                Span::styled("> ", Style::default().fg(self.theme.accent)),
                Span::styled(before, Style::default().fg(self.theme.fg)),
                Span::styled(
                    at.to_string(),
                    Style::default().fg(self.theme.bg).bg(self.theme.accent),
                ),
                Span::styled(&after, Style::default().fg(self.theme.fg)),
            ]),
            Line::from(Span::styled(
                "  Enter to save  ·  Esc to cancel",
                Style::default().fg(self.theme.fg_dim),
            )),
        ];

        f.render_widget(
            Paragraph::new(ratatui::text::Text::from(lines)),
            inner,
        );
    }

    fn render_placeholder(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        use ratatui::{
            style::{Modifier, Style},
            text::Span,
            widgets::{Block, BorderType, Borders, Paragraph},
        };
        let name = self.view.name();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(format!(" {} ", name))
            .title_style(Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD))
            .border_style(Style::default().fg(self.theme.fg_dim))
            .style(Style::default().bg(self.theme.bg));
        let inner = block.inner(area);
        f.render_widget(block, area);
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("  {} view — coming in a future release", name),
                Style::default().fg(self.theme.fg_dim),
            )),
            inner,
        );
    }

    fn render_error(&self, f: &mut Frame, area: ratatui::layout::Rect, msg: &str) {
        use ratatui::{style::Style, text::Span, widgets::Paragraph};
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("  Error: {}", msg),
                Style::default().fg(self.theme.err),
            )),
            area,
        );
    }
}
