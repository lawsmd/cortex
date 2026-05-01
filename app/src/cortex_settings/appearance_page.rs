//! Content rendered on the right side of the Cortex Settings pane when the
//! "Appearance" section is selected.
//!
//! Currently houses a single toggle: `Hide Pane Separator Lines`. New
//! appearance toggles go here.
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

    let label = ui_builder
        .span("Hide Pane Separator Lines".to_string())
        .build()
        .finish();

    let switch = ui_builder
        .switch(state.hide_pane_separators_switch.clone())
        .check(current_value)
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(CortexSettingsAction::ToggleHidePaneSeparators);
        })
        .finish();

    // Mirrors the Warp `build_toggle_element` shape (settings_page.rs:778):
    // label hugs the left edge of the centered max-width column, control
    // hugs the right.
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
