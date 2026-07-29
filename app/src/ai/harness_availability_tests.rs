//! Tests for the harness catalog.
//!
//! These deliberately exercise the *real* catalog function used by
//! `HarnessAvailabilityModel::new`, on the only state this build ever has:
//! no account, no server. The bug these guard against was a catalog that
//! collapsed to a single "Warp" entry because the populating call sat behind
//! an `is_logged_in()` check that is permanently false here.

use warp_cli::agent::Harness;

use super::local_harness_catalog;

fn catalog_harnesses() -> Vec<Harness> {
    local_harness_catalog()
        .into_iter()
        .map(|entry| entry.harness)
        .collect()
}

#[test]
fn catalog_offers_the_local_child_harnesses_without_an_account() {
    // No auth state is constructed, mocked, or signed in anywhere in this
    // test — if the catalog needed one it could not produce these entries.
    let harnesses = catalog_harnesses();

    for expected in [
        Harness::Oz,
        Harness::Claude,
        Harness::OpenCode,
        Harness::Codex,
    ] {
        assert!(
            harnesses.contains(&expected),
            "{expected:?} missing from the local harness catalog: {harnesses:?}"
        );
    }
}

#[test]
fn catalog_is_more_than_the_built_in_agent() {
    // `should_show_harness_selector` requires >1 entry, and every picker
    // renders whatever the catalog holds. A single-entry catalog is the
    // shipped bug, so assert the shape directly rather than the predicate.
    let harnesses = catalog_harnesses();

    assert!(
        harnesses.len() > 1,
        "a one-entry catalog hides the harness selector entirely: {harnesses:?}"
    );
}

#[test]
fn catalog_excludes_gemini() {
    // Gemini is `unreachable!()` in the local child launch path and hangs
    // orchestration. It must never reach a picker.
    assert!(!catalog_harnesses().contains(&Harness::Gemini));
}

#[test]
fn catalog_excludes_unknown() {
    assert!(!catalog_harnesses().contains(&Harness::Unknown));
}

#[test]
fn catalog_entries_use_the_shared_display_names() {
    for entry in local_harness_catalog() {
        assert_eq!(
            entry.display_name,
            crate::ai::harness_display::display_name(entry.harness),
            "catalog label drifted from the shared harness display metadata"
        );
    }
}

#[test]
fn claude_offers_local_model_choices() {
    // A model chosen here becomes `ANTHROPIC_MODEL` for the Claude subprocess
    // (`harness_model_env_vars`), so an empty list is not a cosmetic gap: it
    // removes the only local model control the user has over a local child.
    let claude = local_harness_catalog()
        .into_iter()
        .find(|entry| entry.harness == Harness::Claude)
        .expect("Claude missing from the catalog");

    assert!(
        !claude.available_models.is_empty(),
        "an empty list collapses the model picker to a single inert row"
    );
    let ids: Vec<&str> = claude
        .available_models
        .iter()
        .map(|model| model.id.as_str())
        .collect();
    assert!(ids.contains(&"opus"), "{ids:?}");
    assert!(ids.contains(&"sonnet"), "{ids:?}");
}

#[test]
fn claude_model_ids_are_aliases_not_dated_versions() {
    // Dated ids rot: the CLI resolves the alias itself, so this list stays
    // correct as new versions ship. A pinned id would eventually name a model
    // the user's own CLI refuses.
    for model in local_harness_catalog()
        .into_iter()
        .filter(|entry| entry.harness == Harness::Claude)
        .flat_map(|entry| entry.available_models)
    {
        assert!(
            !model.id.contains(char::is_numeric),
            "{} looks like a pinned version, not an alias",
            model.id
        );
        assert!(!model.display_name.is_empty());
    }
}

#[test]
fn harnesses_that_cannot_apply_a_model_offer_none() {
    // `harness_model_env_vars` matches on Claude alone. Offering a choice for
    // any other harness would present a control that is silently discarded.
    for entry in local_harness_catalog()
        .into_iter()
        .filter(|entry| entry.harness != Harness::Claude)
    {
        assert!(
            entry.available_models.is_empty(),
            "{:?} offers models it cannot pass to the child process",
            entry.harness
        );
    }
}

// ── The production singleton, not a hand-rolled stand-in ─────────────
//
// Everything above calls `local_harness_catalog` directly, and the picker tests
// in `orchestration/snapshots_tests.rs` call the pure builder by hand. Both
// would still pass if `HarnessAvailabilityModel::new` went back to returning a
// single Oz entry, or re-acquired an account gate — which is precisely the bug
// that shipped. These tests build the real singleton with the real constructor
// and read it back through the same `as_ref(ctx)` the GUI uses.

use std::ffi::OsString;
use std::fs;
use std::path::Path;

use tempfile::TempDir;
use warpui::{App, AppContext, SingletonEntity};

