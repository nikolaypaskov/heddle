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
