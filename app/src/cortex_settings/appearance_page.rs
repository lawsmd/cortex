//! Content rendered on the right side of the Cortex Settings pane when the
//! "Appearance" section is selected.
//!
//! Currently houses a single toggle: `Hide Pane Separator Lines`. New
//! appearance toggles go here.
use warpui::{
    elements::{
        Container, CrossAxisAlignment, Element, Flex, MainAxisAlignment, Padding, ParentElement,
    },
    ui_components::{components::UiComponent, switch::SwitchStateHandle},
    AppContext, SingletonEntity,
};

use crate::appearance::Appearance;
use crate::cortex_settings::action::CortexSettingsAction;
use crate::settings::CortexSettings;

const ROW_VERTICAL_PADDING: f32 = 6.0;
const LABEL_RIGHT_MARGIN: f32 = 12.0;

/// Per-toggle UI state that has to outlive a single render frame (mouse-state
/// handles for hover detection, switch animation state, etc.). Owned by
/// `CortexSettingsView` and threaded into the render fns via `&self`.
#[derive(Default)]
pub struct AppearancePageState {
    hide_pane_separators_switch: SwitchStateHandle,
}

pub fn appearance_page_search_terms() -> &'static [&'static str] {
    &["appearance", "hide", "pane", "separator", "lines", "border"]
}

pub fn render_appearance_page(
    state: &AppearancePageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(render_hide_pane_separators_row(state, appearance, app))
        .finish()
}

fn render_hide_pane_separators_row(
    state: &AppearancePageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();
    let current_value = *CortexSettings::as_ref(app).hide_pane_separators;

    let label = Container::new(
        ui_builder
            .span("Hide Pane Separator Lines".to_string())
            .build()
            .finish(),
    )
    .with_margin_right(LABEL_RIGHT_MARGIN)
    .finish();

    let switch = ui_builder
        .switch(state.hide_pane_separators_switch.clone())
        .check(current_value)
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(CortexSettingsAction::ToggleHidePaneSeparators);
        })
        .finish();

    let row = Flex::row()
        .with_main_axis_alignment(MainAxisAlignment::Start)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(label)
        .with_child(switch)
        .finish();

    Container::new(row)
        .with_padding(Padding::uniform(ROW_VERTICAL_PADDING))
        .finish()
}
