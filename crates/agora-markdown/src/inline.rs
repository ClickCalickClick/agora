//! Passes shared by both entry points: URL checks, text merging, Reddit's
//! spoiler / superscript / mention extensions and HTML whitespace rules.

use crate::{Block, Inline, MAX_DEPTH};

/// Origin used to make root-relative links absolute.
const REDDIT_ORIGIN: &str = "https://www.reddit.com";

/// What a URL will be used for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum UrlUse {
    Link,
    Image,
}

/// Returns an absolute, allowed URL, or `None` when the URL must not be
/// kept (unknown scheme, document-relative, control characters).
///
/// Root-relative paths (`/r/x`) become `https://www.reddit.com/r/x` and
/// protocol-relative ones (`//host/x`) become `https://host/x`.
pub(crate) fn safe_url(raw: &str, usage: UrlUse) -> Option<String> {
    let url = raw.trim();
    if url.is_empty() || url.chars().any(char::is_control) {
        return None;
    }
    if let Some(rest) = url.strip_prefix("//") {
        return (!rest.is_empty() && !rest.starts_with('/')).then(|| format!("https://{rest}"));
    }
    if url.starts_with('/') {
        return Some(format!("{REDDIT_ORIGIN}{url}"));
    }
    let (scheme, rest) = url.split_once(':')?;
    let valid_scheme = scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if !valid_scheme || rest.is_empty() {
        return None;
    }
    let allowed = match scheme.to_ascii_lowercase().as_str() {
        "http" | "https" => true,
        "mailto" => usage == UrlUse::Link,
        _ => false,
    };
    allowed.then(|| url.to_owned())
}

/// Calls `f` on every inline sequence of `blocks` with that sequence's
/// container depth.
pub(crate) fn for_each_inlines(blocks: &mut [Block], depth: usize, f: &mut impl FnMut(&mut Vec<Inline>, usize)) {
    for block in blocks {
        match block {
            Block::Paragraph(content) | Block::Heading { content, .. } => f(content, depth + 1),
            Block::BlockQuote(inner) => for_each_inlines(inner, depth + 1, f),
            Block::List { items, .. } => {
                for item in items {
                    for_each_inlines(&mut item.blocks, depth + 2, f);
                }
            }
            Block::Table { header, rows } => {
                for cell in header
                    .iter_mut()
                    .chain(rows.iter_mut().flat_map(|r| r.cells.iter_mut()))
                {
                    f(&mut cell.content, depth + 2);
                }
            }
            Block::CodeBlock { .. } | Block::ThematicBreak => {}
        }
    }
}

/// The children of an inline container, if it is one.
fn children_mut(inline: &mut Inline) -> Option<&mut Vec<Inline>> {
    match inline {
        Inline::Emphasis(c)
        | Inline::Strong(c)
        | Inline::Strikethrough(c)
        | Inline::Superscript(c)
        | Inline::Spoiler(c)
        | Inline::Link { content: c, .. } => Some(c),
        _ => None,
    }
}

/// Plain text of inlines, as used for image alt text and flattening.
pub(crate) fn text_of(inlines: &[Inline]) -> String {
    fn push(inlines: &[Inline], s: &mut String) {
        for inline in inlines {
            match inline {
                Inline::Text(t) | Inline::Code(t) => s.push_str(t),
                Inline::Image { alt, .. } => s.push_str(alt),
                Inline::SubredditLink(n) => {
                    s.push_str("r/");
                    s.push_str(n);
                }
                Inline::UserLink(n) => {
                    s.push_str("u/");
                    s.push_str(n);
                }
                Inline::LineBreak => s.push(' '),
                Inline::Emphasis(c)
                | Inline::Strong(c)
                | Inline::Strikethrough(c)
                | Inline::Superscript(c)
                | Inline::Spoiler(c)
                | Inline::Link { content: c, .. } => push(c, s),
            }
        }
    }
    let mut s = String::new();
    push(inlines, &mut s);
    s
}

