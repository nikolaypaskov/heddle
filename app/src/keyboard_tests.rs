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
