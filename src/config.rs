use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub appearance: AppearanceConfig,
    pub behavior: BehaviorConfig,
    pub focus: FocusConfig,
    pub notifications: NotificationsConfig,
    pub vault: VaultConfig,
    pub startup: StartupConfig,
    pub keymap: KeymapConfig,
    pub dashboard: DashboardConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub first_day_of_week: String,
    pub clock: String,
    pub date_format: String,
    pub time_format: String,
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub theme: String,
    pub density: String,
    pub border: String,
    pub nerd_font: bool,
    pub animations: bool,
    pub follow_macos_dark_mode: bool,
    pub light_theme: String,
    pub dark_theme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    pub auto_review_at: String,
    pub default_priority: i64,
    pub default_energy: String,
    pub default_estimate_m: i64,
    pub warn_overcommit: bool,
    pub overcommit_threshold_h: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FocusConfig {
    pub pomodoro_work_m: u64,
    pub pomodoro_break_m: u64,
    pub long_break_m: u64,
    pub cycles_to_long: u32,
    pub sounds: bool,
    pub sound_pack: String,
    pub do_not_disturb: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationsConfig {
    pub native_macos: bool,
    pub sound: bool,
    pub upcoming_event_lead_min: i64,
    pub end_of_day_summary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VaultConfig {
    pub location: String,
    pub encrypt: bool,
    pub auto_snapshot: String,
    pub snapshot_keep: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StartupConfig {
    pub view: String,
    pub splash: bool,
    pub splash_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeymapConfig {
    pub preset: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DashboardConfig {
    pub widgets: Vec<String>,
}

// ── Defaults ──────────────────────────────────────────────────────────────────

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            appearance: AppearanceConfig::default(),
            behavior: BehaviorConfig::default(),
            focus: FocusConfig::default(),
            notifications: NotificationsConfig::default(),
            vault: VaultConfig::default(),
            startup: StartupConfig::default(),
            keymap: KeymapConfig::default(),
            dashboard: DashboardConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            first_day_of_week: "monday".into(),
            clock: "24h".into(),
            date_format: "%a %d %b".into(),
            time_format: "%H:%M".into(),
            locale: "en".into(),
        }
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: "aurora-borealis".into(),
            density: "comfortable".into(),
            border: "rounded".into(),
            nerd_font: false,
            animations: true,
            follow_macos_dark_mode: true,
            light_theme: "paper".into(),
            dark_theme: "aurora-borealis".into(),
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            auto_review_at: "21:00".into(),
            default_priority: 3,
            default_energy: "medium".into(),
            default_estimate_m: 30,
            warn_overcommit: true,
            overcommit_threshold_h: 8,
        }
    }
}

impl Default for FocusConfig {
    fn default() -> Self {
        Self {
            pomodoro_work_m: 25,
            pomodoro_break_m: 5,
            long_break_m: 20,
            cycles_to_long: 4,
            sounds: false,
            sound_pack: "rain".into(),
            do_not_disturb: true,
        }
    }
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            native_macos: true,
            sound: false,
            upcoming_event_lead_min: 5,
            end_of_day_summary: true,
        }
    }
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            location: "~/Documents/AURORA".into(),
            encrypt: false,
            auto_snapshot: "daily".into(),
            snapshot_keep: 30,
        }
    }
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            view: "dashboard".into(),
            splash: true,
            splash_seconds: 1.5,
        }
    }
}

impl Default for KeymapConfig {
    fn default() -> Self {
        Self {
            preset: "default".into(),
        }
    }
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            widgets: vec![
                "today.summary".into(),
                "today.plan".into(),
                "focus.weekbar".into(),
                "goals.quarter".into(),
                "habits.today".into(),
                "inbox".into(),
                "quote".into(),
            ],
        }
    }
}

// ── Load / save ───────────────────────────────────────────────────────────────

pub fn config_path() -> PathBuf {
    if let Some(cfg) = dirs::config_dir() {
        cfg.join("aurora").join("config.toml")
    } else {
        PathBuf::from(".aurora_config.toml")
    }
}

pub fn load() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(&path)?;
    let config: Config = toml::from_str(&text)?;
    Ok(config)
}

pub fn save(config: &Config) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(config)?;
    std::fs::write(&path, text)?;
    Ok(())
}

pub fn vault_path(config: &Config) -> PathBuf {
    let raw = &config.vault.location;
    if raw.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            return home.join(&raw[2..]);
        }
    }
    PathBuf::from(raw)
}
