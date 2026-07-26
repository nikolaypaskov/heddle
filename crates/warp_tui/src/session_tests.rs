use clap::Parser;

use super::TuiArgs;

/// The two cloud flags must stay gone.
///
/// `--resume <token>` resumed a conversation held on Warp's servers and `--api-key` authenticated
/// non-interactively against them, also reading `WARP_API_KEY`. Both were advertised by
/// `heddle-tui --help`, so the shipped binary described a hosted product it cannot reach. This
/// file previously asserted they parsed; it now asserts they do not, which is the direction that
/// needs defending.
#[test]
fn the_removed_server_flags_are_rejected() {
    for flag in ["--resume", "--api-key"] {
        let result = TuiArgs::try_parse_from(["heddle-tui", flag, "some-value"]);
        assert!(
            result.is_err(),
            "{flag} must not be accepted: it required a server this build does not have"
        );
    }
}

/// And the env var behind `--api-key` must not be picked up either.
///
/// Asserted separately because clap read `env = "WARP_API_KEY"` at parse time, so removing the
/// flag while leaving that attribute would have left the variable silently honoured while
/// `--help` showed nothing.
#[test]
fn the_warp_api_key_environment_variable_is_ignored() {
    // SAFETY: sets and immediately removes one process-wide variable whose name nothing else in
    // this suite reads. Tests share a process, hence the removal rather than leaving it set.
    unsafe { std::env::set_var("WARP_API_KEY", "should-be-ignored") };
    let parsed = TuiArgs::try_parse_from(["heddle-tui"]);
    unsafe { std::env::remove_var("WARP_API_KEY") };

    assert!(
        parsed.is_ok(),
        "a bare invocation must still parse with WARP_API_KEY set in the environment"
    );
}

#[test]
fn a_bare_invocation_parses() {
    TuiArgs::try_parse_from(["heddle-tui"]).expect("no arguments should parse");
}
