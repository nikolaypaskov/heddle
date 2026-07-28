//! Whether the user has agreed to Heddle checking GitHub for new releases.
//!
//! Three states, not two. "Not asked yet" and "said no" both mean *no network request*, but
//! only the first should ever show the prompt. A boolean cannot tell them apart, and
//! collapsing them is exactly how a declined setting turns into a prompt that reappears on
//! every launch.
//!
//! Heddle's README says nothing leaves the device, and today that is literally true: the
//! autoupdate poll loop runs but every fetch path bails before constructing a request,
//! because `server_root_url()` and `releases_base_url()` are both `None`. This setting is
//! what keeps that claim honest once a real fetch exists -- the request is not made invisible,
//! it is made *chosen*.

use serde::{Deserialize, Serialize};
use settings::macros::define_settings_group;
use settings::{SupportedPlatforms, SyncToCloud};

/// Whether Heddle may check for new releases.
///
/// Persisted. An unreadable or unknown stored value decodes to `None` and the settings
/// loader falls back to `Default`, which is [`UpdateConsent::Unanswered`] -- the user is
/// asked again rather than silently opted in. That fallback direction is the important part:
/// it must never be possible for a corrupt settings file to become consent.
#[derive(
    Default,
    PartialEq,
    Eq,
    Hash,
    Clone,
    Copy,
    Debug,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
// The `SettingsValue` derive reads `#[serde(rename_all)]`, NOT `#[schemars(rename_all)]`.
// Both are set so the persisted wire format and the generated JSON schema agree; changing
// only one of them would silently desynchronise the file format from its own schema.
#[serde(rename_all = "snake_case")]
#[schemars(
    description = "Whether Heddle may check GitHub for new releases.",
    rename_all = "snake_case"
)]
pub enum UpdateConsent {
    /// Never asked. Show the prompt once; make no network request until answered.
    #[default]
    Unanswered,
    /// The user agreed. Check on launch.
    Enabled,
    /// The user declined. Never check, and never ask again.
    Disabled,
}

impl UpdateConsent {
    /// The single place that decides whether an update network request is permitted.
    ///
    /// Everything that could reach the network must route through this. Keeping it as one
    /// method rather than a scattering of `== Enabled` comparisons means a new call site
    /// cannot accidentally treat `Unanswered` as permission.
    pub fn should_check(&self) -> bool {
        matches!(self, Self::Enabled)
    }

    /// Whether the one-time prompt should be shown.
    pub fn needs_prompt(&self) -> bool {
        matches!(self, Self::Unanswered)
    }
}

define_settings_group!(UpdateSettings, settings: [
    // `SyncToCloud::Never`: there is no cloud in this build, and a consent decision about
    // network access is the last thing that should travel over the network.
    check_for_updates: CheckForUpdates {
        type: UpdateConsent,
        default: UpdateConsent::Unanswered,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "updates.check_for_updates",
        description: "Whether Heddle may check GitHub for new releases. Defaults to unanswered; \
                      no request is made until you choose.",
    },
]);

#[cfg(test)]
#[path = "update_consent_tests.rs"]
mod tests;
