//! Reddit's HTML to `agora-core` models. Display-free, tested against the
//! fixtures recorded in the live runs (SPEC §19). One module per operation.

pub mod comments;
pub mod listing;
pub mod notifications;
pub mod reply;
pub mod search;
pub mod subreddit;
pub mod subscriptions;
pub mod user;

use agora_core::{AgoraError, Username};
use scraper::{ElementRef, Selector};

/// A selector known to be valid.
pub(crate) fn sel(css: &str) -> Selector {
    Selector::parse(css).expect("valid selector")
}

/// A missing piece that every real response has, so Reddit changed something.
pub(crate) fn broken(operation: &'static str) -> AgoraError {
    AgoraError::ExtractorBroken { operation }
}

/// The attribute, with an empty value treated as absent.
pub(crate) fn attr<'a>(el: &ElementRef<'a>, name: &str) -> Option<&'a str> {
    el.value().attr(name).filter(|v| !v.is_empty())
}

/// Whether a boolean attribute is set (`nsfw`, `nsfw="true"`, not `nsfw="false"`).
pub(crate) fn flag(el: &ElementRef<'_>, names: &[&str]) -> bool {
    names.iter().any(|n| el.value().attr(n).is_some_and(|v| v != "false"))
}

/// An element's text, with runs of whitespace collapsed.
pub(crate) fn text_of(el: &ElementRef<'_>) -> String {
    el.text().flat_map(str::split_whitespace).collect::<Vec<_>>().join(" ")
}

/// The post asked for from a post page, which can hold others (recommendations).
pub fn post_page(html: &str, id: &agora_core::PostId) -> Result<agora_core::Post, AgoraError> {
    let doc = scraper::Html::parse_document(html);
    doc.select(&sel("shreddit-post"))
        .filter(|el| attr(el, "id") == Some(id.as_str()))
        .find_map(|el| listing::post_from(&el))
        .ok_or(broken("post_with_comments"))
}

/// An account name, or `None` for the deleted-account placeholders.
pub(crate) fn username(value: Option<&str>) -> Option<Username> {
    match value {
        None | Some("" | "[deleted]" | "[removed]") => None,
        Some(name) => Some(Username::new(name)),
    }
}

/// Seconds since the Unix epoch from Reddit's timestamps, such as
/// `2026-10-31T16:27:17.357000+0000` or `2026-10-31T00:00:00.000Z`.
pub fn parse_timestamp(value: &str) -> Option<i64> {
    let (date, rest) = value.split_once('T')?;
    let mut d = date.split('-').map(str::parse::<i64>);
    let (year, month, day) = (d.next()?.ok()?, d.next()?.ok()?, d.next()?.ok()?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let time_end = rest.find(['.', '+', '-', 'Z']).unwrap_or(rest.len());
    let mut t = rest[..time_end].split(':').map(str::parse::<i64>);
    let (hour, minute, second) = (t.next()?.ok()?, t.next()?.ok()?, t.next().unwrap_or(Ok(0)).ok()?);
    let tz = rest[time_end..]
        .find(['+', '-'])
        .map(|i| &rest[time_end + i..])
        .and_then(|z| {
            let sign = if z.starts_with('-') { -1 } else { 1 };
            let digits: String = z[1..].chars().filter(char::is_ascii_digit).collect();
            let hours: i64 = digits.get(..2)?.parse().ok()?;
            let minutes: i64 = digits.get(2..4).unwrap_or("0").parse().ok()?;
            Some(sign * (hours * 3600 + minutes * 60))
        })
        .unwrap_or(0);
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - tz)
}

/// Days since 1970-01-01 in the proleptic Gregorian calendar.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The first run of digits in `text`, ignoring thousands separators.
pub(crate) fn first_number(text: &str) -> Option<u32> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    let digits: String = text[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ',')
        .filter(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fixture from `tests/fixtures/`.
    pub(crate) fn fixture(name: &str) -> String {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    #[test]
    fn timestamps() {
        assert_eq!(parse_timestamp("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_timestamp("2026-10-31T16:27:17.357000+0000"), Some(1_793_464_037));
        assert_eq!(parse_timestamp("2026-10-31T00:00:00.000Z"), Some(1_793_404_800));
        assert_eq!(parse_timestamp("2026-10-31T17:27:17+0100"), Some(1_793_464_037));
        assert_eq!(parse_timestamp("2026-10-31T12:27:17-04:00"), Some(1_793_464_037));
        assert_eq!(parse_timestamp("yesterday"), None);
    }

    #[test]
    fn post_page_finds_the_post_asked_for() {
        let html = r#"<shreddit-post id="t3_other" post-title="Other" subreddit-name="s"></shreddit-post>
            <shreddit-post id="t3_a" post-title="This" subreddit-name="s"></shreddit-post>"#;
        let post = post_page(html, &agora_core::PostId::new("t3_a")).unwrap();
        assert_eq!(post.title, "This");
        assert!(post_page(html, &agora_core::PostId::new("t3_b")).is_err());
    }

    #[test]
    fn numbers() {
        assert_eq!(first_number("View 1,234 more replies"), Some(1234));
        assert_eq!(first_number("3 replies"), Some(3));
        assert_eq!(first_number("more"), None);
    }

    #[test]
    fn deleted_authors_have_no_name() {
        assert_eq!(username(Some("[deleted]")), None);
        assert_eq!(username(Some("a")), Some(Username::new("a")));
    }
}
