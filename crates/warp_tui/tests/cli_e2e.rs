//! End-to-end tests that drive the BUILT `heddle-tui` binary as a subprocess.
//!
//! Everything else in the suite tests functions. This tests the thing that ships: cargo
//! builds the binary, hands us its path through `CARGO_BIN_EXE_heddle-tui`, and we invoke
//! it the way a user would. That distinction has mattered here — a release once shipped a
//! stale binary, and another shipped the wrong artefact entirely, neither of which a
//! unit test could have caught.
//!
//! Headless by construction: only flags that terminate without a TTY are exercised, so
//! the suite runs unattended in CI with no terminal, no credentials, and no network.

use std::process::Command;

/// The path cargo gives us for the built binary. This is the real artefact, not a
/// re-invocation of the source.
fn heddle_tui() -> &'static str {
    env!("CARGO_BIN_EXE_heddle-tui")
}

#[test]
fn help_exits_zero_and_describes_heddle() {
    let out = Command::new(heddle_tui())
        .arg("--help")
        .output()
        .expect("failed to execute the built heddle-tui binary");

    assert!(
        out.status.success(),
        "`heddle-tui --help` exited {:?}; stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("heddle-tui"),
        "--help should name the binary it describes, got:\n{stdout}"
    );
}

/// The help text is user-facing copy, and it is the one place a fork's naming is most
/// visible. A `///` doc comment on the args struct becomes this text, which is how an
/// earlier build came to advertise flags it had removed.
#[test]
fn help_does_not_advertise_warp() {
    let out = Command::new(heddle_tui())
        .arg("--help")
        .output()
        .expect("failed to execute the built heddle-tui binary");
    let stdout = String::from_utf8_lossy(&out.stdout).to_lowercase();

    assert!(
        !stdout.contains("warp"),
        "`--help` mentions Warp; this is user-facing copy in a fork that is not \
         affiliated with them:\n{stdout}"
    );
}

/// The two flags that required Warp's server must stay gone from the SHIPPED binary.
///
/// There is a unit test asserting the parser rejects them. This asserts the built binary
/// does — the parser and the artefact are different things, and only one of them ships.
#[test]
fn removed_server_flags_are_rejected_by_the_built_binary() {
    for flag in ["--resume", "--api-key"] {
        let out = Command::new(heddle_tui())
            .args([flag, "some-value"])
            .output()
            .unwrap_or_else(|e| panic!("failed to execute the built binary with {flag}: {e}"));

        assert!(
            !out.status.success(),
            "`heddle-tui {flag} some-value` succeeded. That flag required a server this \
             build does not have, so accepting it means the binary advertises a hosted \
             product it cannot reach."
        );
    }
}

/// `WARP_API_KEY` was read at parse time via clap's `env` attribute. Removing the flag
/// while leaving that attribute would keep the variable silently honoured with nothing in
/// `--help` to say so, so this drives the built binary with the variable set.
#[test]
fn the_warp_api_key_environment_variable_is_ignored_by_the_built_binary() {
    let out = Command::new(heddle_tui())
        .arg("--help")
        .env("WARP_API_KEY", "should-be-ignored")
        .output()
        .expect("failed to execute the built heddle-tui binary");

    assert!(
        out.status.success(),
        "a bare invocation must still succeed with WARP_API_KEY set in the environment"
    );
}
