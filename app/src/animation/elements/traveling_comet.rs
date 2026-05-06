//! Two black-bodied circles travel clockwise around the row's perimeter,
//! antipodal at all times, ~4s revolution. Body is literal black so the
//! comet renders identically over selected (filled) and unselected
//! (transparent) tabs; only the outline carries the project color.
//!
//! Phase comes from the global [`AnimationClock`] so multiple tabs running
//! the Running animation stay locked in formation. See
//! `docs/tab-bar/agent-status-animations.md` § Traveling Comet.

use std::time::Duration;

use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{vec2f, Vector2F};
use warpui::elements::{
    AfterLayoutContext, Border, CornerRadius, Element, EventContext, LayoutContext, PaintContext,
    Point, Radius, SizeConstraint,
};
use warpui::event::DispatchedEvent;
use warpui::{AppContext, SingletonEntity};

use crate::animation::AnimationClock;

const COMET_DIAMETER: f32 = 8.75;
const COMET_OUTLINE_WIDTH: f32 = 1.5625;
const COMET_PERIOD: Duration = Duration::from_millis(4000);
const REPAINT_INTERVAL: Duration = Duration::from_millis(16);

pub struct TravelingCometElement {
    outline_color: ColorU,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl TravelingCometElement {
    pub fn new(outline_color: ColorU) -> Self {
        Self {
            outline_color,
            size: None,
            origin: None,
        }
    }

    /// Map a perimeter coordinate `p` (`0..inner_perimeter`, walking clockwise
    /// from the inner top-left corner) to an `(x, y)` in the *inner* rect's
    /// coordinate space. Caller adds the inset to translate back into the
    /// element's full bounds.
    fn walk_perimeter(p: f32, inner_w: f32, inner_h: f32) -> Vector2F {
        if p < inner_w {
            vec2f(p, 0.0)
        } else if p < inner_w + inner_h {
            vec2f(inner_w, p - inner_w)
        } else if p < 2.0 * inner_w + inner_h {
            vec2f(inner_w - (p - inner_w - inner_h), inner_h)
        } else {
            vec2f(0.0, inner_h - (p - 2.0 * inner_w - inner_h))
        }
    }
}

impl Element for TravelingCometElement {
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

        let radius = COMET_DIAMETER / 2.0;
        // The comet *center* rides on the tab border line — i.e. on the
        // perimeter of `size` itself. Half of each comet draws outside the
        // bounding box; the outer Stack does not clip child paint, so the
        // protrusion renders. Tab spacing must guarantee at least
        // `radius + COMET_OUTLINE_WIDTH/2` (~5.2 px) of visual breathing
        // room around each tab so adjacent comets don't collide.
        let inset = 0.0;
        let inner_w = (size.x() - 2.0 * inset).max(0.0);
        let inner_h = (size.y() - 2.0 * inset).max(0.0);
        let inner_perimeter = 2.0 * (inner_w + inner_h);

        // Reschedule the next frame regardless of whether we paint, so the
        // animation keeps ticking even at degenerate sizes.
        ctx.repaint_after(REPAINT_INTERVAL);

        if inner_perimeter <= 0.0 {
            return;
        }

        let phase = AnimationClock::as_ref(app).phase(COMET_PERIOD);
        // Comet 1 starts at the inner top-right corner (perimeter coord =
        // inner_w); Comet 2 is antipodal (P/2 ahead, i.e. inner_w + inner_h
        // further along, which lands at the inner bottom-left corner).
        let offsets = [inner_w, inner_w + inner_perimeter / 2.0];

        for offset in offsets {
            let p = (phase * inner_perimeter + offset).rem_euclid(inner_perimeter);
            let local = Self::walk_perimeter(p, inner_w, inner_h);
            // Translate from inner-rect coords into the element's coords.
            let center = origin + vec2f(local.x() + inset, local.y() + inset);
            let comet_origin = vec2f(center.x() - radius, center.y() - radius);
            let comet_size = vec2f(COMET_DIAMETER, COMET_DIAMETER);

            ctx.scene
                .draw_rect_with_hit_recording(RectF::new(comet_origin, comet_size))
                .with_background(ColorU::black())
                .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.0)))
                .with_border(
                    Border::all(COMET_OUTLINE_WIDTH).with_border_color(self.outline_color),
                );
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
