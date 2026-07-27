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
/// Asserts the ARGUMENT PARSER rejected them, not merely that the process exited non-zero.
/// A binary that accepted `--resume` and then failed later during startup would also exit
/// non-zero, so an exit-code-only assertion passes against exactly the implementation this
/// test exists to catch.
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
             build does not have."
        );

        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            combined.contains("unexpected argument") && combined.contains(flag),
            "`{flag}` should be rejected by the argument parser with an \"unexpected \
             argument\" error. Got:\n{combined}\n\nA different failure means the flag \
             may have been accepted and the binary failed later for some other reason, \
             which is not what this test is checking."
        );
    }
}

// There is deliberately NO end-to-end test here for `WARP_API_KEY` being ignored.
//
// The obvious one -- run the binary with the variable set and assert it still starts -- is
// worthless: every terminating flag (`--help`, `--version`) short-circuits in
// `session.rs` on `ErrorKind::DisplayHelp` BEFORE any startup path reads the environment,
// so the assertion passes whether or not the variable is honoured. Testing it properly
// needs a running TUI and therefore a TTY, which an unattended suite does not have.
//
// The real coverage is the unit test in `session_tests.rs`, which asserts the parser has
// no `env = "WARP_API_KEY"` attribute. This comment exists because an earlier version of
// this file DID have that e2e test, it passed, and it proved nothing.
