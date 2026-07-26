use std::fs;

use tempfile::TempDir;

use super::migrate_warpify_table;
use super::write_atomically;

/// Writes `contents` to a settings.toml inside a fresh temp dir and returns both.
fn settings_file(contents: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("settings.toml");
    fs::write(&path, contents).expect("write settings.toml");
    (dir, path)
}

#[test]
fn migrates_the_table_and_preserves_every_value() {
    let (_dir, path) = settings_file(
        r#"
[warpify.subshells]
added_subshell_commands = ["mycmd"]

[warpify.ssh]
ssh_hosts_denylist = ["prod.example.com"]
use_ssh_tmux_wrapper = true
"#,
    );

    assert!(migrate_warpify_table(&path).expect("migration succeeds"));

    let after = fs::read_to_string(&path).expect("read back");
    assert!(after.contains("heddlify"), "table was renamed: {after}");
    assert!(!after.contains("warpify"), "no old table remains: {after}");
    // The point of the migration is that the VALUES survive. A rename that dropped them would
    // be indistinguishable from no migration at all, from the user's perspective.
    assert!(after.contains("mycmd"), "subshell value survived: {after}");
    assert!(
        after.contains("prod.example.com"),
        "denylist value survived: {after}"
    );
    assert!(
        after.contains("use_ssh_tmux_wrapper = true"),
        "untouched keys carried across: {after}"
    );
}

#[test]
fn renames_the_one_key_that_carried_the_old_stem() {
    let (_dir, path) = settings_file(
        r#"
[warpify.ssh]
enable_ssh_warpification = false
"#,
    );

    assert!(migrate_warpify_table(&path).expect("migration succeeds"));

    let after = fs::read_to_string(&path).expect("read back");
    assert!(
        after.contains("enable_ssh_heddlification = false"),
        "key renamed AND its value kept: {after}"
    );
    assert!(
        !after.contains("enable_ssh_warpification"),
        "old key gone: {after}"
    );
}

#[test]
fn preserves_comments_and_unrelated_tables() {
    // A migration that silently reformats a hand-edited config is its own kind of damage, so
    // this pins the toml_edit behaviour rather than assuming it.
    let (_dir, path) = settings_file(
        r#"# my notes about ssh
[warpify.ssh]
# keep prod out of this
ssh_hosts_denylist = ["prod.example.com"]

[appearance]
theme = "dark"
"#,
    );

    assert!(migrate_warpify_table(&path).expect("migration succeeds"));

    let after = fs::read_to_string(&path).expect("read back");
    assert!(after.contains("# my notes about ssh"), "comment kept: {after}");
    assert!(
        after.contains("# keep prod out of this"),
        "inner comment kept: {after}"
    );
    assert!(after.contains("theme = \"dark\""), "other tables kept: {after}");
}

#[test]
fn is_idempotent_and_reports_no_change_the_second_time() {
    let (_dir, path) = settings_file(
        r#"
[warpify.ssh]
ssh_hosts_denylist = ["a"]
"#,
    );

    assert!(migrate_warpify_table(&path).expect("first run migrates"));
    assert!(
        !migrate_warpify_table(&path).expect("second run is a no-op"),
        "a migrated file must not be rewritten again"
    );
}

#[test]
fn merges_when_both_tables_exist_with_the_new_one_winning() {
    // Both tables present means a newer build already wrote one and the user may have edited it
    // since. Refusing outright was the first implementation, and it was wrong: it abandoned
    // every setting that existed ONLY in the old table -- the exact data loss this migration is
    // supposed to prevent. So merge, and let the newer value win any conflict.
    let (_dir, path) = settings_file(
        r#"
[warpify.ssh]
ssh_hosts_denylist = ["stale"]
use_ssh_tmux_wrapper = true

[warpify.subshells]
added_subshell_commands = ["only-in-old"]

[heddlify.ssh]
ssh_hosts_denylist = ["current"]
"#,
    );

    assert!(migrate_warpify_table(&path).expect("migration succeeds"));

    let after = fs::read_to_string(&path).expect("read back");
    assert!(
        after.contains("current") && !after.contains("stale"),
        "the newer value wins the conflict: {after}"
    );
    assert!(
        after.contains("use_ssh_tmux_wrapper = true"),
        "a sibling key present only in the old table is rescued, not shadowed by the \
         conflicting one beside it: {after}"
    );
    assert!(
        after.contains("only-in-old"),
        "a whole sub-table present only in the old table is carried across: {after}"
    );
    assert!(!after.contains("[warpify"), "old table removed: {after}");
}

