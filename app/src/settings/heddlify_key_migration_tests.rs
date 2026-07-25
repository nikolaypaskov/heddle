use std::fs;

use tempfile::TempDir;

use super::migrate_warpify_table;

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
fn refuses_to_clobber_an_existing_heddlify_table() {
    // Both tables present means the user has already run a newer build and has been editing the
    // new table since. Overwriting it with stale values would undo those edits.
    let (_dir, path) = settings_file(
        r#"
[warpify.ssh]
ssh_hosts_denylist = ["stale"]

[heddlify.ssh]
ssh_hosts_denylist = ["current"]
"#,
    );

    assert!(
        !migrate_warpify_table(&path).expect("no migration performed"),
        "must not migrate when both tables exist"
    );

    let after = fs::read_to_string(&path).expect("read back");
    assert!(after.contains("current"), "newer value untouched: {after}");
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
