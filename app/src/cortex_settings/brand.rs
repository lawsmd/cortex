//! Cortex brand-mark conventions for in-app surfaces.
//!
//! Anywhere we visually mark a surface as Cortex-fork-only (an avatar-menu
//! entry, the header toolbar button, future Cortex-only commands or
//! palette items) we use the brain glyph (`Icon::Brain`, see
//! `app/assets/cortex/brain.svg`) plus a "Cortex …" label, sized and
//! spaced consistently with the ratios below.
//!
//! See `docs/branding.md` ("In-app surfaces — brand-mark spec") for the
//! design rationale and the icon-shader rule that constrains why the SVG
//! itself must be white-stroked.
//!
//! ## Why ratios live here, not in call sites
//!
//! Scattering raw pixel values across every Cortex-branded UI surface
//! made the existing two call sites (avatar menu + header toolbar) drift
//! by 30%-2× during the brain-icon build-out. The constants here are the
//! canonical values; new call sites should consume them via the
//! [`cortex_brand_menu_item`] helper or the [`TOOLBAR_BRAIN_ICON_SIZE`]
//! constant rather than re-deriving the math.

use warpui::Action;

use crate::appearance::Appearance;
use crate::menu::cortex_extras::MenuItemFieldsCortexExt;
use crate::menu::MenuItemFields;
use crate::ui_components::icons;

/// Leading icon size, expressed as a multiple of the surrounding text's
/// `ui_font_size()`. The OpenMoji silhouette has sparser stroke coverage
/// than typical filled icons, so it needs to be drawn ~40% larger to
/// read at the same visual weight as a same-row label glyph.
pub const BRAND_MENU_ICON_TO_FONT_RATIO: f32 = 1.4;

/// Gap between the leading icon and the label, expressed as a multiple of
/// the rendered icon size. The default in `MenuItemFields::render_icon`
/// is `0.5` (icon_size / 2); the brand mark tightens this to `0.25` so
/// "🧠 Cortex Settings" reads as a single visual unit rather than as a
/// detached glyph next to text.
pub const BRAND_MENU_ICON_LABEL_GAP_RATIO: f32 = 0.25;

/// Title-text scale for the Cortex Settings pane header, expressed as a
/// multiple of the surrounding `ui_font_size()`. Standard pane headers
/// render the title at exactly `ui_font_size()`; the Cortex Settings pane
/// reads as a "section header" rather than a tab label, so it gets a 30%
/// bump to feel more like a heading. The brain glyph next to it is then
/// sized at [`BRAND_HEADER_ICON_TO_TITLE_FONT_RATIO`] of *this* enlarged
/// title size.
pub const BRAND_HEADER_TITLE_TO_FONT_RATIO: f32 = 1.3;

/// Brain-glyph size for the Cortex Settings pane header, expressed as a
/// multiple of the *enlarged* header title font (i.e. of
/// `ui_font_size() * BRAND_HEADER_TITLE_TO_FONT_RATIO`, not of the bare
/// `ui_font_size()`).
///
/// The avatar-menu brand mark sets the icon at 1.4× its adjacent label
/// (see [`BRAND_MENU_ICON_TO_FONT_RATIO`]); the pane header carries a
/// stronger "section heading" weight and reads better with a glyph that
/// dominates the title rather than just sitting beside it, so we apply
/// the same 1.3× bump to the icon ratio that the title itself receives —
/// 1.4 × 1.3 = 1.82. Keeping this as a derived constant rather than a
/// raw `1.82` documents the relationship.
pub const BRAND_HEADER_ICON_TO_TITLE_FONT_RATIO: f32 =
    BRAND_MENU_ICON_TO_FONT_RATIO * BRAND_HEADER_TITLE_TO_FONT_RATIO;

/// Size of the brain glyph in the header toolbar's brand button, in
/// logical pixels. Substantially larger than the standard tab-bar icon
/// (the underlying button defaults to `ICON_DIMENSIONS = 24px`) so the
/// brand mark reads as a brand mark, not a control. Note that drawing
/// the icon at this size also requires overriding the *button's* outer
/// width/height — see `TOOLBAR_BRAIN_BUTTON_OUTER_SIZE` and the call in
/// `WorkspaceView::render_cortex_brain_button`.
pub const TOOLBAR_BRAIN_ICON_SIZE: f32 = 56.25;

/// Outer dimension (width = height) of the toolbar brand button, in
/// logical pixels. Equal to the icon size plus the standard
/// `ICON_BUTTON_PADDING` (4px each side) hardcoded in
/// `app/src/ui_components/buttons.rs`. The button wraps its label in a
/// `ConstrainedBox` sized to the button's own styles (see
/// `crates/warpui_core/src/ui_components/button.rs:380-383`), so the
/// inner-label `ConstrainedBox` is clamped to whatever this constant
/// resolves to — they must move together.
pub const TOOLBAR_BRAIN_BUTTON_OUTER_SIZE: f32 = TOOLBAR_BRAIN_ICON_SIZE + 8.0;

/// Builds a menu item with the canonical Cortex brand mark: leading
/// brain glyph, label, and the project-wide icon/font/gap ratios applied.
///
/// Use this for any menu entry that opens Cortex-only functionality —
/// avatar-menu's Cortex Settings, future palette commands, any
/// subsequent Cortex-only menus. The text color and hover/select
/// background are deliberately left at the menu's defaults so the entry
/// matches the surrounding rows; only the leading glyph distinguishes it.
pub fn cortex_brand_menu_item<A: Action + Clone>(
    label: impl Into<String>,
    appearance: &Appearance,
) -> MenuItemFields<A> {
    let icon_size = appearance.ui_font_size() * BRAND_MENU_ICON_TO_FONT_RATIO;
    MenuItemFields::new(label)
        .with_icon(icons::Icon::Brain)
        .with_icon_size_override(icon_size)
        .with_icon_label_gap_override(icon_size * BRAND_MENU_ICON_LABEL_GAP_RATIO)
}
