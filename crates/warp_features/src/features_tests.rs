use super::*;

#[test]
#[ignore = "CORE-3768 - need to clean up PREVIEW_FLAGS, but this is a temporary fix for the cluttered changelog"]
fn test_all_preview_flags_have_a_description() {
    for flag in PREVIEW_FLAGS {
        assert!(
            flag.flag_description()
                .is_some_and(|description| !description.is_empty()),
            "Missing description for preview-enabled flag {flag:?}"
        );
    }
}

#[test]
fn local_child_harnesses_ship_in_release_builds() {
    // Upstream had this in LOCAL_FLAGS only, so Codex was filtered out of the
    // local harness picker in every real build while Claude Code and OpenCode
    // were offered. Heddle ships all three or none.
    //
    // This asserts the DATA, not the behaviour: no test can observe the effect,
    // because RELEASE_FLAGS is only applied when ChannelState::is_release_bundle()
    // is true and it never is under `cargo test`. The chain it completes is
    // RELEASE_FLAGS contains the flag (here) + a release bundle extends
    // RELEASE_FLAGS (app/src/features.rs) + the flag being enabled permits Codex
    // (prepare_local_codex_child_launch_succeeds_when_testing_flag_is_enabled).
    // Each link is tested; the join is not. Saying so beats implying otherwise.
    assert!(RELEASE_FLAGS.contains(&FeatureFlag::LocalClaudeCodexChildHarnesses));
    // Still in LOCAL_FLAGS: the developer build is not a release bundle, so it
    // would otherwise lose the harness the release just gained.
    assert!(LOCAL_FLAGS.contains(&FeatureFlag::LocalClaudeCodexChildHarnesses));
    assert!(!DEBUG_FLAGS.contains(&FeatureFlag::LocalClaudeCodexChildHarnesses));
    assert!(!DOGFOOD_FLAGS.contains(&FeatureFlag::LocalClaudeCodexChildHarnesses));
}
