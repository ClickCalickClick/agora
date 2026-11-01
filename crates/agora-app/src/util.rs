//! Small formatting helpers shared by the UI.

use gettextrs::gettext;
use gtk::glib;

use crate::i18n::{i18n_f, ni18n_f};

/// Seconds since the Unix epoch, now.
pub fn now() -> i64 {
    glib::DateTime::now_utc().map(|d| d.to_unix()).unwrap_or(0)
}

/// Short age for row subtitles: "now", "5m", "3h", "2d", "4mo", "1y".
pub fn short_age(created: i64) -> String {
    let secs = (now() - created).max(0);
    let (n, unit) = match secs {
        0..60 => return gettext("now"),
        60..3600 => (secs / 60, gettext("{}m")),
        3600..86_400 => (secs / 3600, gettext("{}h")),
        86_400..2_592_000 => (secs / 86_400, gettext("{}d")),
        2_592_000..31_536_000 => (secs / 2_592_000, gettext("{}mo")),
        _ => (secs / 31_536_000, gettext("{}y")),
    };
    unit.replace("{}", &n.to_string())
}

/// Spoken age for accessible descriptions: "3 hours ago".
pub fn long_age(created: i64) -> String {
    let secs = (now() - created).max(0);
    let n = |d: i64| (secs / d) as u32;
    match secs {
        0..60 => gettext("just now"),
        60..3600 => ni18n_f("{} minute ago", "{} minutes ago", n(60), &[&n(60).to_string()]),
        3600..86_400 => ni18n_f("{} hour ago", "{} hours ago", n(3600), &[&n(3600).to_string()]),
        86_400..2_592_000 => ni18n_f("{} day ago", "{} days ago", n(86_400), &[&n(86_400).to_string()]),
        2_592_000..31_536_000 => ni18n_f(
            "{} month ago",
            "{} months ago",
            n(2_592_000),
            &[&n(2_592_000).to_string()],
        ),
        _ => ni18n_f(
            "{} year ago",
            "{} years ago",
            n(31_536_000),
            &[&n(31_536_000).to_string()],
        ),
    }
}

/// Compact count: 999, 1.2k, 12k, 1.4M.
pub fn compact(n: i64) -> String {
    let a = n.unsigned_abs();
    let sign = if n < 0 { "-" } else { "" };
    if a < 1000 {
        format!("{n}")
    } else if a < 10_000 {
        let tenths = a / 100;
        if tenths.is_multiple_of(10) {
            format!("{sign}{}k", tenths / 10)
        } else {
            format!("{sign}{}.{}k", tenths / 10, tenths % 10)
        }
    } else if a < 1_000_000 {
        format!("{sign}{}k", a / 1000)
    } else {
        let tenths = a / 100_000;
        format!("{sign}{}.{}M", tenths / 10, tenths % 10)
    }
}

/// "1,204 points" style spoken score.
pub fn spoken_points(score: i64) -> String {
    ni18n_f(
        "{} point",
        "{} points",
        score.unsigned_abs().min(u32::MAX as u64) as u32,
        &[&score.to_string()],
    )
}

/// "12 comments".
pub fn spoken_comments(count: u64) -> String {
    ni18n_f(
        "{} comment",
        "{} comments",
        count.min(u32::MAX as u64) as u32,
        &[&count.to_string()],
    )
}

/// Members line for a subreddit header.
pub fn members(count: u64) -> String {
    i18n_f("{} members", &[&compact(count as i64)])
}

/// Full https URL for a reddit.com path.
pub fn reddit_url(path: &str) -> String {
    if path.starts_with("http") {
        path.to_string()
    } else {
        format!("https://www.reddit.com{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_numbers() {
        assert_eq!(compact(0), "0");
        assert_eq!(compact(999), "999");
        assert_eq!(compact(1000), "1k");
        assert_eq!(compact(1250), "1.2k");
        assert_eq!(compact(-1250), "-1.2k");
        assert_eq!(compact(12_400), "12k");
        assert_eq!(compact(1_450_000), "1.4M");
    }

    #[test]
    fn ages() {
        assert_eq!(short_age(now()), "now");
        assert_eq!(short_age(now() - 3 * 3600), "3h");
        assert_eq!(long_age(now() - 2 * 86_400), "2 days ago");
    }
}
