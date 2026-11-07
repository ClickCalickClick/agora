//! What the bridge hands back for one HTTP request, and how pushback is told
//! apart from ordinary answers (SPEC §4.5 "Block detection", §4.6).

use agora_core::AgoraError;
use serde::Deserialize;

/// One response, as `agora-bridge.js` reports it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reply {
    /// HTTP status.
    pub status: u16,
    /// The URL after redirects.
    pub final_url: String,
    /// The body, once reassembled.
    #[serde(default)]
    pub body: String,
    /// How many chunks the body comes in through the message handler; 0 means
    /// it is in `body` already.
    #[serde(default)]
    pub chunks: u32,
    /// Set when `fetch` itself failed.
    #[serde(default)]
    pub error: Option<String>,
}

/// How a response ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A normal answer; parse the body.
    Ok,
    /// Reddit pushed back: stop all traffic.
    Pushback,
    /// The session is signed out.
    LoggedOut,
    /// The error to report.
    Failed(AgoraError),
}

/// Phrases of Reddit's block and rate-limit pages.
const PUSHBACK_PHRASES: &[&str] = &[
    "blocked by network security",
    "whoa there, pardner",
    "prove your humanity",
    "are you a robot",
];

/// Classifies a response. A page of Reddit's app (any `shreddit-` element) is
/// never read as a block page, because a post title can contain those phrases.
pub fn classify(reply: &Reply) -> Outcome {
    if let Some(error) = &reply.error {
        return Outcome::Failed(AgoraError::Network(error.clone()));
    }
    if reply.status == 429 {
        return Outcome::Pushback;
    }
    if is_login_url(&reply.final_url) {
        return Outcome::LoggedOut;
    }
    let lower = reply.body.to_ascii_lowercase();
    let block_page = PUSHBACK_PHRASES.iter().any(|p| lower.contains(p));
    if reply.status == 403 {
        return if block_page {
            Outcome::Pushback
        } else {
            Outcome::Failed(AgoraError::Forbidden)
        };
    }
    if block_page && !lower.contains("<shreddit-") {
        return Outcome::Pushback;
    }
    match reply.status {
        200..=299 => Outcome::Ok,
        401 => Outcome::LoggedOut,
        404 => Outcome::Failed(AgoraError::NotFound),
        s => Outcome::Failed(AgoraError::Network(format!("HTTP {s}"))),
    }
}

/// Whether a URL is Reddit's login page.
pub fn is_login_url(url: &str) -> bool {
    let path = path_of(url);
    path == "/login" || path.starts_with("/login/") || path.starts_with("/account/login")
}

/// The path of a URL, without host, query or fragment.
pub fn path_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let path = if url.contains("://") {
        rest.find('/').map_or("", |i| &rest[i..])
    } else {
        rest
    };
    path.split(['?', '#']).next().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::tests::fixture;

    fn reply(status: u16, url: &str, body: &str) -> Reply {
        Reply {
            status,
            final_url: url.into(),
            body: body.into(),
            ..Reply::default()
        }
    }

    #[test]
    fn the_recorded_block_page_is_pushback() {
        let r = reply(
            403,
            "https://www.reddit.com/x",
            &fixture("blocked_network_security.html"),
        );
        assert_eq!(classify(&r), Outcome::Pushback);
    }

    #[test]
    fn status_429_is_pushback() {
        assert_eq!(classify(&reply(429, "https://www.reddit.com/x", "")), Outcome::Pushback);
    }

    #[test]
    fn a_redirect_to_login_is_signed_out() {
        assert_eq!(
            classify(&reply(200, "https://www.reddit.com/login/?dest=x", "")),
            Outcome::LoggedOut
        );
        assert_eq!(
            classify(&reply(200, "https://www.reddit.com/account/login", "")),
            Outcome::LoggedOut
        );
    }

    #[test]
    fn a_post_about_blocks_is_not_a_block() {
        let body = "<shreddit-post post-title=\"You've been blocked by network security\"></shreddit-post>";
        assert_eq!(classify(&reply(200, "https://www.reddit.com/x", body)), Outcome::Ok);
    }

    #[test]
    fn a_challenge_page_with_status_200_is_pushback() {
        let body = "<html><body>Prove your humanity</body></html>";
        assert_eq!(
            classify(&reply(200, "https://www.reddit.com/x", body)),
            Outcome::Pushback
        );
    }

    #[test]
    fn other_statuses() {
        let url = "https://www.reddit.com/r/x";
        assert_eq!(classify(&reply(200, url, "")), Outcome::Ok);
        assert_eq!(classify(&reply(404, url, "")), Outcome::Failed(AgoraError::NotFound));
        assert_eq!(
            classify(&reply(403, url, "private")),
            Outcome::Failed(AgoraError::Forbidden)
        );
        assert_eq!(classify(&reply(401, url, "")), Outcome::LoggedOut);
        assert!(matches!(
            classify(&reply(503, url, "")),
            Outcome::Failed(AgoraError::Network(_))
        ));
    }

    #[test]
    fn a_failed_fetch_is_a_network_error() {
        let r = Reply {
            error: Some("TypeError: Load failed".into()),
            ..Reply::default()
        };
        assert!(matches!(classify(&r), Outcome::Failed(AgoraError::Network(_))));
    }

    #[test]
    fn paths() {
        assert_eq!(path_of("https://www.reddit.com/user/x/?a=1"), "/user/x/");
        assert_eq!(path_of("/user/x#y"), "/user/x");
        assert_eq!(path_of("https://www.reddit.com"), "");
    }
}
