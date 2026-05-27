//! Actions and section identifiers for the Cortex Settings UI.

use warpui::fonts::Weight;

use crate::settings::{TabStyle, TabsMetadataAlignment, TabsTitleAlignment};

/// Top-level categories shown down the left side of the Cortex Settings pane.
/// Add new sections here as the toggle set grows.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CortexSettingsSection {
    WorkingPanes,
    Tabs,
    TopBar,
    Toolbar,
    Editor,
    FileExplorer,
    Ai,
    /// Cortex-only: live health of the external-status hook bridge plus a
    /// "Test bridge" button. See `app/src/cortex_settings/diagnostics_page.rs`
    /// and `app/src/terminal/cli_agent_sessions/bridge_health.rs`.
    Diagnostics,
}

impl CortexSettingsSection {
    pub fn label(self) -> &'static str {
        match self {
            Self::WorkingPanes => "Panes",
            Self::Tabs => "Tabs",
            Self::TopBar => "Top Bar",
            Self::Toolbar => "Toolbar",
            Self::Editor => "Editor",
            Self::FileExplorer => "File Explorer",
            Self::Ai => "AI",
            Self::Diagnostics => "Diagnostics",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::WorkingPanes,
            Self::Tabs,
            Self::TopBar,
            Self::Toolbar,
            Self::Editor,
            Self::FileExplorer,
            Self::Ai,
            Self::Diagnostics,
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
    /// Flip the `rounded_pane_borders` toggle on the Panes page.
    ToggleRoundedPaneBorders,
    /// Flip the "Hide Previous Session Recap on Launch" toggle on the Panes page.
    ToggleStartWithBlankPaneOnLaunch,
    /// Flip the "Match Recap Style to Active Terminal" toggle on the Panes page.
    ToggleRecapMatchesTerminalStyle,
    /// Set the font family for pane header titles. Empty string = inherit UI font.
    SetPaneTitleFontName(String),
    /// Flip the "Bar/Panel Background Matches Terminal Background" toggle.
    ToggleTabsPanelMatchesTerminalBg,
    /// Flip the "Hide Tabs Search Button" toggle (Vertical Tab Bar/Panel section).
    /// When on, the magnifying-glass button in the panel's bottom action row is
    /// removed entirely — tab-name filtering becomes unavailable.
    ToggleTabsPanelHideSearchButton,
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
    /// Flip the "Orchestrated sub-agents start in Plan Mode" toggle on the AI
    /// page. Controls whether Cortex's `/orchestrate` skill spawns each child
    /// Claude Code sub-agent with `--permission-mode plan` (on) or
    /// `--dangerously-skip-permissions` (off). Read by the orchestrate IPC
    /// bridge at request time.
    ToggleOrchestratedSubagentsStartInPlanMode,
    /// Toggle the AI Assistant button in the block action toolbar.
    ToggleShowBlockAiButton,
    /// Toggle the Save as Workflow button in the block action toolbar.
    ToggleShowBlockSaveWorkflowButton,
    /// Toggle the Filter button in the block action toolbar.
    ToggleShowBlockFilterButton,
    /// Toggle the overflow menu button in the block action toolbar.
    ToggleShowBlockOverflowButton,
    /// Flip the "Wrap Lines in 'Raw' View" toggle on the Editor page. Persisted to
    /// [`crate::settings::CortexSettings::editor_wrap_long_lines`]; read at
    /// editor model construction (reopen the file to apply a change).
    ToggleEditorWrapLongLines,
    /// Flip the "Top Bar Matches Terminal Background Color" toggle on the
    /// Top Bar page.
    ToggleTopBarMatchesTerminalBg,
    /// Flip the "Hide Top Bar Divider Line" toggle on the Top Bar page.
    ToggleTopBarHideDivider,
    /// Set the title-bar search bar opacity (10..=100).
    SetTopBarSearchBarOpacity(u8),
    /// Flip the "Compact Search Bar" placeholder toggle.
    ToggleTopBarSearchBarCompact,
    /// Flip the "Hide Tabs Panel Collapse Button" toggle.
    ToggleTopBarHideTabsPanelCollapseButton,
    /// Flip the "Hide Agent Management Panel Button" toggle.
    ToggleTopBarHideAgentManagementButton,
    /// Flip the "Hide Notifications Button" toggle.
    ToggleTopBarHideNotificationsButton,
    /// Set the top-bar font family. Empty string = inherit UI font.
    SetTopBarFontName(String),
    /// Flip the "Replace Profile Button with Generic Icon" toggle.
    ToggleTopBarGenericProfileIcon,
    /// Toggle visibility of the File Explorer icon in the toolbar.
    ToggleToolbarShowFileExplorer,
    /// Toggle visibility of the Global Search icon in the toolbar.
    ToggleToolbarShowGlobalSearch,
    /// Toggle visibility of the Warp Drive icon in the toolbar.
    ToggleToolbarShowWarpDrive,
    /// Toggle visibility of the Agent Conversations icon in the toolbar.
    ToggleToolbarShowAgentConversations,
    /// Set the font family for the file explorer. Empty string = inherit UI font.
    SetFileExplorerFontName(String),
    /// Toggle box-drawing tree lines in the file explorer.
    ToggleFileExplorerTreeLines,
    /// Toggle Nerd Font icons in the file explorer.
    ToggleFileExplorerNerdIcons,
    /// Toggle per-file-type icon coloring in the file explorer.
    ToggleFileExplorerColoredIcons,
    /// Cortex-only: trigger a manual sweep of the bridge-health watchdog
    /// from the Diagnostics page's "Test bridge" button. Refreshes the
    /// state shown in the UI without waiting for the next 5 s sweep tick.
    /// See `app/src/terminal/cli_agent_sessions/bridge_health.rs`.
    TriggerBridgeHealthSweep,
}
