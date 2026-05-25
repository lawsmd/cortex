//! Content rendered on the right side of the Cortex Settings pane when the
//! "Toolbar" section is selected.
//!
//! Houses switches for showing/hiding each toolbar icon (File Explorer, Global
//! Search, Warp Drive, Agent Conversations). New toolbar toggles go here.
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

#[derive(Default)]
pub struct ToolbarPageState {
    file_explorer_switch: SwitchStateHandle,
    global_search_switch: SwitchStateHandle,
    warp_drive_switch: SwitchStateHandle,
    agent_conversations_switch: SwitchStateHandle,
}

pub fn toolbar_page_search_terms() -> &'static [&'static str] {
    &[
        "toolbar", "file", "explorer", "search", "global", "drive", "warp", "agent",
        "conversations", "icon", "show", "hide",
    ]
}

pub fn render_toolbar_page(
    state: &ToolbarPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(render_toggle_row(
            "File Explorer",
            *CortexSettings::as_ref(app).toolbar_show_file_explorer,
            state.file_explorer_switch.clone(),
            CortexSettingsAction::ToggleToolbarShowFileExplorer,
            appearance,
        ))
        .with_child(render_toggle_row(
            "Global Search",
            *CortexSettings::as_ref(app).toolbar_show_global_search,
            state.global_search_switch.clone(),
            CortexSettingsAction::ToggleToolbarShowGlobalSearch,
            appearance,
        ))
        .with_child(render_toggle_row(
            "Warp Drive",
            *CortexSettings::as_ref(app).toolbar_show_warp_drive,
            state.warp_drive_switch.clone(),
            CortexSettingsAction::ToggleToolbarShowWarpDrive,
            appearance,
        ))
        .with_child(render_toggle_row(
            "Agent Conversations",
            *CortexSettings::as_ref(app).toolbar_show_agent_conversations,
            state.agent_conversations_switch.clone(),
            CortexSettingsAction::ToggleToolbarShowAgentConversations,
            appearance,
        ))
        .finish()
}

fn render_toggle_row(
    label_text: &str,
    current_value: bool,
    switch_state: SwitchStateHandle,
    action: CortexSettingsAction,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();

    let label = ui_builder
        .span(label_text.to_string())
        .build()
        .finish();

    let switch = ui_builder
        .switch(switch_state)
        .check(current_value)
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
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
