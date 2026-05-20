//! Cortex: gradient used to color sub-project rows in the saved-projects
//! picker fly-out.
//!
//! The endpoint of the gradient is fixed in lightness regardless of how
//! many sub-projects exist, so a project with 2 sub-projects spans the same
//! visual color distance as one with 100. We mix the parent hex toward
//! white in linear-RGB space (perceptually steadier than sRGB lerp) by
//! `t = (i + 1) / N * SUB_PROJECT_LIGHTEN_END`. With N sub-projects, the
//! final entry lands at exactly `SUB_PROJECT_LIGHTEN_END` of the way toward
//! white; earlier entries are evenly spaced inside that range.
//!
//! Starting at `(i + 1)/N` instead of `i/(N - 1)` deliberately keeps the
//! first sub-project visually distinct from the parent row — a sub-project
//! shouldn't be a pixel-exact duplicate of its parent's color.

use warp_core::ui::color::hex_color::{coloru_from_hex_string, coloru_to_hex_string};
use warpui::color::ColorU;

/// How much lighter (toward white) the *last* sub-project's color sits
/// relative to the parent's. Same value for N=2 and N=100, so the
/// flyout's visual range stays consistent.
pub const SUB_PROJECT_LIGHTEN_END: f32 = 0.40;

/// Returns the gradient hex shade for sub-project `index` of `total`, mixed
/// from the parent's hex toward white in linear-RGB space. Returns `None`
/// when the parent hex is malformed or `total == 0`.
pub fn sub_project_color(parent_hex: &str, index: usize, total: usize) -> Option<String> {
    if total == 0 {
        return None;
    }
    let parent = coloru_from_hex_string(parent_hex).ok()?;
    let t = ((index as f32) + 1.0) / (total as f32) * SUB_PROJECT_LIGHTEN_END;
    Some(coloru_to_hex_string(&mix_toward_white(parent, t)))
}

/// Linear-RGB mix of `c` toward white by factor `t` (clamped to [0, 1]).
/// `t = 0` returns `c` unchanged; `t = 1` returns pure white.
fn mix_toward_white(c: ColorU, t: f32) -> ColorU {
    let t = t.clamp(0.0, 1.0);
    let r = mix_channel(c.r, t);
    let g = mix_channel(c.g, t);
    let b = mix_channel(c.b, t);
    ColorU { r, g, b, a: c.a }
}

fn mix_channel(channel: u8, t: f32) -> u8 {
    let linear = srgb_to_linear(channel);
    let mixed = linear + (1.0 - linear) * t;
    linear_to_srgb(mixed)
}

fn srgb_to_linear(channel: u8) -> f32 {
    let v = (channel as f32) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(linear: f32) -> u8 {
    let v = if linear <= 0.0031308 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_is_constant_regardless_of_total() {
        // N=2 and N=100 should produce the same color for their last entry.
        let last_with_2 = sub_project_color("#2bd7fb", 1, 2).unwrap();
        let last_with_100 = sub_project_color("#2bd7fb", 99, 100).unwrap();
        assert_eq!(last_with_2, last_with_100);
    }

    #[test]
    fn first_sub_project_is_not_parent_color() {
        // (i + 1)/N at i=0,N=anything > 0 always produces t > 0, so the
        // first sub-project is visibly lighter than the parent.
        let parent = "#2bd7fb";
        let first = sub_project_color(parent, 0, 4).unwrap();
        assert_ne!(first, parent);
    }

    #[test]
    fn malformed_hex_returns_none() {
        assert!(sub_project_color("not-a-color", 0, 3).is_none());
        assert!(sub_project_color("#zzz", 0, 3).is_none());
    }

    #[test]
    fn zero_total_returns_none() {
        assert!(sub_project_color("#ffffff", 0, 0).is_none());
    }

    #[test]
    fn black_endpoint_lightens_partway() {
        // Pure black mixed 40% toward white in linear RGB lands around 0x6B
        // in sRGB (roughly — the linear-RGB midpoint of 0 and 1 is not 0.5).
        // We only assert it's brighter than 0 and dimmer than full white.
        let endpoint = sub_project_color("#000000", 0, 1).unwrap();
        assert_ne!(endpoint, "#000000");
        assert_ne!(endpoint, "#ffffff");
    }
}
