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
    AppContext, SingletonEntity,
};

use crate::appearance::Appearance;
use crate::cortex_settings::action::CortexSettingsAction;
use crate::settings::CortexSettings;

const ROW_VERTICAL_PADDING: f32 = 6.0;
const CONTROL_RIGHT_PADDING: f32 = 5.0;

/// Per-toggle UI state that has to outlive a single render frame (switch
/// animation handles, etc.). Owned by `CortexSettingsView` and threaded into
/// the render fn via `&self`.
#[derive(Default)]
pub struct EditorPageState {
    wrap_long_lines_switch: SwitchStateHandle,
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
    ]
}

pub fn render_editor_page(
    state: &EditorPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(render_wrap_long_lines_row(state, appearance, app))
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
