//! Where a pane's follow-up prompt should be submitted.
//!
//! Heddle (FOSS): the ambient cloud-agent runtime is gone, so there is no cloud task cache to
//! consult and no cloud-to-cloud continuation. What remains is the routing every LOCAL surface
//! still needs — most importantly `LiveRemoteVm`, which forwards a shared-session viewer's
//! follow-up to the sharer for shared *local* sessions.

use warpui::EntityId;

use crate::ai::ambient_agents::{
    AmbientAgentTaskId, AmbientConversationStatus, conversation_output_status_from_conversation,
};
use crate::ai::blocklist::BlocklistAIHistoryModel;
use crate::terminal::TerminalModel;

/// How a follow-up prompt for this pane should be routed. Single source of truth shared by the
/// submission router (so a remote conversation never continues on the local agent) and the
/// agent input footer live-VM indicator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AIQueryRouting {
    /// Connected as a live shared-session viewer; the follow-up is forwarded to the sharer via
    /// the viewer prompt path. `is_executor` is true when this viewer may submit (read-only
    /// viewers are blocked). `ambient_agent_task_id` is set only for ambient agent shared
    /// sessions (it is `None` for a shared local session), so the footer shows the live-VM
    /// indicator only for ambient runs.
    LiveRemoteVm {
        is_executor: bool,
        ambient_agent_task_id: Option<AmbientAgentTaskId>,
    },
    /// A finished/non-resumable remote conversation. The input is non-editable; a follow-up must
    /// never run locally.
    UnconnectedReadOnly,
    /// Continues on the local machine. Covers ordinary local agent panes and local ambient
    /// sharers (e.g. `run_agents(local)` orchestration children, `/remote-control` of a local
    /// session).
    Local,
}

impl AIQueryRouting {
    pub(crate) fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }
}

/// Resolves the [`AIQueryRouting`] for a pane from its terminal model. `terminal_model` must
/// already be locked by the caller; this function does not lock it.
pub(crate) fn resolve_ai_query_routing(terminal_model: &TerminalModel) -> AIQueryRouting {
    let status = terminal_model.shared_session_status();
    let is_ambient = terminal_model.is_shared_ambient_agent_session();
    let is_transcript_viewer = terminal_model.is_conversation_transcript_viewer();
    // The ambient task this pane is associated with, if any. `None` for a shared *local* session,
    // which keeps the footer's live-VM indicator hidden for non-ambient shared sessions.
    let ambient_agent_task_id = terminal_model.ambient_agent_task_id();

    // A live shared-session viewer forwards its follow-up to the sharer via the viewer prompt path,
    // whether the shared session is an ambient run or a shared local session. `is_executor`
    // tells the submission router whether this viewer may actually submit; `ambient_agent_task_id`
    // is set only for ambient runs so the footer indicator stays hidden for shared local sessions.
    if status.is_active_viewer() {
        return AIQueryRouting::LiveRemoteVm {
            is_executor: status.is_executor(),
            ambient_agent_task_id,
        };
    }

    // Ordinary local pane (not an ambient or transcript pane), or a sharer running locally
    // (e.g. a local orchestration child, `/remote-control` of a local session): local behavior.
    if !is_ambient && !is_transcript_viewer {
        return AIQueryRouting::Local;
    }
    if status.is_active_sharer() {
        return AIQueryRouting::Local;
    }

    // Disconnected / ended ambient or transcript pane. Without an ambient task id this is a fresh
    // composing pane, a replay/loading pane, or a generic local transcript: defer to local
    // handling. With one, the run is remote and non-resumable here — never local.
    if ambient_agent_task_id.is_none() {
        return AIQueryRouting::Local;
    }
    AIQueryRouting::UnconnectedReadOnly
}

/// Whether the pane's conversation errored out before any run was created, which is the one case
/// where a pane with no task still deserves an "ended" tombstone.
pub(in crate::terminal::view) fn conversation_failed_before_task_creation(
    terminal_view_id: EntityId,
    history_model: &BlocklistAIHistoryModel,
) -> bool {
    if history_model.is_terminal_surface_conversation_transcript_viewer(terminal_view_id) {
        return false;
    }
    history_model
        .all_live_conversations_for_terminal_surface(terminal_view_id)
        .next()
        .and_then(conversation_output_status_from_conversation)
        .is_some_and(|status| matches!(status, AmbientConversationStatus::Error { .. }))
}

#[cfg(test)]
#[path = "ai_query_routing_tests.rs"]
mod tests;
