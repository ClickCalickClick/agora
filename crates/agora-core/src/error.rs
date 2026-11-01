use std::time::Duration;

/// Result type of every [`Backend`](crate::Backend) operation.
pub type BackendResult<T> = Result<T, AgoraError>;

/// Errors an operation can end with (SPEC §4.7).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AgoraError {
    /// The network is unreachable or the request failed in transit.
    #[error("network error: {0}")]
    Network(String),
    /// Reddit answered as if the user were signed out.
    #[error("signed out of Reddit")]
    SessionExpired,
    /// Reddit pushed back (HTTP 429, a block page, a captcha or interstitial).
    /// All traffic stops until `retry_after` has passed (SPEC §4.6).
    #[error("Reddit asked Agora to slow down")]
    Blocked {
        /// How long the backend waits before it sends anything again.
        retry_after: Duration,
    },
    /// The post, comment, subreddit or user does not exist.
    #[error("not found")]
    NotFound,
    /// The item exists but the user may not see it (private or banned subreddit).
    #[error("forbidden")]
    Forbidden,
    /// Reddit's page or endpoint changed and this operation no longer works.
    #[error("the {operation} operation no longer works")]
    ExtractorBroken {
        /// The operation name from the SPEC §4.5 table.
        operation: &'static str,
    },
    /// A response could not be parsed.
    #[error("could not read Reddit's response: {0}")]
    Parse(String),
    /// A local file could not be read or written.
    #[error("file error: {0}")]
    Io(String),
    /// The request was dropped before it was sent, because the user moved to
    /// another view (interactive lane, SPEC §4.6). Not shown to the user.
    #[error("cancelled")]
    Cancelled,
    /// Posting needs the person to finish a captcha in Reddit's own form
    /// (posting rule 3, SPEC §4.5). `fallback_url` is the reddit.com page to
    /// open in a dialog.
    #[error("Reddit wants to check that you are a person")]
    ChallengeRequired {
        /// Reddit's own reply or submit page for the same target.
        fallback_url: String,
    },
}

impl AgoraError {
    /// Whether offering "Retry" makes sense for this error.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Network(_) | Self::Parse(_) | Self::Io(_))
    }
}
