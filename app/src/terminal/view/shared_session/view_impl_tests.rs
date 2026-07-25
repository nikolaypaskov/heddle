use std::collections::HashMap;

use pathfinder_geometry::vector::vec2f;
use session_sharing_protocol::sharer::SessionSourceType;
use warp_multi_agent_api::{self as api, client_action as api_client_action};
use warpui::platform::WindowStyle;
use warpui::{App, ViewHandle};

use super::*;
use crate::ai::agent::AIAgentInput;
use crate::ai::blocklist::history_model::BlocklistAIHistoryModel;
use crate::context_chips::prompt_type::PromptType;
use crate::terminal::TerminalView;
use crate::terminal::model::blocks::{INLINE_BANNER_HEIGHT, ToTotalIndex as _};
use crate::terminal::view::TerminalAction;
use crate::terminal::view::shared_session::test_utils::terminal_view_for_viewer;
use crate::test_util::add_window_with_terminal;
use crate::test_util::terminal::initialize_app_for_terminal_view;
use crate::{FeatureFlag, assert_lines_approx_eq};

#[test]
fn suppressed_auto_open_survives_the_first_maybe_auto_open() {
    // Local orchestration restores hidden child panes and calls
    // `suppress_initial_conversation_details_panel_auto_open()` on them
    // (`pane_group/child_agent/restoration.rs`) so a child does not pop its details panel.
    // The one-shot must consume without opening, and a later manual toggle must still work.
    // The original coverage lived in tests keyed on the deleted ambient child-pane helper
    // and went with it, though the suppression policy itself is live and local.
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let tips_model = app.add_model(|_| Default::default());
        let (_, terminal) = app.add_window(WindowStyle::NotStealFocus, |ctx| {
            TerminalView::new_for_test(tips_model, None, ctx)
        });

        terminal.update(&mut app, |view, _| {
            assert!(
                !view.is_initial_conversation_details_panel_auto_open_suppressed_for_test(),
                "default policy should allow auto-open"
            );
            view.suppress_initial_conversation_details_panel_auto_open();
            assert!(view.is_initial_conversation_details_panel_auto_open_suppressed_for_test());
        });

        terminal.update(&mut app, |view, ctx| {
            view.maybe_auto_open_conversation_details_panel(ctx);
        });

        terminal.read(&app, |view, _| {
            assert!(
                !view.is_conversation_details_panel_open,
                "a suppressed pane must not auto-open its details panel"
            );
            assert!(
                view.has_auto_opened_conversation_details_panel,
                "the one-shot must still be consumed, so a later call cannot open it either"
            );
        });
    });
}

#[test]
fn test_prompt_context_menu_items_shared_session_viewer_no_edit_prompt() {
    App::test((), |mut app| async move {
        let terminal = terminal_view_for_viewer(&mut app);

        terminal.update(&mut app, |view, ctx| {
            let mut model = view.model.lock();
            view.current_prompt.update(ctx, |prompt, ctx| {
                model.set_shared_session_status(SharedSessionStatus::ActiveViewer {
                    role: Default::default(),
                });

                let PromptType::Dynamic { prompt } = prompt else {
                    return;
                };
                prompt.update(ctx, |prompt, ctx| {
                    prompt.update_context(model.block_list().active_block(), ctx)
                });
            })
        });

        let session_settings = SessionSettings::handle(&app);
        session_settings.update(&mut app, |settings, ctx| {
            let _ = settings.honor_ps1.set_value(false, ctx);
        });

        terminal.read(&app, |view, ctx| {
            let items: Vec<MenuItem<TerminalAction>> = view.prompt_context_menu_items(ctx);
            assert_eq!(items.len(), 3);

            // We expect the prompt menu items to be something like the following when no context chips exist:
            // Copy prompt
            // ------------
            // Edit prompt (disabled for shared-session viewers)
            assert_eq!(items[0].fields().unwrap().label(), "Copy prompt");
            assert!(items[1].is_separator());
            assert_eq!(items[2].fields().unwrap().label(), "Edit prompt");
            assert!(items[2].fields().unwrap().is_disabled());
        });
    })
}

