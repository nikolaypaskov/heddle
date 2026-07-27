use rudder_message::Track;
use virtual_fs::VirtualFS;

use super::*;

/// Nothing is persisted, and no queue file is left behind.
///
/// Upstream this function wrote the pending event batch to disk so it could be flushed on
/// the next launch, filtering out events containing user-generated content. This build has
/// no telemetry destination, so that flush can never happen -- the file would only ever
/// grow. `flush_and_persist_events_at_path` therefore discards the batch and removes any
/// file a previous build left.
///
/// This test previously asserted upstream's behaviour (open the file, expect exactly the
/// non-UGC event). It was failing after that change and the failure was NOT noticed,
/// because `cargo test` reported it among a dozen genuine test-isolation failures that
/// were dismissed as pre-existing. Running each test in its own process separates the
/// two: the isolation failures disappear and this one stands alone.
#[test]
fn test_persist_events_writes_nothing_and_leaves_no_file() {
    let telemetry_api = TelemetryApi::new();

    VirtualFS::test(
        "test_persist_events_writes_nothing_and_leaves_no_file",
        |dirs, _sandbox| {
            let user_id = Some("user".into());
            let anonymous_id = "anonymous_id".to_owned();

            // One event with user-generated content and one without: neither should reach
            // disk, so the UGC distinction no longer changes the outcome.
            warpui::telemetry::record_event(
                user_id.clone(),
                anonymous_id.clone(),
                "non UGC event name".into(),
                None,  /* payload */
                false, /* contains_ugc  */
                warpui::time::get_current_time(),
            );

            warpui::telemetry::record_event(
                user_id.clone(),
                anonymous_id.clone(),
                "UGC event name".into(),
                None, /* payload */
                true, /* contains_ugc  */
                warpui::time::get_current_time(),
            );

            let file_path = dirs.root().join("rudderstack");

            telemetry_api
                .flush_and_persist_events_at_path(10, PrivacySettingsSnapshot::mock(), &file_path)
                .expect("persisting should succeed even though it writes nothing");

            assert!(
                !file_path.exists(),
                "a telemetry queue file was written to {}. This build transmits no \
                 analytics, so a persisted queue can never be flushed and would grow \
                 without bound.",
                file_path.display()
            );
        },
    );
}

/// And a file left by an earlier build is removed rather than inherited.
#[test]
fn test_persist_events_removes_a_pre_existing_queue_file() {
    let telemetry_api = TelemetryApi::new();

    VirtualFS::test(
        "test_persist_events_removes_a_pre_existing_queue_file",
        |dirs, _sandbox| {
            let file_path = dirs.root().join("rudderstack");
            std::fs::write(&file_path, b"[]").expect("failed to plant a queue file");
            assert!(file_path.exists(), "planted file should exist before the call");

            telemetry_api
                .flush_and_persist_events_at_path(10, PrivacySettingsSnapshot::mock(), &file_path)
                .expect("persisting should succeed");

            assert!(
                !file_path.exists(),
                "a queue file left by an earlier build survived. Anyone upgrading from a \
                 build that persisted events keeps that file forever otherwise."
            );
        },
    );
}

impl RudderBatchMessage {
    fn unwrap_track(&self) -> &Track {
        match self {
            RudderBatchMessage::Track(track) => track,
            _ => panic!("Expected a track event"),
        }
    }
}

/// No telemetry destination is configured in this build, and the send path must refuse on that
/// basis rather than relying on the HTTP client to fail.
///
/// Running the shipped v0.2.0 app produced these lines in heddle.log:
///
///   [INFO] Start to send telemetry events to RudderStack
///   [INFO] Failed to flush events from Telemetry queue: builder error
///
/// Nothing was transmitted, but only because `telemetry_config: None` leaves `root_url` empty,
/// so the POST target became the relative string "/v1/batch" and reqwest could not build a
/// request from it. A privacy guarantee resting on someone else's URL parser is the wrong shape,
/// and the log line reads like telemetry being sent.
#[test]
fn no_rudderstack_destination_is_configured_in_this_build() {
    use warp_core::channel::ChannelState;

    assert!(
        ChannelState::rudderstack_ugc_destination().root_url.is_empty(),
        "the oss channel must configure no UGC telemetry destination"
    );
    assert!(
        ChannelState::rudderstack_non_ugc_destination()
            .root_url
            .is_empty(),
        "the oss channel must configure no telemetry destination"
    );
}
