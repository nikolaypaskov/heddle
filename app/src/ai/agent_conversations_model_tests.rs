use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use parking_lot::Mutex;
use persistence::model::ConversationUsageMetadata;
use warp_cli::agent::Harness;
use warp_core::features::FeatureFlag;
use warpui::{App, EntityId, ModelHandle, SingletonEntity};

use super::entry::{
    AgentConversationEntryId, AgentConversationNavigationSubject, AgentConversationProvenance,
};
use super::{
    AgentConversationsModel, AgentConversationsModelEvent, AgentManagementFilters, ArtifactFilter,
    ConversationMetadata, ConversationUpdateKind, HarnessFilter, InitialConversationLoadState,
    OwnerFilter, StatusFilter,
};
use crate::ai::active_agent_views_model::ActiveAgentViewsModel;
use crate::ai::agent::api::ServerConversationToken;
use crate::ai::agent::conversation::{
    AIAgentHarness, AIConversation, AIConversationId, ConversationStatus,
    ServerAIConversationMetadata,
};
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::ai::artifacts::Artifact;
use crate::ai::blocklist::history_model::{
    BlocklistAIHistoryEvent, BlocklistAIHistoryModel, ConversationStatusUpdate,
};
use crate::ai::conversation_navigation::ConversationNavigationData;
use crate::auth::AuthStateProvider;
use crate::cloud_object::{Owner, Revision, ServerMetadata, ServerPermissions};
use crate::server::ids::ServerId;
use crate::workspace::{WorkspaceAction, WorkspaceRegistry};

type CapturedConversationUpdate = Mutex<Option<ConversationUpdateKind>>;

/// Test-only handler that mirrors the production view subscription: extracts the
/// `ConversationUpdated` payload and stashes it on a shared cell that test cases assert
/// against.
fn create_test_model() -> AgentConversationsModel {
    AgentConversationsModel {
        conversations: HashMap::new(),
        active_data_consumers_per_window: HashMap::new(),
        initial_load_state: InitialConversationLoadState::LoadingLocal,
    }
}

fn handle_agent_conversation_model_event(
    captured: &CapturedConversationUpdate,
    event: &AgentConversationsModelEvent,
) {
    if let AgentConversationsModelEvent::ConversationUpdated { kind } = event {
        *captured.lock() = Some(*kind);
    }
}

/// Subscribes a [`handle_agent_conversation_model_event`] capture cell to `model` and
/// returns the cell so individual cases can assert on the most recent emission without
/// re-implementing the subscription bookkeeping.
fn subscribe_to_conversation_updated(
    app: &mut App,
    model: &ModelHandle<AgentConversationsModel>,
) -> Arc<CapturedConversationUpdate> {
    let captured = Arc::new(Mutex::new(None));
    let captured_clone = captured.clone();
    app.update(|ctx| {
        ctx.subscribe_to_model(model, move |_, event, _| {
            handle_agent_conversation_model_event(&captured_clone, event);
        });
    });
    captured
}

fn create_server_conversation_metadata(
    title: &str,
    server_token: &str,
    ambient_agent_task_id: Option<AmbientAgentTaskId>,
) -> ServerAIConversationMetadata {
    ServerAIConversationMetadata {
        title: title.to_string(),
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
        metadata: mock_server_metadata(),
        creator: None,
        permissions: mock_server_permissions(),
        ambient_agent_task_id,
        server_conversation_token: ServerConversationToken::new(server_token.to_string()),
        artifacts: Vec::new(),
    }
}

#[test]
fn test_restored_conversation_emits_restored_kind() {
    App::test((), |mut app| async move {
        let _interactive_management_guard =
            FeatureFlag::InteractiveConversationManagementView.override_enabled(true);
        let agent_model = app.add_singleton_model(|_| create_test_model());
        let captured = subscribe_to_conversation_updated(&mut app, &agent_model);

        agent_model.update(&mut app, |model, ctx| {
            model.handle_history_event(
                &BlocklistAIHistoryEvent::UpdatedConversationStatus {
                    conversation_id: AIConversationId::new(),
                    terminal_surface_id: EntityId::new(),
                    update: ConversationStatusUpdate::Restored,
                    new_status: ConversationStatus::Success,
                },
                ctx,
            );
        });

        let captured = *captured.lock();
        assert_eq!(captured, Some(ConversationUpdateKind::Restored));
    });
}