#[test]
fn test_shared_session_banners() {
    let _flag = FeatureFlag::CreatingSharedSessions.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);

        let terminal = add_window_with_terminal(&mut app, None);
        let mut expected_block_heights_len = terminal.read(&app, |view, _| {
            assert!(matches!(
                view.inline_banners_state.shared_session_banner_state,
                SharedSessionBanners::None
            ));
            view.model.lock().block_list().block_heights().items().len()
        });

        // Make a block and then insert the shared session starter banner.
        terminal.update(&mut app, |view, ctx| {
            view.model.lock().simulate_block("ls", "foo");
            view.insert_shared_session_started_banner(
                SharedSessionScrollbackType::All,
                false,
                Local::now(),
                ctx,
            );
            expected_block_heights_len += 2;
        });

        terminal.read(&app, |view, _ctx| {
            let model = view.model.lock();

            // Make sure the state has changed.
            assert!(matches!(
                view.inline_banners_state.shared_session_banner_state,
                SharedSessionBanners::ActiveShare { .. }
            ));

            // We should have inserted a block and a banner.
            let block_height_items = model.block_list().block_heights().items();
            assert_eq!(block_height_items.len(), expected_block_heights_len);

            // The banner should have been inserted before the first visible block.
            let first_block_total_index = model
                .block_list()
                .first_non_hidden_block_by_index()
                .unwrap()
                .to_total_index(model.block_list());
            assert_lines_approx_eq!(
                block_height_items[first_block_total_index.0 - 1]
                    .height()
                    .into_lines(),
                INLINE_BANNER_HEIGHT
            );
        });

        // Insert another block and then the shared session ended banner.
        terminal.update(&mut app, |view, ctx| {
            view.model.lock().simulate_block("ls", "foo");
            view.insert_shared_session_ended_banner(ctx);
            expected_block_heights_len += 2;
        });

        terminal.read(&app, |view, _ctx| {
            let model = view.model.lock();

            // Make sure the state has changed.
            assert!(matches!(
                view.inline_banners_state.shared_session_banner_state,
                SharedSessionBanners::LastShared { .. }
            ));

            // by now, we've inserted two new blocks and two new banners since the initialization of the view.
            let block_height_items = model.block_list().block_heights().items();
            assert_eq!(block_height_items.len(), expected_block_heights_len);

            // The first banner should continue to be at the start of the blocklist.
            let first_block_total_index = model
                .block_list()
                .first_non_hidden_block_by_index()
                .unwrap()
                .to_total_index(model.block_list());
            assert_lines_approx_eq!(
                block_height_items[first_block_total_index.0 - 1]
                    .height()
                    .into_lines(),
                INLINE_BANNER_HEIGHT
            );

            // The second banner should be at the end of the blocklist, before the active block.
            let last_block_total_index = model
                .block_list()
                .last_non_hidden_block_by_index()
                .unwrap()
                .to_total_index(model.block_list());
            assert_lines_approx_eq!(
                block_height_items[last_block_total_index.0 + 1]
                    .height()
                    .into_lines(),
                INLINE_BANNER_HEIGHT
            );
        });

        // Mimic starting a shared session again in the same view.
        terminal.update(&mut app, |view, ctx| {
            view.insert_shared_session_started_banner(
                SharedSessionScrollbackType::None,
                false,
                Local::now(),
                ctx,
            );

            // We should have removed two banners and inserted one. So overall,
            // we lost one item in the blocklist since the last time.
            expected_block_heights_len -= 1;
        });

        terminal.read(&app, |view, _ctx| {
            let model = view.model.lock();

            // Make sure the state has changed.
            assert!(matches!(
                view.inline_banners_state.shared_session_banner_state,
                SharedSessionBanners::ActiveShare { .. }
            ));

            // We should have removed two banners and inserted one. So overall,
            // we lost one item in the blocklist since the last time.
            let block_height_items = model.block_list().block_heights().items();
            assert_eq!(block_height_items.len(), expected_block_heights_len);

            // The banner should have been inserted at the end of the blocklist, before the active block.
            let last_block_total_index = model
                .block_list()
                .last_non_hidden_block_by_index()
                .unwrap()
                .to_total_index(model.block_list());
            assert_lines_approx_eq!(
                block_height_items[last_block_total_index.0 + 1]
                    .height()
                    .into_lines(),
                INLINE_BANNER_HEIGHT
            );
        });
    })
}

