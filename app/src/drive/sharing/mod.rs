//! Permission *types* for locally-stored objects.
//!
//! Despite the module name, what survives here is not a sharing feature. Sharing an object
//! meant publishing it to Warp's server and handing out a warp.dev link, and this build has
//! no server to publish to -- every `ShareableObject::link()` variant bottomed out in
//! `ChannelState::server_root_url()`, which is `None` in a Heddle binary. The dialog that
//! drove it, its QR code, and its avatar styling are gone.
//!
//! What remains is `ContentEditability`, which notebooks, workflows and env-var collections
//! use to decide whether their contents are editable, plus the access-level types they read
//! it from. Those are local decisions about local objects and they are still made.

// Re-exported from cloud_objects. `SharingAccessLevel` describes an object's stored
// permissions, which are still read when deciding editability; it is not a sharing surface.
// The subject types (`Subject`, `UserKind`, `TeamKind`, `LinkSharingSubjectType`) were only
// ever re-exported for the sharing dialog's ACL rows and are no longer referenced here --
// the crate still defines them for the persistence layer.
pub use cloud_objects::drive::sharing::SharingAccessLevel;

/// Whether or not an object's contents are editable by the current user.
///
/// This is not purely a function of their access level since anonymous users are not allowed to
/// edit (due to the lack of attribution).
#[derive(Debug, Clone, Copy)]
pub enum ContentEditability {
    ReadOnly,
    RequiresLogin,
    Editable,
}

impl ContentEditability {
    pub fn can_edit(self) -> bool {
        matches!(self, ContentEditability::Editable)
    }
}
