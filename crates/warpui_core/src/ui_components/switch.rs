use crate::color::ColorU;
use crate::elements::{
    Align, Border, ChildAnchor, Fill, OffsetPositioning, ParentAnchor, ParentOffsetBounds, Stack,
    Text,
};
use crate::fonts::{Properties, Weight};
use crate::geometry::vector::vec2f;
use crate::platform::Cursor;
use crate::scene::Radius;
use crate::{
    elements::{
        ConstrainedBox, Container, CornerRadius, Element, Flex, Hoverable, MouseState,
        MouseStateHandle, ParentElement,
    },
    ui_components::components::{UiComponent, UiComponentStyles},
    ui_components::text::Span,
    ui_components::tool_tip::Tooltip,
};
use lazy_static::lazy_static;

const DEFAULT_SWITCH_HEIGHT: f32 = 22.;
const SWITCH_CORNER_RADIUS: f32 = 4.;
const SWITCH_LABEL_FONT_SIZE: f32 = 11.;

lazy_static! {
    // Retained as a public export because external code (notably
    // `warp_core::ui::builder`) used to reference it. The new ON/OFF
    // rectangle doesn't render the gray track, but we leave the constant in
    // place to avoid breaking any out-of-tree consumers.
    pub static ref TRACK_COLOR: ColorU = ColorU::new(170, 170, 170, 255);
}

/// A config to provide both the text and the styles for a tooltip.
/// Bundling these together prevents any callers from passing in just one
/// without the other (and this ui element is not capable of coming up with sensible, themed defaults for the tooltip styles).
#[derive(Clone)]
pub struct TooltipConfig {
    pub text: String,
    pub styles: UiComponentStyles,
}

/// A switch element used to toggle the on/off state of a single value. The
/// chip is a rectangle with a 4px corner radius (matching the vertical-tab
/// shape) that displays "OFF" with an accent-colored border on a transparent
/// background when unchecked, and "ON" filled with the active terminal theme
/// accent color (with the label switched to the terminal background color)
/// when checked. The switch optionally includes a label that can also be
/// clicked to activate the element. Note the switch does not contain any
/// state, it's up to the caller to rebuild the switch with the correct value
/// for "checked" when the switch is clicked.
pub struct Switch {
    checked: bool,
    disabled: bool,
    label: Option<Span>, // optional label for the Switch, also clickable
    styles: UiComponentStyles,
    hovered_styles: Option<UiComponentStyles>,
    checked_styles: Option<UiComponentStyles>,
    disabled_styles: Option<UiComponentStyles>,
    hover_border_size: Option<f32>,
    mouse_state: SwitchStateHandle,
    tooltip: Option<TooltipConfig>,
}

/// State handles necessary for the Switch component. The `thumb_mouse_state`
/// field is no longer used by the new ON/OFF rectangle, but is kept for
/// backward compatibility with the previous pill+thumb design.
#[derive(Default, Clone)]
pub struct SwitchStateHandle {
    component_mouse_state: MouseStateHandle,
    #[allow(dead_code)]
    thumb_mouse_state: MouseStateHandle,
}

impl UiComponent for Switch {
    type ElementType = Hoverable;
    fn build(self) -> Hoverable {
        let tooltip = self.tooltip.clone();

        let hoverable = Hoverable::new(self.mouse_state.component_mouse_state.clone(), |state| {
            let styles = self.styles(state);
            let switch_height = styles.height.unwrap_or(DEFAULT_SWITCH_HEIGHT);

            let switch_element = self.render_switch(styles);
            let switch_element = if let Some(label) = self.label.clone() {
                let label = label.with_style(self.styles).build();
                let font_size = self.styles.font_size.unwrap_or_default();

                // If the chip is taller than the label font, apply padding so the switch is
                // centered with the label.
                let padding_top = if switch_height > font_size {
                    (switch_height - font_size) / 2.
                } else {
                    0.
                };

                Flex::row()
                    .with_child(label.finish())
                    .with_child(
                        Container::new(switch_element)
                            .with_padding_top(padding_top)
                            .finish(),
                    )
                    .finish()
            } else {
                switch_element
            };

            // If a tooltip is configured and we're hovered, show it above the switch
            if let Some(TooltipConfig { text, styles }) = &tooltip {
                if state.is_hovered() {
                    let tooltip_element = Tooltip::new(text.clone(), *styles).build().finish();
                    return Stack::new()
                        .with_child(switch_element)
                        .with_positioned_child(
                            tooltip_element,
                            OffsetPositioning::offset_from_parent(
                                vec2f(0., -3.),
                                ParentOffsetBounds::Unbounded,
                                ParentAnchor::TopRight,
                                ChildAnchor::BottomRight,
                            ),
                        )
                        .finish();
                }
            }

            switch_element
        });

        if !self.disabled {
            hoverable.with_cursor(Cursor::PointingHand)
        } else {
            hoverable
        }
    }