#[test]
fn test_resize_shared_session_viewer_from_server() {
    let _flag = FeatureFlag::CreatingSharedSessions.override_enabled(true);

    App::test((), |mut app| async move {
        let terminal = terminal_view_for_viewer(&mut app);
        terminal.update(&mut app, |view, ctx| {
            // Refresh the size at the start of the test to make sure
            // we're using a consistent size throughout.
            view.refresh_size(ctx);
        });

        let model = terminal.read(&app, |view, _| view.model.clone());
        model
            .lock()
            .set_shared_session_status(SharedSessionStatus::ActiveViewer {
                role: Default::default(),
            });

        // The viewer's current size info.
        let original_size_info = *model.lock().block_list().size();
        let original_num_rows = original_size_info.rows();
        let original_num_cols = original_size_info.columns();

        // Case 1: suppose the sharer has a larger size.
        // The size info we expect is the old one with the greater
        // number of rows and columns (nothing else changed).
        let new_num_rows = original_num_rows + 1;
        let new_num_cols = original_num_cols + 1;
        let expected_size_info =
            original_size_info.with_rows_and_columns(new_num_rows, new_num_cols);

        terminal.update(&mut app, |view, ctx| {
            view.resize_from_sharer_update(
                WindowSize {
                    num_rows: new_num_rows,
                    num_cols: new_num_cols,
                },
                ctx,
            );
        });

        // Make sure the view and model reflect the new, expected size info.
        terminal.read(&app, |view, _ctx| {
            assert_eq!(*view.size_info(), expected_size_info);
            assert_eq!(*view.model.lock().block_list().size(), expected_size_info);
        });

        // Case 2: suppose the sharer has a smaller size.
        // The size info we expect is our old, larger one; nothing changed.
        let new_num_rows = original_num_rows - 1;
        let new_num_cols = original_num_cols - 1;
        let expected_size_info = original_size_info;

        terminal.update(&mut app, |view, ctx| {
            view.resize_from_sharer_update(
                WindowSize {
                    num_rows: new_num_rows,
                    num_cols: new_num_cols,
                },
                ctx,
            );
        });

        // Make sure the view and model reflect the old, expected size info.
        terminal.read(&app, |view, _ctx| {
            assert_eq!(*view.size_info(), expected_size_info);
            assert_eq!(*view.model.lock().block_list().size(), expected_size_info);
        });
    })
}

