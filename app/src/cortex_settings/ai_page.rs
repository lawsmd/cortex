//! Content rendered on the right side of the Cortex Settings pane when the
//! "AI" section is selected.
//!
//! Currently houses orchestration toggles and block action button visibility
//! toggles. The orchestration toggles control
//! [`warp_core::features::FeatureFlag::LocalClaudeCodexChildHarnesses`] and
//! plan-mode behaviour; the block button toggles control which buttons appear
//! in the per-block hover toolbar.
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
/// animation state). Owned by `CortexSettingsView`.
#[derive(Default)]
pub struct AiPageState {
    allow_local_claude_codex_child_harnesses_switch: SwitchStateHandle,
    orchestrated_subagents_start_in_plan_mode_switch: SwitchStateHandle,
    show_block_ai_button_switch: SwitchStateHandle,
    show_block_save_workflow_button_switch: SwitchStateHandle,
    show_block_filter_button_switch: SwitchStateHandle,
    show_block_overflow_button_switch: SwitchStateHandle,
}

pub fn ai_page_search_terms() -> &'static [&'static str] {
    &[
        "ai",
        "agent",
        "orchestrate",
        "orchestration",
        "claude",
        "claude code",
        "codex",
        "child",
        "harness",
        "subagent",
        "sub-agent",
        "plan mode",
        "plan",
        "permission",
        "block",
        "button",
        "toolbar",
        "filter",
        "overflow",
        "workflow",
    ]
}

pub fn render_ai_page(
    state: &AiPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let cortex = CortexSettings::as_ref(app);

    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(render_toggle_row(
            "Allow Claude Code / Codex as orchestrate child agents",
            *cortex.allow_local_claude_codex_child_harnesses,
            state.allow_local_claude_codex_child_harnesses_switch.clone(),
            CortexSettingsAction::ToggleAllowLocalClaudeCodexChildHarnesses,
            appearance,
        ))
        .with_child(render_toggle_row(
            "Orchestrated sub-agents start in Plan Mode",
            *cortex.orchestrated_subagents_start_in_plan_mode,
            state
                .orchestrated_subagents_start_in_plan_mode_switch
                .clone(),
            CortexSettingsAction::ToggleOrchestratedSubagentsStartInPlanMode,
            appearance,
        ))
        .with_child(render_toggle_row(
            "Show AI Assistant button in block toolbar",
            *cortex.show_block_ai_button,
            state.show_block_ai_button_switch.clone(),
            CortexSettingsAction::ToggleShowBlockAiButton,
            appearance,
        ))
        .with_child(render_toggle_row(
            "Show Save as Workflow button in block toolbar",
            *cortex.show_block_save_workflow_button,
            state.show_block_save_workflow_button_switch.clone(),
            CortexSettingsAction::ToggleShowBlockSaveWorkflowButton,
            appearance,
        ))
        .with_child(render_toggle_row(
            "Show Filter button in block toolbar",
            *cortex.show_block_filter_button,
            state.show_block_filter_button_switch.clone(),
            CortexSettingsAction::ToggleShowBlockFilterButton,
            appearance,
        ))
        .with_child(render_toggle_row(
            "Show Overflow menu button in block toolbar",
            *cortex.show_block_overflow_button,
            state.show_block_overflow_button_switch.clone(),
            CortexSettingsAction::ToggleShowBlockOverflowButton,
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
