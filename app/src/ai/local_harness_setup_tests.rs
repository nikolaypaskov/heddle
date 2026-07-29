use super::*;
use crate::features::FeatureFlag;

#[test]
fn claude_is_product_enabled_when_cli_is_installed() {
    assert_eq!(
        local_harness_setup_state_with_cli_resolver(Harness::Claude, |_| true),
        LocalHarnessSetupState::Ready
    );
}

#[test]
fn claude_is_disabled_for_missing_cli() {
    assert_eq!(
        local_harness_setup_state_with_cli_resolver(Harness::Claude, |_| false),
        LocalHarnessSetupState::MissingHarness {
            tooltip: LOCAL_HARNESS_INSTALLATION_REQUIRED_TOOLTIP,
        }
    );
}

#[test]
fn codex_is_enabled_when_flag_is_on() {
    let _local_codex = FeatureFlag::LocalClaudeCodexChildHarnesses.override_enabled(true);

    assert_eq!(
        local_harness_setup_state_with_cli_resolver(Harness::Codex, |_| true),
        LocalHarnessSetupState::Ready
    );
}

#[test]
fn codex_requires_cli_when_flag_is_on() {
    let _local_codex = FeatureFlag::LocalClaudeCodexChildHarnesses.override_enabled(true);

    assert_eq!(
        local_harness_setup_state_with_cli_resolver(Harness::Codex, |_| false),
        LocalHarnessSetupState::MissingHarness {
            tooltip: LOCAL_CODEX_HARNESS_INSTALLATION_REQUIRED_TOOLTIP,
        }
    );
}

#[test]
fn codex_remains_product_disabled() {
    assert_eq!(
        local_harness_setup_state_with_cli_resolver(Harness::Codex, |_| true),
        LocalHarnessSetupState::ProductDisabled {
            message: LOCAL_CODEX_HARNESS_DISABLED_MESSAGE,
        }
    );
}

#[test]
fn opencode_is_ready_when_cli_is_installed() {
    assert_eq!(
        local_harness_setup_state_with_cli_resolver(Harness::OpenCode, |_| true),
        LocalHarnessSetupState::Ready
    );
}

#[test]
fn opencode_is_disabled_for_missing_cli() {
    // The launch path already refuses to start OpenCode without its CLI;
    // before this the picker still offered it as selectable.
    assert_eq!(
        local_harness_setup_state_with_cli_resolver(Harness::OpenCode, |_| false),
        LocalHarnessSetupState::MissingHarness {
            tooltip: LOCAL_OPENCODE_HARNESS_INSTALLATION_REQUIRED_TOOLTIP,
        }
    );
}

#[test]
fn cli_probes_are_per_harness() {
    // A resolver that only knows about `claude` must not make `opencode` look
    // installed — the harness-to-command mapping has to be exact.
    let only_claude = |command: &str| command == "claude";

    assert_eq!(
        local_harness_setup_state_with_cli_resolver(Harness::Claude, only_claude),
        LocalHarnessSetupState::Ready
    );
    assert_eq!(
        local_harness_setup_state_with_cli_resolver(Harness::OpenCode, only_claude),
        LocalHarnessSetupState::MissingHarness {
            tooltip: LOCAL_OPENCODE_HARNESS_INSTALLATION_REQUIRED_TOOLTIP,
        }
    );
}
