//! Actions and section identifiers for the Cortex Settings UI.

use warpui::fonts::Weight;

use crate::settings::{SearchBarStyle, TabStyle, TabsMetadataAlignment, TabsTitleAlignment};

/// Top-level categories shown down the left side of the Cortex Settings pane.
/// Add new sections here as the toggle set grows.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CortexSettingsSection {
    WorkingPanes,
    Tabs,
    TopBar,
    Toolbar,
    Editor,
    Ai,
}

impl CortexSettingsSection {
    pub fn label(self) -> &'static str {
        match self {
            Self::WorkingPanes => "Panes",
            Self::Tabs => "Tabs",
            Self::TopBar => "Top Bar",
            Self::Toolbar => "Toolbar",
            Self::Editor => "Editor",
            Self::Ai => "AI",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::WorkingPanes,
            Self::Tabs,
            Self::TopBar,
            Self::Toolbar,
            Self::Editor,
            Self::Ai,
        ]
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
    /// Flip the `hide_pane_separators` toggle on the Panes page.
    ToggleHidePaneSeparators,
    /// Flip the "Hide Previous Session Recap on Launch" toggle on the Panes page.
    ToggleStartWithBlankPaneOnLaunch,
    /// Flip the "Match Recap Style to Active Terminal" toggle on the Panes page.
    ToggleRecapMatchesTerminalStyle,
    /// Flip the "Bar/Panel Background Matches Terminal Background" toggle.
    ToggleTabsPanelMatchesTerminalBg,
    /// Flip the "Hide Icon Backdrop" toggle (Tab Icons section).
    ToggleTabsHideIconBackdrop,
    /// Set the tab style preset (e.g. Cortex Modern).
    SetTabStyle(TabStyle),
    /// Set the unified title alignment for all vertical tabs.
    SetTabsTitleAlignment(TabsTitleAlignment),
    /// Set the unified metadata alignment for all vertical tabs.
    SetTabsMetadataAlignment(TabsMetadataAlignment),
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
    /// Flip the "Allow Claude Code / Codex as orchestrate child agents" toggle
    /// on the AI page. Mirrors the persisted [`crate::settings::CortexSettings`]
    /// bool *and* pushes the same value into
    /// `FeatureFlag::LocalClaudeCodexChildHarnesses` via `set_user_preference`,
    /// so the orchestration controls (`local_child_harnesses.rs`,
    /// `orchestration_controls.rs`) react without a restart.
    ToggleAllowLocalClaudeCodexChildHarnesses,
    /// Flip the "Wrap Lines in 'Raw' View" toggle on the Editor page. Persisted to
    /// [`crate::settings::CortexSettings::editor_wrap_long_lines`]; read at
    /// editor model construction (reopen the file to apply a change).
    ToggleEditorWrapLongLines,
    /// Flip the "Top Bar Matches Terminal Background Color" toggle on the
    /// Top Bar page.
    ToggleTopBarMatchesTerminalBg,
    /// Flip the "Hide Top Bar Divider Line" toggle on the Top Bar page.
    ToggleTopBarHideDivider,
    /// Set the title-bar search bar visual style (Cortex Default / Warp Default).
    SetTopBarSearchBarStyle(SearchBarStyle),
    /// Toggle visibility of the File Explorer icon in the toolbar.
    ToggleToolbarShowFileExplorer,
    /// Toggle visibility of the Global Search icon in the toolbar.
    ToggleToolbarShowGlobalSearch,
    /// Toggle visibility of the Warp Drive icon in the toolbar.
    ToggleToolbarShowWarpDrive,
    /// Toggle visibility of the Agent Conversations icon in the toolbar.
    ToggleToolbarShowAgentConversations,
}