use super::HarnessAvailabilityModel;
use crate::ai::orchestration::{OrchestrationConfigState, harness_snapshot, model_snapshot};
use crate::auth::AuthStateProvider;
use crate::auth::auth_manager::AuthManager;
use crate::server::server_api::ServerApiProvider;
use ai::agent::action::RunAgentsExecutionMode;

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
    fs::write(
        &executable_path,
        if cfg!(windows) {
            "@echo off\r\n"
        } else {
            "#!/bin/sh\n"
        },
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&executable_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable_path, permissions).unwrap();
    }
}

/// Registers the real singletons in the state every user of this build is
/// permanently in: a fully logged-out `AuthState` — no user, no credentials —
/// and the real `HarnessAvailabilityModel::new` on top of it.
///
/// `new_logged_out_for_test` rather than `new_for_test`: the latter carries a
/// user, and a suite built on it is exactly how an account-gated capability
/// passes its own tests while being dead for everyone. `assert_logged_out`
/// below makes the premise an assertion rather than a comment.
fn register_logged_out_singletons(app: &mut App) {
    app.add_singleton_model(|_| ServerApiProvider::new_for_test());
    app.add_singleton_model(|_| AuthStateProvider::new_logged_out_for_test());
    app.add_singleton_model(AuthManager::new_for_test);
    app.add_singleton_model(HarnessAvailabilityModel::new);
}

fn assert_logged_out(ctx: &AppContext) {
    assert!(
        !AuthStateProvider::as_ref(ctx).get().is_logged_in(),
        "this test only means something against a logged-out auth state"
    );
}

fn local_state(harness_type: &str, model_id: &str) -> OrchestrationConfigState {
    OrchestrationConfigState::from_run_agents_fields(
        Some(model_id),
        Some(harness_type),
        &RunAgentsExecutionMode::Local,
    )
}

#[test]
#[serial_test::serial]
fn production_singleton_offers_more_than_the_built_in_agent() {
    // The singleton is built by its real constructor, over a real logged-out
    // auth state. If the catalog were account-gated again, or back to the
    // one-entry pre-fetch fallback, this fails.
    App::test((), |mut app| async move {
        register_logged_out_singletons(&mut app);

        app.update(|ctx| {
            assert_logged_out(ctx);

            let harnesses: Vec<Harness> = HarnessAvailabilityModel::as_ref(ctx)
                .available_harnesses()
                .iter()
                .map(|entry| entry.harness)
                .collect();

            assert!(
                harnesses.contains(&Harness::Claude),
                "the singleton does not offer Claude: {harnesses:?}"
            );
            assert!(harnesses.len() > 1, "single-entry catalog: {harnesses:?}");
            assert!(!harnesses.contains(&Harness::Gemini));
        });
    });
}

#[test]
#[serial_test::serial]
fn production_harness_picker_offers_an_installed_local_harness() {
    // `populate_harness_picker` in the GUI is exactly this call. Reaching it
    // through the singleton is what makes this a production-path test.
    let bin_dir = TempDir::new().unwrap();
    write_fake_cli(bin_dir.path(), "claude");
    let _path = PathVarGuard::set(bin_dir.path());

    App::test((), |mut app| async move {
        register_logged_out_singletons(&mut app);

        app.update(|ctx| {
            assert_logged_out(ctx);

            let snapshot = harness_snapshot(&local_state("oz", ""), ctx);
            let claude = snapshot
                .rows
                .iter()
                .find(|row| row.id == "claude")
                .expect("Claude missing from the production harness picker");

            assert_eq!(
                claude.disabled_reason, None,
                "Claude is installed but the picker offered it disabled"
            );
        });
    });
}

#[test]
#[serial_test::serial]
fn production_model_picker_offers_claude_models() {
    // The other half of the same hard constraint. `model_snapshot` is what the
    // GUI's `populate_model_picker_for_harness` calls; a catalog with no models
    // collapses it to a single inert "Default model" row.
    App::test((), |mut app| async move {
        register_logged_out_singletons(&mut app);

        app.update(|ctx| {
            assert_logged_out(ctx);

            let snapshot = model_snapshot(&local_state("claude", ""), ctx);
            let ids: Vec<&str> = snapshot.rows.iter().map(|row| row.id.as_str()).collect();

            assert!(
                ids.len() > 1,
                "only the default row is offered, so no model can be chosen: {ids:?}"
            );
            assert!(ids.contains(&"opus"), "{ids:?}");
            assert!(ids.contains(&"sonnet"), "{ids:?}");
            // "Default model" (empty id) must stay: it is how a user opts out
            // of sending ANTHROPIC_MODEL at all.
            assert!(ids.contains(&""), "{ids:?}");
        });
    });
}

#[test]
#[serial_test::serial]
fn production_model_picker_keeps_a_chosen_claude_model_selected() {
    // A row that exists but can never be the selection is no better than no row.
    App::test((), |mut app| async move {
        register_logged_out_singletons(&mut app);

        app.update(|ctx| {
            assert_logged_out(ctx);

            let snapshot = model_snapshot(&local_state("claude", "sonnet"), ctx);
            assert_eq!(snapshot.selected_id.as_deref(), Some("sonnet"));
        });
    });
}