#[test]
fn test_status_transition_emits_status_set_with_filter_buckets() {
    App::test((), |mut app| async move {
        let _interactive_management_guard =
            FeatureFlag::InteractiveConversationManagementView.override_enabled(true);
        let agent_model = app.add_singleton_model(|_| create_test_model());
        let captured = subscribe_to_conversation_updated(&mut app, &agent_model);

        agent_model.update(&mut app, |model, ctx| {
            model.handle_history_event(
                &BlocklistAIHistoryEvent::UpdatedConversationStatus {
                    conversation_id: AIConversationId::new(),
                    terminal_surface_id: EntityId::new(),
                    update: ConversationStatusUpdate::Changed {
                        prev_status: ConversationStatus::InProgress,
                    },
                    new_status: ConversationStatus::Success,
                },
                ctx,
            );
        });

        let captured = *captured.lock();
        assert_eq!(
            captured,
            Some(ConversationUpdateKind::StatusSet {
                prev_filter: StatusFilter::Working,
                new_filter: StatusFilter::Done,
            }),
        );
    });
}

#[test]
fn test_same_bucket_re_emission_emits_status_set_with_equal_filters() {
    App::test((), |mut app| async move {
        let _interactive_management_guard =
            FeatureFlag::InteractiveConversationManagementView.override_enabled(true);
        let agent_model = app.add_singleton_model(|_| create_test_model());
        let captured = subscribe_to_conversation_updated(&mut app, &agent_model);

        agent_model.update(&mut app, |model, ctx| {
            model.handle_history_event(
                &BlocklistAIHistoryEvent::UpdatedConversationStatus {
                    conversation_id: AIConversationId::new(),
                    terminal_surface_id: EntityId::new(),
                    update: ConversationStatusUpdate::Changed {
                        prev_status: ConversationStatus::InProgress,
                    },
                    new_status: ConversationStatus::InProgress,
                },
                ctx,
            );
        });

        let captured = *captured.lock();
        assert_eq!(
            captured,
            Some(ConversationUpdateKind::StatusSet {
                prev_filter: StatusFilter::Working,
                new_filter: StatusFilter::Working,
            }),
        );
    });
}

fn create_test_conversation_metadata(
    conversation_id: AIConversationId,
    title: &str,
) -> ConversationMetadata {
    ConversationMetadata {
        nav_data: ConversationNavigationData {
            id: conversation_id,
            title: title.to_string(),
            initial_query: None,
            last_updated: chrono::Local::now(),
            terminal_view_id: None,
            window_id: None,
            pane_view_locator: None,
            initial_working_directory: None,
            latest_working_directory: None,
            is_selected: false,
            is_in_active_pane: false,
            is_closed: false,
            server_conversation_token: None,
        },
    }
}

fn all_owner_filters() -> AgentManagementFilters {
    AgentManagementFilters {
        owners: OwnerFilter::All,
        ..Default::default()
    }
}

fn add_entry_projection_test_models(app: &mut App) {
    app.add_singleton_model(|_| AuthStateProvider::new_for_test());
    app.add_singleton_model(|_| BlocklistAIHistoryModel::new(vec![], vec![], &[]));
    app.add_singleton_model(|_| ActiveAgentViewsModel::new());
    app.add_singleton_model(|_| WorkspaceRegistry::new());
}

fn mock_server_metadata() -> ServerMetadata {
    ServerMetadata {
        uid: ServerId::default(),
        revision: Revision::now(),
        metadata_last_updated_ts: Utc::now().into(),
        trashed_ts: None,
        folder_id: None,
        is_welcome_object: false,
        creator_uid: None,
        last_editor_uid: None,
        current_editor_uid: None,
    }
}

fn mock_server_permissions() -> ServerPermissions {
    ServerPermissions {
        space: Owner::mock_current_user(),
        guests: Vec::new(),
        anyone_link_sharing: None,
        permissions_last_updated_ts: Utc::now().into(),
    }
}

