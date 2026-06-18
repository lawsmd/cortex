//! Cortex-only: the "reviewed checklist" for the code-review panel.
//!
//! Lets the user mark each changed file as reviewed so a large diff becomes a
//! shrinking worklist. The mark is stored as a *fingerprint of the file's diff*
//! at the moment it was checked off — a file reads as reviewed only while its
//! current diff fingerprint still matches the stored one, so any change to the
//! file's diff silently un-reviews it (there is nothing to actively clear).
//!
//! Marks persist across restarts in a local JSON file that never touches the
//! repository working tree (see
//! [`warp_core::paths::warp_home_code_review_reviews_file_path`]), so they are
//! invisible to git and to collaborators on the reviewed branch.

use std::collections::HashMap;
use std::fs;

use pathfinder_color::ColorU;
use warp_core::ui::color::blend::Blend;
use warp_core::ui::theme::color::internal_colors::{neutral_2, neutral_3};
use warp_core::ui::theme::{Fill, WarpTheme};

use crate::code_review::diff_state::{DiffLineType, FileDiff, GitFileStatus};
use crate::ui_components::icons::Icon;

/// On-disk schema: repo identity → (repo-relative file path → diff fingerprint).
type ReviewedMarksFile = HashMap<String, HashMap<String, u64>>;

/// FNV-1a accumulator. We roll our own (rather than reach for
/// `std::collections::hash_map::DefaultHasher`) because the result is persisted
/// to disk and must be **stable across runs and platforms** — `DefaultHasher`
/// is seeded by `RandomState` and is not.
struct Fnv1a(u64);

impl Fnv1a {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn new() -> Self {
        Fnv1a(Self::OFFSET)
    }

    fn eat(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn eat_usize(&mut self, v: usize) {
        self.eat(&(v as u64).to_le_bytes());
    }

    fn finish(self) -> u64 {
        self.0
    }
}

/// A stable fingerprint of a file's *current diff content* — git status plus
/// every hunk's geometry and line text. The file path is intentionally excluded
/// (it is the lookup key, not content); changing a single character in any diff
/// line changes the fingerprint and therefore drops any "reviewed" mark.
pub fn diff_fingerprint(file_diff: &FileDiff) -> u64 {
    let mut h = Fnv1a::new();

    match &file_diff.status {
        GitFileStatus::New => h.eat(&[0]),
        GitFileStatus::Modified => h.eat(&[1]),
        GitFileStatus::Deleted => h.eat(&[2]),
        GitFileStatus::Renamed { old_path } => {
            h.eat(&[3]);
            h.eat(old_path.as_bytes());
        }
        GitFileStatus::Copied { old_path } => {
            h.eat(&[4]);
            h.eat(old_path.as_bytes());
        }
        GitFileStatus::Untracked => h.eat(&[5]),
        GitFileStatus::Conflicted => h.eat(&[6]),
    }

    h.eat(&[file_diff.is_binary as u8]);

    for hunk in file_diff.hunks.iter() {
        h.eat_usize(hunk.old_start_line);
        h.eat_usize(hunk.old_line_count);
        h.eat_usize(hunk.new_start_line);
        h.eat_usize(hunk.new_line_count);
        for line in &hunk.lines {
            let kind: u8 = match line.line_type {
                DiffLineType::Context => 0,
                DiffLineType::Add => 1,
                DiffLineType::Delete => 2,
                DiffLineType::HunkHeader => 3,
            };
            h.eat(&[kind]);
            h.eat(line.text.as_bytes());
            h.eat(&[0xff]); // line separator so adjacent lines can't merge
        }
    }

    h.finish()
}

/// Load/save of the local, never-committed reviewed-marks store. All methods
/// degrade gracefully (returning empty / doing nothing) when the home config
/// directory can't be resolved or the file is missing/corrupt.
pub struct ReviewedMarksStore;

impl ReviewedMarksStore {
    /// Returns the persisted marks for a single repository, keyed by
    /// repo-relative file path → fingerprint-at-time-of-marking.
    pub fn load_for_repo(repo_key: &str) -> HashMap<String, u64> {
        let Some(path) = warp_core::paths::warp_home_code_review_reviews_file_path() else {
            return HashMap::new();
        };
        let Ok(contents) = fs::read_to_string(&path) else {
            return HashMap::new();
        };
        let parsed: ReviewedMarksFile = serde_json::from_str(&contents).unwrap_or_default();
        parsed.get(repo_key).cloned().unwrap_or_default()
    }

    /// Persists the marks for a single repository, preserving any other repos'
    /// entries already in the file. An empty map removes the repo's entry.
    pub fn save_for_repo(repo_key: &str, marks: &HashMap<String, u64>) {
        let Some(path) = warp_core::paths::warp_home_code_review_reviews_file_path() else {
            return;
        };

        // Read-modify-write so marks for other repositories survive.
        let mut parsed: ReviewedMarksFile = fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default();

        if marks.is_empty() {
            parsed.remove(repo_key);
        } else {
            parsed.insert(repo_key.to_string(), marks.clone());
        }

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(serialized) = serde_json::to_string_pretty(&parsed) {
            let _ = fs::write(&path, serialized);
        }
    }
}

/// The icon and tooltip for the header "mark as reviewed" check button, given
/// whether the file is currently reviewed. An outline check-circle invites the
/// click; a solid check confirms the reviewed state.
pub fn reviewed_button_appearance(is_reviewed: bool) -> (Icon, &'static str) {
    if is_reviewed {
        (Icon::Check, "Mark as not reviewed")
    } else {
        (Icon::CheckCircleBroken, "Mark as reviewed")
    }
}

/// Background fill for a file header that has been marked reviewed: the normal
/// neutral header fill with a low-opacity green wash blended over it, so a
/// reviewed file reads as "done" at a glance while keeping its text legible.
pub fn reviewed_header_bg(theme: &WarpTheme, hovered: bool) -> ColorU {
    let base = if hovered {
        neutral_3(theme)
    } else {
        neutral_2(theme)
    };
    let green_opacity = if hovered { 34 } else { 22 };
    Fill::Solid(base)
        .blend(&Fill::Solid(theme.ui_green_color()).with_opacity(green_opacity))
        .into_solid()
}

/// Background fill for a file header whose file is being *removed* (a git
/// deletion): the normal neutral header fill with a low-opacity red wash blended
/// over it, so a removed file reads as "gone" at a glance. Mirrors
/// [`reviewed_header_bg`] but uses the theme's error red instead of green.
pub fn removed_header_bg(theme: &WarpTheme, hovered: bool) -> ColorU {
    let base = if hovered {
        neutral_3(theme)
    } else {
        neutral_2(theme)
    };
    let red_opacity = if hovered { 34 } else { 22 };
    Fill::Solid(base)
        .blend(&Fill::Solid(theme.ui_error_color()).with_opacity(red_opacity))
        .into_solid()
}
