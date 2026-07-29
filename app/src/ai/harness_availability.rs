use std::collections::HashMap;
use std::time::Duration;

use instant::Instant;
use serde::{Deserialize, Serialize};
use warp_cli::agent::Harness;
use warp_core::features::FeatureFlag;
use warp_errors::report_error;
use warp_managed_secrets::client::SecretOwner;
use warp_managed_secrets::{ManagedSecretManager, ManagedSecretValue};
use warpui::{Entity, ModelContext, RequestState, SingletonEntity};

use crate::ai::harness_display;
use crate::auth::AuthStateProvider;
use crate::auth::auth_manager::{AuthManager, AuthManagerEvent};
use crate::server::retry_strategies::{
    OUT_OF_BAND_REQUEST_RETRY_STRATEGY, is_transient_graphql_or_http_error,
};
use crate::server::server_api::ServerApiProvider;

const AUTH_SECRET_FETCH_FAILURE_COOLDOWN: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HarnessModelInfo {
    pub id: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_level: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HarnessAvailability {
    pub harness: Harness,
    pub display_name: String,
    pub enabled: bool,
    #[serde(default)]
    pub available_models: Vec<HarnessModelInfo>,
}

/// Harnesses that can be driven as a local child process.
///
/// Mirrors [`Harness::parse_local_child_harness`]. Gemini is deliberately
/// absent: `prepare_local_harness_child_launch` treats it as unreachable and
/// it hangs the orchestration flow (see `orchestration/snapshots.rs`).
const LOCAL_CHILD_HARNESSES: [Harness; 3] = [Harness::Claude, Harness::OpenCode, Harness::Codex];

/// The harness catalog, derived entirely from local facts.
///
/// There is no account and no harness endpoint to query here, so the catalog
/// is a fixed client-side list: the built-in agent plus every harness that can
/// run as a local child process. Membership is deliberately install-agnostic —
/// whether a harness's CLI is actually present, and whether it is disabled by
/// product policy, is resolved live at render time by
/// `local_harness_setup_state`, so a CLI installed while the app is running is
/// picked up on the next picker open instead of requiring a restart.
pub(crate) fn local_harness_catalog() -> Vec<HarnessAvailability> {
    std::iter::once(Harness::Oz)
        .chain(LOCAL_CHILD_HARNESSES)
        .map(|harness| HarnessAvailability {
            harness,
            display_name: harness_display::display_name(harness).to_string(),
            // `enabled` carried the server's org-policy verdict. With no
            // server there is no policy to deny anything; local readiness is
            // the picker's job.
            enabled: true,
            available_models: local_models_for(harness),
        })
        .collect()
}

/// Models offered for `harness`, derived client-side.
///
/// A harness only gets a list when selecting a model has an effect this build
/// can actually produce. The single mechanism for that is
/// `harness_model_env_vars` (`ai/agent_sdk/driver/harness/mod.rs`), which
/// translates the selection into an environment variable for the child
/// process — and it matches on exactly one harness. Per harness:
///
/// - **Claude**: gets `ANTHROPIC_MODEL`, so the choice reaches the subprocess.
///   Listed below.
/// - **OpenCode**: `harness_model_env_vars` emits nothing, and
///   `build_local_opencode_child_command` passes only `--prompt`. A selection
///   would be silently dropped, so offering one would be a lie.
/// - **Codex**: same — no env var — and the launch path documents that "Codex
///   local children never receive a model override". `model_snapshot` already
///   hard-codes local Codex to a lone "Default model" row.
/// - **Oz**: the built-in agent draws its models from `LLMPreferences` (the
///   BYOK catalog), not from here; `model_snapshot` routes it to a different
///   branch entirely.
fn local_models_for(harness: Harness) -> Vec<HarnessModelInfo> {
    match harness {
        Harness::Claude => CLAUDE_LOCAL_MODELS
            .iter()
            .map(|(id, display_name)| HarnessModelInfo {
                id: (*id).to_string(),
                display_name: (*display_name).to_string(),
                reasoning_level: None,
            })
            .collect(),
        Harness::Oz | Harness::OpenCode | Harness::Codex | Harness::Gemini | Harness::Unknown => {
            Vec::new()
        }
    }
}

/// Model choices for a local Claude Code child, as `ANTHROPIC_MODEL` values.
///
/// These are Claude Code's own aliases rather than dated model ids. The alias
/// is resolved by the CLI at run time, so this list does not go stale — and a
/// stale hard-coded id would be worse than no list, because it would name a
/// model the user's CLI may refuse. Users who want a specific pinned version
/// still have "Default model", which sends no override and lets Claude Code
/// use whatever the user configured for themselves.
const CLAUDE_LOCAL_MODELS: [(&str, &str); 3] =
    [("opus", "Opus"), ("sonnet", "Sonnet"), ("haiku", "Haiku")];

#[derive(Debug, Clone)]
pub enum AuthSecretFetchState {
    NotFetched,
    Loading,
    Loaded(Vec<AuthSecretEntry>),
    Failed(#[allow(dead_code)] String),
}

#[derive(Debug, Clone)]
pub struct AuthSecretEntry {
    pub name: String,
    pub owner: SecretOwner,
}

/// The catalog itself is static, so the only thing left to announce is
/// auth-secret state — hence the uniform `AuthSecret*` prefix.
#[allow(clippy::enum_variant_names)]
pub enum HarnessAvailabilityEvent {
    AuthSecretsLoaded,
    /// Emitted when a lazy auth-secrets fetch fails. Subscribers should
    /// re-render so any "Loading…" placeholders can transition to an
    /// error state — without this signal the picker would otherwise be
    /// stuck on the loading placeholder until the next refetch.
    AuthSecretsFetchFailed,
    AuthSecretCreated {
        harness: Harness,
        name: String,
    },
    AuthSecretCreationFailed {
        error: String,
    },
    AuthSecretDeleted {
        harness: Harness,
        name: String,
        owner: SecretOwner,
    },
    AuthSecretDeletionFailed {
        harness: Harness,
        name: String,
        owner: SecretOwner,
        error: String,
    },
}

pub struct HarnessAvailabilityModel {
    harnesses: Vec<HarnessAvailability>,
    auth_secrets: HashMap<Harness, AuthSecretFetchState>,
    auth_secret_retry_after: HashMap<Harness, Instant>,
}

impl HarnessAvailabilityModel {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        ctx.subscribe_to_model(&AuthManager::handle(ctx), |me, _, event, _ctx| {
            if let AuthManagerEvent::AuthComplete = event {
                let cached_harnesses: Vec<Harness> = me.auth_secrets.keys().copied().collect();
                for harness in cached_harnesses {
                    me.invalidate_auth_secrets(harness);
                }
            }
        });

        Self {
            harnesses: local_harness_catalog(),
            auth_secrets: HashMap::new(),
            auth_secret_retry_after: HashMap::new(),
        }
    }

    pub fn available_harnesses(&self) -> &[HarnessAvailability] {
        &self.harnesses
    }

    pub fn display_name_for(&self, harness: Harness) -> &str {
        self.harnesses
            .iter()
            .find(|h| h.harness == harness)
            .map(|h| h.display_name.as_str())
            .unwrap_or_else(|| harness_display::display_name(harness))
    }

    /// Whether the harness selector should be shown (>1 known harness, including disabled).
    pub fn should_show_harness_selector(&self) -> bool {
        FeatureFlag::AgentHarness.is_enabled() && self.harnesses.len() > 1
    }

    /// Whether any harness is available at all (at least one enabled).
    pub fn has_any_enabled_harness(&self) -> bool {
        self.harnesses.iter().any(|h| h.enabled)
    }

    /// Whether a harness is both known and enabled.
    pub fn is_harness_enabled(&self, harness: Harness) -> bool {
        self.harnesses
            .iter()
            .any(|h| h.harness == harness && h.enabled)
    }

    pub fn models_for(&self, harness: Harness) -> Option<&[HarnessModelInfo]> {
        self.harnesses
            .iter()
            .find(|h| h.harness == harness)
            .map(|h| h.available_models.as_slice())
            .filter(|m| !m.is_empty())
    }

    pub fn auth_secrets_for(&self, harness: Harness) -> &AuthSecretFetchState {
        self.auth_secrets
            .get(&harness)
            .unwrap_or(&AuthSecretFetchState::NotFetched)
    }

    pub fn ensure_auth_secrets_fetched(&mut self, harness: Harness, ctx: &mut ModelContext<Self>) {
        match self.auth_secrets_for(harness) {
            AuthSecretFetchState::NotFetched => self.fetch_auth_secrets(harness, ctx),
            AuthSecretFetchState::Failed(_) if self.can_retry_auth_secret_fetch(harness) => {
                self.fetch_auth_secrets(harness, ctx);
            }
            AuthSecretFetchState::Failed(_)
            | AuthSecretFetchState::Loading
            | AuthSecretFetchState::Loaded(_) => {}
        }
    }

    fn fetch_auth_secrets(&mut self, harness: Harness, ctx: &mut ModelContext<Self>) {
        let Some(agent_harness) = harness_to_graphql_harness(harness) else {
            return;
        };

        if !AuthStateProvider::as_ref(ctx).get().is_logged_in() {
            return;
        }

        self.auth_secrets
            .insert(harness, AuthSecretFetchState::Loading);
        self.auth_secret_retry_after.remove(&harness);

        let api = ServerApiProvider::as_ref(ctx).get_managed_secrets_client();
        ctx.spawn_with_retry_on_error_when(
            move || {
                let api = api.clone();
                let agent_harness = agent_harness.clone();
                async move { api.list_harness_auth_secrets(agent_harness).await }
            },
            OUT_OF_BAND_REQUEST_RETRY_STRATEGY,
            is_transient_graphql_or_http_error,
            move |me,
                  result: RequestState<Vec<warp_graphql::managed_secrets::ManagedSecret>>,
                  ctx| match result {
                RequestState::RequestSucceeded(secrets) => {
                    let entries = secrets
                        .into_iter()
                        .map(|s| AuthSecretEntry {
                            owner: secret_owner_from_space(&s.owner),
                            name: s.name,
                        })
                        .collect();
                    me.auth_secrets
                        .insert(harness, AuthSecretFetchState::Loaded(entries));
                    me.auth_secret_retry_after.remove(&harness);
                    ctx.emit(HarnessAvailabilityEvent::AuthSecretsLoaded);
                }
                RequestState::RequestFailedRetryPending(e) => {
                    log::warn!("Failed to fetch harness auth secrets; retrying: {e:#}");
                }
                RequestState::RequestFailed(e) => {
                    let msg = e.to_string();
                    report_error!(e.context("Failed to fetch harness auth secrets"));
                    me.auth_secrets
                        .insert(harness, AuthSecretFetchState::Failed(msg));
                    me.auth_secret_retry_after
                        .insert(harness, Instant::now() + AUTH_SECRET_FETCH_FAILURE_COOLDOWN);
                    // Notify subscribers so they can drop any
                    // "Loading…" placeholder rendered during the
                    // in-flight fetch and surface the error state.
                    ctx.emit(HarnessAvailabilityEvent::AuthSecretsFetchFailed);
                }
            },
        );
    }

    fn can_retry_auth_secret_fetch(&self, harness: Harness) -> bool {
        self.auth_secret_retry_after
            .get(&harness)
            .map(|retry_after| Instant::now() >= *retry_after)
            .unwrap_or(true)
    }

    pub fn invalidate_auth_secrets(&mut self, harness: Harness) {
        self.auth_secrets.remove(&harness);
        self.auth_secret_retry_after.remove(&harness);
    }

    pub fn create_auth_secret(
        &mut self,
        harness: Harness,
        name: String,
        value: ManagedSecretValue,
        owner: SecretOwner,
        ctx: &mut ModelContext<Self>,
    ) {
        let manager = ManagedSecretManager::handle(ctx);
        let create_future = manager.as_ref(ctx).create_secret(owner, name, value, None);
        ctx.spawn(create_future, move |me, result, ctx| match result {
            Ok(secret) => {
                let entry = AuthSecretEntry {
                    name: secret.name.clone(),
                    owner: secret_owner_from_space(&secret.owner),
                };
                match me.auth_secrets.get_mut(&harness) {
                    Some(AuthSecretFetchState::Loaded(entries)) => {
                        entries.push(entry);
                    }
                    _ => {
                        me.auth_secrets
                            .insert(harness, AuthSecretFetchState::Loaded(vec![entry]));
                    }
                }
                ctx.emit(HarnessAvailabilityEvent::AuthSecretCreated {
                    harness,
                    name: secret.name,
                });
            }
            Err(e) => {
                let msg = e.to_string();
                report_error!(e.context("Failed to create harness auth secret"));
                ctx.emit(HarnessAvailabilityEvent::AuthSecretCreationFailed { error: msg });
            }
        });
    }

    pub fn delete_auth_secret(
        &mut self,
        harness: Harness,
        name: String,
        owner: SecretOwner,
        ctx: &mut ModelContext<Self>,
    ) {
        let manager = ManagedSecretManager::handle(ctx);
        let delete_future = manager
            .as_ref(ctx)
            .delete_secret(owner.clone(), name.clone());
        ctx.spawn(delete_future, move |me, result, ctx| match result {
            Ok(()) => {
                if let Some(AuthSecretFetchState::Loaded(entries)) =
                    me.auth_secrets.get_mut(&harness)
                {
                    remove_deleted_auth_secret_entry(entries, &name, &owner);
                }
                ctx.emit(HarnessAvailabilityEvent::AuthSecretDeleted {
                    harness,
                    name,
                    owner,
                });
            }
            Err(e) => {
                let msg = e.to_string();
                report_error!(e.context("Failed to delete harness auth secret"));
                ctx.emit(HarnessAvailabilityEvent::AuthSecretDeletionFailed {
                    harness,
                    name,
                    owner,
                    error: msg,
                });
            }
        });
    }
}

fn secret_owner_from_space(space: &warp_graphql::object::Space) -> SecretOwner {
    match space.type_ {
        warp_graphql::object::SpaceType::Team => SecretOwner::Team {
            team_uid: space.uid.clone().into_inner(),
        },
        warp_graphql::object::SpaceType::User => SecretOwner::CurrentUser,
    }
}

fn remove_deleted_auth_secret_entry(
    entries: &mut Vec<AuthSecretEntry>,
    name: &str,
    owner: &SecretOwner,
) {
    entries.retain(|entry| entry.name.as_str() != name || &entry.owner != owner);
}
fn harness_to_graphql_harness(harness: Harness) -> Option<warp_graphql::ai::AgentHarness> {
    match harness {
        Harness::Oz => Some(warp_graphql::ai::AgentHarness::Oz),
        Harness::Claude => Some(warp_graphql::ai::AgentHarness::ClaudeCode),
        Harness::Gemini => Some(warp_graphql::ai::AgentHarness::Gemini),
        Harness::Codex => Some(warp_graphql::ai::AgentHarness::Codex),
        Harness::OpenCode | Harness::Unknown => None,
    }
}

impl Entity for HarnessAvailabilityModel {
    type Event = HarnessAvailabilityEvent;
}

impl SingletonEntity for HarnessAvailabilityModel {}

#[cfg(test)]
#[path = "harness_availability_tests.rs"]
mod tests;
