use anyhow::Result;
use chrono::{Local, NaiveDate};
use std::path::{Path, PathBuf};

use crate::config::Config;

pub struct Vault {
    pub root: PathBuf,
}

impl Vault {
    pub fn new(config: &Config) -> Self {
        let root = crate::config::vault_path(config);
        Vault { root }
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(self.journal_dir())?;
        std::fs::create_dir_all(self.notes_dir())?;
        Ok(())
    }

    pub fn journal_dir(&self) -> PathBuf {
        self.root.join("journal")
    }

    pub fn notes_dir(&self) -> PathBuf {
        self.root.join("notes")
    }

    pub fn journal_path(&self, date: NaiveDate) -> PathBuf {
        let year = date.format("%Y").to_string();
        let month = date.format("%m").to_string();
        let filename = date.format("%Y-%m-%d.md").to_string();
        self.journal_dir().join(year).join(month).join(filename)
    }

    pub fn read_journal(&self, date: NaiveDate) -> Result<Option<String>> {
        let path = self.journal_path(date);
        if path.exists() {
            Ok(Some(std::fs::read_to_string(path)?))
        } else {
            Ok(None)
        }
    }

    pub fn write_journal(&self, date: NaiveDate, content: &str) -> Result<()> {
        let path = self.journal_path(date);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        atomic_write(&path, content)
    }

    pub fn append_journal(&self, date: NaiveDate, line: &str) -> Result<()> {
        let existing = self.read_journal(date)?.unwrap_or_default();
        let ts = Local::now().format("%H:%M").to_string();
        let new_content = format!("{}\n- **{}** {}", existing.trim_end(), ts, line);
        self.write_journal(date, &new_content)
    }

    pub fn note_path(&self, slug: &str) -> PathBuf {
        self.notes_dir().join(format!("{}.md", slug))
    }

    pub fn read_note(&self, slug: &str) -> Result<Option<String>> {
        let path = self.note_path(slug);
        if path.exists() {
            Ok(Some(std::fs::read_to_string(path)?))
        } else {
            Ok(None)
        }
    }

    pub fn write_note(&self, slug: &str, content: &str) -> Result<()> {
        let path = self.note_path(slug);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        atomic_write(&path, content)
    }

    pub fn create_note(&self, title: &str) -> Result<String> {
        let slug = slugify(title);
        let today = Local::now().format("%Y-%m-%d").to_string();
        let content = format!("# {}\n\ncreated: {}\n\n", title, today);
        self.write_note(&slug, &content)?;
        Ok(slug)
    }

    pub fn all_note_slugs(&self) -> Result<Vec<String>> {
        let dir = self.notes_dir();
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut slugs = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    slugs.push(stem.to_string());
                }
            }
        }
        Ok(slugs)
    }
}

fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
