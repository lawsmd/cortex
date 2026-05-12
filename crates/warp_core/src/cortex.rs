//! Cortex-fork-only branding constants. Promote new branding bits here as
//! they emerge instead of duplicating them across slides / auth surfaces.
//! See `docs/branding.md` for canonical usage rules.

use warpui::color::ColorU;

/// The pink used on the Cortex brain glyph, the CORTEX ASCII title, and
/// Cortex-themed buttons across the auth / login surfaces (`#F4B6C2`).
pub const BRAIN_PINK: ColorU = ColorU {
    r: 244,
    g: 182,
    b: 194,
    a: 255,
};

/// `figlet ansi_shadow CORTEX` rendered ASCII used as the Cortex wordmark on
/// the IntroSlide and as the title block on the AuthView modal. Path is
/// relative to `crates/warp_core/src/cortex.rs`.
pub const CORTEX_ASCII: &str = include_str!("../../../app/assets/cortex/ascii/CortexAscii5.txt");
