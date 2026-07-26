use anyhow::{Ok, Result};
use vec1::vec1;
use warpui::keymap::Keystroke;

use crate::keyboard::{PersistedTrigger, REMOVED_KEYBINDING_SERIALIZATION, UserDefinedKeybinding};

#[test]
fn test_short_user_defined_keybinding_to_persisted_trigger() {
    let keystroke = Keystroke::parse("ctrl-p").unwrap();
    let keybinding = UserDefinedKeybinding::Keystrokes(vec1![keystroke]);
    let persisted_trigger: PersistedTrigger = keybinding.into();

    assert_eq!(persisted_trigger, PersistedTrigger("ctrl-p".to_string()));
}

#[test]
fn test_long_user_defined_keybinding_to_persisted_trigger() {
    let keystroke = Keystroke::parse("ctrl-p").unwrap();
    let other_keystroke = Keystroke::parse("1").unwrap();

    let keybinding = UserDefinedKeybinding::Keystrokes(vec1![keystroke, other_keystroke]);
    let persisted_trigger: PersistedTrigger = keybinding.into();

    assert_eq!(persisted_trigger, PersistedTrigger("ctrl-p 1".to_string()));
}

#[test]
fn test_short_persisted_trigger_to_user_defined_keybinding() -> Result<()> {
    let persisted_trigger = PersistedTrigger("ctrl-x".to_string());
    let keybinding = UserDefinedKeybinding::try_from(persisted_trigger)?;

    let correct_keybinding =
        UserDefinedKeybinding::Keystrokes(vec1![Keystroke::parse("ctrl-x").unwrap()]);

    assert_eq!(keybinding, correct_keybinding);
    Ok(())
}

#[test]
fn test_long_persisted_trigger_to_user_defined_keybinding() -> Result<()> {
    let persisted_trigger = PersistedTrigger("ctrl-x 8".to_string());
    let keybinding = UserDefinedKeybinding::try_from(persisted_trigger)?;

    let correct_keybinding = UserDefinedKeybinding::Keystrokes(vec1![
        Keystroke::parse("ctrl-x").unwrap(),
        Keystroke::parse("8").unwrap()
    ]);

    assert_eq!(keybinding, correct_keybinding);
    Ok(())
}

#[test]
fn test_persisted_trigger_to_removed_user_keybinding() -> Result<()> {
    let persisted_trigger = PersistedTrigger(REMOVED_KEYBINDING_SERIALIZATION.to_string());
    let keybinding = UserDefinedKeybinding::try_from(persisted_trigger)?;

    assert_eq!(keybinding, UserDefinedKeybinding::Removed);
    Ok(())
}

#[test]
fn test_removed_user_keybinding_to_persisted_trigger() {
    let keybinding = UserDefinedKeybinding::Removed;
    let persisted_trigger: PersistedTrigger = keybinding.into();

    assert_eq!(
        persisted_trigger,
        PersistedTrigger(REMOVED_KEYBINDING_SERIALIZATION.to_string())
    );
}

#[test]
fn test_unparsable_persisted_trigger() {
    let persisted_trigger = PersistedTrigger("".to_string());
    let keybinding = UserDefinedKeybinding::try_from(persisted_trigger);

    assert!(keybinding.is_err());
}

/// Renamed action IDs must keep resolving, because an ID in keybindings.yaml is a persistence
/// contract. The loader silently ignores an ID matching no registered action, so without this
/// map a rename turns every existing user binding for it inert -- and worse, a user who had
/// REMOVED a default binding gets that keystroke back, having deliberately turned it off.
#[test]
fn legacy_action_ids_resolve_to_their_current_names() {
    assert_eq!(
        super::canonical_action_name("terminal:warpify_subshell"),
        "terminal:heddlify_subshell"
    );
    assert_eq!(
        super::canonical_action_name("workspace:show_settings_warpify_page"),
        "workspace:show_settings_heddlify_page"
    );
}

