use std::ffi::OsString;
use std::fs;

use tempfile::TempDir;
use warp_cli::agent::Harness;
use warp_core::features::FeatureFlag;

use super::{
    build_local_claude_child_command, build_local_codex_child_command,
    build_local_opencode_child_command, local_claude_child_prompt, normalize_local_child_harness,
    prepare_local_harness_child_launch, validate_local_harness_shell,
};
use crate::ai::agent_sdk::driver::OZ_MESSAGE_LISTENER_MANAGED_EXTERNALLY_ENV;
use crate::ai::ambient_agents::task::normalize_orchestrator_agent_name;
use crate::ai::local_harness_setup::LOCAL_CODEX_HARNESS_DISABLED_MESSAGE;
use crate::terminal::shell::ShellType;

struct EnvVarGuard {
    key: &'static str,
    original: Option<OsString>,
}
#[test]
fn local_claude_child_prompt_does_not_instruct_a_command_that_always_fails() {
    // The prompt used to tell the child to report "at start, when blocked, and
    // when complete" via `oz run message send`. Every one of those calls is
    // rejected by `run_task` in `ai/agent_sdk/mod.rs`, because the mailbox is
    // Warp's server-side relay. Instructing a guaranteed failure is worse than
    // instructing nothing: the child burns turns on it and may report itself
    // blocked on infrastructure the user cannot install.
    let prompt = local_claude_child_prompt("List files");

    // Assert on what makes a line copy-pasteable rather than on bare mentions:
    // the prompt may still *name* the mailbox in order to warn the child off it,
    // but it must not hand over an invocation with its arguments filled in.
    assert!(!prompt.contains("--sender-run-id"));
    assert!(!prompt.contains("--to \"$OZ_PARENT_RUN_ID\""));
    assert!(!prompt.contains("mark-delivered"));
    assert!(!prompt.contains("\"$OZ_CLI\" run message"));
    assert!(!prompt.contains("--limit 25"));
}

#[test]
fn local_claude_child_prompt_names_the_channel_that_works() {
    // The lead reads the child's conversation locally, so the final response is
    // the reporting channel. The child has to be told that, and told not to wait
    // for a reply that can never arrive.
    let prompt = local_claude_child_prompt("List files");

    assert!(prompt.contains("final response"));
    assert!(prompt.contains("no run mailbox"));
    assert!(prompt.to_lowercase().contains("do not stand by"));
    assert!(prompt.ends_with("Task:\nList files"));
}

impl EnvVarGuard {
    fn set(key: &'static str, value: impl Into<OsString>) -> Self {
        let original = std::env::var_os(key);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var(key, value.into()) };
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(original) = &self.original {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var(self.key, original) };
        } else {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var(self.key) };
        }
    }
}

fn write_fake_cli(bin_dir: &std::path::Path, name: &str) {
    let executable_name = if cfg!(windows) {
        format!("{name}.cmd")
    } else {
        name.to_string()
    };
    let executable_path = bin_dir.join(executable_name);
    let script = if cfg!(windows) {
        "@echo off\r\n"
    } else {
        "#!/bin/sh\n"
    };

    fs::write(&executable_path, script).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&executable_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable_path, permissions).unwrap();
    }
}

#[test]
fn normalize_local_child_harness_accepts_supported_aliases() {
    assert_eq!(
        normalize_local_child_harness("claude"),
        Some(Harness::Claude)
    );
    assert_eq!(
        normalize_local_child_harness("claude-code"),
        Some(Harness::Claude)
    );
    assert_eq!(
        normalize_local_child_harness("claude_code"),
        Some(Harness::Claude)
    );
    assert_eq!(
        normalize_local_child_harness("opencode"),
        Some(Harness::OpenCode)
    );
    assert_eq!(
        normalize_local_child_harness("open-code"),
        Some(Harness::OpenCode)
    );
    assert_eq!(
        normalize_local_child_harness("open_code"),
        Some(Harness::OpenCode)
    );
    assert_eq!(normalize_local_child_harness("codex"), Some(Harness::Codex));
}

#[test]
fn normalize_local_child_harness_rejects_unsupported_values() {
    assert_eq!(normalize_local_child_harness("oz"), None);
    assert_eq!(normalize_local_child_harness("gemini"), None);
    assert_eq!(normalize_local_child_harness(""), None);
}

#[test]
fn validate_local_harness_shell_accepts_supported_shells() {
    assert_eq!(validate_local_harness_shell(Some(ShellType::Bash)), Ok(()));
    assert_eq!(validate_local_harness_shell(Some(ShellType::Zsh)), Ok(()));
    assert_eq!(validate_local_harness_shell(Some(ShellType::Fish)), Ok(()));
}

