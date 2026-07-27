use settings::macros::define_settings_group;
use settings::{RespectUserSyncSetting, SupportedPlatforms, SyncToCloud};

use super::DriveSortOrder;

pub const HAS_AUTO_OPENED_WELCOME_FOLDER: &str = "HasAutoOpenedWelcomeFolder";

define_settings_group!(WarpDriveSettings, settings: [
    sorting_choice: WarpDriveSortingChoice {
        type: DriveSortOrder,
        default: DriveSortOrder::ByObjectType,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "warp_drive.sorting_choice",
        description: "The sort order for items in Drive.",
    },
    sharing_onboarding_block_shown: WarpDriveSharingOnboardingBlockShown {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        surface: settings::SettingSurfaces::GUI,
        private: true,
    },
    // Controls whether Warp Drive appears in the tools panel, command palette, and command search.
    enable_warp_drive: EnableWarpDrive {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "warp_drive.enabled",
        description: "Whether Drive is enabled.",
    },
]);

impl WarpDriveSettings {
    /// Returns whether Drive should be considered enabled.
    ///
    /// This is the user's setting and nothing else.
    ///
    /// It used to be `enable_warp_drive && !is_anonymous_or_logged_out`, where the second
    /// term was meant to hide Drive from users who had not signed up. This build has no
    /// accounts, `SkipFirebaseAnonymousUser` is on by default, and startup never
    /// authenticates, so `is_anonymous_or_logged_out()` was permanently true and the whole
    /// expression permanently false. Drive was therefore absent from the tools panel, the
    /// command palette, command search, the `@` context menu and the block list -- taking
    /// the user's own locally-stored workflows, notebooks, prompts and env-var collections
    /// with it. Nothing here is remote; `CloudModel` is the local sqlite store.
    ///
    /// This is the same defect class as the account gates removed elsewhere in this fork,
    /// and it is the one that hid the most. It was noticed three times and worked around
    /// each time -- the Drive app menu was deleted as "already inert", the onboarding chip
    /// was dropped as pointing at a panel that "refuses to show", and the tools-panel test
    /// disabled `SkipFirebaseAnonymousUser` so it could reach the setting at all -- which is
    /// why it survived. A constant predicate is a bug to fix, not a fact to design around.
    pub fn is_warp_drive_enabled(app: &warpui::AppContext) -> bool {
        use warpui::SingletonEntity as _;
        *Self::as_ref(app).enable_warp_drive
    }
}
