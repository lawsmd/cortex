//! Edge Halo Breath — sinusoidal pulse of a 2px frame inset 1px from the
//! row's bounds, in the tab's project color (lightened for visibility on
//! selected/filled rows). ~1.2s period, peak 70% opacity, returning to 0
//! between cycles for the inhale-exhale-pause feel.
//!
//! Replaces the prior warm-amber interior wash. The interior wash clashed
//! with project-color tab titles on unselected tabs — yellow over a purple
//! title rectangle reads as jarring. Edge-only painting sidesteps the
//! fill-clash entirely: the frame paints at the perimeter and never
//! competes with the row's interior fill.
//!
//! Phase comes from the global [`AnimationClock`] so multiple tabs in the
//! Attention Needed state breathe in lockstep. See
//! `docs/tab-bar/agent-status-animations.md` § Row Glow Breath.

use std::f32::consts::PI;
use std::time::Duration;

use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;
use warpui::elements::{
    AfterLayoutContext, Element, EventContext, LayoutContext, PaintContext, Point, SizeConstraint,
};
use warpui::event::DispatchedEvent;
use warpui::{AppContext, SingletonEntity};

use crate::animation::AnimationClock;

const BREATH_DEPTH: f32 = 0.70;
const BREATH_PERIOD: Duration = Duration::from_millis(1200);
const REPAINT_INTERVAL: Duration = Duration::from_millis(32);
/// Frame thickness in logical pixels.
const FRAME_THICKNESS: f32 = 2.0;
/// Inset from the row's outer rect, so the frame doesn't visually butt into
/// adjacent UI gaps.
const FRAME_INSET: f32 = 1.0;
/// HSL-lightness lift toward white. Required so the frame stays visible
/// when painted over a row whose fill is the same project color (selected
/// tabs). At 0.0 the frame would be invisible on selected saved-project
/// tabs; at 1.0 it would always be white. 0.25 keeps the project-color
/// identity readable while putting enough lightness contrast against the
/// underlying fill to register.
const TINT_LIGHTEN: f32 = 0.25;

pub struct RowGlowBreathElement {
    tint: ColorU,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl RowGlowBreathElement {
    /// `tint` is the un-lightened identity color the caller wants the frame
    /// to read as — typically the pane's project color, or a neutral fallback
    /// for unsaved tabs. The element lightens it internally before painting
    /// so callers don't have to know about the visibility-on-selected-tabs
    /// constraint.
    pub fn new(tint: ColorU) -> Self {
        Self {
            tint: lighten_toward_white(tint, TINT_LIGHTEN),
            size: None,
            origin: None,
        }
    }
}

fn lighten_toward_white(c: ColorU, amount: f32) -> ColorU {
    let amount = amount.clamp(0.0, 1.0);
    let blend = |channel: u8| -> u8 {
        let v = channel as f32;
        (v + (255.0 - v) * amount).round().clamp(0.0, 255.0) as u8
    };
    ColorU::new(blend(c.r), blend(c.g), blend(c.b), c.a)
}

impl Element for RowGlowBreathElement {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        _ctx: &mut LayoutContext,
        _app: &AppContext,
    ) -> Vector2F {
        let size = constraint.max;
        self.size = Some(size);
        size
    }

    fn after_layout(&mut self, _ctx: &mut AfterLayoutContext, _app: &AppContext) {}

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        let Some(size) = self.size else {
            return;
        };

        ctx.repaint_after(REPAINT_INTERVAL);

        let phase = AnimationClock::as_ref(app).phase(BREATH_PERIOD);
        // sin(phase * π) traces 0 → 1 → 0 across one cycle, which gives the
        // inhale-exhale-pause shape (both ends of the cycle are at 0).
        let opacity = BREATH_DEPTH * (phase * PI).sin();
        let alpha = (opacity.clamp(0.0, 1.0) * 255.0) as u8;
        if alpha == 0 {
            return;
        }

        // Frame geometry. Inset from the row's outer rect so the strips
        // don't butt up against the adjacent UI; left/right strips are
        // shorter than top/bottom by 2 * thickness so the corners overlap
        // exactly once (top + left meet at the corner without
        // double-stamping the alpha).
        let inset_x = origin.x() + FRAME_INSET;
        let inset_y = origin.y() + FRAME_INSET;
        let inner_w = (size.x() - 2.0 * FRAME_INSET).max(0.0);
        let inner_h = (size.y() - 2.0 * FRAME_INSET).max(0.0);
        if inner_w <= 0.0 || inner_h <= 0.0 {
            return;
        }

        let color = ColorU::new(self.tint.r, self.tint.g, self.tint.b, alpha);

        // For very small rows (< 2 * thickness in either dimension) the
        // separated horizontal/vertical strips would overlap or invert.
        // Fall back to a single filled rect at the inset — visually a
        // solid breath of the tint, still better than no animation.
        let thickness = FRAME_THICKNESS.min(inner_w / 2.0).min(inner_h / 2.0);
        if thickness < 1.0 {
            ctx.scene
                .draw_rect_with_hit_recording(RectF::new(
                    Vector2F::new(inset_x, inset_y),
                    Vector2F::new(inner_w, inner_h),
                ))
                .with_background(color);
            return;
        }

        // Top strip — full inner width.
        ctx.scene
            .draw_rect_with_hit_recording(RectF::new(
                Vector2F::new(inset_x, inset_y),
                Vector2F::new(inner_w, thickness),
            ))
            .with_background(color);
        // Bottom strip — full inner width.
        ctx.scene
            .draw_rect_with_hit_recording(RectF::new(
                Vector2F::new(inset_x, inset_y + inner_h - thickness),
                Vector2F::new(inner_w, thickness),
            ))
            .with_background(color);
        // Left strip — between top and bottom, no double-stamp at corners.
        let side_h = (inner_h - 2.0 * thickness).max(0.0);
        if side_h > 0.0 {
            ctx.scene
                .draw_rect_with_hit_recording(RectF::new(
                    Vector2F::new(inset_x, inset_y + thickness),
                    Vector2F::new(thickness, side_h),
                ))
                .with_background(color);
            // Right strip — symmetric to left.
            ctx.scene
                .draw_rect_with_hit_recording(RectF::new(
                    Vector2F::new(inset_x + inner_w - thickness, inset_y + thickness),
                    Vector2F::new(thickness, side_h),
                ))
                .with_background(color);
        }
    }

    fn dispatch_event(
        &mut self,
        _event: &DispatchedEvent,
        _ctx: &mut EventContext,
        _app: &AppContext,
    ) -> bool {
        false
    }

    fn size(&self) -> Option<Vector2F> {
        self.size
    }

    fn origin(&self) -> Option<Point> {
        self.origin
    }
}
