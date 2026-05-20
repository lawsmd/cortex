//! Content rendered on the right side of the Cortex Settings pane when the
//! "AI" section is selected.
//!
//! Currently houses a single toggle: `Allow Claude Code / Codex as
//! orchestrate child agents`. The toggle is the user-facing surface for
//! [`warp_core::features::FeatureFlag::LocalClaudeCodexChildHarnesses`]; the
//! flag stays the single source of truth read by
//! `app/src/ai/local_child_harnesses.rs` and
//! `app/src/ai/blocklist/inline_action/orchestration_controls.rs`, and the
//! setting just hydrates that flag at startup and on each user flip.
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
    ]
}

pub fn render_ai_page(
    state: &AiPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(render_allow_local_claude_codex_child_harnesses_row(
            state, appearance, app,
        ))
        .finish()
}

fn render_allow_local_claude_codex_child_harnesses_row(
    state: &AiPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();
    let current_value = *CortexSettings::as_ref(app).allow_local_claude_codex_child_harnesses;

    let label = ui_builder
        .span("Allow Claude Code / Codex as orchestrate child agents".to_string())
        .build()
        .finish();

    let switch = ui_builder
        .switch(
            state
                .allow_local_claude_codex_child_harnesses_switch
                .clone(),
        )
        .check(current_value)
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(
                CortexSettingsAction::ToggleAllowLocalClaudeCodexChildHarnesses,
            );
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
