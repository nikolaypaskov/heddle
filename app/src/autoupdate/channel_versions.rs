use std::env;
use std::fs::read_to_string;

use anyhow::{Context as _, Result};
use channel_versions::ChannelVersions;

use crate::server::server_api::FETCH_CHANNEL_VERSIONS_TIMEOUT;
use crate::settings::UpdateConsent;

/// Where Heddle publishes its release manifest.
///
/// GitHub resolves `releases/latest/download/<asset>` to the asset on the current release,
/// so publishing one extra file per release is the entire infrastructure: no server, no
/// hostname to operate, no secret to hold.
const MANIFEST_URL: &str =
    "https://github.com/nikolaypaskov/heddle/releases/latest/download/channel_versions.json";

/// Points the fetch at a local file instead of GitHub. For tests, and for pointing a build
/// at a staging manifest.
const LOCAL_MANIFEST_PATH_VAR: &str = "WARP_CHANNEL_VERSIONS_PATH";

/// Fetch the release manifest, if the user has agreed to that.
///
/// `Ok(None)` means no check was permitted. That is not a failure, and the caller must not
/// treat it as one.
///
/// This replaces a fetch that went through Warp's `ServerApi` and fell back to Warp's GCP
/// release storage. Both are gone with the rest of the backend, and both bailed before
/// constructing a request once `server_root_url()` and `releases_base_url()` became `None`
/// -- which is why no Heddle build has ever made an update request.
///
/// The consent check is deliberately the FIRST thing here, ahead of the local-path override.
/// The plan had it the other way round so that tests could exercise parsing without faking
/// consent, but that ordering lets an environment variable produce a manifest -- and
/// therefore an update offer -- for a user who was never asked. Consent gates the whole
/// operation, not just the network hop; tests pass `Enabled` explicitly instead.
pub async fn fetch_channel_versions(consent: UpdateConsent) -> Result<Option<ChannelVersions>> {
    if !consent.should_check() {
        return Ok(None);
    }

    if let Ok(path) = env::var(LOCAL_MANIFEST_PATH_VAR) {
        let path = shellexpand::tilde(&path);
        let raw = read_to_string::<&str>(&path)
            .with_context(|| format!("Failed to read the manifest at {path}"))?;
        return Ok(Some(
            serde_json::from_str(&raw).context("Failed to parse channel versions JSON")?,
        ));
    }

    let response = http_client::Client::new()
        .get(MANIFEST_URL)
        .timeout(FETCH_CHANNEL_VERSIONS_TIMEOUT)
        .send()
        .await
        .context("Failed to fetch the release manifest")?;

    let body = response
        .error_for_status()
        .context("Release manifest request returned an error status")?
        .text()
        .await
        .context("Failed to read the release manifest body")?;

    Ok(Some(
        serde_json::from_str(&body).context("Failed to parse channel versions JSON")?,
    ))
}

#[cfg(test)]
#[path = "channel_versions_tests.rs"]
mod tests;
