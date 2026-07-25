//! Touched-workspace derivation for local-to-cloud handoff (REMOTE-1486).
//!
//! Given an [`AIConversation`] (or the flat list of paths extracted from one) and
//! the user's currently-known cloud agent environments, this module produces:
//!
//! 1. The flat set of filesystem paths an agent run has touched, walked off the
//!    conversation's action history and the per-exchange `working_directory`
//!    (see [`extract_paths_from_conversation`]).
//! 2. A [`TouchedWorkspace`] enumerating the distinct git repos and orphan files the
//!    local agent has touched. Each repo carries a parsed `repo_id` (`<owner>/<repo>`)
//!    derived from its `origin` remote URL, fetched via an async `git` invocation so
//!    derivation never blocks the UI thread.
//! 3. A repo-aware default environment selection that layers on top of the existing
//!    cloud-agent setup recency-sort.
//!
//! Path extraction is sync and pure (no I/O), and the workspace derivation is async
//! (one `git remote get-url origin` per unique repo). Callers run them in sequence
//! off the main thread; see `app/src/workspace/view.rs::start_local_to_cloud_handoff`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use command::Stdio;
use command::r#async::Command;
use tokio::fs as tokio_fs;
use warpui::r#async::FutureExt as _;

use crate::ai::blocklist::agent_view::agent_input_footer::sort_environments_by_recency;
use crate::ai::cloud_environments::{CloudAmbientAgentEnvironment, GithubRepo};
use crate::server::ids::SyncId;

/// Soft cap on each git invocation we dispatch. Mirrors the cap used by the cloud-side
/// snapshot pipeline so individual filesystem hiccups don't stall the modal indefinitely.
const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

/// The collection of git repos and orphan files the local agent has touched in the
/// active conversation. Drives both the snapshot upload plan and the modal's env-
/// overlap status row.
#[derive(Clone, Debug, Default)]
pub(crate) struct TouchedWorkspace {
    pub repos: Vec<TouchedRepo>,
}

/// A single git repo touched by the local agent.
#[derive(Clone, Debug)]
pub(crate) struct TouchedRepo {
    /// `<owner>/<repo>` parsed from the `origin` remote URL, when discoverable.
    /// Drives env-overlap matching against `CloudAmbientAgentEnvironment.github_repos`
    /// and the modal's per-repo status row label.
    pub repo_id: Option<GithubRepo>,
}

/// Walk `path` up to find the nearest enclosing `.git` directory and return its parent
/// (the working-tree root). Returns `None` if no `.git` is found.
async fn find_git_root(path: &Path) -> Option<PathBuf> {
    let mut cursor: Option<&Path> = if tokio_fs::metadata(path).await.is_ok_and(|m| m.is_dir()) {
        Some(path)
    } else {
        path.parent()
    };
    while let Some(dir) = cursor {
        let candidate = dir.join(".git");
        if tokio_fs::try_exists(&candidate).await.unwrap_or(false) {
            return Some(dir.to_path_buf());
        }
        cursor = dir.parent();
    }
    None
}

