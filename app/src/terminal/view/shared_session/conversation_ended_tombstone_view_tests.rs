//! Which conversation a tombstone is about.
//!
//! A terminal surface can hold several live conversations, and
//! `all_live_conversations_for_terminal_surface` returns them OLDEST FIRST. Taking
//! `.next()` therefore picks the wrong one — the card would describe conversation A while
//! "Continue locally" forked conversation B, and on WASM "Open in Warp" would open a third
//! thing. These lock the resolution order down.

use warp_multi_agent_api as api;
use warpui::{App, EntityId, SingletonEntity};

use super::tombstone_conversation_id;
use crate::ai::agent::conversation::{AIConversation, AIConversationId};
use crate::ai::blocklist::BlocklistAIHistoryModel;

/// A conversation needs a root task to restore (`NoRootTask` otherwise), so give it one
/// agent-output message. Nothing here reads the contents; only identity and liveness matter.
fn restored_conversation(conversation_id: AIConversationId) -> AIConversation {
    let task_id = format!("task-{conversation_id}");
    let task = api::Task {
        id: task_id.clone(),
        messages: vec![api::Message {
            fetched_memories: vec![],
            id: format!("message-{conversation_id}"),
            task_id,
            server_message_data: String::new(),
            citations: vec![],
            message: Some(api::message::Message::AgentOutput(
                api::message::AgentOutput {
                    text: "done".to_string(),
                },
            )),
            request_id: "request-1".to_string(),
            timestamp: None,
        }],
        dependencies: None,
        description: String::new(),
        summary: String::new(),
        server_data: String::new(),
    };
    AIConversation::new_restored(conversation_id, vec![task], None)
        .expect("restored conversation should build")
}

#[test]
fn tombstone_prefers_the_active_conversation_over_the_oldest() {
    App::test((), |mut app| async move {
        let history =
            app.add_singleton_model(|_| BlocklistAIHistoryModel::new(vec![], vec![], &[]));
        let surface = EntityId::new();

        // Three conversations, and the MIDDLE one is active. That makes every candidate
        // rule give a different answer, so the test discriminates: `.next()` (the original
        // bug) yields `oldest`, `.last()` yields `newest`, and only reading the active
        // conversation yields `middle`.
        let oldest = AIConversationId::new();
        let middle = AIConversationId::new();
        let newest = AIConversationId::new();

        history.update(&mut app, |model, ctx| {
            model.restore_conversations(
                surface,
                vec![
                    restored_conversation(oldest),
                    restored_conversation(middle),
                    restored_conversation(newest),
                ],
                ctx,
            );
            model.set_active_conversation_id(middle, surface, ctx);
        });

        app.update(|ctx| {
            let history = BlocklistAIHistoryModel::as_ref(ctx);
            let resolved = tombstone_conversation_id(surface, history);
            assert_eq!(
                resolved,
                Some(middle),
                "the ACTIVE conversation wins over both the oldest and the newest"
            );
            assert_ne!(resolved, Some(oldest), "must not be the oldest-first pick");
            assert_ne!(resolved, Some(newest), "must not be the recency fallback");
        });
    });
}

#[test]
fn tombstone_falls_back_to_the_newest_when_none_is_active() {
    App::test((), |mut app| async move {
        let history =
            app.add_singleton_model(|_| BlocklistAIHistoryModel::new(vec![], vec![], &[]));
        let surface = EntityId::new();

        let oldest = AIConversationId::new();
        let newest = AIConversationId::new();

        history.update(&mut app, |model, ctx| {
            model.restore_conversations(
                surface,
                vec![restored_conversation(oldest), restored_conversation(newest)],
                ctx,
            );
        });

        app.update(|ctx| {
            let history = BlocklistAIHistoryModel::as_ref(ctx);
            assert_eq!(
                tombstone_conversation_id(surface, history),
                Some(newest),
                "with no active conversation the MOST RECENT wins, never the oldest"
            );
        });
    });
}

#[test]
fn tombstone_has_no_conversation_for_an_empty_surface() {
    App::test((), |mut app| async move {
        app.add_singleton_model(|_| BlocklistAIHistoryModel::new(vec![], vec![], &[]));
        app.update(|ctx| {
            let history = BlocklistAIHistoryModel::as_ref(ctx);
            assert_eq!(tombstone_conversation_id(EntityId::new(), history), None);
        });
    });
}
