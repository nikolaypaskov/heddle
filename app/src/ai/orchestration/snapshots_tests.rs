use warp_cli::agent::Harness;

use super::{
    AUTH_SECRET_INHERIT_LABEL, AuthSecretNamesInput, DEFAULT_MODEL_LABEL, HarnessEntryInput,
    ModelChoiceInput, OptionBadge, OptionFooter, OptionSourceStatus, build_api_key_snapshot,
    build_environment_snapshot, build_harness_snapshot, build_host_snapshot,
    build_non_oz_model_snapshot, build_oz_model_snapshot, build_runner_snapshot,
};
use crate::ai::local_harness_setup::LocalHarnessSetupState;
use crate::ai::orchestration::config_state::AuthSecretSelection;

fn entry(harness: Harness, display_name: &str, enabled: bool) -> HarnessEntryInput {
    HarnessEntryInput {
        harness,
        display_name: display_name.to_string(),
        enabled,
    }
}

fn all_ready(_harness: Harness) -> LocalHarnessSetupState {
    LocalHarnessSetupState::Ready
}

// ── Harness ─────────────────────────────────────────────────────────

#[test]
fn harness_snapshot_excludes_gemini_and_selects_initial() {
    let entries = vec![
        entry(Harness::Oz, "Warp", true),
        entry(Harness::Claude, "Claude Code", true),
        entry(Harness::Gemini, "Gemini", true),
    ];

    let snapshot = build_harness_snapshot(entries, "claude", None, false, &all_ready);

    let ids: Vec<&str> = snapshot.rows.iter().map(|r| r.id.as_str()).collect();
    assert!(!ids.contains(&"gemini"));
    assert_eq!(snapshot.selected_id.as_deref(), Some("claude"));
    assert_eq!(snapshot.status, OptionSourceStatus::Ready);
    assert!(snapshot.rows.iter().all(|r| r.harness.is_some()));
}

#[test]
fn harness_snapshot_filters_product_disabled_local_harness() {
    let entries = vec![
        entry(Harness::Oz, "Warp", true),
        entry(Harness::Codex, "Codex", true),
    ];

    // Local Codex is product-disabled (feature flag off in tests).
    let snapshot = build_harness_snapshot(entries, "oz", None, true, &all_ready);

    let ids: Vec<&str> = snapshot.rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["oz"]);
}

#[test]
fn harness_snapshot_keeps_cloud_opencode_selectable() {
    let entries = vec![
        entry(Harness::Oz, "Warp", true),
        entry(Harness::OpenCode, "OpenCode", true),
    ];

    let snapshot = build_harness_snapshot(entries, "oz", None, false, &all_ready);

    let opencode = snapshot
        .rows
        .iter()
        .find(|r| r.id == "opencode")
        .expect("OpenCode row present on Cloud");
    // The harness list doesn't disable OpenCode; the accept gate does.
    assert_eq!(opencode.disabled_reason, None);
}

#[test]
fn harness_snapshot_marks_missing_local_cli_disabled_and_sorts_last() {
    let entries = vec![
        entry(Harness::Claude, "Claude Code", true),
        entry(Harness::Oz, "Warp", true),
    ];
    let setup = |harness: Harness| match harness {
        Harness::Claude => LocalHarnessSetupState::MissingHarness {
            tooltip: "Install Claude Code to use this local harness.",
        },
        Harness::Oz | Harness::OpenCode | Harness::Gemini | Harness::Codex | Harness::Unknown => {
            LocalHarnessSetupState::Ready
        }
    };

    let snapshot = build_harness_snapshot(entries, "oz", None, true, &setup);

    let ids: Vec<&str> = snapshot.rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["oz", "claude"]);
    assert_eq!(
        snapshot.rows[1].disabled_reason.as_deref(),
        Some("Install Claude Code to use this local harness.")
    );
}

#[test]
fn harness_snapshot_marks_server_disabled_entries() {
    let entries = vec![
        entry(Harness::Oz, "Warp", true),
        entry(Harness::Claude, "Claude Code", false),
    ];

    let snapshot = build_harness_snapshot(entries, "oz", None, false, &all_ready);

    assert_eq!(
        snapshot.rows[1].disabled_reason.as_deref(),
        Some("Disabled by your administrator")
    );
}

#[test]
fn harness_snapshot_matches_selection_by_display_name_for_stale_cache() {
    // Stale cache: harness deserialized as Unknown but display_name intact.
    let entries = vec![entry(Harness::Unknown, "Claude Code", true)];

    let snapshot = build_harness_snapshot(
        entries,
        "claude",
        Some("Claude Code".to_string()),
        false,
        &all_ready,
    );

    assert_eq!(snapshot.selected_id.as_deref(), Some("claude"));
}