    /// Overwrites _some_ styles passed in `style` parameter
    fn with_style(self, styles: UiComponentStyles) -> Self {
        Self {
            checked: self.checked,
            disabled: self.disabled,
            label: self.label,
            styles: self.styles.merge(styles),
            hovered_styles: Some(self.hovered_styles.unwrap_or(self.styles).merge(styles)),
            checked_styles: Some(self.checked_styles.unwrap_or(self.styles).merge(styles)),
            disabled_styles: Some(self.disabled_styles.unwrap_or(self.styles).merge(styles)),
            mouse_state: self.mouse_state,
            hover_border_size: self.hover_border_size,
            tooltip: self.tooltip,
        }
    }
}

impl Switch {
    pub fn new(
        mouse_state: SwitchStateHandle,
        default_styles: UiComponentStyles,
        hovered_styles: Option<UiComponentStyles>,
        checked_styles: Option<UiComponentStyles>,
        disabled_styles: Option<UiComponentStyles>,
    ) -> Self {
        Self {
            checked: false,
            disabled: false,
            label: None,
            styles: default_styles,
            hovered_styles,
            checked_styles,
            disabled_styles,
            mouse_state,
            hover_border_size: None,
            tooltip: None,
        }
    }

    /// Retained for backward compatibility with the old pill+thumb design.
    /// The new ON/OFF rectangle has no thumb, so this is a no-op.
    pub fn with_thumb_hover_border(mut self, border_size: f32) -> Self {
        self.hover_border_size = Some(border_size);
        self
    }

    pub fn with_disabled_styles(mut self, styles: UiComponentStyles) -> Self {
        self.disabled_styles = Some(self.disabled_styles.unwrap_or_default().merge(styles));
        self
    }

    pub fn check(mut self, check: bool) -> Self {
        self.checked = check;
        self
    }

    pub fn disable(mut self) -> Self {
        self.disabled = true;
        self
    }

    pub fn with_disabled(mut self, is_disabled: bool) -> Self {
        self.disabled = is_disabled;
        self
    }

    pub fn label(mut self, label: Span) -> Self {
        self.label = Some(label);
        self
    }

    /// Adds a tooltip that appears above the switch on hover.
    pub fn with_tooltip(mut self, config: TooltipConfig) -> Self {
        self.tooltip = Some(config);
        self
    }

    fn styles(&self, state: &MouseState) -> UiComponentStyles {
        if self.disabled {
            return self.disabled_styles.unwrap_or(self.styles);
        }

        if self.checked {
            return self.checked_styles.unwrap_or(self.styles);
        }

        if state.is_mouse_over_element() {
            return self.hovered_styles.unwrap_or(self.styles);
        }
        self.styles
    }

    fn render_switch(&self, styles: UiComponentStyles) -> Box<dyn Element> {
        let height = styles.height.unwrap_or(DEFAULT_SWITCH_HEIGHT);
        let width = height * 2.;

        let label_text = if self.checked { "ON" } else { "OFF" };
        // `UiBuilder::switch` always sets the font family in the style block,
        // so `expect` is safe here. If a future caller wires up the Switch
        // without going through `UiBuilder`, they need to set this explicitly.
        let font_family = styles
            .font_family_id
            .expect("switch styles must include font_family_id");
        let font_size = styles.font_size.unwrap_or(SWITCH_LABEL_FONT_SIZE);
        let label_color = styles.font_color.unwrap_or(ColorU::white());

        let label = Text::new_inline(label_text.to_string(), font_family, font_size)
            .with_color(label_color)
            .with_style(Properties::default().weight(Weight::Medium))
            .finish();

        let background = styles.background.unwrap_or(Fill::None);
        let border_color = styles.border_color.unwrap_or(Fill::None);
        let border_width = styles.border_width.unwrap_or(0.);

        // `Align::new` defaults to centering both axes, so the label sits in
        // the middle of the chip.
        let mut container = Container::new(
            ConstrainedBox::new(Align::new(label).finish())
                .with_width(width)
                .with_height(height)
                .finish(),
        )
        .with_background(background)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(SWITCH_CORNER_RADIUS)));

        if border_width > 0. {
            container =
                container.with_border(Border::all(border_width).with_border_fill(border_color));
        }

        container.finish()
    }
}