/// Run `git remote get-url origin` in `git_root` with a bounded timeout, returning the
/// trimmed remote URL or `None` if the invocation fails, times out, exits non-zero, or
/// yields empty/non-UTF-8 output. [`GIT_COMMAND_TIMEOUT`] caps the call so a stalled git
/// process can't pin the loading state forever.
async fn git_origin_url(git_root: &Path) -> Option<String> {
    let mut command = Command::new("git");
    command
        .args(["remote", "get-url", "origin"])
        .current_dir(git_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    let Ok(Ok(output)) = command.output().with_timeout(GIT_COMMAND_TIMEOUT).await else {
        return None;
    };
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Parse a GitHub remote URL of either the SSH (`git@github.com:owner/repo.git`) or
/// HTTPS (`https://github.com/owner/repo[.git]`) flavor into a [`GithubRepo`].
/// Returns `None` for non-GitHub remotes (we only support env-overlap for GitHub today,
/// matching the env-creation flow).
fn parse_github_repo(remote_url: &str) -> Option<GithubRepo> {
    let trimmed = remote_url.trim();
    let path_part = if let Some(rest) = trimmed.strip_prefix("git@github.com:") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("ssh://git@github.com/") {
        rest
    } else {
        return None;
    };

    let path_part = path_part.strip_suffix(".git").unwrap_or(path_part);
    let mut segments = path_part.splitn(2, '/');
    let owner = segments.next()?.to_string();
    let repo = segments.next()?.to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(GithubRepo::new(owner, repo))
}

/// Resolve a single directory path to its enclosing git repo and parsed GitHub
/// remote, if any.
pub(crate) async fn resolve_repo_for_path(path: &Path) -> Option<TouchedRepo> {
    let git_root = find_git_root(path).await?;
    let repo_id = git_origin_url(&git_root)
        .await
        .as_deref()
        .and_then(parse_github_repo);
    Some(TouchedRepo { repo_id })
}

/// Pick the env that has the most overlap with the touched repos, breaking ties by
/// recency. Returns `None` when no env contains any of the touched repos (or when
/// `envs` is empty / the workspace touched no GitHub-mapped repos).
///
/// This is the "strict" overlap-aware pick used by the handoff pane bootstrap,
/// which calls it unconditionally and applies the result on top of whatever the
/// `EnvironmentSelector`'s `ensure_default_selection` had already picked. When
/// this returns `None`, callers leave the existing selection alone.
pub(crate) fn pick_handoff_overlap_env(
    workspace: &TouchedWorkspace,
    mut envs: Vec<CloudAmbientAgentEnvironment>,
) -> Option<SyncId> {
    if envs.is_empty() {
        return None;
    }

    let touched_repo_ids: Vec<&GithubRepo> = workspace
        .repos
        .iter()
        .filter_map(|r| r.repo_id.as_ref())
        .collect();
    if touched_repo_ids.is_empty() {
        return None;
    }

    // Sort most-recent-first so that ties on overlap count resolve to the most-
    // recently-used env. We then iterate and keep the first-best score.
    sort_environments_by_recency(&mut envs);
    let mut best: Option<(&CloudAmbientAgentEnvironment, usize)> = None;
    for env in &envs {
        let env_repos = &env.model().string_model.github_repos;
        let score = touched_repo_ids
            .iter()
            .filter(|id| env_repos.iter().any(|r| &r == *id))
            .count();
        if score == 0 {
            continue;
        }
        match best {
            None => best = Some((env, score)),
            Some((_, current)) if score > current => best = Some((env, score)),
            _ => {}
        }
    }
    best.map(|(env, _)| env.id)
}

// --- Path extraction from `AIConversation` ---
//
// Walks an [`AIConversation`] and collects the filesystem paths the local agent
// actually wrote to, plus the per-exchange `working_directory`. The output
// feeds [`derive_touched_workspace`], which groups paths by enclosing `.git`
// repo and produces the [`TouchedWorkspace`] the orchestrator uploads from.
//
// Read-only actions (`ReadFiles`, `Grep`, `FileGlob*`, `SearchCodebase`,
// `InsertCodeReviewComments`) are intentionally NOT walked. The handoff
// snapshot uploads orphan-file contents verbatim, so including a read-only
// reference like `~/.ssh/id_rsa` would leak unrelated local files into the
// cloud agent. Limiting the walk to writes (`RequestFileEdits`,
// `UploadArtifact`) keeps the snapshot to files the user knowingly let the
// agent author. Repos the agent only browsed are still discoverable through
// the per-exchange cwd, which is captured below.
//
// `Path::is_absolute()` paths pass through unchanged; relative paths are
// resolved against the exchange's `working_directory` (and dropped when there
// is no cwd to resolve against). Empty entries are dropped.
//
// Cost is bounded by walking only the [`MAX_TOOL_CALLS_TO_SCAN`] most recent
// action results across all exchanges. Older actions are skipped under the
// assumption that the workspace state the user wants to hand off is dominated
// by recent work; this keeps very long conversations from paying an unbounded
// per-handoff scan cost.

#[cfg(test)]
#[path = "touched_repos_tests.rs"]
mod tests;
