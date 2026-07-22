// On Windows, we don't want to display a console window when the application is running in release
// builds. See https://doc.rust-lang.org/reference/runtime.html#the-windows_subsystem-attribute.
#![cfg_attr(feature = "release_bundle", windows_subsystem = "windows")]

use anyhow::Result;
use warp_core::AppId;
use warp_core::channel::{Channel, ChannelConfig, ChannelState};

// Simple wrapper around warp::run() for Warp OSS builds.
fn main() -> Result<()> {
    let mut state = ChannelState::new(
        Channel::Oss,
        ChannelConfig {
            // Heddle identity. AGPL grants the code, not the "Warp" trademark,
            // so the fork must not present itself as Warp. This also gives
            // Heddle its own data directory rather than sharing Warp's.
            app_id: AppId::new("dev", "heddle", "Heddle"),
            logfile_name: "heddle.log".into(),
            // Heddle: no Warp server, no Oz. These endpoints are absent from
            // the binary entirely -- not disabled by a flag that could be
            // flipped, and not reachable by any server-pushed configuration.
            server_config: None,
            oz_config: None,
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
    <string>Heddle</string>
    <key>CFBundleExecutable</key>
    <string>heddle</string>
    <key>CFBundleIdentifier</key>
    <string>dev.heddle.Heddle</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>Heddle</string>
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
    <array><dict><key>CFBundleURLName</key><string>Custom App</string><key>CFBundleURLSchemes</key><array><string>heddle</string></array></dict></array>
    <key>NSHumanReadableCopyright</key>
    <string>© 2026 Denver Technologies, Inc. Modified work © 2026 Heddle contributors. Licensed under AGPL-3.0.</string>
    </dict>
    </plist>
"#.as_bytes());
