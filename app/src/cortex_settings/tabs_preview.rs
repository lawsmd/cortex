//! Live animated preview of tab styling for the Cortex Settings > Tabs page.
//!
//! Renders a 2-column × 3-row grid: "Selected" and "Unselected" columns, each
//! showing a tab in Running (comet), AttentionNeeded (breath), and Idle states.
//! All cells respond live to font, alignment, and style setting changes.

use pathfinder_color::ColorU;
use settings::Setting as _;
use warpui::{
    elements::{
        Align, Border, ConstrainedBox, Container, CrossAxisAlignment, Element, Flex, Padding,
        ParentElement, Text,
    },
    fonts::Properties,
    AppContext, SingletonEntity,
};

use crate::appearance::Appearance;
use crate::settings::{CortexSettings, TabStyle, TabsMetadataAlignment, TabsTitleAlignment};
use crate::tab::TabAnimationKind;
use crate::themes::theme::Fill as ThemeFill;
use crate::workspace::view::vertical_tabs::cortex_extensions::{
    wrap_with_agent_animation_layers, VERTICAL_TAB_UNSELECTED_BORDER_GRAY,
};
use crate::workspace::view::vertical_tabs::cortex_tab_title_style;

const PREVIEW_TAB_WIDTH: f32 = 180.0;
const PREVIEW_TAB_HEIGHT: f32 = 48.0;
const PREVIEW_TAB_PADDING_H: f32 = 10.0;
const PREVIEW_TAB_PADDING_V: f32 = 6.0;
const PREVIEW_COLUMN_GAP: f32 = 12.0;
const PREVIEW_ROW_GAP: f32 = 8.0;
const PREVIEW_LABEL_WIDTH: f32 = 120.0;
const PREVIEW_LABEL_GAP: f32 = 12.0;
const PREVIEW_CORNER_RADIUS: f32 = 4.0;
const PREVIEW_BORDER_WIDTH: f32 = 1.0;
const PREVIEW_HEADER_FONT_SIZE: f32 = 12.0;
const PREVIEW_HEADER_MARGIN_BOTTOM: f32 = 6.0;
const METADATA_TEXT: &str = "~/projects";
const TITLE_TEXT: &str = "Cortex";
const METADATA_OPACITY: u8 = 70;

pub fn render_preview_section(
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let header_font = appearance.ui_font_family();
    let header_color = theme.nonactive_ui_text_color();

    let rows: &[(&str, Option<TabAnimationKind>)] = &[
        ("Agent Working", Some(TabAnimationKind::Running)),
        ("Attention Needed", Some(TabAnimationKind::AttentionNeeded)),
        ("Idle", None),
    ];

    let header_row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(
            ConstrainedBox::new(warpui::elements::Empty::new().finish())
                .with_width(PREVIEW_LABEL_WIDTH + PREVIEW_LABEL_GAP)
                .finish(),
        )
        .with_child(
            ConstrainedBox::new(
                Align::new(
                    Text::new_inline(
                        "Selected".to_string(),
                        header_font,
                        PREVIEW_HEADER_FONT_SIZE,
                    )
                    .with_color(header_color.into())
                    .finish(),
                )
                .finish(),
            )
            .with_width(PREVIEW_TAB_WIDTH)
            .finish(),
        )
        .with_child(
            ConstrainedBox::new(warpui::elements::Empty::new().finish())
                .with_width(PREVIEW_COLUMN_GAP)
                .finish(),
        )
        .with_child(
            ConstrainedBox::new(
                Align::new(
                    Text::new_inline(
                        "Unselected".to_string(),
                        header_font,
                        PREVIEW_HEADER_FONT_SIZE,
                    )
                    .with_color(header_color.into())
                    .finish(),
                )
                .finish(),
            )
            .with_width(PREVIEW_TAB_WIDTH)
            .finish(),
        )
        .finish();

    let mut grid = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(
            Container::new(header_row)
                .with_margin_bottom(PREVIEW_HEADER_MARGIN_BOTTOM)
                .finish(),
        );

    for (i, (label, anim)) in rows.iter().enumerate() {
        let margin = if i > 0 { PREVIEW_ROW_GAP } else { 0.0 };

        let label_el = ConstrainedBox::new(
            Align::new(
                Text::new_inline(
                    label.to_string(),
                    header_font,
                    PREVIEW_HEADER_FONT_SIZE,
                )
                .with_color(header_color.into())
                .finish(),
            )
            .left()
            .finish(),
        )
        .with_width(PREVIEW_LABEL_WIDTH)
        .finish();

        let data_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(label_el)
            .with_child(
                ConstrainedBox::new(warpui::elements::Empty::new().finish())
                    .with_width(PREVIEW_LABEL_GAP)
                    .finish(),
            )
            .with_child(render_preview_tab(true, *anim, appearance, app))
            .with_child(
                ConstrainedBox::new(warpui::elements::Empty::new().finish())
                    .with_width(PREVIEW_COLUMN_GAP)
                    .finish(),
            )
            .with_child(render_preview_tab(false, *anim, appearance, app))
            .finish();

        grid = grid.with_child(
            Container::new(data_row)
                .with_margin_top(margin)
                .finish(),
        );
    }

    Align::new(
        Container::new(grid.finish())
            .with_padding(Padding::uniform(4.0))
            .finish(),
    )
    .top_center()
    .finish()
}

