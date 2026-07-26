#[cfg(not(test))]
use std::env::var_os;

use anyhow::Context;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use vec1::{Vec1, vec1};
use warpui::AppContext;
use warpui::keymap::Keystroke;
#[cfg(not(test))]
use warpui::keymap::Trigger;

/// Environment variable to disable saving keybindings to file (used in integration tests)
pub const DISABLE_SAVE_ENV_VAR: &str = "WARP_TEST_DISABLE_KEYBINDING_SAVE";
const REMOVED_KEYBINDING_SERIALIZATION: &str = "none";

#[derive(PartialEq, Debug)]
/// A type to encapsulate the valid states of a keybinding
/// provided by a user in their keybindings.yaml file
pub enum UserDefinedKeybinding {
    /// Keybinding we can normalize/parse and will be recognized
    Keystrokes(Vec1<Keystroke>),
    /// User chose to remove the keybinding for an action
    Removed,
}

impl UserDefinedKeybinding {
    pub fn keystroke(value: Keystroke) -> Self {
        UserDefinedKeybinding::Keystrokes(vec1![value])
    }
}

const KEYBINDINGS_FILE_NAME: &str = "keybindings.yaml";

/// Action IDs that have been renamed, as `(old, current)`.
///
/// An action ID written into `keybindings.yaml` is a persistence contract, not just a name. The
/// loader hands whatever the file says to `set_custom_trigger`, and an ID matching no registered
/// action is silently ignored -- so renaming one turns every existing user binding for it inert
/// with no warning.
///
/// The quiet case is the damaging one. A user who had *removed* a default binding wrote `none`
/// against the old ID; once that entry stops matching, the removal stops applying and the
/// default keystroke comes back. They did not get their custom binding back, they got a
/// keystroke they had deliberately turned off.
///
/// Keep entries here forever. They cost one string comparison at startup.
const RENAMED_ACTIONS: &[(&str, &str)] = &[
    // Renamed when the fork replaced Warp's `warpify` with `heddlify`.
    (
        "terminal:warpify_subshell",
        "terminal:heddlify_subshell",
    ),
    (
        "workspace:show_settings_warpify_page",
        "workspace:show_settings_heddlify_page",
    ),
];

/// Maps a possibly-legacy action ID to the one the app registers today.
///
/// Unknown IDs pass through untouched: this exists to rescue renamed actions, not to validate
/// them, and a typo should still reach the loader's existing warning path.
pub fn canonical_action_name(name: &str) -> &str {
    RENAMED_ACTIONS
        .iter()
        .find_map(|(old, current)| (*old == name).then_some(*current))
        .unwrap_or(name)
}

/// The legacy IDs that alias `current`.
///
/// Used to clean stale spellings out of the file whenever the current one is written or
/// removed. Without that, canonicalisation is in-memory only and the old entry survives on
/// disk: resetting a migrated binding deletes the current ID, the legacy `none` beside it is
/// left behind, and after a restart it applies again -- the reset silently undoes itself.
fn legacy_aliases_for(current: &str) -> impl Iterator<Item = &'static str> + '_ {
    RENAMED_ACTIONS
        .iter()
        .filter_map(move |(old, canonical)| (*canonical == current).then_some(*old))
}

/// Removes every legacy spelling of `current` from `map`, returning how many were dropped.
///
/// Split out from the write paths so it can be tested: those are `#[cfg(not(test))]`, since
/// they touch the real keybindings file.
fn purge_legacy_aliases<V>(map: &mut std::collections::HashMap<String, V>, current: &str) -> usize {
    legacy_aliases_for(current)
        .filter(|alias| map.remove(*alias).is_some())
        .count()
}

/// Records `trigger` for `name`, dropping any stale spelling of the same action.
///
/// This exists as its own function so the purge is covered by tests. `write_custom_keybinding`
/// is `#[cfg(not(test))]` because it touches the real keybindings file, so a test that only
/// called `purge_legacy_aliases` directly would stay green if the production call site were
/// deleted -- it would be testing the helper, not the behaviour.
fn apply_binding_write<V>(
    map: &mut std::collections::HashMap<String, V>,
    name: String,
    trigger: V,
) {
    purge_legacy_aliases(map, &name);
    map.insert(name, trigger);
}

