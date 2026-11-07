//! `subscriptions`: the JSON in `left-nav-communities-controller`.

use agora_core::{AgoraError, Subreddit, SubredditName};
use scraper::Html;
use serde::Deserialize;

use super::*;

/// One entry of `initialStateJSON`. Reddit's field names are read loosely,
/// because only the empty list was seen in slice 0.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    #[serde(default, alias = "subredditId")]
    id: String,
    #[serde(default, alias = "displayName", alias = "display_name")]
    name: String,
    #[serde(default)]
    prefixed_name: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default, alias = "icon")]
    icon_url: Option<String>,
}

/// Parses the left-nav partial.
pub fn parse(html: &str) -> Result<Vec<Subreddit>, AgoraError> {
    let doc = Html::parse_document(html);
    let controller = doc
        .select(&sel("left-nav-communities-controller"))
        .next()
        .ok_or(broken("subscriptions"))?;
    let json = attr(&controller, "initialstatejson").unwrap_or("[]");
    let entries: Vec<Entry> = serde_json::from_str(json).map_err(|e| AgoraError::Parse(e.to_string()))?;
    Ok(entries
        .into_iter()
        .filter_map(|e| {
            let name = if e.name.is_empty() {
                e.prefixed_name.trim_start_matches("r/").to_string()
            } else {
                e.name.trim_start_matches("r/").to_string()
            };
            if name.is_empty() || name.starts_with("u_") {
                return None;
            }
            Some(Subreddit {
                id: e.id,
                title: e.title,
                icon_url: e.icon_url,
                subscribed: true,
                ..Subreddit::from_name(SubredditName::new(name))
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::tests::fixture;

    #[test]
    fn recorded_empty_list() {
        assert_eq!(parse(&fixture("subscriptions_left_nav_empty.html")).unwrap(), vec![]);
    }

    #[test]
    fn a_page_without_the_controller_means_reddit_changed() {
        assert!(matches!(
            parse("<html></html>"),
            Err(AgoraError::ExtractorBroken { .. })
        ));
    }
}