#[test]
fn merging_prefers_a_new_style_ssh_key_over_the_renamed_old_one() {
    let (_dir, path) = settings_file(
        r#"
[warpify.ssh]
enable_ssh_warpification = true

[heddlify.ssh]
enable_ssh_heddlification = false
"#,
    );

    assert!(migrate_warpify_table(&path).expect("migration succeeds"));

    let after = fs::read_to_string(&path).expect("read back");
    assert!(
        after.contains("enable_ssh_heddlification = false"),
        "the user's newer explicit opt-out survives: {after}"
    );
    assert!(
        !after.contains("enable_ssh_heddlification = true"),
        "the stale value must not be reintroduced: {after}"
    );
}

#[test]
fn preserves_file_permissions() {
    // The replacement is a newly created file, which would otherwise take the default mode and
    // could widen access to whatever the user has put in their settings.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let (_dir, path) = settings_file("[warpify.ssh]\nssh_hosts_denylist = [\"a\"]\n");
        // Deliberately NOT 0600. NamedTempFile creates its file at 0600, so asserting that mode
        // would pass whether or not the permissions were actually carried across -- the test
        // would be measuring tempfile's default, not this code. Mutation-testing caught exactly
        // that: deleting the permission-copy left the assertion green. 0640 differs from the
        // default in both directions of the comparison, so only a real copy satisfies it.
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("chmod");

        assert!(migrate_warpify_table(&path).expect("migration succeeds"));

        let mode = fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
        assert_eq!(mode, 0o640, "the original's permissions carried across");
    }
}

#[test]
fn leaves_no_temp_file_behind() {
    let (dir, path) = settings_file("[warpify.ssh]\nssh_hosts_denylist = [\"a\"]\n");
    assert!(migrate_warpify_table(&path).expect("migration succeeds"));

    let leftovers: Vec<_> = fs::read_dir(dir.path())
        .expect("read dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name != "settings.toml")
        .collect();
    assert!(leftovers.is_empty(), "temp files cleaned up: {leftovers:?}");
}

#[test]
fn does_nothing_when_there_is_nothing_to_migrate() {
    let (_dir, path) = settings_file("[appearance]\ntheme = \"dark\"\n");
    assert!(!migrate_warpify_table(&path).expect("no-op"));

    let after = fs::read_to_string(&path).expect("read back");
    assert_eq!(after, "[appearance]\ntheme = \"dark\"\n", "file untouched");
}

#[test]
fn a_missing_file_is_not_an_error() {
    // A fresh install has no settings.toml. That must not be reported as a failure, or every
    // first launch would log a spurious migration error.
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("settings.toml");
    assert!(!migrate_warpify_table(&path).expect("absent file is fine"));
}

#[test]
fn malformed_toml_is_reported_rather_than_silently_skipped() {
    let (_dir, path) = settings_file("[warpify.ssh\nthis is not toml");
    assert!(
        migrate_warpify_table(&path).is_err(),
        "a parse failure must surface, not masquerade as 'nothing to migrate'"
    );

    let after = fs::read_to_string(&path).expect("read back");
    assert_eq!(
        after, "[warpify.ssh\nthis is not toml",
        "the user's file is left exactly as it was"
    );
}

#[test]
fn a_failed_replacement_propagates_and_leaves_no_debris() {
    // The existing cleanup test only ever exercises a SUCCESSFUL persist, which really just
    // observes NamedTempFile behaving normally. This drives the failure path: the destination
    // is a directory, so the temp file is created and written fine and the final replacement is
    // the step that fails. Both halves matter -- the error must surface, and the temp file must
    // not be left sitting next to the user's settings.
    let dir = TempDir::new().expect("temp dir");
    let target = dir.path().join("settings.toml");
    fs::create_dir(&target).expect("make the destination a directory");

    let err = write_atomically(&target, "anything").expect_err("replacing a directory must fail");
    assert!(
        format!("{err:#}").contains("settings.toml"),
        "the error names the file it could not replace: {err:#}"
    );

    let leftovers: Vec<_> = fs::read_dir(dir.path())
        .expect("read dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name != "settings.toml")
        .collect();
    assert!(
        leftovers.is_empty(),
        "a failed migration must not strand a temp file: {leftovers:?}"
    );
}

#[test]
fn an_unreadable_source_mode_is_reported_rather_than_defaulted() {
    // `if let Ok(meta)` used to swallow a stat failure and carry on, writing the replacement
    // with the temp file's default mode instead of the user's -- silently widening access by a
    // different route than the one the permission copy guards.
    let dir = TempDir::new().expect("temp dir");
    let missing = dir.path().join("does-not-exist.toml");

    let err = write_atomically(&missing, "x").expect_err("a missing source must fail");
    assert!(
        format!("{err:#}").contains("existing mode"),
        "the failure is attributed to reading the mode: {err:#}"
    );
}