#[test]
fn test_resize_shared_session_viewer_independent_of_sharer() {
    let _create_flag = FeatureFlag::CreatingSharedSessions.override_enabled(true);
    let _view_flag = FeatureFlag::ViewingSharedSessions.override_enabled(true);

    App::test((), |mut app| async move {
        let terminal = terminal_view_for_viewer(&mut app);
        terminal.update(&mut app, |view, ctx| {
            // Refresh the size at the start of the test to make sure
            // we're using a consistent size throughout.
            view.after_terminal_view_layout(vec2f(100., 100.), ctx);

            // Set the sharer's size.
            let num_rows = view.size_info().rows();
            let num_cols = view.size_info().columns();
            view.resize_from_sharer_update(WindowSize { num_rows, num_cols }, ctx);
        });

        let original_size_info = terminal.read(&app, |view, _| *view.size_info());
        let original_num_rows = original_size_info.rows();
        let original_num_cols = original_size_info.columns();

        // Case 1: make the viewer winsize smaller by making the pane narrower.
        terminal.update(&mut app, |view, ctx| {
            let narrower = vec2f(
                original_size_info.pane_width_px().as_f32() - 10.,
                original_size_info.pane_height_px().as_f32(),
            );
            view.after_terminal_view_layout(narrower, ctx);
        });

        // Make sure the overall size info was changed but the rows, columns
        // were unchanged because we're respecting the sharer's larger size.
        terminal.read(&app, |view, _ctx| {
            let new_size_info = *view.size_info();
            assert_ne!(original_size_info, new_size_info);

            let expected_size_info =
                new_size_info.with_rows_and_columns(original_num_rows, original_num_cols);
            assert_eq!(*view.size_info(), expected_size_info);
            assert_eq!(*view.model.lock().block_list().size(), expected_size_info);
        });

        // Case 2: make the viewer winsize larger by making the pane wider.
        terminal.update(&mut app, |view, ctx| {
            let wider = vec2f(
                original_size_info.pane_width_px().as_f32() + 10.,
                original_size_info.pane_height_px().as_f32(),
            );
            view.after_terminal_view_layout(wider, ctx);
        });

        // Make sure the overall size info was changed, and that the rows, columns
        // were updated because we're respecting the viewer's larger size.
        terminal.read(&app, |view, _ctx| {
            let new_size_info = *view.size_info();
            assert_ne!(original_size_info, new_size_info);

            assert!(new_size_info.columns() > original_num_cols);
            assert!(view.model.lock().block_list().size().columns() > original_num_cols);
        });
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_on_session_share_ended_restores_size_after_viewer_driven_resize() {
    let _flag = FeatureFlag::CreatingSharedSessions.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);

        terminal.update(&mut app, |view, ctx| {
            // Refresh the size at the start of the test to make sure
            // we're using a consistent size throughout.
            view.after_terminal_view_layout(vec2f(100., 100.), ctx);
        });

        let original_size = terminal.read(&app, |view, _| *view.size_info());
        let viewer_rows = original_size.rows().saturating_sub(2).max(1);
        let viewer_cols = original_size.columns().saturating_sub(4).max(1);
        assert!(viewer_rows < original_size.rows() || viewer_cols < original_size.columns());

        // Resize the view as if a viewer with a smaller winsize has joined the session.
        terminal.update(&mut app, |view, ctx| {
            view.resize_from_viewer_report(
                WindowSize {
                    num_rows: viewer_rows,
                    num_cols: viewer_cols,
                },
                ctx,
            );
        });

        terminal.read(&app, |view, _| {
            assert_eq!(view.size_info().rows(), viewer_rows);
            assert_eq!(view.size_info().columns(), viewer_cols);
            assert_eq!(
                view.active_viewer_driven_size,
                Some((viewer_rows, viewer_cols))
            );
            assert_eq!(*view.model.lock().block_list().size(), *view.size_info());
        });

        // End the session, assert that the winsize was restored to the original.
        terminal.update(&mut app, |view, ctx| {
            view.on_session_share_ended(ctx);
        });

        terminal.read(&app, |view, _| {
            assert_eq!(view.size_info().rows(), original_size.rows());
            assert_eq!(view.size_info().columns(), original_size.columns());
            assert_eq!(view.active_viewer_driven_size, None);
            assert_eq!(*view.model.lock().block_list().size(), original_size);
        });
    })
}

#[test]
fn test_on_session_share_ended_does_not_insert_tombstone_for_ambient_session_under_cloud_mode_setup_v2()
 {
    let _flag = FeatureFlag::CloudModeSetupV2.override_enabled(true);

    App::test((), |mut app| async move {
        let terminal = terminal_view_for_viewer(&mut app);
        let initial_block_height_items = terminal.read(&app, |view, _| {
            view.model.lock().block_list().block_heights().items().len()
        });

        terminal.update(&mut app, |view, ctx| {
            view.model
                .lock()
                .set_shared_session_source(SharedSessionSource::ambient_agent(None));
            view.on_session_share_ended(ctx);
        });

        terminal.read(&app, |view, _| {
            let final_block_height_items =
                view.model.lock().block_list().block_heights().items().len();
            // Only shared session ended banner. Ended-state continuation UI is
            // driven by the task resolver path under CloudModeSetupV2.
            assert_eq!(final_block_height_items, initial_block_height_items + 1);
            assert!(view.conversation_ended_tombstone_view_id.is_none());
        });
    });
}

