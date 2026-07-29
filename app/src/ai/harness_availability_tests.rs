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
