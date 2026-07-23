
pub const USER_DOCS_URL: &str = "https://github.com/nikolaypaskov/warp#readme";
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const GITHUB_ISSUES_URL: &str = "https://github.com/nikolaypaskov/warp/issues";
pub const PRIVACY_POLICY_URL: &str = "https://github.com/nikolaypaskov/warp#readme";

pub fn feedback_form_url() -> String {
    // Fork issue tracker. Deliberately does not auto-attach OS/app version — the
    // privacy-oriented build sends nothing the user did not type themselves.
    "https://github.com/nikolaypaskov/warp/issues/new/choose".to_string()
}
