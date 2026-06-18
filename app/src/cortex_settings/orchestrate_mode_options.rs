//! Permission-mode options for the AI page's orchestrate sub-agent dropdown.
//!
//! The three values map directly onto `ClaudePermissionMode` in the orchestrate
//! IPC bridge: `auto` → `--permission-mode auto`, `plan` → `--permission-mode
//! plan`, `skip` → `--dangerously-skip-permissions`. Stored as the
//! `cortex.ai.orchestrated_subagents_permission_mode` string setting.

use crate::cortex_settings::action::CortexSettingsAction;
use crate::view_components::DropdownItem;

/// `(label, value)` pairs, in display order. The first entry is the default
/// (`auto`) and the fallback for any unrecognized stored value.
pub const ORCHESTRATE_MODE_OPTIONS: &[(&str, &str)] = &[
    ("Auto — approve plan once, then hands-off", "auto"),
    ("Plan — review and approve every step", "plan"),
    ("Skip — no plan gate, no prompts", "skip"),
];

pub fn orchestrate_mode_label_for_value(value: &str) -> &'static str {
    ORCHESTRATE_MODE_OPTIONS
        .iter()
        .find(|(_, v)| *v == value)
        .map(|(label, _)| *label)
        .unwrap_or(ORCHESTRATE_MODE_OPTIONS[0].0)
}

/// Build the dropdown items, each dispatching the provided action variant with
/// the raw mode value (`"auto" | "plan" | "skip"`).
pub fn orchestrate_mode_dropdown_items<F>(
    action_for_value: F,
) -> Vec<DropdownItem<CortexSettingsAction>>
where
    F: Fn(String) -> CortexSettingsAction,
{
    ORCHESTRATE_MODE_OPTIONS
        .iter()
        .map(|(label, value)| DropdownItem::new(*label, action_for_value((*value).to_string())))
        .collect()
}
