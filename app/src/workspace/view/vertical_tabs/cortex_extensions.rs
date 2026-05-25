//! Cortex-only additions layered on top of upstream's vertical-tabs view.
//!
//! Everything here is downstream of `warpdotdev/warp` — moved out of
//! `vertical_tabs.rs` so the upstream-shared file stays close to its original
//! shape and merges cleanly. The parent file references this module by name
//! and only retains call-sites (`cortex_extensions::wrap_with_agent_animation_layers(...)`,
//! `cortex_extensions::render_pane_icon_with_status(...)`, etc.).
//!
//! Items are `pub(super)` so the parent module can re-export selectively
//! (`use self::cortex_extensions::*`) without leaking into other crates.

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::Vector2F;
use settings::Setting;

use crate::animation::elements::row_glow_breath::RowGlowBreathElement;
use crate::animation::elements::traveling_comet::TravelingCometElement;
use crate::appearance::Appearance;
use crate::tab::TabAnimationKind;
use crate::themes::theme::Fill as ThemeFill;
use crate::ui_components::icon_with_status::{render_icon_with_status, IconWithStatusVariant};
use warp_core::ui::theme::{Fill as WarpThemeFill, WarpTheme};
use warpui::elements::{
    ChildAnchor, Element, OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds,
    Stack,
};
use warpui::text_layout::ClipConfig;
use warpui::AppContext;

use super::TypedPane;

/// Default font size for Cortex-side summary-tab text lines. Pulled out of
/// the call-site literals so the wrapper below has a single knob for the
/// `font_size` parameter upstream added to `render_text_line`. If upstream
/// removes or renames `font_size`, only [`cortex_text_line`] needs an edit —
/// not the three callers in `render_summary_tab_item` /
/// `render_summary_primary_label_line`.
pub(super) const CORTEX_SUMMARY_LINE_FONT_SIZE: f32 = 12.;

/// Theme-independent gray used for the 1px border on every unselected vertical
/// tab in the Cortex appearance. Sits between Warp's existing 100-gray (used
/// elsewhere in the panel) and white so it reads on both dark and light themes
/// without being claimable by either.
pub(crate) const VERTICAL_TAB_UNSELECTED_BORDER_GRAY: ColorU = ColorU {
    r: 140,
    g: 140,
    b: 140,
    a: 255,
};

/// Icon size for the per-line conversation status pill in Summary mode. Pairs with
/// `STATUS_ELEMENT_PADDING` (2px) for an overall ~14px element next to a 12pt title.
pub(super) const VERTICAL_TABS_SUMMARY_STATUS_ICON_SIZE: f32 = 10.;

/// Sizing override for the Cortex Settings tab's brain glyph. The brain SVG
/// has more inherent whitespace inside its viewBox than the other neutral
/// glyphs, so at the default 24px it reads optically smaller next to them.
/// Bumped to 30 so the icon slot grows; the brain row's title sits ~6px
/// farther right than other rows — a deliberate trade for the optical
/// weight on this single tab.
pub(super) const VERTICAL_TABS_BRAIN_ICON_SIZE: f32 = 30.;

/// Whether the current tab style implies inverse fill on selected tabs.
pub(crate) fn cortex_inverse_fill_active(
    cortex: &crate::settings::CortexSettings,
) -> bool {
    matches!(
        *cortex.tab_style.value(),
        crate::settings::TabStyle::CortexModern
    )
}

/// Resolve a pane's color into a single `ColorU` for the comet outline. Saved
/// projects use their accent color; unsaved projects fall back to the same
/// 140-gray that draws the unselected row border, so the comet visually
/// belongs to the row in both cases.
pub(crate) fn comet_outline_color(pane_color: Option<&ThemeFill>) -> ColorU {
    match pane_color {
        Some(ThemeFill::Solid(c)) => *c,
        _ => VERTICAL_TAB_UNSELECTED_BORDER_GRAY,
    }
}