/// Merges adjacent `Text` nodes and drops empty ones, recursively.
pub(crate) fn merge_text(v: &mut Vec<Inline>) {
    let mut out: Vec<Inline> = Vec::with_capacity(v.len());
    for mut inline in v.drain(..) {
        if let Some(children) = children_mut(&mut inline) {
            merge_text(children);
        }
        match (&mut inline, out.last_mut()) {
            (Inline::Text(t), _) if t.is_empty() => {}
            (Inline::Text(t), Some(Inline::Text(prev))) => prev.push_str(t),
            _ => out.push(inline),
        }
    }
    *v = out;
}

/// Removes formatting containers left without content, recursively.
pub(crate) fn prune_empty(v: &mut Vec<Inline>) {
    v.retain_mut(|inline| match inline {
        Inline::Text(t) => !t.is_empty(),
        Inline::Link { url, content } => {
            prune_empty(content);
            if content.is_empty() {
                content.push(Inline::Text(url.clone()));
            }
            true
        }
        other => match children_mut(other) {
            Some(children) => {
                prune_empty(children);
                !children.is_empty()
            }
            None => true,
        },
    });
}

/// A position inside a sibling sequence: (node index, byte offset in it).
type Pos = (usize, usize);

/// Finds `pat` in the `Text` nodes of `v`, starting at `from`.
fn find_text(v: &[Inline], from: Pos, pat: &str) -> Option<Pos> {
    let (mut i, mut off) = from;
    while i < v.len() {
        if let Inline::Text(t) = &v[i]
            && let Some(p) = t.get(off..).and_then(|s| s.find(pat))
        {
            return Some((i, off + p));
        }
        i += 1;
        off = 0;
    }
    None
}

/// Replaces the span from `open` (delimiter of `open_len` bytes) to `close`
/// (delimiter of `close_len` bytes) with `wrap(inner)`. Returns the index of
/// the node after the wrapper.
fn wrap_span(
    v: &mut Vec<Inline>,
    open: Pos,
    open_len: usize,
    close: Pos,
    close_len: usize,
    wrap: impl FnOnce(Vec<Inline>) -> Inline,
) -> usize {
    let (i, a) = open;
    let (j, b) = close;
    let text_at = |v: &Vec<Inline>, k: usize| match &v[k] {
        Inline::Text(t) => t.clone(),
        // Delimiters are only ever found in text nodes.
        _ => String::new(),
    };
    let first = text_at(v, i);
    let before = first[..a].to_owned();
    let mut inner = Vec::new();
    let after = if i == j {
        inner.push(Inline::Text(first[a + open_len..b].to_owned()));
        first[b + close_len..].to_owned()
    } else {
        let last = text_at(v, j);
        inner.push(Inline::Text(first[a + open_len..].to_owned()));
        inner.extend(v[i + 1..j].iter().cloned());
        inner.push(Inline::Text(last[..b].to_owned()));
        last[b + close_len..].to_owned()
    };
    merge_text(&mut inner);
    let mut replacement = Vec::with_capacity(3);
    if !before.is_empty() {
        replacement.push(Inline::Text(before));
    }
    replacement.push(wrap(inner));
    let after_index = i + replacement.len();
    if !after.is_empty() {
        replacement.push(Inline::Text(after));
    }
    v.splice(i..=j, replacement);
    after_index
}

/// Reddit spoilers: `>!text!<` becomes [`Inline::Spoiler`]. The delimiters
/// must sit in text nodes of the same sequence; text inside inline code is
/// never touched because code is not a `Text` node.
pub(crate) fn spoilers(v: &mut Vec<Inline>, depth: usize) {
    for inline in v.iter_mut() {
        if let Some(children) = children_mut(inline) {
            spoilers(children, depth + 1);
        }
    }
    if depth >= MAX_DEPTH {
        return;
    }
    let mut pos = (0, 0);
    while let Some(open) = find_text(v, pos, ">!") {
        let Some(close) = find_text(v, (open.0, open.1 + 2), "!<") else {
            break;
        };
        if close == (open.0, open.1 + 2) {
            // `>!!<` has nothing to hide; leave it as text.
            pos = (close.0, close.1 + 2);
            continue;
        }
        let next = wrap_span(v, open, 2, close, 2, Inline::Spoiler);
        pos = (next, 0);
    }
}

