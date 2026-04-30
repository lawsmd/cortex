use settings::{
    macros::define_settings_group, RespectUserSyncSetting, SupportedPlatforms, SyncToCloud,
};

define_settings_group!(CortexSettings, settings: [
    hide_pane_separators: HidePaneSeparators {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "cortex.appearance.hide_pane_separators",
        description: "Whether the thin separator lines between panels and around input boxes are hidden.",
    }
]);
