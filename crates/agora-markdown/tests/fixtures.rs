//! Real (scrubbed) comment bodies captured from reddit.com partials.

use std::path::{Path, PathBuf};

use agora_markdown::{Block, Document, Inline, ListItem, html_to_ast, markdown_to_ast};

fn data(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn t(s: &str) -> Inline {
    Inline::Text(s.into())
}

fn p(s: &str) -> Block {
    Block::Paragraph(vec![t(s)])
}

#[test]
fn mod_bot_comment() {
    // Wrapped in <shreddit-mod-comment><span><div>…; links carry Reddit's
    // `rpl` attribute and a root-relative compose link.
    let doc = html_to_ast(&data("comment_mod_bot.html"));
    assert_eq!(
        doc.blocks,
        vec![
            p("Thank you for your submission."),
            Block::Paragraph(vec![
                t("You too can support GNOME! Become a "),
                Inline::Link {
                    url: "https://donate.gnome.org".into(),
                    content: vec![t("Friend of GNOME")],
                },
                t(" and contribute to keeping our project going!"),
            ]),
            Block::Paragraph(vec![
                Inline::Emphasis(vec![t(
                    "I am a bot, and this action was performed automatically. Please "
                )]),
                Inline::Link {
                    url: "https://www.reddit.com/message/compose/?to=/r/gnome".into(),
                    content: vec![Inline::Emphasis(vec![t("contact the moderators of this subreddit")])],
                },
                Inline::Emphasis(vec![t(" if you have any questions or concerns.")]),
            ]),
        ]
    );
}

#[test]
fn image_comment() {
    // <figure class="rte-media"><a href><img srcset …></a></figure><p>…
    let doc = html_to_ast(&data("comment_image.html"));
    let url = "https://preview.redd.it/adwair-icon-theme-release-v0-f6q5zpyjqtsh1.png?width=1187&format=png&auto=webp&s=162b66156f9ce0377ed4fc2eb997a3539b7dab74";
    assert_eq!(
        doc.blocks,
        vec![
            Block::Paragraph(vec![Inline::Link {
                url: url.into(),
                content: vec![Inline::Image {
                    url: url.into(),
                    alt: "Comment Image".into(),
                }],
            }]),
            p("i really like the gtk-theme, its really cool"),
        ]
    );
}

#[test]
fn ordered_list_comment() {
    let doc = html_to_ast(&data("comment_ordered_list.html"));
    assert_eq!(
        doc.blocks,
        vec![
            p("Hi, this looks good. 3 questions:"),
            Block::List {
                ordered: true,
                start: Some(1),
                items: vec![
                    ListItem {
                        blocks: vec![p("How did you set the accent color to the file manager?")],
                    },
                    ListItem {
                        blocks: vec![p("Is it blur-my-shell that is used to give the blur effect?")],
                    },
                    ListItem {
                        blocks: vec![p("How did you make the corners sharp?")],
                    },
                ],
            },
        ]
    );
    assert_eq!(
        doc.plain_text(),
        "Hi, this looks good. 3 questions:\n\
         1. How did you set the accent color to the file manager?\n\
         2. Is it blur-my-shell that is used to give the blur effect?\n\
         3. How did you make the corners sharp?"
    );
}

#[test]
fn strong_comment_with_entity() {
    let doc = html_to_ast(&data("comment_strong.html"));
    assert_eq!(
        doc.blocks,
        vec![Block::Paragraph(vec![
            t("I said "),
            Inline::Strong(vec![t("ORIGINAL")]),
            t(
                " thought. We all know that vibe coders haven't had an original thought since \
               the second Bush Administration."
            ),
        ])]
    );
}

#[test]
fn markdown_and_html_agree() {
    let from_md = markdown_to_ast(
        "Hi, this looks good. 3 questions:\n\n\
         1. How did you set the accent color to the file manager?\n\n\
         2. Is it blur-my-shell that is used to give the blur effect?\n\n\
         3. How did you make the corners sharp?\n",
    );
    assert_eq!(from_md, html_to_ast(&data("comment_ordered_list.html")));

    let from_md = markdown_to_ast(
        "I said **ORIGINAL** thought. We all know that vibe coders haven't had an original \
         thought since the second Bush Administration.",
    );
    assert_eq!(from_md, html_to_ast(&data("comment_strong.html")));
}

/// Inner HTML of every `<div … slot="comment">` in a page.
fn comment_bodies(page: &str) -> Vec<&str> {
    let mut bodies = Vec::new();
    let mut rest = page;
    while let Some(slot) = rest.find("slot=\"comment\"") {
        let tag_start = rest[..slot].rfind('<').expect("attribute inside a tag");
        let open_end = slot + rest[slot..].find('>').expect("tag end") + 1;
        if !rest[tag_start..].starts_with("<div") {
            rest = &rest[open_end..];
            continue;
        }
        let body = &rest[open_end..];
        let mut depth = 1usize;
        let mut i = 0;
        let end = loop {
            let next_open = body[i..].find("<div").map(|p| p + i);
            let next_close = body[i..].find("</div").map(|p| p + i).expect("closing div");
            match next_open {
                Some(o) if o < next_close => {
                    depth += 1;
                    i = o + 4;
                }
                _ => {
                    depth -= 1;
                    if depth == 0 {
                        break next_close;
                    }
                    i = next_close + 5;
                }
            }
        };
        bodies.push(&body[..end]);
        rest = &body[end..];
    }
    bodies
}

fn assert_clean(doc: &Document) {
    let text = doc.plain_text();
    assert!(!doc.is_empty(), "empty body");
    assert!(!text.contains('<') && !text.contains("&#"), "markup leaked: {text:?}");
    assert!(!text.starts_with(' ') && !text.ends_with(' '), "untrimmed: {text:?}");
    assert!(!text.contains("  "), "uncollapsed whitespace: {text:?}");
}

#[test]
fn every_comment_body_in_the_captured_pages() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../agora-reddit-web/tests/fixtures");
    let mut seen = 0;
    for name in ["post_comments.html", "more_comments.html"] {
        let Ok(page) = std::fs::read_to_string(dir.join(name)) else {
            eprintln!("skipping {name}: fixture not present");
            continue;
        };
        for body in comment_bodies(&page) {
            assert_clean(&html_to_ast(body));
            seen += 1;
        }
    }
    if dir.exists() {
        assert!(seen >= 10, "found only {seen} comment bodies");
    }
}
