mod app;
mod config;
mod db;
mod keymap;
mod macos;
mod models;
mod overlays;
mod parser;
mod snapshot;
mod theme;
mod vault;
mod views;

use anyhow::Result;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, time::Duration};

fn main() -> Result<()> {
    // Restore terminal on panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    // CLI mode — handle subcommands without the TUI
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        return run_cli(&args);
    }

    // TUI mode
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = app::App::new()?;
    let result = run_app(&mut terminal, &mut app);

    // Always restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut app::App,
) -> Result<()> {
    loop {
        terminal.draw(|f| app.render(f))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if app.handle_key(key) || app.should_quit {
                    break;
                }
            }
        }

        app.tick();
    }

    Ok(())
}

fn run_cli(args: &[String]) -> Result<()> {
    let db = db::Database::open()?;

    match args[0].as_str() {
        "capture" | "add" => {
            let text = args[1..].join(" ");
            if text.trim().is_empty() {
                eprintln!("Usage: aurora capture <task description>");
                eprintln!("  Examples:");
                eprintln!("    aurora capture \"Buy milk tomorrow #personal\"");
                eprintln!("    aurora capture \"Submit report friday 4pm #work !p1 ~30m\"");
                return Ok(());
            }
            let parsed = parser::parse(&text);
            let new_task: models::NewTask = parsed.into();
            let id = db.create_task(&new_task)?;
            println!("Captured task #{}: {}", id, args[1..].join(" "));
        }

        "today" => {
            let tasks = db.tasks_due_today()?;
            if tasks.is_empty() {
                println!("No tasks for today.");
            } else {
                println!("Today's tasks:");
                for t in &tasks {
                    let mark = if t.is_done() { "x" } else { " " };
                    let pri = match t.priority {
                        models::Priority::P0 => "!!!",
                        models::Priority::P1 => "!! ",
                        models::Priority::P2 => "!  ",
                        _ => "   ",
                    };
                    let due = t
                        .due_at
                        .map(|d| format!(" [{}]", d.format("%H:%M")))
                        .unwrap_or_default();
                    println!("  [{}] #{} {}{} {}", mark, t.id, pri, due, t.title);
                }
            }
        }

        "version" | "--version" | "-V" => {
            println!("AURORA v0.1.0 — A Unified, Refined, Organized, Resilient Agenda");
            println!("  « your day, composed »");
        }

        "help" | "--help" | "-h" => {
            print_cli_help();
        }

        "snapshot" => {
            let snaps = snapshot::Snapshots::new(&config::load()?);
            match args.get(1).map(|s| s.as_str()) {
                Some("create") => {
                    let label = args.get(2).map(|s| s.as_str());
                    let path = snaps.create(label)?;
                    println!("Snapshot created: {}", path.display());
                }
                Some("list") => {
                    let list = snaps.list()?;
                    if list.is_empty() {
                        println!("No snapshots found.");
                    } else {
                        for (name, _) in &list {
                            println!("  {}", name);
                        }
                    }
                }
                _ => {
                    eprintln!("Usage: aurora snapshot [create|list]");
                }
            }
        }

        "export" => {
            let format = args.get(1).map(|s| s.as_str()).unwrap_or("markdown");
            match format {
                "markdown" => {
                    let tasks = db.all_tasks()?;
                    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                    println!("# AURORA Export — {}", today);
                    println!();
                    println!("## Tasks");
                    println!();
                    for t in &tasks {
                        let check = if t.is_done() { "x" } else { " " };
                        let due = t
                            .due_at
                            .map(|d| format!(" (due {})", d.format("%Y-%m-%d")))
                            .unwrap_or_default();
                        let tags = if t.tags.is_empty() {
                            String::new()
                        } else {
                            format!(
                                " {}",
                                t.tags
                                    .iter()
                                    .map(|g| format!("#{}", g))
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            )
                        };
                        println!("- [{}] {}{}{}", check, t.title, due, tags);
                    }
                }
                _ => {
                    eprintln!("Supported export formats: markdown");
                }
            }
        }

        unknown => {
            eprintln!("aurora: unknown command '{}'", unknown);
            eprintln!("Run 'aurora help' for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn print_cli_help() {
    println!("AURORA v0.1.0 — A Unified, Refined, Organized, Resilient Agenda");
    println!("  « your day, composed »");
    println!();
    println!("USAGE:");
    println!("  aurora                    Launch the TUI");
    println!("  aurora <command> [args]   Run a CLI command");
    println!();
    println!("COMMANDS:");
    println!("  capture <text>            Quick capture a task (NLP parsed)");
    println!("  today                     Print today's task list");
    println!("  export [markdown]         Export data to stdout");
    println!("  snapshot create [label]   Create a backup snapshot");
    println!("  snapshot list             List all snapshots");
    println!("  version                   Show version info");
    println!("  help                      Show this help");
    println!();
    println!("QUICK CAPTURE SYNTAX:");
    println!("  aurora capture \"Submit report friday 4pm #work !p1 ~30m\"");
    println!();
    println!("  #tag          Add a tag");
    println!("  !p0..!p4      Set priority (p0=urgent, p4=someday)");
    println!("  ~30m ~2h      Set time estimate");
    println!("  today         Due today");
    println!("  tomorrow      Due tomorrow");
    println!("  monday..sun   Due on a weekday");
    println!("  4pm 14:30     Due time");
    println!();
    println!("KEYBOARD QUICK REFERENCE (inside TUI):");
    println!("  ?             Help overlay");
    println!("  Space         Quick capture");
    println!("  Ctrl+P / :    Command palette");
    println!("  g d/t/w/m     Navigate to Dashboard/Today/Week/Month");
    println!("  n             New task");
    println!("  x             Complete selected task");
    println!("  F             Focus mode");
    println!("  Q             Quit");
    println!();
    println!("DATA:");
    println!("  DB:     ~/.aurora/aurora.db");
    println!("  Vault:  ~/Documents/AURORA/");
    println!("  Config: ~/.config/aurora/config.toml");
}
