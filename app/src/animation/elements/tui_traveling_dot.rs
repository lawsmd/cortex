//! TUI-style traveling dot animation for the Cortex TUI tab style.
//!
//! Two small dots travel clockwise around the tab border, antipodal at all
//! times. Where each dot passes, a gap appears in the border — the dot rides
//! through the gap instead of over the border. The element draws the border
//! itself (with gaps built in) so the Container's own border is suppressed
//! when this animation is active.
//!
//! Phase comes from the global [`AnimationClock`], same 4 s period as the
//! comet, so the two styles stay visually synchronized if a user switches
//! mid-session.

use std::time::Duration;

use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{vec2f, Vector2F};
use warpui::elements::{
    AfterLayoutContext, CornerRadius, Element, EventContext, LayoutContext, PaintContext, Point,
    Radius, SizeConstraint,
};
use warpui::event::DispatchedEvent;
use warpui::{AppContext, SingletonEntity};

use crate::animation::AnimationClock;

const DOT_DIAMETER: f32 = 4.0;
const GAP_HALF_WIDTH: f32 = 10.0;
const DOT_PERIOD: Duration = Duration::from_millis(4000);
const REPAINT_INTERVAL: Duration = Duration::from_millis(16);
const BORDER_WIDTH: f32 = 1.0;

pub struct TuiTravelingDotElement {
    border_color: ColorU,
    dot_color: ColorU,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl TuiTravelingDotElement {
    pub fn new(border_color: ColorU, dot_color: ColorU) -> Self {
        Self {
            border_color,
            dot_color,
            size: None,
            origin: None,
        }
    }

    fn walk_perimeter(p: f32, w: f32, h: f32) -> Vector2F {
        if p < w {
            vec2f(p, 0.0)
        } else if p < w + h {
            vec2f(w, p - w)
        } else if p < 2.0 * w + h {
            vec2f(w - (p - w - h), h)
        } else {
            vec2f(0.0, h - (p - 2.0 * w - h))
        }
    }
}

/// Compute the sub-ranges of `[edge_start, edge_end]` not covered by any gap.
/// Each gap is centered at a dot's perimeter coordinate ± `gap_half`, wrapping
/// at `perimeter`.
fn solid_ranges(
    edge_start: f32,
    edge_end: f32,
    dot_coords: &[f32; 2],
    gap_half: f32,
    perimeter: f32,
) -> Vec<(f32, f32)> {
    let mut cuts: Vec<(f32, f32)> = Vec::new();
    for &center in dot_coords {
        let g_lo = center - gap_half;
        let g_hi = center + gap_half;

        if g_lo >= 0.0 && g_hi <= perimeter {
            let s = g_lo.max(edge_start);
            let e = g_hi.min(edge_end);
            if s < e {
                cuts.push((s, e));
            }
        } else if g_lo < 0.0 {
            let wrapped_lo = perimeter + g_lo;
            let s1 = wrapped_lo.max(edge_start);
            let e1 = perimeter.min(edge_end);
            if s1 < e1 {
                cuts.push((s1, e1));
            }
            let s2 = 0.0f32.max(edge_start);
            let e2 = g_hi.min(edge_end);
            if s2 < e2 {
                cuts.push((s2, e2));
            }
        } else {
            let s = g_lo.max(edge_start);
            let e = perimeter.min(edge_end);
            if s < e {
                cuts.push((s, e));
            }
            let wrapped_hi = g_hi - perimeter;
            let s2 = 0.0f32.max(edge_start);
            let e2 = wrapped_hi.min(edge_end);
            if s2 < e2 {
                cuts.push((s2, e2));
            }
        }
    }

    cuts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let mut segments = Vec::new();
    let mut cursor = edge_start;
    for &(cs, ce) in &cuts {
        if cs > cursor {
            segments.push((cursor, cs));
        }
        if ce > cursor {
            cursor = ce;
        }
    }
    if cursor < edge_end {
        segments.push((cursor, edge_end));
    }
    segments
}

impl Element for TuiTravelingDotElement {
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

        let w = size.x();
        let h = size.y();
        let perimeter = 2.0 * (w + h);

        ctx.repaint_after(REPAINT_INTERVAL);

        if perimeter <= 0.0 {
            return;
        }

        let phase = AnimationClock::as_ref(app).phase(DOT_PERIOD);
        let offsets = [w, w + perimeter / 2.0];
        let dot_coords: [f32; 2] = [
            (phase * perimeter + offsets[0]).rem_euclid(perimeter),
            (phase * perimeter + offsets[1]).rem_euclid(perimeter),
        ];

        // --- Border with gaps ---
        // Each edge is drawn as 1–3 strip segments, skipping gap zones.

        // Top edge — perimeter [0, w), pixel x = perim_coord, y = 0.
        for (ps, pe) in solid_ranges(0.0, w, &dot_coords, GAP_HALF_WIDTH, perimeter) {
            ctx.scene
                .draw_rect_with_hit_recording(RectF::new(
                    origin + vec2f(ps, 0.0),
                    vec2f(pe - ps, BORDER_WIDTH),
                ))
                .with_background(self.border_color);
        }

        // Right edge — perimeter [w, w+h), pixel y = perim_coord − w.
        for (ps, pe) in solid_ranges(w, w + h, &dot_coords, GAP_HALF_WIDTH, perimeter) {
            ctx.scene
                .draw_rect_with_hit_recording(RectF::new(
                    origin + vec2f(w - BORDER_WIDTH, ps - w),
                    vec2f(BORDER_WIDTH, pe - ps),
                ))
                .with_background(self.border_color);
        }

        // Bottom edge — perimeter [w+h, 2w+h), REVERSED: pixel x = 2w+h − p.
        for (ps, pe) in solid_ranges(w + h, 2.0 * w + h, &dot_coords, GAP_HALF_WIDTH, perimeter)
        {
            let px_start = 2.0 * w + h - pe;
            let px_end = 2.0 * w + h - ps;
            ctx.scene
                .draw_rect_with_hit_recording(RectF::new(
                    origin + vec2f(px_start, h - BORDER_WIDTH),
                    vec2f(px_end - px_start, BORDER_WIDTH),
                ))
                .with_background(self.border_color);
        }

        // Left edge — perimeter [2w+h, 2(w+h)), REVERSED: pixel y = h − (p − 2w − h).
        for (ps, pe) in solid_ranges(
            2.0 * w + h,
            perimeter,
            &dot_coords,
            GAP_HALF_WIDTH,
            perimeter,
        ) {
            let py_start = h - (pe - 2.0 * w - h);
            let py_end = h - (ps - 2.0 * w - h);
            ctx.scene
                .draw_rect_with_hit_recording(RectF::new(
                    origin + vec2f(0.0, py_start),
                    vec2f(BORDER_WIDTH, py_end - py_start),
                ))
                .with_background(self.border_color);
        }

        // --- Dots ---
        let dot_r = DOT_DIAMETER / 2.0;
        for &dp in &dot_coords {
            let local = Self::walk_perimeter(dp, w, h);
            let center = origin + local;
            ctx.scene
                .draw_rect_with_hit_recording(RectF::new(
                    vec2f(center.x() - dot_r, center.y() - dot_r),
                    vec2f(DOT_DIAMETER, DOT_DIAMETER),
                ))
                .with_background(self.dot_color)
                .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.0)));
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
