use pathfinder_geometry::vector::{vec2f, Vector2F};
use warpui::elements::{Element, Point};
use warpui::event::DispatchedEvent;
use warpui::fonts::{FamilyId, Properties};
use warpui::platform::LineStyle;
use warpui::text_layout::{ClipConfig, StyleAndFont, TextStyle, DEFAULT_TOP_BOTTOM_RATIO};
use warpui::{
    AfterLayoutContext, AppContext, EventContext, LayoutContext, PaintContext, SizeConstraint,
    ViewHandle,
};

use crate::editor::EditorView;

/// Wraps an inline rename `TextInput` so the input box is sized to the
/// editor's current text plus a few pixels of caret room — not its full
/// allocated width. Lets `Align::new(...)` actually visually center the
/// rename UI on the typed content; without this, `TextInput`'s underlying
/// `ChildView<EditorView>` fills the row and the caret pins to the row's
/// left edge, defeating the centered-title alignment setting.
///
/// Measurement reuses the same `text_layout_cache.layout_line(...)` path
/// `Text::layout` uses for static text (`crates/warpui_core/src/elements/
/// text.rs:964`), so the measured width matches what `Text::new_inline`
/// would have rendered for the same string.
pub struct ContentSizedEditor {
    editor: ViewHandle<EditorView>,
    child: Box<dyn Element>,
    font_family: FamilyId,
    font_size: f32,
    caret_padding: f32,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl ContentSizedEditor {
    pub fn new(
        editor: ViewHandle<EditorView>,
        child: Box<dyn Element>,
        font_family: FamilyId,
        font_size: f32,
    ) -> Self {
        Self {
            editor,
            child,
            font_family,
            font_size,
            caret_padding: CARET_PADDING_PX,
            size: None,
            origin: None,
        }
    }

    pub fn finish(self) -> Box<dyn Element> {
        Box::new(self)
    }
}

const CARET_PADDING_PX: f32 = 8.;
const LINE_HEIGHT_RATIO: f32 = 1.2;

impl Element for ContentSizedEditor {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        ctx: &mut LayoutContext,
        app: &AppContext,
    ) -> Vector2F {
        let text = self.editor.as_ref(app).buffer_text(app);
        let style_run = (
            0..text.len(),
            StyleAndFont::new(self.font_family, Properties::default(), TextStyle::new()),
        );
        let line = ctx.text_layout_cache.layout_line(
            &text,
            LineStyle {
                font_size: self.font_size,
                line_height_ratio: LINE_HEIGHT_RATIO,
                baseline_ratio: DEFAULT_TOP_BOTTOM_RATIO,
                fixed_width_tab_size: None,
            },
            std::slice::from_ref(&style_run),
            f32::INFINITY,
            ClipConfig::default(),
            &app.font_cache().text_layout_system(),
        );
        let desired_width = (line.width + self.caret_padding).min(constraint.max.x());
        let child_constraint = SizeConstraint::new(
            vec2f(desired_width, constraint.min.y()),
            vec2f(desired_width, constraint.max.y()),
        );
        let size = self.child.layout(child_constraint, ctx, app);
        self.size = Some(size);
        size
    }

    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        self.child.after_layout(ctx, app);
    }

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        self.child.paint(origin, ctx, app);
    }

    fn dispatch_event(
        &mut self,
        event: &DispatchedEvent,
        ctx: &mut EventContext,
        app: &AppContext,
    ) -> bool {
        self.child.dispatch_event(event, ctx, app)
    }

    fn size(&self) -> Option<Vector2F> {
        self.size
    }

    fn origin(&self) -> Option<Point> {
        self.origin
    }
}
