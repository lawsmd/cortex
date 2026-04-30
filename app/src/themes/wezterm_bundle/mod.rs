//! Bundled community color schemes.
//!
//! Cortex ships ~1,078 themes embedded as YAML files under `yaml/`, loaded into
//! [`WarpThemeConfig`] alongside the curated builtins via `include_dir!` at
//! build time.
//!
//! These themes are not Cortex-original work. They're aggregated from four
//! upstream community projects (all MIT-licensed):
//!
//! - **Gogh** (~361 schemes) — <https://github.com/Gogh-Co/Gogh>
//! - **iTerm2-Color-Schemes** (~381 schemes) —
//!   <https://github.com/mbadolato/iTerm2-Color-Schemes>
//! - **base16** (~179 schemes) — <https://github.com/chriskempson/base16>
//! - **terminal.sexy** (~157 schemes) —
//!   <https://github.com/stayradiated/terminal.sexy>
//!
//! Each theme's display name preserves its source tag, e.g. `"3024 (base16)"`,
//! `"Adventure Time (Gogh)"`. Full attribution lives in the repo `README.md`
//! and `docs/roadmap/themes.md`.
//!
//! The YAML on disk is the form WezTerm exposes via
//! `wezterm.color.get_builtin_schemes()`, which is how this collection reaches
//! us; the generator/refresh script that pulls from that source lives under
//! `scripts/`.

use include_dir::{Dir, include_dir};
use warp_core::ui::theme::WarpTheme;

use super::theme::{ThemeKind, WarpThemeConfig};

static YAML_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/themes/wezterm_bundle/yaml");

pub fn register(config: &mut WarpThemeConfig) {
    for entry in YAML_DIR.files() {
        let bytes = entry.contents();
        let theme: WarpTheme = match serde_yaml::from_slice(bytes) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    "skipping malformed bundled wezterm theme {:?}: {e}",
                    entry.path()
                );
                continue;
            }
        };
        let Some(name) = theme.name() else {
            tracing::warn!(
                "skipping bundled wezterm theme {:?}: missing name",
                entry.path()
            );
            continue;
        };
        config.add_new_theme(ThemeKind::Wezterm(name), theme);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_bundled_themes_parse() {
        let mut count = 0;
        for entry in YAML_DIR.files() {
            let theme: WarpTheme = serde_yaml::from_slice(entry.contents())
                .unwrap_or_else(|e| panic!("failed to parse {:?}: {e}", entry.path()));
            assert!(
                theme.name().is_some(),
                "{:?} missing name",
                entry.path()
            );
            count += 1;
        }
        assert!(
            count >= 1000,
            "expected >= 1000 bundled wezterm themes, got {count}"
        );
    }

    #[test]
    fn config_includes_wezterm_themes() {
        let config = WarpThemeConfig::new();
        let wezterm_count = config
            .theme_items()
            .filter(|(kind, _)| matches!(kind, ThemeKind::Wezterm(_)))
            .count();
        assert!(
            wezterm_count >= 1000,
            "WarpThemeConfig::new() registered {wezterm_count} wezterm themes, expected >= 1000"
        );
    }
}