/// Forgets `name`, along with any stale spelling of the same action.
///
/// The purge matters more here than on write. Removing only the current ID leaves a legacy
/// entry behind to be re-applied on the next launch, quietly reversing the reset the user just
/// asked for -- and because that entry is usually `none`, what comes back is a keystroke they
/// had deliberately turned off.
fn apply_binding_removal<V>(map: &mut std::collections::HashMap<String, V>, name: &str) {
    purge_legacy_aliases(map, name);
    map.remove(name);
}

/// Load all stored custom keybindings into the UI framework so that they are used
#[cfg(not(test))]
pub fn load_custom_keybindings(app: &mut AppContext) {
    if let Some(keybindings) = read_custom_keybindings() {
        // Legacy IDs are applied FIRST so that a current-name entry overwrites them. If a file
        // somehow carries both spellings, the one the user's current build wrote is the one they
        // last chose, and it must win rather than being clobbered by a stale entry.
        let (legacy, current): (Vec<_>, Vec<_>) = keybindings
            .0
            .into_iter()
            .partition(|(name, _)| canonical_action_name(name) != name.as_str());

        for (name, trigger) in legacy.into_iter().chain(current) {
            let name = canonical_action_name(&name).to_owned();
            let keybinding_type = UserDefinedKeybinding::try_from(trigger.clone());

            match keybinding_type {
                Ok(UserDefinedKeybinding::Removed) => {
                    app.set_custom_trigger(name, Trigger::Empty);
                }
                Ok(UserDefinedKeybinding::Keystrokes(keystrokes)) => {
                    app.set_custom_trigger(name, Trigger::Keystrokes(keystrokes.to_vec()));
                }
                Err(e) => {
                    log::warn!(
                        "Tried to load an unparsable keybinding of {trigger:?} for action: {name}. error: {e}"
                    );
                }
            }
        }
    }
}

/// Write a new custom keybinding to disk
/// using the name of the editable binding and the new keystrokes
/// if keystrokes is UserDefinedKeybinding::Removed
/// we write a special value to disk to save that state
#[cfg(not(test))]
pub fn write_custom_keybinding(name: String, keybinding: UserDefinedKeybinding) {
    // In tests, we don't want to write the actual keybindings file, since that could clobber the
    // user's current settings, so we no-op
    if var_os(DISABLE_SAVE_ENV_VAR).is_some() {
        return;
    }

    let mut map = read_custom_keybindings().unwrap_or_default();

    apply_binding_write(&mut map.0, name, keybinding.into());
    save_custom_keybindings(map);
}

/// Remove a custom keybinding from disk.
#[cfg(not(test))]
pub fn remove_custom_keybinding<N>(name: N)
where
    N: AsRef<str>,
{
    // In tests, we don't want to write the actual keybindings file, since that could clobber the
    // users current settings, so we no-op
    if var_os(DISABLE_SAVE_ENV_VAR).is_some() {
        return;
    }

    let mut map = read_custom_keybindings().unwrap_or_default();

    apply_binding_removal(&mut map.0, name.as_ref());
    save_custom_keybindings(map);
}

pub fn keybinding_file_path() -> std::path::PathBuf {
    warp_core::paths::config_local_dir().join(KEYBINDINGS_FILE_NAME)
}

/// Save the custom keybindings map to disk.
#[cfg(not(test))]
// Allow unused variables when no local filesystem exists as the arg is unused.
#[cfg_attr(not(feature = "local_fs"), allow(unused_variables))]
fn save_custom_keybindings(map: CustomKeybindings) {
    cfg_if::cfg_if! {
        if #[cfg(feature = "local_fs")] {
            let file = match crate::util::file::create_file(keybinding_file_path()) {
                Ok(f) => f,
                Err(e) => {
                    log::warn!("Unable to open file for storing custom keybindings: {e}");
                    return;
                }
            };
            let writer = std::io::BufWriter::new(file);

            if let Err(e) = serde_yaml::to_writer(writer, &map) {
                log::warn!("Unable to serialize custom keybindings to file: {e}");
            }
        } else {
            log::warn!("TODO(wasm): need to implement keybindings support");
        }
    }
}

