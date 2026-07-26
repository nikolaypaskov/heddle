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
use std::io::Write;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use tempfile::NamedTempFile;
use toml_edit::DocumentMut;
use toml_edit::Item;

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

    let Some(mut old) = root.remove(OLD_TABLE) else {
        return Ok(false);
    };

    // Rename the one key whose own name carried the old stem, before any merging, so the two
    // tables are described in the same vocabulary when they meet.
    if let Some(ssh) = old.get_mut("ssh").and_then(|ssh| ssh.as_table_like_mut())
        && ssh.contains_key(OLD_SSH_KEY)
        && let Some(value) = ssh.remove(OLD_SSH_KEY)
    {
        // A new-style key already present wins; it is what the running build wrote.
        if !ssh.contains_key(NEW_SSH_KEY) {
            ssh.insert(NEW_SSH_KEY, value);
        }
    }

    if root.contains_key(NEW_TABLE) {
        // Both tables exist, which means a newer build has already written one and the user may
        // have edited it since. Refusing outright was wrong: it abandoned every setting that
        // existed ONLY in the old table, which is the exact data loss this migration is for.
        //
        // So MERGE, with the new table winning every conflict -- new values are the user's more
        // recent intent -- and carry across only what the new table has no opinion about.
        merge_missing(root.get_mut(NEW_TABLE), &old);
        log::info!(
            "Merged leftover [{OLD_TABLE}] settings into [{NEW_TABLE}] in {}; \
             existing [{NEW_TABLE}] values were kept.",
            path.display()
        );
    } else {
        root.insert(NEW_TABLE, old);
    }

    write_atomically(path, &doc.to_string())?;

    log::info!(
        "Migrated [{OLD_TABLE}] settings to [{NEW_TABLE}] in {}",
        path.display()
    );
    Ok(true)
}

/// Copies every key of `old` that `target` does not already define.
///
/// Recurses into sub-tables so that `[warpify.ssh]` and `[heddlify.ssh]` merge key by key
/// rather than one whole table displacing the other. `target` always wins a conflict.
fn merge_missing(target: Option<&mut Item>, old: &Item) {
    let (Some(target), Some(old_table)) = (
        target.and_then(|item| item.as_table_like_mut()),
        old.as_table_like(),
    ) else {
        return;
    };

    for (key, old_value) in old_table.iter() {
        match target.get_mut(key) {
            // Present on both sides and both are tables: descend, so a single shared key does
            // not shadow the siblings underneath it.
            Some(existing) if existing.is_table_like() && old_value.is_table_like() => {
                merge_missing(Some(existing), old_value);
            }
            // Present on the new side as a plain value: the user's newer choice, left alone.
            Some(_) => {}
            None => {
                target.insert(key, old_value.clone());
            }
        }
    }
}

/// Replaces `path`'s contents with `contents`, atomically, keeping the original's permissions.
///
/// Built on `NamedTempFile` rather than a hand-rolled temp path, because the hand-rolled
/// version got three things wrong that this gets right for free:
///
///   * The name was predictable (target plus process id). `NamedTempFile` uses a random name
///     created with `O_EXCL`, so it cannot collide with a concurrent migration and cannot be
///     pre-created by anyone else. The old `File::create` would have followed and truncated a
///     symlink planted at that predictable path.
///   * It starts at mode 0600, so the window between creating the file and applying the
///     target's permissions is not a window in which the contents are readable. The old
///     version wrote the data first and adjusted the mode afterwards.
///   * `persist` replaces an existing destination on every supported platform. Plain
///     `fs::rename` has platform-dependent behaviour when the destination exists, and this
///     project ships a Windows build; a migration that only worked on Unix would leave every
///     Windows user's `[warpify]` values unread with no indication anything had gone wrong.
///
/// The data is synced before the replacement, so a crash cannot leave a file that exists, has
/// the right name, and contains nothing.
fn write_atomically(path: &Path, contents: &str) -> Result<()> {
    // Read the source mode BEFORE creating anything. `if let Ok(meta)` swallowed a stat failure
    // and carried on, so the migration could report success having written the file with
    // NamedTempFile's default mode instead of the user's -- the same silent-widening outcome
    // the permission copy exists to prevent, just reached by a different route. The caller has
    // already read this file, so a stat failure here is genuinely exceptional and worth saying.
    let permissions = fs::metadata(path)
        .with_context(|| format!("could not read the existing mode of {}", path.display()))?
        .permissions();

    // Same directory as the target: the replacement is only atomic within one filesystem.
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = NamedTempFile::new_in(dir)
        .with_context(|| format!("could not create a temp file in {}", dir.display()))?;

    // Permissions BEFORE contents. If the original is more restrictive than the 0600 the temp
    // file starts at, this narrows it before there is anything to read; if it is laxer, the
    // contents are about to be readable under that path anyway.
    fs::set_permissions(tmp.path(), permissions).with_context(|| {
        format!("could not carry permissions across to {}", tmp.path().display())
    })?;

    tmp.write_all(contents.as_bytes())
        .with_context(|| format!("could not write {}", tmp.path().display()))?;
    tmp.as_file()
        .sync_all()
        .with_context(|| format!("could not flush {}", tmp.path().display()))?;

    // On failure the NamedTempFile is dropped and removes itself, so no debris is left behind.
    tmp.persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("could not replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
#[path = "heddlify_key_migration_tests.rs"]
mod tests;
