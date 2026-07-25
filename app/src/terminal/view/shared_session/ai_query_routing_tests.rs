//! Routing invariants for shared sessions.
//!
//! These four cases lived in `cloud_conversation_continuation_tests.rs` and were deleted
//! wholesale when that module went, even though [`resolve_ai_query_routing`] survived the
//! ambient removal and is still consumed by the submission router and the agent input
//! footer. Three of the four are about purely LOCAL panes and shared *local* sessions, so
//! losing them removed coverage of behaviour the fork depends on.

use warpui::App;

use super::{AIQueryRouting, resolve_ai_query_routing};
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::terminal::TerminalModel;
use crate::terminal::shared_session::{SharedSessionSource, SharedSessionStatus};

fn ambient_task_id(index: usize) -> AmbientAgentTaskId {
    format!("550e8400-e29b-41d4-a716-{index:012}")
        .parse()
        .unwrap()
}

fn ambient_pane_model(task_id: AmbientAgentTaskId, status: SharedSessionStatus) -> TerminalModel {
    let mut model = TerminalModel::mock(None, None);
    model.set_shared_session_source(SharedSessionSource::ambient_agent(Some(
        task_id.to_string(),
    )));
    model.set_shared_session_status(status);
    model
}

#[test]
fn routing_is_local_for_non_cloud_pane() {
    // The ordinary case: a plain local terminal pane submits locally.
    App::test((), |mut app| async move {
        let model = TerminalModel::mock(None, None);
        app.update(|_ctx| {
            assert_eq!(resolve_ai_query_routing(&model), AIQueryRouting::Local);
        });
    });
}

#[test]
fn routing_is_live_remote_vm_for_active_viewer() {
    App::test((), |mut app| async move {
        let model = ambient_pane_model(ambient_task_id(1), SharedSessionStatus::reader());
        app.update(|_ctx| {
            assert_eq!(
                resolve_ai_query_routing(&model),
                AIQueryRouting::LiveRemoteVm {
                    is_executor: false,
                    ambient_agent_task_id: Some(ambient_task_id(1)),
                }
            );
        });
    });
}

#[test]
fn routing_omits_task_id_for_non_ambient_shared_session_viewer() {
    // A viewer of a shared *local* session (no ambient task) still forwards to the sharer,
    // but carries no ambient task id, so the footer live-VM indicator stays hidden. This is
    // the invariant Codex flagged in slice 4c as the reason `AIQueryRouting` must not
    // collapse to `Local`.
    App::test((), |mut app| async move {
        let mut model = TerminalModel::mock(None, None);
        model.set_shared_session_status(SharedSessionStatus::executor());
        app.update(|_ctx| {
            assert_eq!(
                resolve_ai_query_routing(&model),
                AIQueryRouting::LiveRemoteVm {
                    is_executor: true,
                    ambient_agent_task_id: None,
                }
            );
        });
    });
}

#[test]
fn routing_is_local_for_active_sharer_local_orchestration_child() {
    // A local orchestration child that is SHARING its session runs locally -- it must not be
    // routed to a remote executor just because it carries an orchestrator task id.
    App::test((), |mut app| async move {
        let model = ambient_pane_model(ambient_task_id(1), SharedSessionStatus::ActiveSharer);
        app.update(|_ctx| {
            assert_eq!(resolve_ai_query_routing(&model), AIQueryRouting::Local);
        });
    });
}
