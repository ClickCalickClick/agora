//! `whoami` and `user_profile`.

use agora_core::{AgoraError, User, Username};
use scraper::Html;

use super::*;

/// The name from the URL `/user/me/` redirects to; `None` for any other page.
pub fn whoami(final_url: &str) -> Option<Username> {
    let path = reply::path_of(final_url);
    let name = path.strip_prefix("/user/")?.split('/').next()?;
    (!name.is_empty() && name != "me").then(|| Username::new(name))
}

/// Parses `/user/{name}/`. Karma is only in the tooltip text
/// ("1 post karma, 0 comment karma").
pub fn profile(html: &str, name: &Username) -> Result<User, AgoraError> {
    let doc = Html::parse_document(html);
    let created = doc
        .select(&sel("time[data-testid=cake-day]"))
        .next()
        .and_then(|t| attr(&t, "datetime"))
        .and_then(parse_timestamp);
    let karma_text = doc
        .select(&sel("[data-testid=karma-number]"))
        .next()
        .and_then(|k| k.parent())
        .and_then(scraper::ElementRef::wrap)
        .map(|p| text_of(&p))
        .unwrap_or_default();
    if created.is_none() && karma_text.is_empty() {
        return Err(broken("user_profile"));
    }
    Ok(User {
        name: name.clone(),
        post_karma: karma_before(&karma_text, "post karma"),
        comment_karma: karma_before(&karma_text, "comment karma"),
        created,
        avatar_url: None,
    })
}

/// The number right before `label` in `text`.
fn karma_before(text: &str, label: &str) -> Option<i64> {
    let end = text.find(label)?;
    let word = text[..end].split_whitespace().next_back()?;
    word.replace(',', "").parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::tests::fixture;

    #[test]
    fn whoami_reads_the_redirect() {
        assert_eq!(
            whoami("https://www.reddit.com/user/agora_tester/"),
            Some(Username::new("agora_tester"))
        );
        assert_eq!(whoami("https://www.reddit.com/user/me/"), None);
        assert_eq!(whoami("https://www.reddit.com/"), None);
    }

    #[test]
    fn recorded_own_profile() {
        let user = profile(&fixture("user_profile_own.html"), &Username::new("agora_tester")).unwrap();
        assert_eq!(user.post_karma, Some(1));
        assert_eq!(user.comment_karma, Some(0));
        assert_eq!(user.created, parse_timestamp("2026-10-31T00:00:00.000Z"));
    }

    #[test]
    fn a_page_without_either_means_reddit_changed() {
        assert!(matches!(
            profile("<html></html>", &Username::new("x")),
            Err(AgoraError::ExtractorBroken { .. })
        ));
    }
}
