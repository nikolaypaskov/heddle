//! Ordering for Heddle's own version tags.
//!
//! `crates/channel_versions::ParsedVersion` exists and looks like it would do this job. It
//! will not: its regex is `v(\d+)\.(.+)\.(.+)_(\d+)`, which matches upstream's dated scheme
//! (`v0.2026.07.26.18.00.stable_01`) and rejects `v0.3.1`. Building the downgrade guard on a
//! parser that cannot read our own tags would defeat the guard.
//!
//! The same mismatch has already cost this project a real bug: `script/update_plist` only
//! rewrote `CFBundleShortVersionString` for the dated format, so three releases shipped
//! claiming to be `0.1.0`.

/// A parsed `MAJOR.MINOR.PATCH` version, with or without a leading `v`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HeddleVersion {
    // Field order matters: the derived `Ord` compares fields in declaration order, which is
    // exactly major-then-minor-then-patch. Reordering these silently changes the ordering.
    major: u64,
    minor: u64,
    patch: u64,
}

impl HeddleVersion {
    /// Parse `v0.3.1` or `0.3.1`. Returns `None` for anything else.
    ///
    /// Deliberately strict, and the strictness is the point. An unparseable version must
    /// become "do not update". A lenient parser that returned `0.0.0` for junk would make
    /// every real running version compare as newer than the manifest -- silently disabling
    /// updates for everyone, with no error anywhere.
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.strip_prefix('v').unwrap_or(raw);
        let mut parts = raw.split('.');
        let major = parse_component(parts.next()?)?;
        let minor = parse_component(parts.next()?)?;
        let patch = parse_component(parts.next()?)?;
        // A fourth component means this is not our scheme.
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }

    pub fn major(&self) -> u64 {
        self.major
    }

    pub fn minor(&self) -> u64 {
        self.minor
    }

    pub fn patch(&self) -> u64 {
        self.patch
    }
}

/// Parse one component, rejecting anything `u64::from_str` would tolerate but we should not.
///
/// `from_str` accepts a leading `+` (`"+1"` parses as 1). It does not accept surrounding
/// whitespace, so that needs no extra handling here, but an explicit digits-only check is
/// cheaper to verify by reading than a list of what the standard parser happens to allow.
fn parse_component(part: &str) -> Option<u64> {
    if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    part.parse().ok()
}

#[cfg(test)]
#[path = "heddle_version_tests.rs"]
mod tests;
