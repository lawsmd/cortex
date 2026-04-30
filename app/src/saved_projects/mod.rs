//! Cortex saved-projects list, fed into the vertical tab panel's bottom "+" picker.
//!
//! Lives at `~/.warp-oss/projects.json` (channel-aware via `warp_core::paths`).
//! See `docs/roadmap/reskin.md` and the SideQuest reference at
//! `~/.config/terminal/docs/sidequest/data-files.md` for design rationale.
//!
//! Named `saved_projects` (not `projects`) to avoid colliding with Warp's
//! existing `projects` module which manages the in-app project list.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "saved_projects_test.rs"]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub cwd: PathBuf,
    pub color: String,
    pub rank: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectsConfig {
    #[serde(default)]
    pub projects: Vec<Project>,
}

/// Reads `~/.warp-oss/projects.json` (or whatever `warp_home_projects_file_path()` resolves
/// to for the current channel), expands `~` in each `cwd`, and returns the list sorted by
/// `rank` ascending. Missing file or empty `projects` array → empty vec (debug-logged, not
/// an error). Malformed JSON → empty vec with an error log so the picker never crashes the
/// app over a typo.
pub fn load_projects() -> Vec<Project> {
    let Some(path) = warp_core::paths::warp_home_projects_file_path() else {
        return vec![];
    };
    load_projects_from(&path)
}

fn load_projects_from(path: &Path) -> Vec<Project> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::debug!("projects.json not found at {}; skipping picker section", path.display());
            return vec![];
        }
        Err(e) => {
            log::warn!("failed to read {}: {}", path.display(), e);
            return vec![];
        }
    };
    let config: ProjectsConfig = match serde_json::from_slice(&bytes) {
        Ok(config) => config,
        Err(e) => {
            log::error!("malformed {}: {}", path.display(), e);
            return vec![];
        }
    };
    let mut projects = config.projects;
    for project in &mut projects {
        if let Some(expanded) = expand_tilde(&project.cwd) {
            project.cwd = expanded;
        }
    }
    projects.sort_by_key(|p| p.rank);
    projects
}

fn expand_tilde(path: &Path) -> Option<PathBuf> {
    let s = path.to_str()?;
    let rest = s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\"))?;
    let home = dirs::home_dir()?;
    Some(home.join(rest))
}
