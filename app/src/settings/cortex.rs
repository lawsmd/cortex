use settings::{
    macros::define_settings_group, RespectUserSyncSetting, SupportedPlatforms, SyncToCloud,
};
use warpui::fonts::Weight;

/// Visual style preset for vertical tabs.
///
/// `CortexModern` implies inverse fill on selected tabs (accent-colored
/// background, terminal-background text). `CortexTui` uses an accent-colored
/// border on selected tabs with no fill, and a minimalist traveling-dot
/// animation instead of the comet.
#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    Copy,
    Clone,
    Hash,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(rename_all = "snake_case")]
pub enum TabStyle {
    CortexModern,
    #[default]
    CortexTui,
}

settings::macros::implement_setting_for_enum!(
    TabStyle,
    CortexSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "cortex.tabs.style",
    description: "Visual style preset for vertical tabs. cortex_modern uses inverse fill on selected tabs; cortex_tui uses accent-colored borders and traveling-dot animation.",
);

/// Horizontal alignment of the title line on vertical tabs (both selected and
/// unselected).
#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    Copy,
    Clone,
    Hash,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(rename_all = "snake_case")]
pub enum TabsTitleAlignment {
    #[default]
    Centered,
    WarpDefault,
}

settings::macros::implement_setting_for_enum!(
    TabsTitleAlignment,
    CortexSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "cortex.tabs.title_alignment",
    description: "Horizontal alignment of the title line on Cortex vertical tabs.",
);

/// Horizontal alignment of the metadata subtitle line on vertical tabs (both
/// selected and unselected).
#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    Copy,
    Clone,
    Hash,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(rename_all = "snake_case")]
pub enum TabsMetadataAlignment {
    #[default]
    Centered,
    WarpDefault,
}

settings::macros::implement_setting_for_enum!(
    TabsMetadataAlignment,
    CortexSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "cortex.tabs.metadata_alignment",
    description: "Horizontal alignment of the metadata subtitle line on Cortex vertical tabs.",
);

/// Visual style of the title-bar search bar.
///
/// `CortexDefault` replaces the filled background with a thin border in the
/// search-text color (brighter on hover). `WarpDefault` keeps the upstream
/// semi-transparent pill background.
#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    Copy,
    Clone,
    Hash,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(rename_all = "snake_case")]
pub enum SearchBarStyle {
    #[default]
    CortexDefault,
    WarpDefault,
}

settings::macros::implement_setting_for_enum!(
    SearchBarStyle,
    CortexSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "cortex.top_bar.search_bar_style",
    description: "Visual style of the title-bar search bar. cortex_default replaces the filled background with a thin border; warp_default keeps the upstream pill background.",
);

