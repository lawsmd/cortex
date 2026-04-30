//! Theme favorites: a user-pinned list persisted as JSON next to the
//! other file-backed user data (`themes/`, `launch_configurations/`, …).
//!
//! See `docs/themes/favorites.md` for the full design — this module is the
//! disk layer (load / save / prune orphans / mutate the in-memory list).

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::themes::theme::{ThemeKind, WarpThemeConfig};

const CURRENT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FavoritesFile {
    version: u32,
    favorites: Vec<ThemeKind>,
}

impl Default for FavoritesFile {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            favorites: Vec::new(),
        }
    }
}

/// Read the favorites list from disk. Missing file → empty list. Malformed
/// or unknown-version file → empty list, with the bad file moved aside as
/// `<path>.bak` so the user can recover it.
pub fn load_favorites(path: &Path) -> Vec<ThemeKind> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            log::warn!("favorites: failed to read {}: {}", path.display(), e);
            return Vec::new();
        }
    };
    match serde_json::from_slice::<FavoritesFile>(&bytes) {
        Ok(file) if file.version == CURRENT_VERSION => file.favorites,
        Ok(file) => {
            log::warn!(
                "favorites: unexpected version {} in {}; backing up and ignoring",
                file.version,
                path.display()
            );
            backup_file(path);
            Vec::new()
        }
        Err(e) => {
            log::warn!(
                "favorites: malformed JSON in {} ({}); backing up and ignoring",
                path.display(),
                e
            );
            backup_file(path);
            Vec::new()
        }
    }
}

/// Atomically write the favorites list to disk (write to `<path>.tmp` then
/// rename) so a crash mid-write can't leave a half-formed file.
pub fn save_favorites(path: &Path, favorites: &[ThemeKind]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let file = FavoritesFile {
        version: CURRENT_VERSION,
        favorites: favorites.to_vec(),
    };
    let json = serde_json::to_vec_pretty(&file).context("serializing favorites")?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &json).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} to {}", tmp.display(), path.display()))?;
    Ok(())
}

/// Drop favorites whose `ThemeKind` no longer resolves against the current
/// theme registry (e.g. a community theme dropped from a bundle refresh, or
/// a custom theme whose YAML was deleted). Returns `true` if anything was
/// removed — caller should `save_favorites` afterward to persist the prune.
pub fn prune_orphans(favorites: &mut Vec<ThemeKind>, theme_config: &WarpThemeConfig) -> bool {
    let original_len = favorites.len();
    favorites.retain(|k| theme_config.contains_theme(k));
    favorites.len() != original_len
}

/// Append `kind` to the favorites list if not already present. Returns
/// `true` if the list was modified.
pub fn add(favorites: &mut Vec<ThemeKind>, kind: ThemeKind) -> bool {
    if favorites.iter().any(|k| k == &kind) {
        return false;
    }
    favorites.push(kind);
    true
}

/// Remove `kind` from the favorites list. Returns `true` if the list was
/// modified.
pub fn remove(favorites: &mut Vec<ThemeKind>, kind: &ThemeKind) -> bool {
    let len = favorites.len();
    favorites.retain(|k| k != kind);
    favorites.len() != len
}

pub fn is_favorite(favorites: &[ThemeKind], kind: &ThemeKind) -> bool {
    favorites.iter().any(|k| k == kind)
}

fn backup_file(path: &Path) {
    let bak = path.with_extension("json.bak");
    if let Err(e) = fs::rename(path, &bak) {
        log::warn!(
            "favorites: couldn't move {} to {}: {}",
            path.display(),
            bak.display(),
            e
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp_path() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("favorite_themes.json");
        (dir, path)
    }

    #[test]
    fn missing_file_is_empty() {
        let (_dir, path) = tmp_path();
        assert!(load_favorites(&path).is_empty());
    }

    #[test]
    fn round_trip_preserves_mixed_kinds() {
        let (_dir, path) = tmp_path();
        let original = vec![
            ThemeKind::Dracula,
            ThemeKind::Wezterm("Catppuccin Mocha (Gogh)".to_string()),
            ThemeKind::Light,
        ];
        save_favorites(&path, &original).expect("save");
        let loaded = load_favorites(&path);
        assert_eq!(loaded, original);
    }

    #[test]
    fn malformed_json_returns_empty_and_backs_up() {
        let (_dir, path) = tmp_path();
        fs::write(&path, b"{ this is not json }").unwrap();
        assert!(load_favorites(&path).is_empty());
        assert!(path.with_extension("json.bak").exists());
        assert!(!path.exists());
    }

    #[test]
    fn unknown_version_returns_empty_and_backs_up() {
        let (_dir, path) = tmp_path();
        fs::write(
            &path,
            br#"{"version":999,"favorites":["Dracula"]}"#,
        )
        .unwrap();
        assert!(load_favorites(&path).is_empty());
        assert!(path.with_extension("json.bak").exists());
    }

    #[test]
    fn prune_orphans_drops_unknown_wezterm_themes() {
        let config = WarpThemeConfig::new();
        let mut favorites = vec![
            ThemeKind::Dracula,
            ThemeKind::Wezterm(
                "DefinitelyNotAReal Theme That Will Never Exist".to_string(),
            ),
            ThemeKind::Light,
        ];
        assert!(prune_orphans(&mut favorites, &config));
        assert_eq!(favorites, vec![ThemeKind::Dracula, ThemeKind::Light]);
    }

    #[test]
    fn add_is_idempotent_and_preserves_order() {
        let mut favorites = Vec::new();
        assert!(add(&mut favorites, ThemeKind::Dracula));
        assert!(add(&mut favorites, ThemeKind::Light));
        assert!(!add(&mut favorites, ThemeKind::Dracula));
        assert_eq!(favorites, vec![ThemeKind::Dracula, ThemeKind::Light]);
    }

    #[test]
    fn remove_returns_false_when_absent() {
        let mut favorites = vec![ThemeKind::Dracula];
        assert!(!remove(&mut favorites, &ThemeKind::Light));
        assert!(remove(&mut favorites, &ThemeKind::Dracula));
        assert!(favorites.is_empty());
    }

    #[test]
    fn full_round_trip_with_orphan_prune_rewrites_file() {
        let config = WarpThemeConfig::new();
        let (_dir, path) = tmp_path();
        let original = vec![
            ThemeKind::Dracula,
            ThemeKind::Wezterm(
                "DefinitelyNotAReal Theme That Will Never Exist".to_string(),
            ),
            ThemeKind::Light,
        ];
        save_favorites(&path, &original).expect("save");

        let mut loaded = load_favorites(&path);
        assert_eq!(loaded.len(), 3);
        assert!(prune_orphans(&mut loaded, &config));
        save_favorites(&path, &loaded).expect("rewrite");

        let after = load_favorites(&path);
        assert_eq!(after, vec![ThemeKind::Dracula, ThemeKind::Light]);
    }
}