#[test]
fn validate_local_harness_shell_rejects_unsupported_shells() {
    assert_eq!(
        validate_local_harness_shell(Some(ShellType::PowerShell)),
        Err(
            "Local child harnesses currently require bash, zsh, or fish; PowerShell is not supported."
                .to_string()
        )
    );
    assert_eq!(
        validate_local_harness_shell(None),
        Err(
            "Local child harnesses currently require a detected bash, zsh, or fish session."
                .to_string()
        )
    );
}

#[test]
fn build_local_claude_child_command_quotes_the_prompt() {
    let command = build_local_claude_child_command("hello world");

    assert!(command.starts_with("claude --session-id "));
    assert!(command.ends_with(" --dangerously-skip-permissions 'hello world'"));
}

#[test]
fn build_local_opencode_child_command_quotes_the_prompt() {
    assert_eq!(
        build_local_opencode_child_command("hello world"),
        "opencode --prompt 'hello world'"
    );
}

#[test]
fn build_local_codex_child_command_quotes_the_prompt() {
    assert_eq!(
        build_local_codex_child_command("hello world"),
        "codex --dangerously-bypass-approvals-and-sandbox 'hello world'"
    );
}

#[test]
fn normalize_orchestrator_agent_name_trims_and_drops_empty() {
    assert_eq!(
        normalize_orchestrator_agent_name("frontend-tests"),
        Some("frontend-tests".to_string())
    );
    assert_eq!(
        normalize_orchestrator_agent_name("  frontend-tests  "),
        Some("frontend-tests".to_string())
    );
    assert_eq!(normalize_orchestrator_agent_name(""), None);
    assert_eq!(normalize_orchestrator_agent_name("   "), None);
    assert_eq!(normalize_orchestrator_agent_name("\t\n  "), None);
}

#[tokio::test]
#[serial_test::serial]
async fn prepare_local_codex_child_launch_rejects_without_rewriting_global_codex_state() {
    let fake_home = TempDir::new().unwrap();
    let fake_bin_dir = TempDir::new().unwrap();
    let working_dir = fake_home.path().join("workspace");
    fs::create_dir_all(&working_dir).unwrap();
    write_fake_cli(fake_bin_dir.path(), "codex");

    let _home = EnvVarGuard::set("HOME", fake_home.path().as_os_str().to_os_string());
    let _path = EnvVarGuard::set("PATH", fake_bin_dir.path().as_os_str().to_os_string());

    let result = prepare_local_harness_child_launch(
        "hello world".to_string(),
        "codex".to_string(),
        None,
        Some("parent-run".to_string()),
        Some(ShellType::Zsh),
        Some(working_dir),
    )
    .await;

    match result {
        Ok(_) => panic!("disabled local codex should be rejected"),
        Err(err) => assert_eq!(err, LOCAL_CODEX_HARNESS_DISABLED_MESSAGE),
    }
    assert!(!fake_home.path().join(".codex").exists());
}

#[tokio::test]
#[serial_test::serial]
async fn prepare_local_codex_child_launch_succeeds_when_testing_flag_is_enabled() {
    let _local_codex = FeatureFlag::LocalClaudeCodexChildHarnesses.override_enabled(true);
    let fake_home = TempDir::new().unwrap();
    let fake_bin_dir = TempDir::new().unwrap();
    let working_dir = fake_home.path().join("workspace");
    fs::create_dir_all(&working_dir).unwrap();
    write_fake_cli(fake_bin_dir.path(), "codex");

    let _home = EnvVarGuard::set("HOME", fake_home.path().as_os_str().to_os_string());
    let _path = EnvVarGuard::set("PATH", fake_bin_dir.path().as_os_str().to_os_string());

    let prepared = prepare_local_harness_child_launch(
        "hello world".to_string(),
        "codex".to_string(),
        Some("ignored-model".to_string()),
        Some("parent-run".to_string()),
        Some(ShellType::Zsh),
        Some(working_dir),
    )
    .await
    .unwrap();

    assert_eq!(
        prepared.command,
        "codex --dangerously-bypass-approvals-and-sandbox 'hello world'"
    );
    assert!(
        !prepared
            .env_vars
            .contains_key(&OsString::from("ANTHROPIC_MODEL"))
    );
    assert_eq!(prepared.run_id, prepared.task_id.to_string());
    assert!(!fake_home.path().join(".codex").exists());
}