define_settings_group!(CortexSettings, settings: [
    hide_pane_separators: HidePaneSeparators {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.appearance.hide_pane_separators",
        description: "Whether the thin separator lines between panels and around input boxes are hidden.",
    },
    start_with_blank_pane_on_launch: StartWithBlankPaneOnLaunch {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.panes.start_with_blank_pane_on_launch",
        description: "When restoring a session on launch, drop the block-level recap of the previous session inside each restored pane — open the pane as a blank shell instead. Tabs and panes are still restored; only the prior conversation/scrollback content is suppressed.",
    },
    recap_matches_terminal_style: RecapMatchesTerminalStyle {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.panes.recap_matches_terminal_style",
        description: "Render restored session blocks with the same background and text colors as live terminal content, instead of the dim gray foreground overlay upstream Warp uses to mark them as inactive scrollback. Cortex default: on (no visual distinction).",
    },
    hide_tab_icon: HideTabIcon {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.appearance.hide_tab_icon",
        description: "Whether the leading icon (and its border) inside each vertical-tab row is hidden.",
    },
    hide_tab_metadata: HideTabMetadata {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.appearance.hide_tab_metadata",
        description: "Whether the per-tab metadata subtitle (the line that 'Additional metadata' configures in compact view) is hidden.",
    },
    tabs_panel_matches_terminal_bg: TabsPanelMatchesTerminalBg {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.tabs.panel.matches_terminal_background",
        description: "Whether the vertical tab bar/panel background matches the terminal background instead of the theme's default panel color.",
    },
    tabs_panel_row_spacing: TabsPanelRowSpacing {
        type: f32,
        default: 8.0,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.tabs.panel.row_spacing",
        description: "Vertical spacing in pixels between tab rows in the vertical tab panel. Adjusted via the slider in the vertical-tabs settings popup. Range 8–24 px; widened from 0–16 to prevent tab-edge animations from clipping into neighbors.",
    },
    tabs_inverse_fill_on_selection: TabsInverseFillOnSelection {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.tabs.selected.inverse_fill",
        description: "Whether the selected vertical tab inverts its colors so the tab fills with its accent color and the title/metadata text become the terminal background color.",
    },
    tabs_hide_icon_backdrop: TabsHideIconBackdrop {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.tabs.icon.hide_backdrop",
        description: "Whether the small circular backdrop rendered behind a vertical tab's leading icon is hidden. Affects neutral pane icons (terminal, settings, code, etc.); CLI/Oz agent badge backgrounds are unchanged because their colors carry identity meaning.",
    },
    tab_style: TabStyle,
    tabs_title_alignment: TabsTitleAlignment,
    tabs_metadata_alignment: TabsMetadataAlignment,
    stack_left_column: StackLeftColumn {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.layout.stack_left_column",
        description: "Whether the vertical tab bar is stacked on top of the multi-view side panel (Agent Conversations / File Explorer / Global Search / Warp Drive) in a single left-rail column with a draggable horizontal divider, instead of rendering them as horizontal siblings. Cortex default: on.",
    },
    stacked_left_top_height_px: StackedLeftTopHeightPx {
        type: f32,
        default: 300.0,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.layout.stacked_left_top_height_px",
        description: "When the left rail is stacked, the height in pixels of the top half (vertical tab bar). Updated when the user drags the horizontal divider; clamped on each render to the column's safe min/max."
    },
    tabs_title_font_name: TabsTitleFontName {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.tabs.title.font_name",
        description: "Font family for tab titles in both the horizontal tab bar and the vertical tab rail. Empty string falls back to the UI font.",
    },
    tabs_title_font_size: TabsTitleFontSize {
        type: f32,
        default: 12.0,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.tabs.title.font_size",
        description: "Tab title font size in logical px. Clamped to 8..=32 at the consumption site.",
    },
    tabs_title_font_weight: TabsTitleFontWeight {
        type: Weight,
        default: Weight::Normal,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.tabs.title.font_weight",
        description: "Tab title font weight. Active tabs are bumped to at least Medium so a Normal/Light base weight still reads as differentiated.",
    },
    tabs_title_italic: TabsTitleItalic {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.tabs.title.italic",
        description: "Render tab titles in italic.",
    },
    cli_agent_clear_scrolls_to_top: CliAgentClearScrollsToTop {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.terminal.cli_agent_clear_scrolls_to_top",
        description: "When a running CLI agent (Claude Code, Codex, Cursor, Gemini) runs /clear, scroll the viewport so the agent's freshly-cleared UI sits at the top of the visible pane and the prior conversation remains in scrollback. Claude is wired via Cortex's OSC-777 → SessionEnd hook; other agents are detected when they emit ESC[2J. Off restores upstream Warp behavior.",
    },
    allow_local_claude_codex_child_harnesses: AllowLocalClaudeCodexChildHarnesses {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.ai.allow_local_claude_codex_child_harnesses",
        description: "Whether /orchestrate's Local execution mode may spawn child agents using the Claude Code or Codex CLI harnesses instead of being limited to Oz. Upstream Warp keeps this gated behind FeatureFlag::LocalClaudeCodexChildHarnesses; Cortex hydrates that flag from this setting at startup and on each toggle, so checks at the existing call sites (local_child_harnesses.rs, orchestration_controls.rs) react without a restart. Default on — the whole point of the Cortex fork on this branch is to route /orchestrate children through your local Claude Code login.",
    },
    orchestrated_subagents_start_in_plan_mode: OrchestratedSubagentsStartInPlanMode {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.ai.orchestrated_subagents_start_in_plan_mode",
        description: "Whether Cortex's `/orchestrate` skill spawns each child Claude Code sub-agent in Plan Mode (`--permission-mode plan`). When off, sub-agents launch with `--dangerously-skip-permissions`. Default on so the user reviews each sub-agent's plan before execution.",
    },
    editor_wrap_long_lines: EditorWrapLongLines {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.editor.wrap_long_lines",
        description: "When on, the Raw file viewer soft-wraps lines to the viewport width instead of horizontally scrolling. Read once at editor model construction; reopen the file to apply a change.",
    },
    top_bar_matches_terminal_bg: TopBarMatchesTerminalBg {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.top_bar.matches_terminal_background",
        description: "Whether the top/title bar background matches the terminal background instead of the theme's semi-transparent foreground overlay. Cortex default: on (fully transparent).",
    },
    top_bar_hide_divider: TopBarHideDivider {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.top_bar.hide_divider",
        description: "Whether the thin horizontal divider line at the bottom of the top bar is hidden. Cortex default: on (hidden).",
    },
    top_bar_search_bar_style: SearchBarStyle,
    toolbar_show_file_explorer: ToolbarShowFileExplorer {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.toolbar.show_file_explorer",
        description: "Show the File Explorer icon in the toolbar.",
    },
    toolbar_show_global_search: ToolbarShowGlobalSearch {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.toolbar.show_global_search",
        description: "Show the Global Search icon in the toolbar.",
    },
    toolbar_show_warp_drive: ToolbarShowWarpDrive {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.toolbar.show_warp_drive",
        description: "Show the Warp Drive icon in the toolbar.",
    },
    toolbar_show_agent_conversations: ToolbarShowAgentConversations {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.toolbar.show_agent_conversations",
        description: "Show the Agent Conversations icon in the toolbar.",
    },
    file_explorer_font_name: FileExplorerFontName {
        type: String,
        default: String::from("FiraCode Nerd Font Mono"),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.file_explorer.font_name",
        description: "Font family for file/folder labels in the file explorer. Empty string falls back to the UI font.",
    },
    file_explorer_tree_lines: FileExplorerTreeLines {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.file_explorer.tree_lines",
        description: "Draw box-drawing characters connecting sibling entries in the file tree.",
    },
    file_explorer_nerd_icons: FileExplorerNerdIcons {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.file_explorer.nerd_icons",
        description: "Replace SVG file-type icons with colored Nerd Font glyphs.",
    },
    file_explorer_colored_icons: FileExplorerColoredIcons {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.file_explorer.colored_icons",
        description: "Color Nerd Font icons per file type (Rust=orange, Python=blue, etc.). When off, all icons use the theme text color.",
    },
    file_explorer_font_size: FileExplorerFontSize {
        type: f32,
        default: 13.0,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.file_explorer.font_size",
        description: "Font size for file and folder labels in the file explorer. Clamped to 10..=18.",
    }
]);
