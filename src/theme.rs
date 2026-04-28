use ratatui::style::Color;
use ratatui::widgets::BorderType;

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: &'static str,
    pub bg: Color,
    pub bg_alt: Color,
    pub bg_inset: Color,
    pub fg: Color,
    pub fg_dim: Color,
    pub accent: Color,
    pub accent_alt: Color,
    pub ok: Color,
    pub warn: Color,
    pub err: Color,
    pub info: Color,
    pub border_type: BorderType,
}

impl Theme {
    pub fn by_name(name: &str) -> Self {
        match name {
            "nord" => Theme::nord(),
            "tokyo-night" => Theme::tokyo_night(),
            "gruvbox-dark" => Theme::gruvbox_dark(),
            "dracula" => Theme::dracula(),
            "paper" => Theme::paper(),
            _ => Theme::aurora_borealis(),
        }
    }

    pub fn aurora_borealis() -> Self {
        Self {
            name: "aurora-borealis",
            bg: rgb(11, 15, 26),
            bg_alt: rgb(17, 23, 42),
            bg_inset: rgb(7, 10, 20),
            fg: rgb(216, 224, 255),
            fg_dim: rgb(108, 117, 147),
            accent: rgb(122, 240, 196),
            accent_alt: rgb(197, 163, 255),
            ok: rgb(122, 217, 127),
            warn: rgb(245, 200, 122),
            err: rgb(255, 122, 138),
            info: rgb(122, 208, 255),
            border_type: BorderType::Rounded,
        }
    }

    pub fn nord() -> Self {
        Self {
            name: "nord",
            bg: rgb(46, 52, 64),
            bg_alt: rgb(59, 66, 82),
            bg_inset: rgb(36, 41, 51),
            fg: rgb(236, 239, 244),
            fg_dim: rgb(76, 86, 106),
            accent: rgb(136, 192, 208),
            accent_alt: rgb(180, 142, 173),
            ok: rgb(163, 190, 140),
            warn: rgb(235, 203, 139),
            err: rgb(191, 97, 106),
            info: rgb(129, 161, 193),
            border_type: BorderType::Plain,
        }
    }

    pub fn tokyo_night() -> Self {
        Self {
            name: "tokyo-night",
            bg: rgb(26, 27, 46),
            bg_alt: rgb(36, 40, 59),
            bg_inset: rgb(22, 22, 30),
            fg: rgb(192, 202, 245),
            fg_dim: rgb(86, 95, 137),
            accent: rgb(125, 207, 255),
            accent_alt: rgb(187, 154, 247),
            ok: rgb(158, 206, 106),
            warn: rgb(224, 175, 104),
            err: rgb(247, 118, 142),
            info: rgb(42, 195, 222),
            border_type: BorderType::Rounded,
        }
    }

    pub fn gruvbox_dark() -> Self {
        Self {
            name: "gruvbox-dark",
            bg: rgb(40, 40, 40),
            bg_alt: rgb(50, 48, 47),
            bg_inset: rgb(29, 32, 33),
            fg: rgb(235, 219, 178),
            fg_dim: rgb(124, 111, 100),
            accent: rgb(184, 187, 38),
            accent_alt: rgb(211, 134, 155),
            ok: rgb(142, 192, 124),
            warn: rgb(250, 189, 47),
            err: rgb(251, 73, 52),
            info: rgb(131, 165, 152),
            border_type: BorderType::Plain,
        }
    }

    pub fn dracula() -> Self {
        Self {
            name: "dracula",
            bg: rgb(40, 42, 54),
            bg_alt: rgb(68, 71, 90),
            bg_inset: rgb(33, 34, 44),
            fg: rgb(248, 248, 242),
            fg_dim: rgb(98, 114, 164),
            accent: rgb(80, 250, 123),
            accent_alt: rgb(189, 147, 249),
            ok: rgb(80, 250, 123),
            warn: rgb(241, 250, 140),
            err: rgb(255, 85, 85),
            info: rgb(139, 233, 253),
            border_type: BorderType::Rounded,
        }
    }

    pub fn paper() -> Self {
        Self {
            name: "paper",
            bg: rgb(255, 255, 255),
            bg_alt: rgb(240, 240, 240),
            bg_inset: rgb(250, 250, 250),
            fg: rgb(30, 30, 30),
            fg_dim: rgb(130, 130, 130),
            accent: rgb(0, 90, 180),
            accent_alt: rgb(120, 0, 160),
            ok: rgb(0, 120, 0),
            warn: rgb(180, 100, 0),
            err: rgb(180, 0, 0),
            info: rgb(0, 100, 180),
            border_type: BorderType::Plain,
        }
    }

    pub fn all_names() -> &'static [&'static str] {
        &[
            "aurora-borealis",
            "nord",
            "tokyo-night",
            "gruvbox-dark",
            "dracula",
            "paper",
        ]
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

// Sparkline bar characters (low to high intensity)
pub const SPARK_CHARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
pub const BLOCK_CHARS: [char; 4] = ['░', '▒', '▓', '█'];
pub const PROGRESS_FILLED: char = '▰';
pub const PROGRESS_EMPTY: char = '▱';

pub fn sparkline(values: &[i64], max: i64) -> String {
    if max == 0 {
        return values.iter().map(|_| ' ').collect();
    }
    values
        .iter()
        .map(|&v| {
            if v <= 0 {
                '░'
            } else {
                let idx = ((v * 7) / max).min(7) as usize;
                SPARK_CHARS[idx]
            }
        })
        .collect()
}

pub fn progress_bar(progress: f64, width: usize) -> String {
    let filled = (progress * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;
    format!(
        "{}{}",
        PROGRESS_FILLED.to_string().repeat(filled),
        PROGRESS_EMPTY.to_string().repeat(empty)
    )
}

pub fn mood_dots(mood: i64, max: i64) -> String {
    let filled = mood.max(0) as usize;
    let empty = (max - mood).max(0) as usize;
    format!("{}{}",  "●".repeat(filled), "○".repeat(empty))
}