#[tokio::test]
#[serial_test::serial]
async fn prepare_local_claude_child_merges_anthropic_model_env_var() {
    let fake_home = TempDir::new().unwrap();
    let fake_bin_dir = TempDir::new().unwrap();
    let working_dir = fake_home.path().join("workspace");
    fs::create_dir_all(&working_dir).unwrap();
    write_fake_cli(fake_bin_dir.path(), "claude");

    let _home = EnvVarGuard::set("HOME", fake_home.path().as_os_str().to_os_string());
    let _claude_home = EnvVarGuard::set(
        "CLAUDE_HOME",
        fake_home.path().join(".claude").as_os_str().to_os_string(),
    );
    let _path = EnvVarGuard::set("PATH", fake_bin_dir.path().as_os_str().to_os_string());

    let prepared = prepare_local_harness_child_launch(
        "hello world".to_string(),
        "claude".to_string(),
        Some("opus".to_string()),
        Some("parent-run".to_string()),
        Some(ShellType::Zsh),
        Some(working_dir),
    )
    .await
    .unwrap();

    assert_eq!(
        prepared.env_vars.get(&OsString::from("ANTHROPIC_MODEL")),
        Some(&OsString::from("opus"))
    );
    assert!(
        !prepared
            .env_vars
            .contains_key(&OsString::from(OZ_MESSAGE_LISTENER_MANAGED_EXTERNALLY_ENV))
    );
    assert!(
        !prepared
            .env_vars
            .contains_key(&OsString::from("OZ_PARENT_LISTENER_MANAGED_EXTERNALLY"))
    );
    // The launched command carries the child's instructions, so it must not
    // smuggle back the mailbox commands `run_task` rejects outright.
    assert!(!prepared.command.contains("--sender-run-id"));
    assert!(!prepared.command.contains("mark-delivered"));
    assert!(prepared.command.contains("final response"));
}

#[tokio::test]
#[serial_test::serial]
async fn prepare_local_claude_child_no_anthropic_model_when_empty() {
    let fake_home = TempDir::new().unwrap();
    let fake_bin_dir = TempDir::new().unwrap();
    let working_dir = fake_home.path().join("workspace");
    fs::create_dir_all(&working_dir).unwrap();
    write_fake_cli(fake_bin_dir.path(), "claude");

    let _home = EnvVarGuard::set("HOME", fake_home.path().as_os_str().to_os_string());
    let _claude_home = EnvVarGuard::set(
        "CLAUDE_HOME",
        fake_home.path().join(".claude").as_os_str().to_os_string(),
    );
    let _path = EnvVarGuard::set("PATH", fake_bin_dir.path().as_os_str().to_os_string());

    let prepared = prepare_local_harness_child_launch(
        "hello world".to_string(),
        "claude".to_string(),
        None,
        Some("parent-run".to_string()),
        Some(ShellType::Zsh),
        Some(working_dir),
    )
    .await
    .unwrap();

    assert!(
        !prepared
            .env_vars
            .contains_key(&OsString::from("ANTHROPIC_MODEL"))
    );
}

#[tokio::test]
async fn prepare_local_harness_child_launch_rejects_disabled_codex_before_shell_validation() {
    let result = prepare_local_harness_child_launch(
        "hello world".to_string(),
        "codex".to_string(),
        None,
        Some("parent-run".to_string()),
        None,
        None,
    )
    .await;

    match result {
        Ok(_) => panic!("disabled local codex should be rejected"),
        Err(err) => assert_eq!(err, LOCAL_CODEX_HARNESS_DISABLED_MESSAGE),
    }
}

#[tokio::test]
#[serial_test::serial]
async fn prepare_local_claude_child_needs_no_task_service() {
    // Launching a local child used to require `AIClient::create_agent_task`,
    // a GraphQL round-trip behind an account this build never has — so every
    // local harness launch failed before the subprocess was ever built. The
    // run id is now minted locally: unique per launch, and never nil.
    let fake_home = TempDir::new().unwrap();
    let fake_bin_dir = TempDir::new().unwrap();
    let working_dir = fake_home.path().join("workspace");
    fs::create_dir_all(&working_dir).unwrap();
    write_fake_cli(fake_bin_dir.path(), "claude");

    let _home = EnvVarGuard::set("HOME", fake_home.path().as_os_str().to_os_string());
    let _claude_home = EnvVarGuard::set(
        "CLAUDE_HOME",
        fake_home.path().join(".claude").as_os_str().to_os_string(),
    );
    let _path = EnvVarGuard::set("PATH", fake_bin_dir.path().as_os_str().to_os_string());

    let prepare = || {
        prepare_local_harness_child_launch(
            "hello world".to_string(),
            "claude".to_string(),
            None,
            Some("parent-run".to_string()),
            Some(ShellType::Zsh),
            Some(working_dir.clone()),
        )
    };

    let first = prepare().await.unwrap();
    let second = prepare().await.unwrap();

    assert_eq!(first.run_id, first.task_id.to_string());
    assert_ne!(
        first.run_id, second.run_id,
        "every local child needs its own run id to address its parent"
    );
    assert_eq!(
        first.env_vars.get(&OsString::from("OZ_RUN_ID")),
        Some(&OsString::from(first.run_id.clone()))
    );
    assert_eq!(
        first.env_vars.get(&OsString::from("OZ_PARENT_RUN_ID")),
        Some(&OsString::from("parent-run"))
    );
}
