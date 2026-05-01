//! Actions and section identifiers for the Cortex Settings UI.

use crate::settings::{
    TabsSelectedMetadataAlignment, TabsSelectedTitleAlignment, TabsUnselectedMetadataAlignment,
    TabsUnselectedTitleAlignment,
};

/// Top-level categories shown down the left side of the Cortex Settings pane.
/// Add new sections here as the toggle set grows.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CortexSettingsSection {
    Appearance,
    TabsPanes,
}

impl CortexSettingsSection {
    pub fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::TabsPanes => "Tabs/Panes",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::Appearance, Self::TabsPanes]
    }
}

impl Default for CortexSettingsSection {
    fn default() -> Self {
        Self::Appearance
    }
}

/// Actions emitted by the Cortex Settings view in response to user input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CortexSettingsAction {
    /// Switch the active sidebar section.
    SelectSection(CortexSettingsSection),
    /// Flip the `hide_pane_separators` toggle on the Appearance page.
    ToggleHidePaneSeparators,
    /// Flip the "Bar/Panel Background Matches Terminal Background" toggle.
    ToggleTabsPanelMatchesTerminalBg,
    /// Flip the "Inverse Fill on Selection" toggle.
    ToggleTabsInverseFillOnSelection,
    /// Flip the "Hide Icon Backdrop" toggle (Tab Icons section).
    ToggleTabsHideIconBackdrop,
    /// Set the selected-tab title alignment.
    SetTabsSelectedTitleAlignment(TabsSelectedTitleAlignment),
    /// Set the selected-tab metadata alignment.
    SetTabsSelectedMetadataAlignment(TabsSelectedMetadataAlignment),
    /// Set the unselected-tab title alignment.
    SetTabsUnselectedTitleAlignment(TabsUnselectedTitleAlignment),
    /// Set the unselected-tab metadata alignment.
    SetTabsUnselectedMetadataAlignment(TabsUnselectedMetadataAlignment),
    /// Flip the "Stack vertical tabs over side panel" toggle. Stacks the
    /// vertical tab bar on top of the side panel (Agent Conversations / File
    /// Explorer / Global Search / Warp Drive) in a single left-rail column
    /// with a draggable horizontal divider. Mirrored by the icon button in
    /// the side panel header.
    ToggleStackLeftColumn,
}
