//! Cortex-only knobs layered onto upstream's `MenuItemFields`.
//!
//! All Cortex additions to menu items live in a single nested struct
//! (`CortexMenuItemExtras`) plumbed onto `MenuItemFields` via the
//! `cortex_extras` field. Builder methods sit on an extension trait so the
//! upstream `MenuItemFields` impl block stays pristine. The net of this
//! arrangement: any future Cortex-only menu attribute is added to this
//! file, not to `menu.rs`, so the upstream-shared surface stops growing.

use warpui::Action;

use crate::menu::MenuItemFields;

/// Cortex-only attributes layered onto `MenuItemFields`. Add new
/// Cortex-only fields here, not to `MenuItemFields` directly.
#[derive(Clone, Debug, Default)]
pub struct CortexMenuItemExtras {
    /// Override (in logical pixels) for the gap between the leading icon
    /// and the label. When `None`, the gap is `icon_size / 2` (upstream
    /// default). Set by [`MenuItemFieldsCortexExt::with_icon_label_gap_override`],
    /// consumed by `MenuItemFields::render_icon`.
    pub icon_label_gap_override: Option<f32>,
}

/// Extension trait wiring Cortex knobs onto `MenuItemFields` without
/// adding inherent methods to upstream's impl block. Consumers import this
/// trait at the call site.
pub trait MenuItemFieldsCortexExt: Sized {
    /// Override the gap (in logical pixels) between the leading icon and
    /// the label. Default is `icon_size / 2`.
    fn with_icon_label_gap_override(self, gap: f32) -> Self;
}

impl<A: Action + Clone> MenuItemFieldsCortexExt for MenuItemFields<A> {
    fn with_icon_label_gap_override(mut self, gap: f32) -> Self {
        self.cortex_extras.icon_label_gap_override = Some(gap);
        self
    }
}