fn render_preview_tab(
    is_selected: bool,
    animation: Option<TabAnimationKind>,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let cortex = CortexSettings::as_ref(app);
    let (title_family, title_size, title_props) = cortex_tab_title_style(appearance, app);
    let metadata_family = appearance.ui_font_family();
    let metadata_size = 12.0_f32;

    let title_centered = matches!(
        *cortex.tabs_title_alignment.value(),
        TabsTitleAlignment::Centered
    );
    let metadata_centered = matches!(
        *cortex.tabs_metadata_alignment.value(),
        TabsMetadataAlignment::Centered
    );

    let tab_style = *cortex.tab_style.value();

    let white_fill = ThemeFill::white();
    let (title_color, metadata_color, bg_fill, border) = match tab_style {
        TabStyle::CortexModern if is_selected => (
            theme.background(),
            theme.background().with_opacity(METADATA_OPACITY),
            Some(white_fill.clone()),
            Border::default(),
        ),
        TabStyle::CortexTui if is_selected => {
            let text_fill = ThemeFill::Solid(ColorU::white());
            let tui_border = if matches!(animation, Some(TabAnimationKind::Running)) {
                Border::default()
            } else {
                Border::all(PREVIEW_BORDER_WIDTH)
                    .with_border_fill(ThemeFill::Solid(ColorU::white()))
            };
            (
                text_fill.clone(),
                text_fill.with_opacity(METADATA_OPACITY),
                None,
                tui_border,
            )
        }
        _ => {
            let text_fill = ThemeFill::Solid(ColorU::white());
            let unselected_border = if matches!(animation, Some(TabAnimationKind::Running))
                && matches!(tab_style, TabStyle::CortexTui)
            {
                Border::default()
            } else {
                Border::all(PREVIEW_BORDER_WIDTH)
                    .with_border_fill(ThemeFill::Solid(VERTICAL_TAB_UNSELECTED_BORDER_GRAY))
            };
            (
                text_fill.clone(),
                text_fill.with_opacity(METADATA_OPACITY),
                None,
                unselected_border,
            )
        }
    };

    let title_element = Text::new_inline(TITLE_TEXT.to_string(), title_family, title_size)
        .with_style(title_props)
        .with_color(title_color.into())
        .finish();

    let metadata_element = Text::new_inline(
        METADATA_TEXT.to_string(),
        metadata_family,
        metadata_size,
    )
    .with_style(Properties::default())
    .with_color(metadata_color.into())
    .finish();

    let title_child: Box<dyn Element> = if title_centered {
        Align::new(title_element).finish()
    } else {
        title_element
    };

    let metadata_child: Box<dyn Element> = if metadata_centered {
        Align::new(metadata_element).finish()
    } else {
        metadata_element
    };

    let text_column = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(title_child)
        .with_child(metadata_child)
        .finish();

    let mut container = Container::new(text_column)
        .with_padding(
            Padding::uniform(0.0)
                .with_top(PREVIEW_TAB_PADDING_V)
                .with_bottom(PREVIEW_TAB_PADDING_V)
                .with_left(PREVIEW_TAB_PADDING_H)
                .with_right(PREVIEW_TAB_PADDING_H),
        )
        .with_corner_radius(warpui::elements::CornerRadius::with_all(
            warpui::elements::Radius::Pixels(PREVIEW_CORNER_RADIUS),
        ))
        .with_border(border);

    if let Some(fill) = bg_fill {
        container = container.with_background(fill);
    }

    let content = ConstrainedBox::new(container.finish())
        .with_width(PREVIEW_TAB_WIDTH)
        .with_height(PREVIEW_TAB_HEIGHT)
        .finish();

    let pane_color = Some(white_fill);
    wrap_with_agent_animation_layers(content, animation, pane_color.as_ref(), tab_style, is_selected)
}
