use settings::{
    macros::define_settings_group, RespectUserSyncSetting, SupportedPlatforms, SyncToCloud,
};
use warpui::fonts::Weight;

/// Horizontal alignment of a line of text in a Cortex vertical-tab row.
///
/// Four separate enums (one per setting field) all share this two-variant shape
/// because `implement_setting_for_enum!` binds the `Setting` impl to a single
/// concrete type — collapsing them into one shared enum would force all four
/// settings to share a single TOML key. Same trick Warp's `tab_settings.rs`
/// uses for `VerticalTabsPrimaryInfo` / `VerticalTabsCompactSubtitle` (different
/// enums, same shape). The variants serialize as `centered` and `warp_default`
/// respectively.
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
pub enum TabsSelectedTitleAlignment {
    #[default]
    Centered,
    WarpDefault,
}

settings::macros::implement_setting_for_enum!(
    TabsSelectedTitleAlignment,
    CortexSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "cortex.tabs.selected.title_alignment",
    description: "Horizontal alignment of the title line on a selected Cortex vertical tab.",
);

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
pub enum TabsSelectedMetadataAlignment {
    #[default]
    Centered,
    WarpDefault,
}

settings::macros::implement_setting_for_enum!(
    TabsSelectedMetadataAlignment,
    CortexSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "cortex.tabs.selected.metadata_alignment",
    description: "Horizontal alignment of the metadata subtitle line on a selected Cortex vertical tab.",
);

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
pub enum TabsUnselectedTitleAlignment {
    #[default]
    Centered,
    WarpDefault,
}

settings::macros::implement_setting_for_enum!(
    TabsUnselectedTitleAlignment,
    CortexSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "cortex.tabs.unselected.title_alignment",
    description: "Horizontal alignment of the title line on an unselected Cortex vertical tab.",
);

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
pub enum TabsUnselectedMetadataAlignment {
    #[default]
    Centered,
    WarpDefault,
}

settings::macros::implement_setting_for_enum!(
    TabsUnselectedMetadataAlignment,
    CortexSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "cortex.tabs.unselected.metadata_alignment",
    description: "Horizontal alignment of the metadata subtitle line on an unselected Cortex vertical tab.",
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
    tabs_selected_title_alignment: TabsSelectedTitleAlignment,
    tabs_selected_metadata_alignment: TabsSelectedMetadataAlignment,
    tabs_unselected_title_alignment: TabsUnselectedTitleAlignment,
    tabs_unselected_metadata_alignment: TabsUnselectedMetadataAlignment,
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
    }
]);