// ── Model ───────────────────────────────────────────────────────────

fn model(id: &str, label: &str) -> ModelChoiceInput {
    ModelChoiceInput {
        id: id.to_string(),
        label: label.to_string(),
        disabled_reason: None,
    }
}

#[test]
fn oz_model_snapshot_empty_catalog_reports_empty_status() {
    let snapshot = build_oz_model_snapshot(Vec::new(), "auto");
    assert!(matches!(snapshot.status, OptionSourceStatus::Empty { .. }));
}
/// Disabled model metadata remains available to every snapshot consumer.
#[test]
fn oz_model_snapshot_carries_disabled_reason() {
    let mut disabled_model = model("unavailable", "Unavailable");
    disabled_model.disabled_reason = Some("This model is unavailable.".to_string());

    let snapshot = build_oz_model_snapshot(vec![disabled_model], "");

    assert_eq!(
        snapshot.rows[0].disabled_reason.as_deref(),
        Some("This model is unavailable.")
    );
}

#[test]
fn non_oz_model_snapshot_puts_default_first_and_selects_server_model() {
    let snapshot = build_non_oz_model_snapshot(
        Some(vec![model("opus", "Opus"), model("sonnet", "Sonnet")]),
        "sonnet",
    );

    assert_eq!(snapshot.rows[0].label, DEFAULT_MODEL_LABEL);
    assert_eq!(snapshot.rows[0].id, "");
    assert_eq!(snapshot.selected_id.as_deref(), Some("sonnet"));
}

#[test]
fn non_oz_model_snapshot_falls_back_to_default_for_unknown_or_empty_id() {
    for initial in ["", "gone"] {
        let snapshot = build_non_oz_model_snapshot(Some(vec![model("opus", "Opus")]), initial);
        assert_eq!(snapshot.selected_id.as_deref(), Some(""));
    }
    // No server catalog at all: only the Default model row.
    let snapshot = build_non_oz_model_snapshot(None, "");
    assert_eq!(snapshot.rows.len(), 1);
    assert_eq!(snapshot.selected_id.as_deref(), Some(""));
}

// ── API key ─────────────────────────────────────────────────────────

#[test]
fn api_key_snapshot_lists_skip_then_names() {
    let snapshot = build_api_key_snapshot(
        AuthSecretNamesInput::Loaded(vec!["key-a".to_string(), "key-b".to_string()]),
        &AuthSecretSelection::Named("key-b".to_string()),
        true,
    );

    let labels: Vec<&str> = snapshot.rows.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(labels, vec![AUTH_SECRET_INHERIT_LABEL, "key-a", "key-b"]);
    assert_eq!(snapshot.selected_id.as_deref(), Some("key-b"));
    assert_eq!(snapshot.status, OptionSourceStatus::Ready);
    assert_eq!(snapshot.footer, Some(OptionFooter::CreateNewAuthSecret));
}

#[test]
fn api_key_snapshot_keeps_named_selection_while_loading() {
    let snapshot = build_api_key_snapshot(
        AuthSecretNamesInput::NotLoaded,
        &AuthSecretSelection::Named("my-key".to_string()),
        true,
    );
    assert_eq!(snapshot.selected_id.as_deref(), Some("my-key"));
}

#[test]
fn api_key_snapshot_maps_inherit_and_unset_selection() {
    let inherit = build_api_key_snapshot(
        AuthSecretNamesInput::Loaded(vec![]),
        &AuthSecretSelection::Inherit,
        true,
    );
    assert_eq!(inherit.selected_id.as_deref(), Some(""));

    let unset = build_api_key_snapshot(
        AuthSecretNamesInput::Loaded(vec![]),
        &AuthSecretSelection::Unset,
        true,
    );
    assert_eq!(unset.selected_id, None);
}

// ── Host ────────────────────────────────────────────────────────────

#[test]
fn host_snapshot_orders_default_warp_connected_recent() {
    let snapshot = build_host_snapshot(
        Some("team-default".to_string()),
        Some("recent-host".to_string()),
        vec!["worker-1".to_string()],
        "warp",
    );

    let ids: Vec<&str> = snapshot.rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["team-default", "warp", "worker-1", "recent-host"]);
    assert_eq!(snapshot.rows[0].badge, Some(OptionBadge::Default));
    assert_eq!(snapshot.rows[2].badge, Some(OptionBadge::Connected));
    assert_eq!(snapshot.rows[3].badge, Some(OptionBadge::Recent));
    assert_eq!(snapshot.selected_id.as_deref(), Some("warp"));
    assert!(matches!(
        snapshot.footer,
        Some(OptionFooter::CustomText { .. })
    ));
}

