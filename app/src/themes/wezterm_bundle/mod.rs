//! Bundled WezTerm color schemes.
//!
//! Cortex ships ~1,078 themes from `wezterm.color.get_builtin_schemes()` as
//! YAML files committed under `yaml/`, embedded into the binary at build time
//! via `include_dir!` and registered into [`WarpThemeConfig`] alongside the
//! curated builtins. Source data, generator script, and refresh helper live
//! in `scripts/`.

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