/// Finds the `)` that closes a `^(` whose content starts at `from`.
fn find_close_paren(v: &[Inline], from: Pos) -> Option<Pos> {
    let mut level = 0usize;
    let (mut i, mut off) = from;
    while i < v.len() {
        if let Inline::Text(t) = &v[i] {
            for (p, c) in t.get(off..).unwrap_or("").char_indices() {
                match c {
                    '(' => level += 1,
                    ')' if level == 0 => return Some((i, off + p)),
                    ')' => level -= 1,
                    _ => {}
                }
            }
        }
        i += 1;
        off = 0;
    }
    None
}

/// Reddit superscript: `^word` (up to the next whitespace) and
/// `^(several words)`. `^^x` nests. A `^` followed by whitespace or the end
/// of the text stays literal.
pub(crate) fn superscripts(v: &mut Vec<Inline>, depth: usize) {
    for inline in v.iter_mut() {
        if let Some(children) = children_mut(inline) {
            superscripts(children, depth + 1);
        }
    }
    scan_superscripts(v, depth);
}

fn scan_superscripts(v: &mut Vec<Inline>, depth: usize) {
    if depth >= MAX_DEPTH {
        return;
    }
    let mut i = 0;
    let mut off = 0;
    while i < v.len() {
        let Inline::Text(t) = &v[i] else {
            i += 1;
            off = 0;
            continue;
        };
        let Some(p) = t.get(off..).and_then(|s| s.find('^')) else {
            i += 1;
            off = 0;
            continue;
        };
        let caret = off + p;
        let rest = &t[caret + 1..];
        if rest.starts_with('(')
            && let Some(close) = find_close_paren(v, (i, caret + 2))
            && close != (i, caret + 2)
        {
            i = wrap_span(v, (i, caret), 2, close, 1, |mut inner| {
                scan_superscripts(&mut inner, depth + 1);
                Inline::Superscript(inner)
            });
            off = 0;
            continue;
        }
        let word_len = rest.find(char::is_whitespace).unwrap_or(rest.len());
        if word_len == 0 {
            off = caret + 1;
            continue;
        }
        let before = t[..caret].to_owned();
        let word = rest[..word_len].to_owned();
        let after = rest[word_len..].to_owned();
        let mut inner = vec![Inline::Text(word)];
        scan_superscripts(&mut inner, depth + 1);
        let mut replacement = Vec::with_capacity(3);
        if !before.is_empty() {
            replacement.push(Inline::Text(before));
        }
        replacement.push(Inline::Superscript(inner));
        let next = i + replacement.len();
        if !after.is_empty() {
            replacement.push(Inline::Text(after));
        }
        v.splice(i..=i, replacement);
        i = next;
        off = 0;
    }
}

/// A mention found in text.
struct Mention {
    start: usize,
    end: usize,
    user: bool,
    name: String,
}

/// Characters that, directly before `r/` or `/r/`, mean the text is part of
/// a word, path or URL rather than a mention.
fn blocks_mention(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(
            c,
            '_' | '/' | '-' | '.' | '@' | '%' | '&' | '=' | '?' | '#' | '+' | '~' | ':' | '\\'
        )
}

fn find_mention(t: &str) -> Option<Mention> {
    let bytes = t.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let b = bytes[i];
        if (b == b'r' || b == b'u') && bytes[i + 1] == b'/' {
            let user = b == b'u';
            let start = if i > 0 && bytes[i - 1] == b'/' { i - 1 } else { i };
            let boundary = !t[..start].chars().next_back().is_some_and(blocks_mention);
            if boundary {
                let is_name = |c: char| c.is_ascii_alphanumeric() || c == '_' || (user && c == '-');
                let name_start = i + 2;
                let name_len = t[name_start..]
                    .find(|c: char| !is_name(c))
                    .unwrap_or(t.len() - name_start);
                let end = name_start + name_len;
                let (min, max) = if user { (3, 20) } else { (2, 21) };
                let continues = t[end..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-');
                if (min..=max).contains(&name_len) && !continues {
                    return Some(Mention {
                        start,
                        end,
                        user,
                        name: t[name_start..end].to_owned(),
                    });
                }
            }
        }
        i += 1;
    }
    None
}

