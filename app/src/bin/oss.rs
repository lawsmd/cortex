// On Windows, we don't want to display a console window when the application is running in release
// builds. See https://doc.rust-lang.org/reference/runtime.html#the-windows_subsystem-attribute.
#![cfg_attr(feature = "release_bundle", windows_subsystem = "windows")]

use anyhow::Result;
use warp_core::{
    channel::{Channel, ChannelConfig, ChannelState, OzConfig, WarpServerConfig},
    AppId,
};

// Simple wrapper around warp::run() for Warp OSS builds.
fn main() -> Result<()> {
    // macOS-only: distinct AppId for debug builds so prod + dev coexist
    // without sharing the macOS Keychain entry (which would prompt on every
    // dev launch as the bundle identity changed). The logfile name forks for
    // the same reason — keeps prod's `warp-oss.log` and dev's
    // `warp-oss-dev.log` independent on macOS.
    //
    // Cross-platform data isolation between prod and dev (separate
    // `~/.warp-oss[-dev]/`, separate `WarpOss[-dev]` AppData paths, separate
    // SQLite, etc.) is handled *separately* by the `WARP_DATA_PROFILE=dev`
    // env var that both `scripts/launch-cortex-dev.sh` (macOS) and
    // `scripts/launch-cortex-dev.bat` (Windows) export. That env var is
    // honored only in debug builds and forks the home-dir + ProjectDirs
    // paths automatically — see `crates/warp_core/src/paths.rs` and
    // `crates/warp_core/src/channel/state.rs::data_profile`. Linux's
    // `script/run` will pick up the same mechanism if a launcher ever
    // exports `WARP_DATA_PROFILE=dev` there.
    let (app_name, logfile_name) = if cfg!(all(target_os = "macos", debug_assertions)) {
        ("WarpOssDev", "warp-oss-dev.log")
    } else {
        ("WarpOss", "warp-oss.log")
    };

    let mut state = ChannelState::new(
        Channel::Oss,
        ChannelConfig {
            app_id: AppId::new("dev", "warp", app_name),
            logfile_name: logfile_name.into(),
            server_config: WarpServerConfig::production(),
            oz_config: OzConfig::production(),
            telemetry_config: None,
            crash_reporting_config: None,
            autoupdate_config: None,
            mcp_static_config: None,
        },
    );
    if cfg!(debug_assertions) {
        state = state.with_additional_features(warp_core::features::DEBUG_FLAGS);
    }
    ChannelState::set(state);

    warp::run()
}

// If we're not using an external plist, embed the following as the Info.plist.
#[cfg(all(not(feature = "extern_plist"), target_os = "macos"))]
embed_plist::embed_info_plist_bytes!(r#"
    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple Computer//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0">
    <dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleDisplayName</key>
    <string>WarpOss</string>
    <key>CFBundleExecutable</key>
    <string>warp-oss</string>
    <key>CFBundleIdentifier</key>
    <string>dev.warp.WarpOss</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>WarpOss</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>UIDesignRequiresCompatibility</key>
    <true/>
    <key>CFBundleURLTypes</key>
    <array><dict><key>CFBundleURLName</key><string>Custom App</string><key>CFBundleURLSchemes</key><array><string>warposs</string></array></dict></array>
    <key>NSHumanReadableCopyright</key>
    <string>© 2026, Denver Technologies, Inc</string>
    </dict>
    </plist>
"#.as_bytes());
