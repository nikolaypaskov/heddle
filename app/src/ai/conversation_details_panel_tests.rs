use std::collections::HashMap;

use chrono::{Local, Utc};
use persistence::model::{AgentConversationData, ConversationUsageMetadata};
use warp_cli::agent::Harness;
use warp_multi_agent_api as api;
use warpui::App;

use super::ConversationDetailsData;
use crate::ai::agent::api::ServerConversationToken;
use crate::ai::agent::conversation::{
    AIAgentHarness, AIConversation, AIConversationId, ServerAIConversationMetadata,
};
use crate::auth::UserUid;
use crate::cloud_object::{Revision, ServerMetadata, ServerPermissions};
use crate::server::ids::ServerId;
use crate::workspaces::user_profiles::UserProfileWithUID;

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
