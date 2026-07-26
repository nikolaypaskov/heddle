use anyhow::Result;
use objc2_foundation::NSBundle;

/// Apple Developer Team ID that signs Heddle, used for code signing and signature validation.
///
/// This was "2BBY89MBSN" -- Warp's team. It is what `autoupdate::verify_code_signature` checks a
/// staged update against, so the updater would have ACCEPTED an update signed by Warp and
/// REJECTED one signed by Heddle: exactly backwards for a fork. Autoupdate is inert in this build
/// (`autoupdate_config: None`, and the feature is not in the default set), so the trust
/// relationship was dormant rather than exploited, but a dormant one still has to go.
pub const APPLE_TEAM_ID: &str = "4STAAHTNCN";

/// Warp's team ID, kept ONLY to locate state written by an earlier build.
///
/// Upstream nested state inside the app group container `2BBY89MBSN.dev.warp`. Heddle no longer
/// requests that entitlement and no longer writes there, but data already in it has to be found in
/// order to be moved out. See `paths::migrate_legacy_app_group_state`. Nothing else may use this.
pub const LEGACY_WARP_APP_GROUP_TEAM_ID: &str = "2BBY89MBSN";

/// Get the path to the macOS `.app` bundle.
pub fn get_bundle_path() -> Result<String> {
    let bundle = NSBundle::mainBundle();
    let path = bundle.bundlePath();
    Ok(path.to_string())
}