#[test]
fn host_snapshot_dedupes_connected_and_recent_against_known_rows() {
    let snapshot = build_host_snapshot(
        Some("team-default".to_string()),
        Some("team-default".to_string()),
        vec!["warp".to_string(), "team-default".to_string()],
        "team-default",
    );

    let ids: Vec<&str> = snapshot.rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["team-default", "warp"]);
}

// ── Environment ─────────────────────────────────────────────────────

#[test]
fn environment_snapshot_puts_empty_option_first() {
    let snapshot = build_environment_snapshot(
        vec![
            ("env-a".to_string(), "Alpha".to_string()),
            ("env-b".to_string(), "Beta".to_string()),
        ],
        "env-b",
    );

    assert_eq!(snapshot.rows[0].id, "");
    assert_eq!(snapshot.rows[0].label, super::ORCHESTRATION_ENV_NONE_LABEL);
    assert_eq!(snapshot.selected_id.as_deref(), Some("env-b"));
}

// ── Runner ──────────────────────────────────────────────────────

#[test]
fn runner_snapshot_puts_use_default_first_and_selects() {
    let snapshot = build_runner_snapshot(
        vec![
            ("r-a".to_string(), "Alpha".to_string()),
            ("r-b".to_string(), "Beta".to_string()),
        ],
        "r-b",
        false,
    );

    assert_eq!(snapshot.rows[0].id, "");
    assert_eq!(
        snapshot.rows[0].label,
        super::ORCHESTRATION_RUNNER_NONE_LABEL
    );
    assert_eq!(snapshot.selected_id.as_deref(), Some("r-b"));
    assert_eq!(snapshot.status, OptionSourceStatus::Ready);
}

#[test]
fn runner_snapshot_loading_reports_loading_status() {
    let snapshot = build_runner_snapshot(vec![], "", true);
    assert_eq!(snapshot.status, OptionSourceStatus::Loading);
    // Empty selection maps to the "use environment default" row.
    assert_eq!(snapshot.selected_id.as_deref(), Some(""));
}

// ── Harness picker on the real logged-out path ──────────────────────
//
// Everything above feeds `build_harness_snapshot` synthetic entries, which is
// exactly why a permanently-empty catalog shipped unnoticed. The tests below
// drive the *real* catalog through the *real* builder with the *real* PATH
// probe, with only the PATH itself controlled — no auth state exists in this
// build, so this is the state every user is permanently in.

use std::ffi::OsString;
use std::fs;
use std::path::Path;

use tempfile::TempDir;
use warp_core::features::FeatureFlag;

use super::{OptionRow, OptionSnapshot, harness_entry_inputs, local_harness_setup_state};
use crate::ai::harness_availability::local_harness_catalog;
use crate::ai::local_harness_setup::{
    LOCAL_CODEX_HARNESS_DISABLED_MESSAGE, LOCAL_CODEX_HARNESS_INSTALLATION_REQUIRED_TOOLTIP,
    LOCAL_HARNESS_INSTALLATION_REQUIRED_TOOLTIP,
    LOCAL_OPENCODE_HARNESS_INSTALLATION_REQUIRED_TOOLTIP,
};

struct PathVarGuard(Option<OsString>);

impl PathVarGuard {
    fn set(dir: &Path) -> Self {
        let original = std::env::var_os("PATH");
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("PATH", dir.as_os_str()) };
        Self(original)
    }
}

impl Drop for PathVarGuard {
    fn drop(&mut self) {
        // TODO: Audit that the environment access only happens in single-threaded code.
        match self.0.take() {
            Some(original) => unsafe { std::env::set_var("PATH", original) },
            None => unsafe { std::env::remove_var("PATH") },
        }
    }
}

fn write_fake_cli(bin_dir: &Path, name: &str) {
    let executable_name = if cfg!(windows) {
        format!("{name}.cmd")
    } else {
        name.to_string()
    };
    let executable_path = bin_dir.join(executable_name);
    let script = if cfg!(windows) {
        "@echo off\r\n"
    } else {
        "#!/bin/sh\n"
    };
    fs::write(&executable_path, script).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&executable_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable_path, permissions).unwrap();
    }
}

