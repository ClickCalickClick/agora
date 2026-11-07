//! `subreddit_info`: the right-rail partial.

use agora_core::{AgoraError, Body, Rule, Subreddit, SubredditName};
use scraper::Html;

use super::*;

/// Parses the partial. `name` is the subreddit asked for; the right rail
/// also lists other communities, so everything is matched against its id.
pub fn parse(html: &str, name: &SubredditName) -> Result<Subreddit, AgoraError> {
    let doc = Html::parse_document(html);
    let header = doc
        .select(&sel("shreddit-subreddit-header"))
        .find(|h| attr(h, "name").is_some_and(|n| n.eq_ignore_ascii_case(name.as_str())))
        .ok_or(broken("subreddit_info"))?;
    let id = attr(&header, "subreddit-id").ok_or(broken("subreddit_info"))?;
    let subscribed = doc
        .select(&sel("shreddit-join-button"))
        .filter(|b| attr(b, "subreddit-id") == Some(id))
        .any(|b| flag(&b, &["subscribed"]));
    let rules = doc
        .select(&sel("details"))
        .filter_map(|d| {
            let summary = d.select(&sel("summary[aria-controls^=rule-]")).next()?;
            let title = summary.select(&sel("h2")).next().map(|h| text_of(&h))?;
            let content = attr(&summary, "aria-controls")?;
            let description = d
                .select(&sel("[faceplate-auto-height-animator-content]"))
                .find(|c| attr(c, "id") == Some(content))
                .map(|c| c.inner_html())
                .filter(|h| !h.trim().is_empty())
                .map(Body::Html);
            Some(Rule { title, description })
        })
        .collect();
    let created = doc
        .select(&sel("rpl-tooltip faceplate-timeago[ts]"))
        .next()
        .and_then(|t| attr(&t, "ts"))
        .and_then(parse_timestamp);
    Ok(Subreddit {
        id: id.to_string(),
        name: SubredditName::new(attr(&header, "name").unwrap_or(name.as_str())),
        title: attr(&header, "display-name").map(str::to_string),
        description: attr(&header, "description").map(str::to_string),
        icon_url: None,
        banner_url: None,
        members: None,
        subscribed,
        nsfw: flag(&header, &["nsfw", "is-nsfw"]),
        created,
        rules,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::tests::fixture;

    #[test]
    fn recorded_right_rail() {
        let sub = parse(&fixture("subreddit_info_right_rail.html"), &SubredditName::new("GNOME")).unwrap();
        assert_eq!(sub.id, "t5_2qjhn");
        assert_eq!(sub.name.as_str(), "gnome");
        assert_eq!(sub.title.as_deref(), Some("The GNOME Project"));
        assert!(sub.description.unwrap().starts_with("GNOME is a free"));
        assert!(!sub.subscribed, "other communities' buttons do not count");
        assert_eq!(sub.rules[0].title, "Be nice to each other");
        let Some(Body::Html(rule)) = &sub.rules[0].description else {
            panic!("rule text expected")
        };
        assert!(rule.contains("Code of Conduct"));
        assert_eq!(sub.created, parse_timestamp("2008-06-28T11:56:43.495000+0000"));
    }

    #[test]
    fn a_page_without_the_header_means_reddit_changed() {
        assert_eq!(
            parse("<html></html>", &SubredditName::new("x")),
            Err(AgoraError::ExtractorBroken {
                operation: "subreddit_info"
            })
        );
    }

    #[test]
    fn own_join_button_decides_membership() {
        let html = r#"<shreddit-subreddit-header name="x" subreddit-id="t5_x"></shreddit-subreddit-header>
            <shreddit-join-button subreddit-id="t5_y" subscribed></shreddit-join-button>
            <shreddit-join-button subreddit-id="t5_x" subscribed></shreddit-join-button>"#;
        assert!(parse(html, &SubredditName::new("x")).unwrap().subscribed);
    }
}