#[test]
fn current_and_unknown_action_ids_pass_through_untouched() {
    // Current names must not be rewritten...
    assert_eq!(
        super::canonical_action_name("terminal:heddlify_subshell"),
        "terminal:heddlify_subshell"
    );
    // ...and an unrecognised name must reach the loader's existing warning path rather than
    // being silently mapped to something else. This exists to rescue renames, not to validate.
    assert_eq!(
        super::canonical_action_name("terminal:not_a_real_action"),
        "terminal:not_a_real_action"
    );
}

/// Canonicalising only in memory is not enough. `remove_custom_keybinding` deletes the current
/// ID, and if a legacy entry survives beside it, the next launch re-applies that entry --
/// silently reversing the reset the user just asked for. This is the case that actually bites,
/// because the legacy entry is usually `none`.
#[test]
fn purging_removes_legacy_spellings_of_the_current_action() {
    let mut map = std::collections::HashMap::new();
    map.insert("terminal:warpify_subshell".to_string(), "none");
    map.insert("editor:unrelated".to_string(), "cmd-x");

    let dropped = super::purge_legacy_aliases(&mut map, "terminal:heddlify_subshell");

    assert_eq!(dropped, 1, "the legacy spelling was found and dropped");
    assert!(
        !map.contains_key("terminal:warpify_subshell"),
        "stale entry cannot outlive the reset"
    );
    assert!(
        map.contains_key("editor:unrelated"),
        "unrelated bindings are untouched"
    );
}

#[test]
fn purging_an_action_with_no_legacy_spelling_changes_nothing() {
    // The counterpart control: if this removed entries indiscriminately, the test above would
    // pass for entirely the wrong reason.
    let mut map = std::collections::HashMap::new();
    map.insert("terminal:warpify_subshell".to_string(), "none");

    let dropped = super::purge_legacy_aliases(&mut map, "editor:some_other_action");

    assert_eq!(dropped, 0, "no aliases claimed for an unrelated action");
    assert!(
        map.contains_key("terminal:warpify_subshell"),
        "another action's legacy entry must not be collateral damage"
    );
}

/// These target the functions the WRITE PATHS actually call, not the purge helper in
/// isolation. The earlier tests called `purge_legacy_aliases` directly, so deleting either
/// production call site left them green -- they proved the helper worked, not that anything
/// used it.
#[test]
fn writing_a_binding_drops_the_stale_spelling_of_that_action() {
    let mut map = std::collections::HashMap::new();
    map.insert("terminal:warpify_subshell".to_string(), "none");

    super::apply_binding_write(
        &mut map,
        "terminal:heddlify_subshell".to_string(),
        "ctrl-x",
    );

    assert_eq!(map.get("terminal:heddlify_subshell"), Some(&"ctrl-x"));
    assert!(
        !map.contains_key("terminal:warpify_subshell"),
        "the stale entry must not survive the write that supersedes it"
    );
}

#[test]
fn removing_a_binding_drops_the_stale_spelling_too() {
    // The case that actually bites: reset a migrated binding, and if the legacy `none` survives
    // it is re-applied on the next launch, silently undoing the reset.
    let mut map = std::collections::HashMap::new();
    map.insert("terminal:warpify_subshell".to_string(), "none");
    map.insert("terminal:heddlify_subshell".to_string(), "ctrl-x");

    super::apply_binding_removal(&mut map, "terminal:heddlify_subshell");

    assert!(map.is_empty(), "both spellings gone, leaving {map:?}");
}

#[test]
fn writes_and_removals_leave_other_actions_alone() {
    // Control for both of the above: if they removed entries indiscriminately, those tests
    // would pass for entirely the wrong reason.
    let mut map = std::collections::HashMap::new();
    map.insert("terminal:warpify_subshell".to_string(), "none");
    map.insert("editor:unrelated".to_string(), "cmd-x");

    super::apply_binding_write(&mut map, "editor:something_else".to_string(), "cmd-y");
    super::apply_binding_removal(&mut map, "editor:unrelated");

    assert!(
        map.contains_key("terminal:warpify_subshell"),
        "an unrelated action's legacy entry is not collateral damage"
    );
    assert_eq!(map.get("editor:something_else"), Some(&"cmd-y"));
    assert!(!map.contains_key("editor:unrelated"));
}