/// Builds the local-mode harness picker exactly as `harness_snapshot` does,
/// minus the `AppContext` lookup of the catalog singleton.
fn local_harness_picker(initial_harness: &str) -> OptionSnapshot {
    build_harness_snapshot(
        harness_entry_inputs(&local_harness_catalog()),
        initial_harness,
        None,
        true,
        &local_harness_setup_state,
    )
}

fn row<'a>(snapshot: &'a OptionSnapshot, id: &str) -> Option<&'a OptionRow> {
    snapshot.rows.iter().find(|row| row.id == id)
}

#[test]
#[serial_test::serial]
fn harness_picker_offers_installed_local_harnesses_with_no_account() {
    let bin_dir = TempDir::new().unwrap();
    write_fake_cli(bin_dir.path(), "claude");
    write_fake_cli(bin_dir.path(), "opencode");
    let _path = PathVarGuard::set(bin_dir.path());

    let snapshot = local_harness_picker("oz");

    assert_eq!(snapshot.status, OptionSourceStatus::Ready);
    for id in ["claude", "opencode"] {
        let row = row(&snapshot, id).unwrap_or_else(|| panic!("{id} missing from the picker"));
        assert_eq!(
            row.disabled_reason, None,
            "{id} is installed but was offered disabled"
        );
    }
    assert!(
        row(&snapshot, "oz").is_some(),
        "the built-in agent vanished"
    );
    assert_eq!(snapshot.selected_id.as_deref(), Some("oz"));
}

#[test]
#[serial_test::serial]
fn harness_picker_selects_a_local_harness_by_id() {
    let bin_dir = TempDir::new().unwrap();
    write_fake_cli(bin_dir.path(), "claude");
    let _path = PathVarGuard::set(bin_dir.path());

    let snapshot = local_harness_picker("claude");

    assert_eq!(snapshot.selected_id.as_deref(), Some("claude"));
}

#[test]
#[serial_test::serial]
fn harness_picker_flags_local_harnesses_whose_cli_is_absent() {
    let empty_dir = TempDir::new().unwrap();
    let _path = PathVarGuard::set(empty_dir.path());

    let snapshot = local_harness_picker("oz");

    assert_eq!(
        row(&snapshot, "claude").unwrap().disabled_reason.as_deref(),
        Some(LOCAL_HARNESS_INSTALLATION_REQUIRED_TOOLTIP)
    );
    assert_eq!(
        row(&snapshot, "opencode")
            .unwrap()
            .disabled_reason
            .as_deref(),
        Some(LOCAL_OPENCODE_HARNESS_INSTALLATION_REQUIRED_TOOLTIP)
    );
}

#[test]
#[serial_test::serial]
fn harness_picker_never_offers_gemini() {
    let bin_dir = TempDir::new().unwrap();
    write_fake_cli(bin_dir.path(), "gemini");
    let _path = PathVarGuard::set(bin_dir.path());

    assert!(
        row(&local_harness_picker("oz"), "gemini").is_none(),
        "Gemini hangs orchestration and is unreachable in the launch path"
    );
}

#[test]
#[serial_test::serial]
fn harness_picker_hides_codex_while_it_is_product_disabled() {
    let bin_dir = TempDir::new().unwrap();
    write_fake_cli(bin_dir.path(), "codex");
    let _path = PathVarGuard::set(bin_dir.path());

    assert!(
        row(&local_harness_picker("oz"), "codex").is_none(),
        "product-disabled harnesses are filtered from the local picker, \
         not shown with `{LOCAL_CODEX_HARNESS_DISABLED_MESSAGE}`"
    );
}

#[test]
#[serial_test::serial]
fn harness_picker_offers_codex_once_its_flag_is_on() {
    let _local_codex = FeatureFlag::LocalClaudeCodexChildHarnesses.override_enabled(true);
    let bin_dir = TempDir::new().unwrap();
    write_fake_cli(bin_dir.path(), "codex");
    let _path = PathVarGuard::set(bin_dir.path());

    assert_eq!(
        row(&local_harness_picker("oz"), "codex")
            .expect("codex missing once its flag is on")
            .disabled_reason,
        None
    );
}

#[test]
#[serial_test::serial]
fn harness_picker_flags_codex_with_its_flag_on_but_no_cli() {
    let _local_codex = FeatureFlag::LocalClaudeCodexChildHarnesses.override_enabled(true);
    let empty_dir = TempDir::new().unwrap();
    let _path = PathVarGuard::set(empty_dir.path());

    assert_eq!(
        row(&local_harness_picker("oz"), "codex")
            .unwrap()
            .disabled_reason
            .as_deref(),
        Some(LOCAL_CODEX_HARNESS_INSTALLATION_REQUIRED_TOOLTIP)
    );
}