/// Bare `r/name`, `/r/name`, `u/name` and `/u/name` in text become
/// [`Inline::SubredditLink`] / [`Inline::UserLink`]. Text inside links and
/// code is left alone.
pub(crate) fn mentions(v: &mut Vec<Inline>) {
    let mut out = Vec::with_capacity(v.len());
    for mut inline in v.drain(..) {
        match &mut inline {
            Inline::Link { .. } => out.push(inline),
            Inline::Text(t) => {
                let mut rest = t.as_str();
                while let Some(m) = find_mention(rest) {
                    if m.start > 0 {
                        out.push(Inline::Text(rest[..m.start].to_owned()));
                    }
                    out.push(if m.user {
                        Inline::UserLink(m.name)
                    } else {
                        Inline::SubredditLink(m.name)
                    });
                    rest = &rest[m.end..];
                }
                if !rest.is_empty() {
                    out.push(Inline::Text(rest.to_owned()));
                }
            }
            other => {
                if let Some(children) = children_mut(other) {
                    mentions(children);
                }
                out.push(inline);
            }
        }
    }
    *v = out;
}

/// `true` for the characters HTML treats as inter-element whitespace.
pub(crate) fn is_html_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}')
}

/// Collapses each run of HTML whitespace to one space.
pub(crate) fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_space = false;
    for c in s.chars() {
        if is_html_space(c) {
            if !in_space {
                out.push(' ');
            }
            in_space = true;
        } else {
            out.push(c);
            in_space = false;
        }
    }
    out
}

