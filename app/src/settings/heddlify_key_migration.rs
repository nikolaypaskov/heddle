//! One-time migration of the persisted `[warpify]` settings table to `[heddlify]`.
//!
//! The fork renamed `warpify` to `heddlify` throughout the codebase, and the settings those
//! identifiers describe are addressed by TOML path: `warpify.ssh.ssh_hosts_denylist` and
//! friends. Renaming the `toml_path` strings alone would be a silent data loss -- the settings
//! would simply stop being found, every affected preference would revert to its default, and
//! nothing would tell the user why their SSH denylist stopped working. The old keys would sit
//! in their `settings.toml` looking correct.
//!
//! So the rename comes with a migration. It runs before the TOML backend is constructed, since
//! after that point the settings have already been read under their new names and the old ones
//! are invisible.
//!
//! Only two things actually move:
//!   * the top-level table `warpify` -> `heddlify`
//!   * the key `enable_ssh_warpification` -> `enable_ssh_heddlification`
//!
//! Every other key beneath the table (`ssh_hosts_denylist`, `use_ssh_tmux_wrapper`, ...) keeps
//! its name and is carried across untouched.
//!
//! `toml_edit` rather than `toml`: a plain parse-and-reserialize would discard the user's
//! comments, key order and formatting. A migration that silently reformats a hand-edited
//! config is its own kind of damage.

use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use toml_edit::DocumentMut;

const OLD_TABLE: &str = "warpify";
const NEW_TABLE: &str = "heddlify";
const OLD_SSH_KEY: &str = "enable_ssh_warpification";
const NEW_SSH_KEY: &str = "enable_ssh_heddlification";

/// Rewrites `[warpify]` to `[heddlify]` in the settings file at `path`.
///
/// Returns `Ok(true)` when the file was rewritten. Absent files, files with nothing to migrate,
/// and files that already carry a `[heddlify]` table are all `Ok(false)`.
///
/// Errors are for the caller to log, not to fail startup over: a settings file that cannot be
/// migrated is still a settings file the user can edit by hand, and refusing to launch over it
/// would be a far worse outcome than one stale table.
pub fn migrate_warpify_table(path: &Path) -> Result<bool> {
    let Ok(text) = fs::read_to_string(path) else {
        // No settings file yet, or unreadable. A fresh install has nothing to migrate.
        return Ok(false);
    };

    let mut doc = text
        .parse::<DocumentMut>()
        .with_context(|| format!("could not parse {} as TOML", path.display()))?;

    let root = doc.as_table_mut();
    if !root.contains_key(OLD_TABLE) {
        return Ok(false);
    }

    // If both tables are present the user has run a newer build already and has since been
    // editing the new one. Overwriting it with stale values from the old table would undo
    // whatever they changed in between, so leave both alone and say so.
    if root.contains_key(NEW_TABLE) {
        log::warn!(
            "{} contains both [{OLD_TABLE}] and [{NEW_TABLE}] tables; leaving both untouched. \
             [{OLD_TABLE}] is no longer read and can be deleted.",
            path.display()
        );
        return Ok(false);
    }

    let Some(mut item) = root.remove(OLD_TABLE) else {
        return Ok(false);
    };

    // Rename the one key whose own name carried the old stem. Guarded the same way as the
    // table: an existing new-style key wins, because it is the one the running build wrote.
    if let Some(ssh) = item.get_mut("ssh").and_then(|ssh| ssh.as_table_like_mut())
        && ssh.contains_key(OLD_SSH_KEY)
        && !ssh.contains_key(NEW_SSH_KEY)
        && let Some(value) = ssh.remove(OLD_SSH_KEY)
    {
        ssh.insert(NEW_SSH_KEY, value);
    }

    root.insert(NEW_TABLE, item);

    // Write via a sibling temp file and rename, so an interrupted write cannot leave the user
    // with a truncated settings.toml. Renaming within the same directory keeps it on one
    // filesystem, which is what makes the replacement atomic.
    let tmp = path.with_extension("toml.heddlify-migration");
    fs::write(&tmp, doc.to_string())
        .with_context(|| format!("could not write {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .with_context(|| format!("could not replace {}", path.display()))?;

    log::info!(
        "Migrated [{OLD_TABLE}] settings to [{NEW_TABLE}] in {}",
        path.display()
    );
    Ok(true)
}

#[cfg(test)]
#[path = "heddlify_key_migration_tests.rs"]
mod tests;
