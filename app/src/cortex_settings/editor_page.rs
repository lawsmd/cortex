//! Content rendered on the right side of the Cortex Settings pane when the
//! "Editor" section is selected.
//!
//! Houses toggles that affect how files are displayed in the file viewer /
//! code editor (the surface that opens when a user clicks a saved file in a
//! Cortex tab). New editor-level toggles go here.
use warpui::{
    elements::{
        Align, Container, CrossAxisAlignment, Element, Flex, Padding, ParentElement, Shrinkable,
    },
    ui_components::{components::UiComponent, switch::SwitchStateHandle},
    AppContext, SingletonEntity, ViewHandle,
};

use crate::view_components::Dropdown;

use settings::Setting as _;

use crate::appearance::Appearance;
use crate::cortex_settings::action::CortexSettingsAction;
use crate::cortex_settings::font_options::{
    font_family_dropdown_items, font_family_label_for_value,
};
use crate::settings::CortexSettings;

const ROW_VERTICAL_PADDING: f32 = 6.0;
const CONTROL_RIGHT_PADDING: f32 = 5.0;
const TITLE_FONT_DROPDOWN_WIDTH: f32 = 160.0;

/// Per-toggle UI state that has to outlive a single render frame (switch
/// animation handles, etc.). Owned by `CortexSettingsView` and threaded into
/// the render fn via `&self`.
pub struct EditorPageState {
    wrap_long_lines_switch: SwitchStateHandle,
    header_project_color_switch: SwitchStateHandle,
    pub(crate) title_font_dropdown: ViewHandle<Dropdown<CortexSettingsAction>>,
}

impl EditorPageState {
    pub fn new(ctx: &mut warpui::ViewContext<super::view::CortexSettingsView>) -> Self {
        let title_font_dropdown = ctx.add_typed_action_view(|ctx| {
            let mut dropdown = Dropdown::new(ctx);
            dropdown.set_top_bar_max_width(TITLE_FONT_DROPDOWN_WIDTH);
            dropdown.set_menu_width(TITLE_FONT_DROPDOWN_WIDTH, ctx);
            let items =
                font_family_dropdown_items(CortexSettingsAction::SetEditorTitleFontName);
            dropdown.add_items(items, ctx);
            let initial =
                (*CortexSettings::as_ref(ctx).editor_title_font_name.value()).clone();
            dropdown.set_selected_by_name(font_family_label_for_value(&initial), ctx);
            dropdown
        });
        Self {
            wrap_long_lines_switch: Default::default(),
            header_project_color_switch: Default::default(),
            title_font_dropdown,
        }
    }
}

pub fn editor_page_search_terms() -> &'static [&'static str] {
    &[
        "editor",
        "wrap",
        "lines",
        "raw",
        "view",
        "soft",
        "wrap",
        "line",
        "scroll",
        "horizontal",
        "markdown",
        "file",
        "viewer",
        "header",
        "color",
        "project",
        "title",
        "font",
    ]
}

pub fn render_editor_page(
    state: &EditorPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(render_header_project_color_row(state, appearance, app))
        .with_child(render_title_font_row(state, appearance))
        .with_child(render_wrap_long_lines_row(state, appearance, app))
        .finish()
}

fn render_header_project_color_row(
    state: &EditorPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();
    let current_value = *CortexSettings::as_ref(app).editor_header_project_color;

    let label = ui_builder
        .span("Header Project Color".to_string())
        .build()
        .finish();

    let switch = ui_builder
        .switch(state.header_project_color_switch.clone())
        .check(current_value)
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(CortexSettingsAction::ToggleEditorHeaderProjectColor);
        })
        .finish();

    let header = Shrinkable::new(
        1.0,
        Container::new(Align::new(label).left().finish()).finish(),
    )
    .finish();

    let control = Container::new(switch)
        .with_padding_right(CONTROL_RIGHT_PADDING)
        .finish();

    let row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(header)
        .with_child(control)
        .finish();

    Container::new(row)
        .with_padding(Padding::uniform(ROW_VERTICAL_PADDING))
        .finish()
}

fn render_title_font_row(
    state: &EditorPageState,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();

    let label = ui_builder
        .span("Title Font".to_string())
        .build()
        .finish();

    let header = Shrinkable::new(
        1.0,
        Container::new(Align::new(label).left().finish()).finish(),
    )
    .finish();

    let control = Container::new(
        warpui::presenter::ChildView::new(&state.title_font_dropdown).finish(),
    )
    .with_padding_right(CONTROL_RIGHT_PADDING)
    .finish();

    let row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(header)
        .with_child(control)
        .finish();

    Container::new(row)
        .with_padding(Padding::uniform(ROW_VERTICAL_PADDING))
        .finish()
}

fn render_wrap_long_lines_row(
    state: &EditorPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();
    let current_value = *CortexSettings::as_ref(app).editor_wrap_long_lines;

    let label = ui_builder
        .span("Wrap Lines in 'Raw' View".to_string())
        .build()
        .finish();

    let switch = ui_builder
        .switch(state.wrap_long_lines_switch.clone())
        .check(current_value)
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(CortexSettingsAction::ToggleEditorWrapLongLines);
        })
        .finish();

    let header = Shrinkable::new(
        1.0,
        Container::new(Align::new(label).left().finish()).finish(),
    )
    .finish();

    let control = Container::new(switch)
        .with_padding_right(CONTROL_RIGHT_PADDING)
        .finish();

    let row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(header)
        .with_child(control)
        .finish();

    Container::new(row)
        .with_padding(Padding::uniform(ROW_VERTICAL_PADDING))
        .finish()
}