#[test]
fn test_get_entries_includes_local_only_entry() {
    App::test((), |mut app| async move {
        add_entry_projection_test_models(&mut app);

        let conversation_id = AIConversationId::new();
        let mut model = create_test_model();
        model.conversations.insert(
            conversation_id,
            create_test_conversation_metadata(conversation_id, "Local conversation"),
        );

        app.update(|ctx| {
            let entries = model.get_entries(&all_owner_filters(), ctx);

            assert_eq!(entries.len(), 1);
            let entry = &entries[0];
            assert_eq!(
                entry.id,
                AgentConversationEntryId::Conversation(conversation_id)
            );
            assert_eq!(entry.identity.local_conversation_id, Some(conversation_id));
            assert_eq!(entry.identity.ambient_agent_task_id, None);
            assert_eq!(
                entry.provenance,
                AgentConversationProvenance::LocalInteractive
            );
            assert_eq!(entry.display.title, "Local conversation");
        });
    });
}

#[test]
fn test_conversation_metadata_child_predicate_matches_conversation() {
    use crate::ai::blocklist::history_model::AIConversationMetadata;

    // Non-child conversation: neither representation reports a child.
    let plain = AIConversation::new(false, false);
    let plain_metadata = AIConversationMetadata::from(&plain);
    assert!(!plain.is_child_agent_conversation());
    assert_eq!(
        plain_metadata.is_child_agent_conversation(),
        plain.is_child_agent_conversation()
    );

    // Child conversation: the metadata predicate matches the conversation's.
    let mut child = AIConversation::new(false, false);
    child.set_parent_conversation_id(AIConversationId::new());
    let child_metadata = AIConversationMetadata::from(&child);
    assert!(child.is_child_agent_conversation());
    assert_eq!(
        child_metadata.is_child_agent_conversation(),
        child.is_child_agent_conversation()
    );
}

#[test]
fn test_get_entries_includes_cloud_metadata_only_entry() {
    App::test((), |mut app| async move {
        let token = "cloud-token-only";
        add_entry_projection_test_models(&mut app);
        BlocklistAIHistoryModel::handle(&app).update(&mut app, |model, _| {
            model.merge_cloud_conversation_metadata(vec![create_server_conversation_metadata(
                "Cloud conversation",
                token,
                None,
            )]);
        });

        let model = create_test_model();

        app.update(|ctx| {
            let entries = model.get_entries(&all_owner_filters(), ctx);

            assert_eq!(entries.len(), 1);
            let entry = &entries[0];
            assert_eq!(
                entry
                    .identity
                    .server_conversation_token
                    .as_ref()
                    .map(|t| t.as_str()),
                Some(token)
            );
            assert_eq!(
                entry.provenance,
                AgentConversationProvenance::CloudSyncedConversation
            );
            assert!(entry.backing.has_cloud_data);
            assert!(!entry.backing.has_loaded_conversation);
            assert!(!entry.backing.has_local_persisted_data);
        });
    });
}

#[test]
fn test_resolve_open_action_handles_server_token_subject_without_entry() {
    App::test((), |mut app| async move {
        add_entry_projection_test_models(&mut app);
        app.add_singleton_model(|_| create_test_model());

        let server_token = ServerConversationToken::new("server-token-subject".to_string());
        app.update(|ctx| {
            let action = AgentConversationsModel::resolve_open_action(
                AgentConversationNavigationSubject::ServerToken(server_token.clone()),
                None,
                ctx,
            );

            assert!(matches!(
                action,
                Some(WorkspaceAction::OpenConversationTranscriptViewer {
                    conversation_id,
                }) if conversation_id == server_token
            ));
        });
    });
}

