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
/// Three things this is careful about, each of which was wrong in the first version:
///
///   * The temp file name includes the process id, so two processes migrating at once cannot
///     write through each other's half-finished file.
///   * The original's permission bits are copied onto the replacement. Writing a fresh file
///     would hand it the default mode instead, quietly widening access to a file that can
///     contain whatever the user chose to put in it.
///   * The data is flushed and synced before the rename, so a crash cannot leave a file that
///     exists, has the right name, and contains nothing.
fn write_atomically(path: &Path, contents: &str) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    // Same directory as the target: `rename` is only atomic within a filesystem.
    let tmp = dir.join(format!(
        ".{}.heddlify-migration.{}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("settings.toml"),
        std::process::id()
    ));

    let result = (|| -> Result<()> {
        let mut file = fs::File::create(&tmp)
            .with_context(|| format!("could not create {}", tmp.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("could not write {}", tmp.display()))?;
        file.sync_all()
            .with_context(|| format!("could not flush {}", tmp.display()))?;
        drop(file);

        if let Ok(meta) = fs::metadata(path) {
            // Best effort: a filesystem that cannot represent the mode should not abort a
            // migration that has otherwise succeeded.
            let _ = fs::set_permissions(&tmp, meta.permissions());
        }

        fs::rename(&tmp, path)
            .with_context(|| format!("could not replace {}", path.display()))
    })();

    if result.is_err() {
        // Do not leave debris behind on a failed migration.
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
#[path = "heddlify_key_migration_tests.rs"]
mod tests;
