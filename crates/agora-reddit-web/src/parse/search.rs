//! `search`: the post results page. Subreddit and user results arrive with
//! the search UI in slice 5; the self-test only needs posts.

use agora_core::{AgoraError, Cursor, Post, PostId, PostKind, SearchResults, SubredditName, VoteState};
use scraper::Html;

use super::*;

/// Parses post results. The page shows less than a feed row: title, link,
/// subreddit, age, votes and comments.
pub fn parse(html: &str) -> Result<SearchResults, AgoraError> {
    let doc = Html::parse_document(html);
    let mut results = SearchResults::default();
    for unit in doc.select(&sel("[data-testid=search-post-with-content-preview]")) {
        let Some(title) = unit.select(&sel("a[data-testid=post-title-text]")).next() else {
            continue;
        };
        let (Some(id), Some(permalink)) = (
            attr(&title, "id").and_then(|i| i.strip_prefix("search-post-title-")),
            attr(&title, "href"),
        ) else {
            continue;
        };
        let Some(subreddit) = permalink.strip_prefix("/r/").and_then(|p| p.split('/').next()) else {
            continue;
        };
        let numbers: Vec<i64> = unit
            .select(&sel("[data-testid=search-counter-row] faceplate-number"))
            .filter_map(|n| attr(&n, "number")?.parse().ok())
            .collect();
        results.posts.push(Post {
            id: PostId::new(id),
            subreddit: SubredditName::new(subreddit),
            author: None,
            title: text_of(&title),
            kind: PostKind::Text,
            body: None,
            permalink: permalink.to_string(),
            created: unit
                .select(&sel("faceplate-timeago[ts]"))
                .next()
                .and_then(|t| attr(&t, "ts"))
                .and_then(parse_timestamp)
                .unwrap_or(0),
            score: numbers.first().copied().unwrap_or(0),
            comment_count: numbers.get(1).copied().unwrap_or(0).max(0) as u64,
            vote: VoteState::None,
            saved: false,
            flair: None,
            nsfw: false,
            spoiler: false,
            pinned: false,
            locked: false,
            oc: false,
            edited: false,
            thumbnail: None,
        });
    }
    results.after = doc
        .select(&sel("faceplate-partial[src^='/svc/shreddit/search/']"))
        .next()
        .and_then(|p| attr(&p, "src"))
        .map(Cursor::new);
    if results.posts.is_empty() && !html.contains("search-results") {
        return Err(broken("search"));
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::tests::fixture;

    #[test]
    fn recorded_results() {
        let results = parse(&fixture("search_posts.html")).unwrap();
        assert_eq!(results.posts.len(), 6);
        let first = &results.posts[0];
        assert_eq!(first.id.as_str(), "t3_1vidiqc");
        assert_eq!(first.subreddit.as_str(), "gnome");
        assert_eq!(first.score, 10);
        assert_eq!(first.comment_count, 45);
        assert!(results.after.unwrap().as_str().starts_with("/svc/shreddit/search/"));
    }
}