/// Resolve a pane's color into the un-lightened tint for the AttentionNeeded
/// breath frame. Same sourcing as the comet outline (saved → project color,
/// unsaved → 140-gray) so the two animations read as part of the same tab
/// identity. The element lightens the result internally before painting,
/// which is what keeps the frame visible when it's drawn over a row whose
/// fill is already the project color (selected saved-project tabs).
pub(crate) fn breath_frame_tint(pane_color: Option<&ThemeFill>) -> ColorU {
    comet_outline_color(pane_color)
}

/// If the tab has an active animation state, wrap `content` in a `Stack` so
/// the appropriate animation element renders alongside it. Returns `content`
/// unchanged when there's no animation to layer.
///
/// `Running` overlays the comet on top of the content; `AttentionNeeded`
/// puts the breath wash underneath. The two states are mutually exclusive
/// at the aggregator level — see `aggregated_tab_animation` in `tab.rs`.
///
/// The animation element is added as a *positioned* child anchored to the
/// row's top-left and bounded to the parent's size. This is required: the
/// Flex column above us passes an unbounded y-constraint, and a regular
/// (non-positioned) child whose `layout` returns `constraint.max` would
/// poison the Stack with infinite height (see `scene.rs` rect-size assert).
/// `ParentBySize` makes the Stack size from `content` alone, then re-layouts
/// the animation element with a tight constraint matching the row's bounds.
pub(crate) fn wrap_with_agent_animation_layers(
    content: Box<dyn Element>,
    tab_animation: Option<TabAnimationKind>,
    pane_color: Option<&ThemeFill>,
) -> Box<dyn Element> {
    let Some(kind) = tab_animation else {
        return content;
    };
    let fill_parent = OffsetPositioning::offset_from_parent(
        Vector2F::zero(),
        ParentOffsetBounds::ParentBySize,
        ParentAnchor::TopLeft,
        ChildAnchor::TopLeft,
    );
    let stack = match kind {
        TabAnimationKind::Running => {
            let outline = comet_outline_color(pane_color);
            Stack::new().with_child(content).with_positioned_child(
                TravelingCometElement::new(outline).finish(),
                fill_parent,
            )
        }
        TabAnimationKind::AttentionNeeded => {
            let tint = breath_frame_tint(pane_color);
            Stack::new()
                .with_positioned_child(RowGlowBreathElement::new(tint).finish(), fill_parent)
                .with_child(content)
        }
    };
    stack.finish()
}

/// Cortex-side wrapper around upstream's private `render_text_line` helper.
///
/// All three Cortex call sites in `render_summary_tab_item` /
/// `render_summary_primary_label_line` pass the same 12pt font size, so we
/// commit to that here and absorb upstream's signature in one place. When
/// upstream changes the parameter list of `render_text_line` again — they
/// already added `font_size: f32` once mid-rebase — this wrapper is the only
/// thing that needs updating.
pub(super) fn cortex_text_line(
    text: &str,
    text_color: WarpThemeFill,
    clip: ClipConfig,
    appearance: &Appearance,
) -> Box<dyn Element> {
    super::render_text_line(
        text,
        text_color,
        clip,
        CORTEX_SUMMARY_LINE_FONT_SIZE,
        appearance,
    )
}

/// Render the icon-with-status element for a pane row, bumping the total size
/// for the Cortex Settings tab's brain glyph so the optical weight matches the
/// neutral glyphs on other rows.
///
/// NOTE: the `tabs_hide_icon_backdrop` Cortex setting is currently inert —
/// upstream's 2026-05-05 refactor of `render_icon_with_status` to a single
/// `total_size: f32` parameter dropped the per-variant struct that our
/// hide-neutral-backdrop branch used. Re-implementing it on top of upstream's
/// `render_neutral_circle` helper is a follow-up; until then the setting
/// exists in `CortexSettings` but renders identically to the default.
pub(super) fn render_pane_icon_with_status(
    variant: IconWithStatusVariant,
    typed: &TypedPane<'_>,
    theme: &WarpTheme,
    _app: &AppContext,
) -> Box<dyn Element> {
    let total_size = if matches!(typed, TypedPane::CortexSettings) {
        VERTICAL_TABS_BRAIN_ICON_SIZE
    } else {
        super::VERTICAL_TABS_ICON_SIZE
    };
    render_icon_with_status(variant, total_size, 0., theme, theme.background())
}
