use std::sync::Arc;

use warp_core::features::FeatureFlag;
use warp_multi_agent_api as api;

use super::{ConvertToAPITypeError, RequestParams, ResponseStream};
use crate::server::server_api::{AIApiError, ServerApi};
use crate::terminal::model::session::SessionType;

/// Reports the multi-agent (Oz) backend as unavailable.
///
/// This build has no Warp server, so the request could never be sent: the client
/// resolved its endpoint from `ChannelState::server_root_url()`, which is `None`
/// for every shipping channel, and returned `NoServerConfigured` before touching
/// the network. Building the ~120-line request first, only to convert that error
/// into `BackendUnavailable::Oz`, was work whose result was already determined --
/// so the error is produced directly and `warp_multi_agent_client` is gone.
///
/// The typed error matters: `retry_strategies` classifies `BackendUnavailable` as
/// permanent, so callers fail fast instead of retrying forever against an endpoint
/// that does not exist. Emitting it on a stream rather than returning `Err` keeps
/// the shape callers already handle -- this mirrors exactly what the old error
/// path did.
pub async fn generate_multi_agent_output(
    _server_api: Arc<ServerApi>,
    _params: RequestParams,
    _cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<ResponseStream, ConvertToAPITypeError> {
    let (tx, rx) = async_channel::unbounded();
    let _ = tx
        .send(Err(Arc::new(AIApiError::Other(anyhow::Error::new(
            warp_core::channel::BackendUnavailable::Oz,
        )))))
        .await;
    Ok(Box::pin(rx))
}

fn api_keys_with_warp_credit_fallback_setting(
    api_keys: Option<api::request::settings::ApiKeys>,
    allow_use_of_warp_credits: bool,
) -> Option<api::request::settings::ApiKeys> {
    match api_keys {
        Some(mut api_keys) => {
            api_keys.allow_use_of_warp_credits = allow_use_of_warp_credits;
            Some(api_keys)
        }
        None if allow_use_of_warp_credits => Some(api::request::settings::ApiKeys {
            allow_use_of_warp_credits: true,
            ..Default::default()
        }),
        None => None,
    }
}

fn supports_orchestration_v2(orchestration_enabled: bool) -> bool {
    orchestration_enabled
}

fn get_supported_tools(params: &RequestParams) -> Vec<api::ToolType> {
    let mut supported_tools = vec![
        api::ToolType::Grep,
        api::ToolType::FileGlob,
        api::ToolType::FileGlobV2,
        api::ToolType::ReadMcpResource,
        api::ToolType::CallMcpTool,
        api::ToolType::InitProject,
        api::ToolType::OpenCodeReview,
        api::ToolType::RunShellCommand,
        api::ToolType::SuggestNewConversation,
        api::ToolType::Subagent,
        api::ToolType::WriteToLongRunningShellCommand,
        api::ToolType::ReadShellCommandOutput,
        api::ToolType::ReadDocuments,
        api::ToolType::CreateDocuments,
        api::ToolType::EditDocuments,
        api::ToolType::SuggestPrompt,
    ];

    if FeatureFlag::ConversationsAsContext.is_enabled() {
        supported_tools.push(api::ToolType::FetchConversation);
    }

    match params.session_context.session_type() {
        None | Some(SessionType::Local) => {
            supported_tools.extend(&[
                api::ToolType::ReadFiles,
                api::ToolType::ApplyFileDiffs,
                api::ToolType::SearchCodebase,
            ]);

            if FeatureFlag::ArtifactCommand.is_enabled() {
                supported_tools.push(api::ToolType::UploadFileArtifact);
            }
        }
        Some(SessionType::HeddlifiedRemote { host_id: Some(_) }) => {
            // Remote session with a known host — enable tools that route
            // through RemoteServerClient. The host_id is only populated
            // after a successful connection handshake, so its presence is a
            // sufficient proxy for client availability.
            supported_tools.extend(&[api::ToolType::ReadFiles, api::ToolType::ApplyFileDiffs]);
            if FeatureFlag::RemoteCodebaseIndexing.is_enabled() {
                supported_tools.push(api::ToolType::SearchCodebase);
            }
        }
        Some(SessionType::HeddlifiedRemote { host_id: None }) => {
            // Feature flag off or not yet connected — no remote tools.
        }
    }

    if FeatureFlag::AgentModeComputerUse.is_enabled() && params.computer_use_enabled {
        supported_tools.extend(&[api::ToolType::UseComputer]);
        supported_tools.extend(&[api::ToolType::RequestComputerUse]);

        if FeatureFlag::VideoRecording.is_enabled() {
            supported_tools.extend(&[api::ToolType::StartRecording, api::ToolType::StopRecording]);
        }
    }

    supported_tools.push(api::ToolType::InsertReviewComments);

    if FeatureFlag::ListSkills.is_enabled() {
        supported_tools.push(api::ToolType::ReadSkill);
    }

    if params.orchestration_enabled {
        supported_tools.extend([api::ToolType::RunAgents, api::ToolType::SendMessageToAgent]);
        // Declare client-handled wait_for_events so the server doesn't
        // fall back to the legacy server-handled form.
        supported_tools.push(api::ToolType::WaitForEvents);
    }

    if FeatureFlag::AskUserQuestion.is_enabled() && params.ask_user_question_enabled {
        supported_tools.push(api::ToolType::AskUserQuestion);
    }

    supported_tools
}

fn get_supported_cli_agent_tools(params: &RequestParams) -> Vec<api::ToolType> {
    let mut supported_cli_agent_tools = vec![
        api::ToolType::WriteToLongRunningShellCommand,
        api::ToolType::ReadShellCommandOutput,
        api::ToolType::Grep,
        api::ToolType::FileGlob,
        api::ToolType::FileGlobV2,
    ];

    if FeatureFlag::TransferControlTool.is_enabled() {
        supported_cli_agent_tools.push(api::ToolType::TransferShellCommandControlToUser);
    }

    match params.session_context.session_type() {
        None | Some(SessionType::Local) => {
            supported_cli_agent_tools
                .extend(&[api::ToolType::ReadFiles, api::ToolType::SearchCodebase]);
        }
        Some(SessionType::HeddlifiedRemote { host_id: Some(_) }) => {
            supported_cli_agent_tools.push(api::ToolType::ReadFiles);
            if FeatureFlag::RemoteCodebaseIndexing.is_enabled() {
                supported_cli_agent_tools.push(api::ToolType::SearchCodebase);
            }
        }
        Some(SessionType::HeddlifiedRemote { host_id: None }) => {}
    }

    supported_cli_agent_tools
}

#[cfg(test)]
#[path = "impl_tests.rs"]
mod tests;