fn cloud_mode_terminal_for_test(app: &mut App) -> ViewHandle<TerminalView> {
    initialize_app_for_terminal_view(app);
    let tips_model = app.add_model(|_| Default::default());
    let (_, terminal) = app.add_window(WindowStyle::NotStealFocus, |ctx| {
        TerminalView::new_for_test(tips_model, None, ctx)
    });
    terminal
}

#[test]
fn test_ambient_session_join_auto_opens_details_panel() {
    let _cloud_mode_flag = FeatureFlag::CloudMode.override_enabled(true);

    App::test((), |mut app| async move {
        let terminal = cloud_mode_terminal_for_test(&mut app);
        let firebase_uid = UserUid::new("mock_firebase_uid");

        terminal.update(&mut app, |view, ctx| {
            view.model
                .lock()
                .set_shared_session_status(SharedSessionStatus::ViewPending);
            view.on_session_share_joined(
                ParticipantId::new(),
                firebase_uid,
                ReplicaId::random(),
                Box::new(ParticipantList::default()),
                SessionId::new(),
                SessionSourceType::AmbientAgent { task_id: None },
                ctx,
            );
        });

        terminal.read(&app, |view, _| {
            assert!(view.is_conversation_details_panel_open);
            assert!(view.has_auto_opened_conversation_details_panel);
        });
    });
}

#[test]
fn test_on_session_share_ended_does_not_insert_tombstone_for_non_ambient_session_under_cloud_mode_setup_v2()
 {
    let _flag = FeatureFlag::CloudModeSetupV2.override_enabled(true);

    App::test((), |mut app| async move {
        let terminal = terminal_view_for_viewer(&mut app);
        let initial_block_height_items = terminal.read(&app, |view, _| {
            view.model.lock().block_list().block_heights().items().len()
        });

        terminal.update(&mut app, |view, ctx| {
            view.model
                .lock()
                .set_shared_session_source(SharedSessionSource::user(None));
            view.on_session_share_ended(ctx);
        });

        terminal.read(&app, |view, _| {
            let final_block_height_items =
                view.model.lock().block_list().block_heights().items().len();
            // Only shared session ended banner.
            assert_eq!(final_block_height_items, initial_block_height_items + 1);
        });
    });
}

