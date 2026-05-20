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
use warpui::color::ColorU;

pub mod gradient;

#[cfg(test)]
#[path = "saved_projects_test.rs"]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub cwd: PathBuf,
    pub color: String,
    /// Cortex: optional sub-project declaration. `None` (default) means the
    /// project has no sub-projects — the picker row stays a flat entry.
    /// `Some(All)` exposes every direct subdirectory of `cwd` (excluding
    /// dotfile-prefixed ones) as a sub-project. `Some(Named(names))` exposes
    /// only the listed subdirectory names, in the order given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_projects: Option<SubProjects>,
}

/// Either the literal string `"all"` or an array of subdirectory names.
/// Encoded as an untagged enum so JSON can use the shape it prefers:
/// `"sub_projects": "all"` or `"sub_projects": ["foo", "bar"]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SubProjects {
    /// JSON-side: the literal string `"all"`.
    All(AllSentinel),
    /// JSON-side: an array of subdirectory names relative to the parent cwd.
    Named(Vec<String>),
}

/// One-variant enum whose only legal serialization is the lowercase string
/// `"all"`. Used as the carrier of `SubProjects::All` so the untagged enum
/// can disambiguate the string form from the array form.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AllSentinel {
    All,
}

/// A single resolved sub-project: the display name (its directory leaf) and
/// the absolute cwd to open. Built on demand by `Project::resolved_sub_projects`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubProject {
    pub name: String,
    pub cwd: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectsConfig {
    #[serde(default)]
    pub projects: Vec<Project>,
}

impl Project {
    /// Cortex: resolves this project's `sub_projects` declaration into a flat
    /// list of `SubProject`s, ready for the picker fly-out. `None` returns an
    /// empty vec. `Some(All)` reads `cwd` and lists each direct subdirectory
    /// (alphabetically, case-insensitive), skipping entries whose name starts
    /// with `.`. `Some(Named(_))` joins each declared name onto `cwd`
    /// preserving JSON order; no filesystem check — a missing path silently
    /// no-ops if the user clicks it.
    ///
    /// I/O errors (unreadable cwd) → empty vec, debug-logged.
    pub fn resolved_sub_projects(&self) -> Vec<SubProject> {
        match self.sub_projects.as_ref() {
            None => Vec::new(),
            Some(SubProjects::All(_)) => {
                let entries = match fs::read_dir(&self.cwd) {
                    Ok(entries) => entries,
                    Err(e) => {
                        log::debug!(
                            "sub_projects=\"all\" but {} unreadable: {}",
                            self.cwd.display(),
                            e
                        );
                        return Vec::new();
                    }
                };
                let mut subs: Vec<SubProject> = entries
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                    .filter_map(|entry| {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        if name.starts_with('.') {
                            return None;
                        }
                        Some(SubProject {
                            cwd: entry.path(),
                            name,
                        })
                    })
                    .collect();
                subs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                subs
            }
            Some(SubProjects::Named(names)) => names
                .iter()
                .map(|n| SubProject {
                    name: n.clone(),
                    cwd: self.cwd.join(n),
                })
                .collect(),
        }
    }
}

/// Reads `~/.warp-oss/projects.json` (or whatever `warp_home_projects_file_path()` resolves
/// to for the current channel), expands `~` in each `cwd`, and returns the list in the
/// order entries appear in the JSON `projects` array (top-of-file first). Missing file or
/// empty `projects` array → empty vec (debug-logged, not an error). Malformed JSON → empty
/// vec with an error log so the picker never crashes the app over a typo.
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
    projects
}

fn expand_tilde(path: &Path) -> Option<PathBuf> {
    let s = path.to_str()?;
    let rest = s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\"))?;
    let home = dirs::home_dir()?;
    Some(home.join(rest))
}

/// Returns the saved project whose `cwd` is the longest prefix of `path`,
/// or `None` if no project covers it. Used at session-restore time to
/// reapply a project's accent color to a restored tab whose persisted cwd
/// lives under (or at) the project root, since `cortex_accent` is not part
/// of the persisted `TabSnapshot`.
pub fn project_for_path<'a>(path: &Path, projects: &'a [Project]) -> Option<&'a Project> {
    let needle = canonicalize_or_self(path);
    projects
        .iter()
        .filter(|p| needle.starts_with(canonicalize_or_self(&p.cwd)))
        .max_by_key(|p| canonicalize_or_self(&p.cwd).as_os_str().len())
}

/// Cortex: returns the accent `ColorU` for a restored tab's cwd, picking
/// between the parent project's hex color and the gradient shade of a
/// matching sub-project. Falls back to `None` if nothing matches or the
/// parent's hex is malformed.
///
/// Resolution order: find the longest-prefix parent project; if the parent
/// has sub-projects and one of them is an *exact* cwd match for `path`,
/// return that sub-project's gradient shade; otherwise return the parent's
/// color. The gradient is deterministic from (parent hex, index, total),
/// so no extra persisted state is needed to keep sub-project tabs colored
/// consistently across restarts.
pub fn accent_for_path(path: &Path, projects: &[Project]) -> Option<ColorU> {
    let parent = project_for_path(path, projects)?;
    let needle = canonicalize_or_self(path);

    let subs = parent.resolved_sub_projects();
    if !subs.is_empty() {
        let total = subs.len();
        for (idx, sub) in subs.iter().enumerate() {
            if canonicalize_or_self(&sub.cwd) == needle {
                if let Some(hex) = gradient::sub_project_color(&parent.color, idx, total) {
                    if let Ok(c) = warp_core::ui::color::hex_color::coloru_from_hex_string(&hex) {
                        return Some(c);
                    }
                }
                break;
            }
        }
    }

    warp_core::ui::color::hex_color::coloru_from_hex_string(&parent.color).ok()
}

fn canonicalize_or_self(p: &Path) -> PathBuf {
    dunce::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}
