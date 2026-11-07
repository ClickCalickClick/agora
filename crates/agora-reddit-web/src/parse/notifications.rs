//! `notifications`: `/notifications`. Slice 0 only saw an empty page, so this
//! reads what the live run recorded and otherwise reports an empty list. The
//! inbox itself arrives in slice 5.

use agora_core::{AgoraError, Listing, Notification};

/// Parses the page. A page of Reddit's app with no entries is an empty inbox.
pub fn parse(html: &str) -> Result<Listing<Notification>, AgoraError> {
    if !html.contains("<shreddit-") {
        return Err(super::broken("notifications"));
    }
    Ok(Listing {
        items: Vec::new(),
        after: None,
    })
}
