use std::collections::HashMap;

use chrono::{Local, Utc};
use persistence::model::{AgentConversationData, ConversationUsageMetadata};
use warp_cli::agent::Harness;
use warp_multi_agent_api as api;
use warpui::{App, EntityId, SingletonEntity};

use super::{ConversationDetailsData, PanelMode, continuation_target};
use crate::ai::agent::api::ServerConversationToken;
use crate::ai::agent::conversation::{
    AIAgentHarness, AIConversation, AIConversationId, ConversationStatus,
    ServerAIConversationMetadata,
};
use crate::ai::blocklist::BlocklistAIHistoryModel;
use crate::auth::UserUid;
use crate::cloud_object::{Revision, ServerMetadata, ServerPermissions};
use crate::server::ids::ServerId;
use crate::workspaces::user_profiles::UserProfileWithUID;

#[test]
fn continue_locally_is_offered_only_for_terminal_conversations() {
    // The CTA forks the conversation, so it must only appear once the agent is DONE with it.
    // A previous version excluded just `is_in_progress()`, which offered the fork for
    // `TransientError`, `WaitingForEvents` and `Blocked` — all non-terminal and resumable in
    // place, so forking them would branch a conversation the agent is still going to
    // continue. Guarding the rule directly here: the exhaustive `ConversationStatus::is_done`
    // enum test would NOT fail if this gate regressed to excluding only `InProgress`.
    let conversation_id = AIConversationId::new();
    let mode_with = |status: ConversationStatus| PanelMode {
        directory: None,
        server_conversation_id: None,
        ai_conversation_id: Some(conversation_id),
        status: Some(status),
    };

    for status in [
        ConversationStatus::InProgress,
        ConversationStatus::TransientError,
        ConversationStatus::WaitingForEvents,
        ConversationStatus::Blocked {
            blocked_action: "run_shell_command".to_owned(),
        },
    ] {
        assert_eq!(
            continuation_target(&mode_with(status.clone()), true),
            None,
            "{status:?} is resumable in place; forking it would branch a live conversation"
        );
    }

    for status in [
        ConversationStatus::Success,
        ConversationStatus::Error,
        ConversationStatus::Cancelled,
    ] {
        assert_eq!(
            continuation_target(&mode_with(status.clone()), true),
            Some(conversation_id),
            "{status:?} is terminal, so it is a valid fork source"
        );
    }

    // AI disabled hides the CTA regardless of status.
    assert_eq!(
        continuation_target(&mode_with(ConversationStatus::Success), false),
        None
    );
    // No status at all (nothing loaded yet) is not a fork source either.
    assert_eq!(
        continuation_target(
            &PanelMode {
                directory: None,
                server_conversation_id: None,
                ai_conversation_id: Some(conversation_id),
                status: None,
            },
            true
        ),
        None
    );
}

#[test]
fn test_from_conversation_populates_local_conversation_fields() {
    // Locks in that `ConversationDetailsData::from_conversation` surfaces the
    // conversation-derived fields the details panel renders for local Warp Agent runs
    // (APP-3595).
    //
    // REGRESSION GUARD: `ai_conversation_id` used to be `None` here, because only the
    // cloud `PanelMode::Task` arm carried an id. Removing that arm made the panel's
    // "Continue locally" action unreachable, so `from_conversation` now populates it —
    // this is the sole source. An earlier version of this test asserted `is_none()` and
    // was deleted wholesale during the ambient slices instead of being updated.
    App::test((), |mut app| async move {
        let history_model =
            app.add_singleton_model(|_| BlocklistAIHistoryModel::new(vec![], vec![], &[]));

        let conversation_id = AIConversationId::new();
        let directory = "/tmp/local-conversation-directory";
        let conversation = create_restored_conversation(
            conversation_id,
            "root-task",
            directory,
            AgentConversationData {
                server_conversation_token: None,
                conversation_usage_metadata: None,
                reverted_action_ids: None,
                forked_from_server_conversation_token: None,
                artifacts_json: None,
                parent_agent_id: None,
                agent_name: None,
                orchestration_harness_type: None,
                parent_conversation_id: None,
                run_id: None,
                autoexecute_override: None,
                last_event_sequence: None,
                is_remote_child: false,
                root_task_is_optimistic: None,
                pinned: false,
            },
        );

        history_model.update(&mut app, |model, ctx| {
            model.restore_conversations(EntityId::new(), vec![conversation], ctx);
        });

        app.update(|ctx| {
            let conversation = BlocklistAIHistoryModel::as_ref(ctx)
                .conversation(&conversation_id)
                .expect("conversation should be present");
            let data = ConversationDetailsData::from_conversation(conversation, ctx);

            let PanelMode {
                directory: panel_directory,
                server_conversation_id,
                ai_conversation_id,
                status,
            } = &data.mode;
            assert_eq!(panel_directory.as_deref(), Some(directory));
            // Restored without a server token, so there is no server-side id.
            assert!(server_conversation_id.is_none());
            // Must be populated, or "Continue locally" is unreachable.
            assert_eq!(*ai_conversation_id, Some(conversation_id));
            assert!(status.is_some());

            assert_eq!(data.title, "test query");
            assert_eq!(data.source_prompt.as_deref(), Some("test query"));
            assert!(data.credits.is_some());
        });
    });
}

