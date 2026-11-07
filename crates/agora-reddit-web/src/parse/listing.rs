//! `listing`: `<shreddit-post>` elements and the next-page partial.

use agora_core::{AgoraError, Body, Cursor, Listing, Post, PostId, PostKind, SubredditName, VoteState};
use scraper::{ElementRef, Html};

use super::*;

/// Parses a page of posts. Ad posts are a different element and are skipped;
/// promoted posts inside `<shreddit-post>` are skipped too.
pub fn parse(html: &str) -> Result<Listing<Post>, AgoraError> {
    let doc = Html::parse_document(html);
    let items: Vec<Post> = doc
        .select(&sel("shreddit-post"))
        .filter(|el| !flag(el, &["is-promoted", "promoted"]))
        .filter_map(|el| post_from(&el))
        .collect();
    // An empty feed is still one of Reddit's pages or a blank partial.
    if items.is_empty() && !html.trim().is_empty() && !html.contains("<shreddit-") && !html.contains("<faceplate-") {
        return Err(broken("listing"));
    }
    Ok(Listing {
        items,
        after: next_page(&doc),
    })
}

/// The next page is the `src` of the trailing load-after partial, kept whole:
/// that is the exact URL Reddit's front end would request.
fn next_page(doc: &Html) -> Option<Cursor> {
    doc.select(&sel(
        "faceplate-partial[slot=load-after], faceplate-partial[id^=partial-more-posts]",
    ))
    .filter_map(|el| attr(&el, "src"))
    .find(|src| src.starts_with("/svc/shreddit/"))
    .map(Cursor::new)
}

/// One post from a `<shreddit-post>`. `None` when it has no id or title.
pub fn post_from(el: &ElementRef<'_>) -> Option<Post> {
    let id = attr(el, "id").filter(|id| id.starts_with("t3_"))?;
    let title = attr(el, "post-title")?;
    let subreddit = attr(el, "subreddit-name")
        .or_else(|| attr(el, "subreddit-prefixed-name").map(|s| s.trim_start_matches("r/")))?;
    let href = attr(el, "content-href");
    let body = el
        .select(&sel("[slot=text-body]"))
        .next()
        .map(|b| b.inner_html())
        .filter(|b| !b.trim().is_empty())
        .map(Body::Html);
    let kind = match attr(el, "post-type").unwrap_or("text") {
        "image" => PostKind::Image {
            url: href.unwrap_or_default().to_string(),
        },
        "gallery" => PostKind::Gallery {
            urls: href.map(str::to_string).into_iter().collect(),
        },
        "video" | "gif" => PostKind::Video {
            url: href.unwrap_or_default().to_string(),
        },
        "link" | "crosspost" => PostKind::Link {
            url: href.unwrap_or_default().to_string(),
            domain: attr(el, "domain").unwrap_or_default().to_string(),
        },
        _ => PostKind::Text,
    };
    let flair = el
        .select(&sel("shreddit-post-flair"))
        .next()
        .map(|f| text_of(&f))
        .filter(|f| !f.is_empty());
    Some(Post {
        id: PostId::new(id),
        subreddit: SubredditName::new(subreddit),
        author: username(attr(el, "author")),
        title: title.to_string(),
        kind,
        body,
        permalink: attr(el, "permalink").unwrap_or_default().to_string(),
        created: attr(el, "created-timestamp").and_then(parse_timestamp).unwrap_or(0),
        score: attr(el, "score").and_then(|s| s.parse().ok()).unwrap_or(0),
        comment_count: attr(el, "comment-count").and_then(|s| s.parse().ok()).unwrap_or(0),
        vote: vote_state(el),
        saved: flag(el, &["is-saved", "saved"]),
        flair,
        nsfw: flag(el, &["nsfw", "is-nsfw"]),
        spoiler: flag(el, &["spoiler", "is-spoiler"]),
        pinned: flag(el, &["stickied", "is-stickied", "pinned", "is-pinned"]),
        locked: flag(el, &["is-locked", "locked"]),
        oc: flag(el, &["is-original-content", "is-oc"]),
        edited: flag(el, &["is-edited", "edited"]),
        thumbnail: None,
    })
}

