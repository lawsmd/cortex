use settings::{
    macros::define_settings_group, RespectUserSyncSetting, SupportedPlatforms, SyncToCloud,
};

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
        default: 4.0,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.tabs.panel.row_spacing",
        description: "Vertical spacing in pixels between tab rows in the vertical tab panel. Adjusted via the slider in the vertical-tabs settings popup.",
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
    tabs_selected_title_alignment: TabsSelectedTitleAlignment,
    tabs_selected_metadata_alignment: TabsSelectedMetadataAlignment,
    tabs_unselected_title_alignment: TabsUnselectedTitleAlignment,
    tabs_unselected_metadata_alignment: TabsUnselectedMetadataAlignment
]);
