//! Client-side leftovers of the local-to-cloud Oz conversation handoff.
//!
//! Heddle (FOSS): the cloud side of this feature is gone. `OpenLocalToCloudHandoffPane`
//! is a logged no-op, the ambient view model that used to spawn the cloud agent is
//! deleted, and the snapshot-upload pipeline went with it. What survives is the
//! compose-side plumbing that has not been unwound yet:
//!
//! - `HandoffLaunchAttachments` / `PendingCloudLaunch` still carry a compose request
//!   from the input, but nothing reads it any more.
//! - `touched_repos` still resolves a path to its git repo and picks the
//!   most-overlapping environment; `terminal/input.rs` calls both for the
//!   environment selector.
//!
//! Removing the rest is a slice of its own — the compose UI has live entry points
//! (`/move-to-cloud`, the footer chip) that must be retired together.

use super::PendingAttachment;
use crate::server::server_api::ai::AttachmentInput;

#[cfg(feature = "local_fs")]
pub(crate) mod touched_repos;

#[cfg_attr(target_family = "wasm", allow(dead_code))]
#[derive(Debug, Clone, Default)]
pub struct HandoffLaunchAttachments {
    pub(crate) request_attachments: Vec<AttachmentInput>,
    pub(crate) display_attachments: Vec<PendingAttachment>,
}

/// Carries the auto-submit payload for `& query` and `/handoff query`.
/// `request_attachments` feed the spawn request while `display_attachments`
/// are restored into the source input on failure.
#[cfg_attr(target_family = "wasm", allow(dead_code))]
#[derive(Debug, Clone)]
pub struct PendingCloudLaunch {
    pub(crate) prompt: String,
    pub(crate) attachments: HandoffLaunchAttachments,
}
