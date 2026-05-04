//! Actions and section identifiers for the Cortex Settings UI.

use warpui::fonts::Weight;

use crate::settings::{
    TabsSelectedMetadataAlignment, TabsSelectedTitleAlignment, TabsUnselectedMetadataAlignment,
    TabsUnselectedTitleAlignment,
};

/// Top-level categories shown down the left side of the Cortex Settings pane.
/// Add new sections here as the toggle set grows.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CortexSettingsSection {
    WorkingPanes,
    Tabs,
}

impl CortexSettingsSection {
    pub fn label(self) -> &'static str {
        match self {
            Self::WorkingPanes => "Working Panes",
            Self::Tabs => "Tabs",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::WorkingPanes, Self::Tabs]
    }
}

impl Default for CortexSettingsSection {
    fn default() -> Self {
        Self::WorkingPanes
    }
}

/// Actions emitted by the Cortex Settings view in response to user input.
///
/// Carries `f32` (font size) so derives `PartialEq` only — `f32` is not `Eq`.
#[derive(Clone, Debug, PartialEq)]
pub enum CortexSettingsAction {
    /// Switch the active sidebar section.
    SelectSection(CortexSettingsSection),
    /// Flip the `hide_pane_separators` toggle on the Working Panes page.
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
    /// Set the font family for tab titles. Empty string = inherit UI font.
    SetTabTitleFontName(String),
    /// Set the font size for tab titles (logical px). Clamped at the
    /// consumption site to 8..=32.
    SetTabTitleFontSize(f32),
    /// Set the font weight for tab titles.
    SetTabTitleFontWeight(Weight),
    /// Flip the italic toggle for tab titles.
    ToggleTabTitleItalic,
}
