//! Content rendered on the right side of the Cortex Settings pane when the
//! "AI" section is selected.
//!
//! Currently houses orchestration controls and block action button visibility
//! toggles. The orchestration controls cover
//! [`warp_core::features::FeatureFlag::LocalClaudeCodexChildHarnesses`] and the
//! sub-agent permission-mode selector (plan / auto / skip); the block button
//! toggles control which buttons appear in the per-block hover toolbar.
use warpui::{
    elements::{
        Align, ChildView, ConstrainedBox, Container, CrossAxisAlignment, Element, Flex, Padding,
        ParentElement, Shrinkable,
    },
    ui_components::{components::UiComponent, switch::SwitchStateHandle},
    AppContext, SingletonEntity, ViewContext, ViewHandle,
};

use settings::Setting;

use crate::appearance::Appearance;
use crate::cortex_settings::action::CortexSettingsAction;
use crate::cortex_settings::orchestrate_mode_options::{
    orchestrate_mode_dropdown_items, orchestrate_mode_label_for_value,
};
use crate::cortex_settings::view::CortexSettingsView;
use crate::settings::CortexSettings;
use crate::view_components::Dropdown;

const ROW_VERTICAL_PADDING: f32 = 6.0;
const CONTROL_RIGHT_PADDING: f32 = 5.0;
const ORCHESTRATE_MODE_DROPDOWN_WIDTH: f32 = 280.0;

/// Per-control UI state that has to outlive a single render frame (switch
/// animation state, dropdown view handle). Owned by `CortexSettingsView`.
pub struct AiPageState {
    allow_local_claude_codex_child_harnesses_switch: SwitchStateHandle,
    orchestrated_subagents_permission_mode_dropdown: ViewHandle<Dropdown<CortexSettingsAction>>,
    show_block_ai_button_switch: SwitchStateHandle,
    show_block_save_workflow_button_switch: SwitchStateHandle,
    show_block_filter_button_switch: SwitchStateHandle,
    show_block_overflow_button_switch: SwitchStateHandle,
}

impl AiPageState {
    pub fn new(ctx: &mut ViewContext<CortexSettingsView>) -> Self {
        let orchestrated_subagents_permission_mode_dropdown = ctx.add_typed_action_view(|ctx| {
            let mut dropdown = Dropdown::new(ctx);
            dropdown.set_top_bar_max_width(ORCHESTRATE_MODE_DROPDOWN_WIDTH);
            dropdown.set_menu_width(ORCHESTRATE_MODE_DROPDOWN_WIDTH, ctx);
            let items = orchestrate_mode_dropdown_items(
                CortexSettingsAction::SetOrchestratedSubagentsPermissionMode,
            );
            dropdown.add_items(items, ctx);
            let initial_value = (*CortexSettings::as_ref(ctx)
                .orchestrated_subagents_permission_mode
                .value())
            .clone();
            dropdown.set_selected_by_name(orchestrate_mode_label_for_value(&initial_value), ctx);
            dropdown
        });

        Self {
            allow_local_claude_codex_child_harnesses_switch: SwitchStateHandle::default(),
            orchestrated_subagents_permission_mode_dropdown,
            show_block_ai_button_switch: SwitchStateHandle::default(),
            show_block_save_workflow_button_switch: SwitchStateHandle::default(),
            show_block_filter_button_switch: SwitchStateHandle::default(),
            show_block_overflow_button_switch: SwitchStateHandle::default(),
        }
    }
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
        "auto",
        "auto mode",
        "skip",
        "permission",
        "permission mode",
        "hands-off",
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
        .with_child(render_orchestrate_permission_mode_row(state, appearance))
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

fn render_orchestrate_permission_mode_row(
    state: &AiPageState,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let label = appearance
        .ui_builder()
        .span("Orchestrated sub-agent permission mode".to_string())
        .build()
        .finish();
    let dropdown = ConstrainedBox::new(
        ChildView::new(&state.orchestrated_subagents_permission_mode_dropdown).finish(),
    )
    .with_width(ORCHESTRATE_MODE_DROPDOWN_WIDTH)
    .finish();

    let header = Shrinkable::new(
        1.0,
        Container::new(Align::new(label).left().finish()).finish(),
    )
    .finish();

    let control = Container::new(dropdown)
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
