use anyhow::Result;
use chrono::Local;
use std::path::PathBuf;

use crate::config::Config;
use crate::db::db_path;

pub struct Snapshots {
    pub dir: PathBuf,
    pub keep: u32,
}

impl Snapshots {
    pub fn new(config: &Config) -> Self {
        let dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".aurora")
            .join("snapshots");
        Snapshots {
            dir,
            keep: config.vault.snapshot_keep,
        }
    }

    pub fn create(&self, label: Option<&str>) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.dir)?;
        let ts = Local::now().format("%Y-%m-%dT%H-%MZ").to_string();
        let name = match label {
            Some(l) => format!("{}_{}.db.bak", ts, slugify(l)),
            None => format!("{}.db.bak", ts),
        };
        let dest = self.dir.join(&name);
        let src = db_path()?;
        if src.exists() {
            std::fs::copy(&src, &dest)?;
        }
        self.prune()?;
        Ok(dest)
    }

    pub fn list(&self) -> Result<Vec<(String, PathBuf)>> {
        if !self.dir.exists() {
            return Ok(vec![]);
        }
        let mut entries: Vec<_> = std::fs::read_dir(&self.dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|x| x == "bak")
                    .unwrap_or(false)
            })
            .collect();
        entries.sort_by_key(|e| e.file_name());
        entries.reverse();
        Ok(entries
            .into_iter()
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                (name, e.path())
            })
            .collect())
    }

    pub fn restore(&self, path: &PathBuf) -> Result<()> {
        let dest = db_path()?;
        // Backup current before restoring
        let _ = self.create(Some("pre-restore"));
        std::fs::copy(path, &dest)?;
        Ok(())
    }

    fn prune(&self) -> Result<()> {
        let list = self.list()?;
        if list.len() as u32 > self.keep {
            let to_delete = &list[self.keep as usize..];
            for (_, path) in to_delete {
                let _ = std::fs::remove_file(path);
            }
        }
        Ok(())
    }
}

fn slugify(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

pub fn maybe_auto_snapshot(config: &Config) {
    if config.vault.auto_snapshot == "daily" {
        let marker_path = dirs::home_dir()
            .unwrap_or_default()
            .join(".aurora")
            .join(".last_snapshot");
        let today = Local::now().format("%Y-%m-%d").to_string();
        let last = std::fs::read_to_string(&marker_path).unwrap_or_default();
        if last.trim() != today {
            let snaps = Snapshots::new(config);
            if snaps.create(None).is_ok() {
                let _ = std::fs::write(&marker_path, &today);
            }
        }
    }
}