/// Browser-like whitespace handling for one block of collapsed text: a
/// space after a space (even across elements and merged nodes) is dropped, and
/// spaces at the start and end of the block and around line breaks go.
pub(crate) fn trim_block_whitespace(v: &mut Vec<Inline>) {
    fn forward(v: &mut [Inline], prev_space: &mut bool) {
        for inline in v {
            match inline {
                Inline::Text(t) => {
                    let mut out = String::with_capacity(t.len());
                    for c in t.chars() {
                        if c == ' ' {
                            if !*prev_space {
                                out.push(c);
                            }
                            *prev_space = true;
                        } else {
                            out.push(c);
                            *prev_space = false;
                        }
                    }
                    *t = out;
                }
                Inline::LineBreak => *prev_space = true,
                other => match children_mut(other) {
                    Some(children) => forward(children, prev_space),
                    None => *prev_space = false,
                },
            }
        }
    }
    fn backward(v: &mut [Inline], next_break: &mut bool) {
        for inline in v.iter_mut().rev() {
            match inline {
                Inline::Text(t) => {
                    if *next_break && t.ends_with(' ') {
                        t.pop();
                    }
                    if !t.is_empty() {
                        *next_break = false;
                    }
                }
                Inline::LineBreak => *next_break = true,
                other => match children_mut(other) {
                    Some(children) => backward(children, next_break),
                    None => *next_break = false,
                },
            }
        }
    }
    forward(v, &mut true);
    backward(v, &mut true);
    prune_empty(v);
    merge_text(v);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Inline {
        Inline::Text(s.into())
    }

    #[test]
    fn url_rules() {
        let link = |u| safe_url(u, UrlUse::Link);
        assert_eq!(link("https://example.com/a").as_deref(), Some("https://example.com/a"));
        assert_eq!(link(" HTTP://x.org ").as_deref(), Some("HTTP://x.org"));
        assert_eq!(link("mailto:a@b.c").as_deref(), Some("mailto:a@b.c"));
        assert_eq!(safe_url("mailto:a@b.c", UrlUse::Image), None);
        assert_eq!(
            link("/r/rust/comments/x").as_deref(),
            Some("https://www.reddit.com/r/rust/comments/x")
        );
        assert_eq!(link("//i.redd.it/a.png").as_deref(), Some("https://i.redd.it/a.png"));
        assert_eq!(link("javascript:alert(1)"), None);
        assert_eq!(link("JavaScript:alert(1)"), None);
        assert_eq!(link("java\tscript:alert(1)"), None);
        assert_eq!(link("java script:alert(1)"), None);
        assert_eq!(link("data:text/html,x"), None);
        assert_eq!(link("vbscript:x"), None);
        assert_eq!(link("file:///etc/passwd"), None);
        assert_eq!(link("relative/path"), None);
        assert_eq!(link("#anchor"), None);
        assert_eq!(link(""), None);
        assert_eq!(link("https:"), None);
        assert_eq!(link("///x"), None);
    }

    #[test]
    fn merge_and_prune() {
        let mut v = vec![
            t("a"),
            t(""),
            t("b"),
            Inline::Emphasis(vec![t("")]),
            Inline::Link {
                url: "https://x.y".into(),
                content: vec![],
            },
        ];
        prune_empty(&mut v);
        merge_text(&mut v);
        assert_eq!(
            v,
            vec![
                t("ab"),
                Inline::Link {
                    url: "https://x.y".into(),
                    content: vec![t("https://x.y")],
                }
            ]
        );
    }

    #[test]
    fn mention_boundaries() {
        let names = |s: &str| {
            let mut v = vec![t(s)];
            mentions(&mut v);
            v
        };
        assert_eq!(
            names("see r/rust."),
            vec![t("see "), Inline::SubredditLink("rust".into()), t(".")]
        );
        assert_eq!(names("/u/spez"), vec![Inline::UserLink("spez".into())]);
        assert_eq!(names("u/a-b_c"), vec![Inline::UserLink("a-b_c".into())]);
        assert_eq!(names("r/a"), vec![t("r/a")], "subreddit names need 2+ chars");
        assert_eq!(names("u/ab"), vec![t("u/ab")], "user names need 3+ chars");
        assert_eq!(
            names("r/abcdefghijklmnopqrstuvwxyz"),
            vec![t("r/abcdefghijklmnopqrstuvwxyz")]
        );
        assert_eq!(names("u/abcdefghijklmnopqrstu"), vec![t("u/abcdefghijklmnopqrstu")]);
        assert_eq!(names("our/stuff"), vec![t("our/stuff")]);
        assert_eq!(names("example.com/r/rust"), vec![t("example.com/r/rust")]);
        assert_eq!(names("a/r/rust"), vec![t("a/r/rust")]);
        assert_eq!(names("R/rust"), vec![t("R/rust")]);
        assert_eq!(names("r/rust-lang"), vec![t("r/rust-lang")], "no '-' in subreddits");
        assert_eq!(names("r/rusté"), vec![t("r/rusté")]);
        assert_eq!(
            names("(r/gnome, u/foo_bar)"),
            vec![
                t("("),
                Inline::SubredditLink("gnome".into()),
                t(", "),
                Inline::UserLink("foo_bar".into()),
                t(")")
            ]
        );
        assert_eq!(names("r/"), vec![t("r/")]);
        assert_eq!(names("/r/"), vec![t("/r/")]);
    }

    #[test]
    fn whitespace_trim() {
        let mut v = vec![
            t(" a "),
            Inline::Emphasis(vec![t(" b ")]),
            t(" "),
            Inline::LineBreak,
            t(" c "),
        ];
        trim_block_whitespace(&mut v);
        assert_eq!(
            v,
            vec![t("a "), Inline::Emphasis(vec![t("b")]), Inline::LineBreak, t("c")]
        );
        assert_eq!(collapse_ws("a \n\t b\u{a0}c"), "a b\u{a0}c");
    }
}