/// The user's vote from `vote-state` (or `vote-type`).
pub(crate) fn vote_state(el: &ElementRef<'_>) -> VoteState {
    match attr(el, "vote-state")
        .or_else(|| attr(el, "vote-type"))
        .map(str::to_ascii_uppercase)
        .as_deref()
    {
        Some("UP" | "UPVOTE") => VoteState::Up,
        Some("DOWN" | "DOWNVOTE") => VoteState::Down,
        _ => VoteState::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::tests::fixture;

    #[test]
    fn recorded_subreddit_page() {
        let listing = parse(&fixture("listing_subreddit.html")).unwrap();
        assert_eq!(listing.items.len(), 3);
        let first = &listing.items[0];
        assert_eq!(first.id.as_str(), "t3_1xkeknc");
        assert_eq!(first.subreddit.as_str(), "gnome");
        assert_eq!(first.author.as_ref().unwrap().as_str(), "user5");
        assert_eq!(first.title, "Thank you so much to the community!");
        assert_eq!(first.score, 30);
        assert_eq!(first.comment_count, 2);
        assert_eq!(first.created, 1_793_464_037);
        assert_eq!(
            first.kind,
            PostKind::Image {
                url: "https://i.redd.it/d4bdr5ddxtyh1.png".into()
            }
        );
        assert_eq!(
            first.permalink,
            "/r/gnome/comments/1xkeknc/thank_you_so_much_to_the_community/"
        );
        assert_eq!(first.flair.as_deref(), Some("Community"));
        assert!(matches!(listing.items[1].kind, PostKind::Gallery { .. }));
        assert!(matches!(listing.items[2].kind, PostKind::Link { ref domain, .. } if domain == "thisweek.gnome.org"));
    }

    #[test]
    fn next_page_is_the_url_the_front_end_would_request() {
        let listing = parse(&fixture("listing_subreddit.html")).unwrap();
        let after = listing.after.unwrap();
        assert!(
            after
                .as_str()
                .starts_with("/svc/shreddit/community-more-posts/hot/?name=gnome&after=")
        );
        assert!(after.as_str().contains("feedLength=29"));
    }

    #[test]
    fn last_page_has_no_cursor() {
        let html = r#"<shreddit-post id="t3_a" post-title="T" subreddit-name="s"></shreddit-post>"#;
        assert_eq!(parse(html).unwrap().after, None);
    }

    #[test]
    fn ads_and_promoted_posts_are_skipped() {
        let html = r#"
            <shreddit-ad-post id="t3_ad" post-title="Ad" subreddit-name="s"></shreddit-ad-post>
            <shreddit-post id="t3_p" promoted post-title="Paid" subreddit-name="s"></shreddit-post>
            <shreddit-post id="t3_a" post-title="T" subreddit-name="s"></shreddit-post>"#;
        let ids: Vec<_> = parse(html).unwrap().items.into_iter().map(|p| p.id).collect();
        assert_eq!(ids, vec![PostId::new("t3_a")]);
    }

    #[test]
    fn flags_and_deleted_authors() {
        let html = r#"<shreddit-post id="t3_a" post-title="T" subreddit-name="s" author="[deleted]"
            nsfw spoiler="false" vote-state="UP"><div slot="text-body"><p>hi</p></div></shreddit-post>"#;
        let post = parse(html).unwrap().items.remove(0);
        assert_eq!(post.author, None);
        assert!(post.nsfw);
        assert!(!post.spoiler);
        assert_eq!(post.vote, VoteState::Up);
        assert_eq!(post.body, Some(Body::Html("<p>hi</p>".into())));
    }
}