/// Read the stored custom keybindings from disk into a map of Editable Binding Name -> Trigger
///
/// Returns `None` if the file can't be read or the deserialization fails
#[cfg(not(test))]
fn read_custom_keybindings() -> Option<CustomKeybindings> {
    let file = std::fs::File::open(keybinding_file_path()).ok()?;
    let reader = std::io::BufReader::new(file);

    match serde_yaml::from_reader(reader) {
        Ok(map) => Some(map),
        Err(e) => {
            log::warn!("Unable to deserialize stored keybindings: {e}");
            None
        }
    }
}

// For tests, we don't want to read or write from the filesystem.
//
// Unit tests are run with #[cfg(test)] enabled, so we can define custom no-op implementations
#[cfg(test)]
pub fn load_custom_keybindings(_: &mut AppContext) {}
#[cfg(test)]
pub fn write_custom_keybinding(_: String, _: UserDefinedKeybinding) {}
#[cfg(test)]
pub fn remove_custom_keybinding<N>(_: N)
where
    N: AsRef<str>,
{
}

/// Struct that represents the full custom keybindings file for (de-)serialization
///
/// The file format is a top-level YAML map of (Editable Binding Name) -> Keybinding
/// Since many of the editable bindings have a `:` character in their name, the name will need to
/// be quoted in most cases.
/// The format of the keybinding is the normalized version that we use internally, with multiple
/// keystrokes separated by whitespace, if necessary.
///
/// For example:
/// ---
/// "editor:delete_all_left": cmd-shift-A
/// "editor:delete_all_right": cmd-shift-D escape
#[derive(Serialize, Deserialize, Default)]
#[cfg(not(test))]
struct CustomKeybindings(std::collections::HashMap<String, PersistedTrigger>);

/// The normalized version of a keystroke or series of keystrokes that is written into the
/// keybindings file. If there are multiple keystrokes, each is separated by a space
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct PersistedTrigger(String);

impl From<UserDefinedKeybinding> for PersistedTrigger {
    fn from(keybinding: UserDefinedKeybinding) -> Self {
        match keybinding {
            UserDefinedKeybinding::Keystrokes(keystrokes) => {
                PersistedTrigger(keystrokes.iter().map(Keystroke::normalized).join(" "))
            }
            UserDefinedKeybinding::Removed => {
                PersistedTrigger(REMOVED_KEYBINDING_SERIALIZATION.to_string())
            }
        }
    }
}

impl TryFrom<PersistedTrigger> for UserDefinedKeybinding {
    type Error = anyhow::Error;
    fn try_from(trigger: PersistedTrigger) -> anyhow::Result<Self> {
        if trigger.0 == REMOVED_KEYBINDING_SERIALIZATION {
            return Ok(UserDefinedKeybinding::Removed);
        }

        let mut keystrokes: Vec<Keystroke> = Vec::new();

        for keystroke in trigger.0.split_whitespace() {
            let parsed_keystroke: Keystroke = Keystroke::parse(keystroke).context(format!(
                "Failed to parse keystroke \"{}\" in trigger \"{}\"",
                keystroke, trigger.0,
            ))?;
            keystrokes.push(parsed_keystroke);
        }

        let parsed_keystrokes: Vec1<Keystroke> = Vec1::try_from(keystrokes).context(format!(
            "No valid keystrokes were found in trigger: {}",
            trigger.0
        ))?;

        Ok(UserDefinedKeybinding::Keystrokes(parsed_keystrokes))
    }
}

#[cfg(test)]
#[path = "keyboard_tests.rs"]
mod tests;
