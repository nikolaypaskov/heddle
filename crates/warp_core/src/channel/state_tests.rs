//! Unit tests for `derive_http_origin_from_ws_url`.
//!
//! These ran ZERO times until the `not(feature = "test-util")` guard on this module was
//! fixed — building `warp_core` alongside `warp` turns that feature on, which compiled the
//! whole module away. See the note on the function in `state.rs`.
//!
//! KNOWN LIMIT, so nobody reads more into a green run than is there: they call the helper
//! DIRECTLY. Under `test-util` — which is exactly the configuration the gate and CI build —
//! `ChannelState::rtc_http_url` returns a mock and never invokes it (`state.rs:298`). So all
//! three would still pass if the production path stopped calling the helper altogether.
//! They cover the derivation, not its wiring; a test that the caller still uses it would
//! need to run without `test-util`, which no tier currently does.

use super::derive_http_origin_from_ws_url;

#[test]
fn wss_becomes_https_and_strips_path() {
    let got = derive_http_origin_from_ws_url("wss://rtc.app.warp.dev/graphql/v2");
    assert_eq!(got.as_deref(), Some("https://rtc.app.warp.dev"));
}

#[test]
fn ws_becomes_http_and_preserves_port() {
    let got = derive_http_origin_from_ws_url("ws://localhost:8080/graphql/v2");
    assert_eq!(got.as_deref(), Some("http://localhost:8080"));
}

#[test]
fn unparseable_input_returns_none() {
    assert!(derive_http_origin_from_ws_url("not a url").is_none());
    assert!(derive_http_origin_from_ws_url("https://app.warp.dev").is_none());
}