#[test]
fn test_resolve_open_action_opens_metadata_only_cloud_conversation_by_server_token() {
    App::test((), |mut app| async move {
        let token = "metadata-only-token";
        add_entry_projection_test_models(&mut app);
        BlocklistAIHistoryModel::handle(&app).update(&mut app, |model, _| {
            model.merge_cloud_conversation_metadata(vec![create_server_conversation_metadata(
                "Cloud conversation",
                token,
                None,
            )]);
        });
        app.add_singleton_model(|_| create_test_model());

        app.update(|ctx| {
            let entries =
                AgentConversationsModel::as_ref(ctx).get_entries(&all_owner_filters(), ctx);
            let entry = entries
                .iter()
                .find(|entry| {
                    entry
                        .identity
                        .server_conversation_token
                        .as_ref()
                        .is_some_and(|server_token| server_token.as_str() == token)
                })
                .expect("metadata-only cloud entry should exist");

            assert!(entry.backing.has_cloud_data);
            assert!(!entry.backing.has_loaded_conversation);
            assert!(!entry.backing.has_local_persisted_data);

            let action = AgentConversationsModel::resolve_open_action(
                AgentConversationNavigationSubject::Entry(entry.id),
                None,
                ctx,
            );

            assert!(matches!(
                action,
                Some(WorkspaceAction::OpenConversationTranscriptViewer {
                    conversation_id,
                }) if conversation_id.as_str() == token
            ));
        });
    });
}

#[test]
fn test_file_artifact_filter_matches_only_items_with_file_artifacts() {
    let artifacts_with_file = vec![Artifact::File {
        artifact_uid: "artifact-file-1".to_string(),
        filepath: "outputs/report.txt".to_string(),
        filename: "report.txt".to_string(),
        mime_type: "text/plain".to_string(),
        description: Some("Daily summary".to_string()),
        size_bytes: Some(42),
    }];
    let artifacts_with_pr = vec![Artifact::PullRequest {
        url: "https://github.com/org/repo/pull/1".to_string(),
        branch: "main".to_string(),
        repo: Some("repo".to_string()),
        number: Some(1),
    }];

    assert!(super::artifacts_match_filter(
        &artifacts_with_file,
        &ArtifactFilter::File,
    ));
    assert!(!super::artifacts_match_filter(
        &artifacts_with_pr,
        &ArtifactFilter::File,
    ));
    assert!(super::artifacts_match_filter(
        &artifacts_with_file,
        &ArtifactFilter::All,
    ));
}

#[test]
fn test_harness_filter_is_filtering_and_reset() {
    // Default is All → not filtering, and after toggling reset_all_but_owner returns to default.
    let mut filters = AgentManagementFilters::default();
    assert!(!filters.is_filtering());

    filters.harness = HarnessFilter::Specific(Harness::Claude);
    assert!(
        filters.is_filtering(),
        "harness != All should report filtering"
    );

    filters.reset_all_but_owner();
    assert_eq!(filters.harness, HarnessFilter::default());
    assert!(!filters.is_filtering());
}

#[test]
fn test_agent_management_filters_serde_backwards_compat() {
    // Persisted state from older clients has no `harness` key → deserializes to All.
    let legacy = r#"{
        "owners": "PersonalOnly",
        "status": "All",
        "source": "All",
        "created_on": "All",
        "creator": "All",
        "artifact": "All"
    }"#;
    let decoded: AgentManagementFilters =
        serde_json::from_str(legacy).expect("legacy payload without harness must deserialize");
    assert_eq!(decoded.harness, HarnessFilter::All);

    // Round trip a Specific(Claude) value.
    let original = AgentManagementFilters {
        harness: HarnessFilter::Specific(Harness::Claude),
        ..Default::default()
    };
    let encoded = serde_json::to_string(&original).unwrap();
    assert!(
        encoded.contains("\"harness\":\"claude\""),
        "expected serialized form to contain \"harness\":\"claude\", got {encoded}"
    );
    let decoded: AgentManagementFilters = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, original);

    // Unknown harness strings deserialize to All (forward compat).
    let forward = r#"{
        "owners": "PersonalOnly",
        "status": "All",
        "source": "All",
        "created_on": "All",
        "creator": "All",
        "artifact": "All",
        "harness": "some-future-harness"
    }"#;
    let decoded: AgentManagementFilters = serde_json::from_str(forward).unwrap();
    assert_eq!(decoded.harness, HarnessFilter::All);
}