#[test]
fn test_shared_followup_on_existing_conversation_converts_user_query_input() {
    App::test((), |mut app| async move {
        let terminal = cloud_mode_terminal_for_test(&mut app);
        let terminal_view_id = terminal.id();
        let conversation_token = "restored-conversation-token";
        let request_id = "new-followup-request";
        let root_task_id = "root-task";
        let followup_query = "follow up";

        let conversation_id =
            BlocklistAIHistoryModel::handle(&app).update(&mut app, |model, ctx| {
                let conversation_id =
                    model.start_new_conversation(terminal_view_id, false, false, false, ctx);
                model.set_server_conversation_token_for_conversation(
                    conversation_id,
                    conversation_token.to_string(),
                );
                conversation_id
            });

        terminal.update(&mut app, |view, ctx| {
            let init_event = api::ResponseEvent {
                r#type: Some(api::response_event::Type::Init(
                    api::response_event::StreamInit {
                        request_id: request_id.to_string(),
                        conversation_id: conversation_token.to_string(),
                        run_id: String::new(),
                    },
                )),
            };
            view.ai_controller.update(ctx, |controller, ctx| {
                controller.handle_shared_session_response_event(init_event, ctx);
            });

            let create_root_task_event = api::ResponseEvent {
                r#type: Some(api::response_event::Type::ClientActions(
                    api::response_event::ClientActions {
                        actions: vec![api::ClientAction {
                            action: Some(api_client_action::Action::CreateTask(
                                api_client_action::CreateTask {
                                    task: Some(api::Task {
                                        id: root_task_id.to_string(),
                                        messages: vec![],
                                        dependencies: None,
                                        description: String::new(),
                                        summary: String::new(),
                                        server_data: String::new(),
                                    }),
                                },
                            )),
                        }],
                    },
                )),
            };
            view.ai_controller.update(ctx, |controller, ctx| {
                controller.handle_shared_session_response_event(create_root_task_event, ctx);
            });

            let add_user_query_event = api::ResponseEvent {
                r#type: Some(api::response_event::Type::ClientActions(
                    api::response_event::ClientActions {
                        actions: vec![api::ClientAction {
                            action: Some(api_client_action::Action::AddMessagesToTask(
                                api_client_action::AddMessagesToTask {
                                    task_id: root_task_id.to_string(),
                                    messages: vec![api::Message {
                                        fetched_memories: vec![],
                                        id: "user-message".to_string(),
                                        task_id: root_task_id.to_string(),
                                        server_message_data: String::new(),
                                        citations: vec![],
                                        message: Some(api::message::Message::UserQuery(
                                            api::message::UserQuery {
                                                query: followup_query.to_string(),
                                                context: None,
                                                referenced_attachments: HashMap::new(),
                                                mode: None,
                                                intended_agent: Default::default(),
                                            },
                                        )),
                                        request_id: request_id.to_string(),
                                        timestamp: None,
                                    }],
                                },
                            )),
                        }],
                    },
                )),
            };
            view.ai_controller.update(ctx, |controller, ctx| {
                controller.handle_shared_session_response_event(add_user_query_event, ctx);
            });
        });

        BlocklistAIHistoryModel::handle(&app).read(&app, |model, _| {
            let conversation = model
                .conversation(&conversation_id)
                .expect("conversation should exist");
            assert!(conversation.is_viewing_shared_session());

            let input = conversation
                .latest_exchange()
                .and_then(|exchange| exchange.input.first())
                .expect("shared-session replay should reconstruct the user query input");
            assert!(matches!(input, AIAgentInput::UserQuery { .. }));
            assert_eq!(input.display_query().as_deref(), Some(followup_query));
        });
    });
}

#[test]
fn passive_suggestions_suppressed_for_shared_ambient_viewer() {
    // A link-join viewer of a shared *cloud-agent* session starts with no ambient view model
    // (it is created lazily at `SessionJoined` and never propagated back to the
    // passive-suggestions model). In this case, we still should not send passive suggestion requests.
    App::test((), |mut app| async move {
        let terminal = terminal_view_for_viewer(&mut app);

        // A non-ambient (user) shared-session viewer must still get passive suggestions:
        // the fix must not over-suppress ordinary shared sessions.
        let suppressed_for_user_viewer = terminal.update(&mut app, |view, ctx| {
            view.passive_suggestions_models
                .maa
                .update(ctx, |model, ctx| {
                    model.is_ambient_agent_session_for_test(ctx)
                })
        });
        assert!(
            !suppressed_for_user_viewer,
            "passive suggestions must not be suppressed for a non-ambient shared-session viewer"
        );

        // Once the viewer discovers it is viewing an ambient (cloud-agent) run, passive
        // suggestions must be suppressed even though the ambient view model is still absent.
        let suppressed_for_ambient_viewer = terminal.update(&mut app, |view, ctx| {
            view.model
                .lock()
                .set_shared_session_source(SharedSessionSource::ambient_agent(Some(
                    "44444444-4444-4444-4444-444444444444".to_string(),
                )));
            view.passive_suggestions_models
                .maa
                .update(ctx, |model, ctx| {
                    model.is_ambient_agent_session_for_test(ctx)
                })
        });
        assert!(
            suppressed_for_ambient_viewer,
            "passive suggestions must be suppressed for a shared cloud-agent viewer"
        );
    });
}
