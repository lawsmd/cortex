//! Shared font-family options for Cortex Settings dropdowns.
//!
//! Both the Tabs > Tab Title Font and Top Bar > Top Bar Font menus expose the
//! same four-entry list (Roboto/Hack/Fira Code, plus "(use UI font)" sentinel
//! for the empty-string default). Living here avoids duplicating the table
//! across pages.

use crate::cortex_settings::action::CortexSettingsAction;
use crate::view_components::DropdownItem;

pub const FONT_FAMILY_OPTIONS: &[(&str, &str)] = &[
    ("(use UI font)", ""),
    ("Fira Code", "Fira Code"),
    ("Hack", "Hack"),
    ("Roboto", "Roboto"),
];

pub fn font_family_label_for_value(value: &str) -> &'static str {
    FONT_FAMILY_OPTIONS
        .iter()
        .find(|(_, v)| *v == value)
        .map(|(label, _)| *label)
        .unwrap_or(FONT_FAMILY_OPTIONS[0].0)
}

/// Build a dropdown items list with each entry dispatching the provided
/// action variant. The action takes the raw font-family value (or `""` for
/// the UI-font sentinel).
pub fn font_family_dropdown_items<F>(action_for_value: F) -> Vec<DropdownItem<CortexSettingsAction>>
where
    F: Fn(String) -> CortexSettingsAction,
{
    FONT_FAMILY_OPTIONS
        .iter()
        .map(|(label, value)| DropdownItem::new(*label, action_for_value((*value).to_string())))
        .collect()
}
