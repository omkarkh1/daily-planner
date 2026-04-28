use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Quit,
    QuitPane,
    GPrefix,
    NavigateTo(NavTarget),
    ToggleHelp,
    OpenQuickCapture,
    OpenCommandPalette,
    OpenGlobalSearch,
    OpenSettings,
    Undo,
    Redo,

    // Task actions
    NewTask,
    ToggleComplete,
    DeleteItem,
    SetDue,
    StartTimer,
    CyclePriority,
    CycleEnergy,
    SnoozeItem,
    OpenDetail,
    MultiSelect,

    // Calendar actions
    NewEvent,
    FindFreeSlot,
    CycleCalendars,

    // Focus
    ToggleFocus,
    StartPomodoro,
    PauseFocus,
    LogDistraction,

    // Navigation within view
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    GotoTop,
    GotoBottom,
    NextPane,
    PrevPane,
    NextDay,
    PrevDay,

    // Input / text
    Char(char),
    Backspace,
    Delete,
    Enter,
    Escape,
    Tab,
    ShiftTab,
    Home,
    End,

    // Workspace
    SwitchWorkspace(u8),

    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NavTarget {
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

pub struct Keymap {
    pub preset: String,
}

impl Keymap {
    pub fn new(preset: &str) -> Self {
        Keymap { preset: preset.to_string() }
    }

    pub fn resolve(&self, key: &KeyEvent, g_prefix: bool) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        if g_prefix {
            return match key.code {
                KeyCode::Char('d') => Action::NavigateTo(NavTarget::Dashboard),
                KeyCode::Char('t') => Action::NavigateTo(NavTarget::Today),
                KeyCode::Char('w') => Action::NavigateTo(NavTarget::Week),
                KeyCode::Char('m') => Action::NavigateTo(NavTarget::Month),
                KeyCode::Char('y') => Action::NavigateTo(NavTarget::Year),
                KeyCode::Char('p') => Action::NavigateTo(NavTarget::Projects),
                KeyCode::Char('h') => Action::NavigateTo(NavTarget::Habits),
                KeyCode::Char('n') => Action::NavigateTo(NavTarget::Notes),
                KeyCode::Char('j') => Action::NavigateTo(NavTarget::Journal),
                KeyCode::Char('s') => Action::NavigateTo(NavTarget::Stats),
                KeyCode::Char('r') => Action::NavigateTo(NavTarget::Reviews),
                KeyCode::Char('g') => Action::GotoTop,
                _ => Action::None,
            };
        }

        // Ctrl combos (not g_prefix)
        if ctrl {
            return match key.code {
                KeyCode::Char('p') => Action::OpenCommandPalette,
                KeyCode::Char('f') => Action::OpenGlobalSearch,
                KeyCode::Char('z') => {
                    if shift { Action::Redo } else { Action::Undo }
                }
                KeyCode::Char(',') => Action::OpenSettings,
                KeyCode::Char('1') => Action::SwitchWorkspace(1),
                KeyCode::Char('2') => Action::SwitchWorkspace(2),
                KeyCode::Char('3') => Action::SwitchWorkspace(3),
                _ => Action::None,
            };
        }

        match key.code {
            // Universal
            KeyCode::Char('q') => Action::QuitPane,
            KeyCode::Char('Q') => Action::Quit,
            KeyCode::Char('?') => Action::ToggleHelp,
            KeyCode::Char(' ') => Action::OpenQuickCapture,
            KeyCode::Char(':') => Action::OpenCommandPalette,
            KeyCode::Esc => Action::Escape,
            KeyCode::Enter => Action::Enter,
            KeyCode::Tab => Action::NextPane,
            KeyCode::BackTab => Action::PrevPane,
            KeyCode::Backspace => Action::Backspace,
            KeyCode::Delete => Action::Delete,
            KeyCode::Home => Action::Home,
            KeyCode::End => Action::End,

            // Navigation
            KeyCode::Char('g') => Action::GPrefix,
            KeyCode::Char('G') => Action::GotoBottom,
            KeyCode::Char('j') | KeyCode::Down => Action::Down,
            KeyCode::Char('k') | KeyCode::Up => Action::Up,
            KeyCode::Char('h') | KeyCode::Left => Action::Left,
            KeyCode::Char('l') | KeyCode::Right => Action::Right,
            KeyCode::Char('J') => Action::PageDown,
            KeyCode::Char('K') => Action::PageUp,
            KeyCode::Char('H') => Action::PrevDay,
            KeyCode::Char('L') => Action::NextDay,
            KeyCode::PageDown => Action::PageDown,
            KeyCode::PageUp => Action::PageUp,

            // Tasks
            KeyCode::Char('n') => Action::NewTask,
            KeyCode::Char('x') => Action::ToggleComplete,
            KeyCode::Char('D') => Action::DeleteItem,
            KeyCode::Char('d') => Action::SetDue,
            KeyCode::Char('t') => Action::StartTimer,
            KeyCode::Char('p') => Action::CyclePriority,
            KeyCode::Char('e') => Action::CycleEnergy,
            KeyCode::Char('s') => Action::SnoozeItem,
            KeyCode::Char('o') => Action::OpenDetail,
            KeyCode::Char('v') => Action::MultiSelect,

            // Calendar
            KeyCode::Char('N') => Action::NewEvent,
            KeyCode::Char('f') => Action::FindFreeSlot,
            KeyCode::Char('c') => Action::CycleCalendars,

            // Focus
            KeyCode::Char('F') => Action::ToggleFocus,
            KeyCode::Char('P') => Action::StartPomodoro,

            KeyCode::Char(ch) => Action::Char(ch),
            _ => Action::None,
        }
    }
}