#[test]
fn test_from_conversation_prefers_server_creator_profile() {
    App::test((), |mut app| async move {
        let conversation_id = AIConversationId::new();
        let mut conversation = create_restored_conversation(
            conversation_id,
            "root-task",
            "/tmp/server-creator-profile",
            AgentConversationData {
                server_conversation_token: None,
                conversation_usage_metadata: None,
                reverted_action_ids: None,
                forked_from_server_conversation_token: None,
                artifacts_json: None,
                parent_agent_id: None,
                agent_name: None,
                orchestration_harness_type: None,
                parent_conversation_id: None,
                is_remote_child: false,
                root_task_is_optimistic: None,
                run_id: None,
                autoexecute_override: None,
                last_event_sequence: None,
                pinned: false,
            },
        );
        conversation.set_server_metadata(create_test_server_metadata(
            "server-token-creator-profile",
            Some("fallback-uid-that-should-not-render".to_string()),
            Some(UserProfileWithUID {
                firebase_uid: UserUid::new("creator-profile-uid"),
                display_name: Some("ZL".to_string()),
                email: "zl@example.com".to_string(),
                photo_url: "https://example.com/zl.png".to_string(),
            }),
        ));

        app.update(|ctx| {
            let data = ConversationDetailsData::from_conversation(&conversation, ctx);
            let creator = data
                .creator
                .as_ref()
                .expect("server creator profile should be preserved");

            assert_eq!(creator.display_name, "ZL");
            assert_eq!(
                creator.photo_url.as_deref(),
                Some("https://example.com/zl.png")
            );
            assert_eq!(creator.uid.as_deref(), Some("creator-profile-uid"));
        });
    });
}

fn create_message_with_directory(id: &str, task_id: &str, directory: &str) -> api::Message {
    api::Message {
        fetched_memories: vec![],
        id: id.to_string(),
        task_id: task_id.to_string(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::UserQuery(api::message::UserQuery {
            query: "test query".to_string(),
            context: Some(api::InputContext {
                directory: Some(api::input_context::Directory {
                    pwd: directory.to_string(),
                    home: String::new(),
                    pwd_file_symbols_indexed: false,
                }),
                ..Default::default()
            }),
            referenced_attachments: HashMap::new(),
            mode: None,
            intended_agent: Default::default(),
        })),
        request_id: "request-1".to_string(),
        timestamp: None,
    }
}

fn create_agent_output_message(id: &str, task_id: &str) -> api::Message {
    api::Message {
        fetched_memories: vec![],
        id: id.to_string(),
        task_id: task_id.to_string(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::AgentOutput(
            api::message::AgentOutput {
                text: "done".to_string(),
            },
        )),
        request_id: "request-1".to_string(),
        timestamp: None,
    }
}

fn create_restored_conversation(
    conversation_id: AIConversationId,
    root_task_id: &str,
    directory: &str,
    conversation_data: AgentConversationData,
) -> AIConversation {
    let task = api::Task {
        id: root_task_id.to_string(),
        messages: vec![
            create_message_with_directory("message-1", root_task_id, directory),
            create_agent_output_message("message-2", root_task_id),
        ],
        dependencies: None,
        description: String::new(),
        summary: String::new(),
        server_data: String::new(),
    };

    AIConversation::new_restored(conversation_id, vec![task], Some(conversation_data))
        .expect("restored conversation should build")
}

fn create_test_server_metadata(
    server_token: &str,
    creator_uid: Option<String>,
    creator: Option<UserProfileWithUID>,
) -> ServerAIConversationMetadata {
    ServerAIConversationMetadata {
        title: "test conversation".to_string(),
        working_directory: None,
        harness: AIAgentHarness::Oz,
        usage: ConversationUsageMetadata {
            was_summarized: false,
            context_window_usage: 0.0,
            credits_spent: 0.0,
            platform_credits_spent: 0.0,
            credits_spent_for_last_block: None,
            token_usage: vec![],
            tool_usage_metadata: Default::default(),
            context_window_segments: Vec::new(),
        },
        metadata: ServerMetadata {
            uid: ServerId::default(),
            revision: Revision::now(),
            metadata_last_updated_ts: Utc::now().into(),
            trashed_ts: None,
            folder_id: None,
            is_welcome_object: false,
            creator_uid,
            last_editor_uid: None,
            current_editor_uid: None,
        },
        creator,
        permissions: ServerPermissions::mock_personal(),
        ambient_agent_task_id: None,
        server_conversation_token: ServerConversationToken::new(server_token.to_string()),
        artifacts: vec![],
    }
}

#[test]
fn test_from_conversation_metadata_passes_harness_through() {
    for harness in [
        None,
        Some(Harness::Oz),
        Some(Harness::Claude),
        Some(Harness::Gemini),
        Some(Harness::Unknown),
    ] {
        let data = ConversationDetailsData::from_conversation_metadata(
            AIConversationId::new(),
            "Title".to_string(),
            None,
            Utc::now().with_timezone(&Local),
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
            None,
            harness,
        );
        assert_eq!(
            data.harness, harness,
            "harness {harness:?} should pass through"
        );
    }
}
