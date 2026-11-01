//! Reddit post and comment bodies as one display-free document AST.
//!
//! Reddit's front-end partials return bodies as HTML, while the composer
//! preview and mock data use Reddit-flavoured Markdown. Both go through this
//! crate and come out as the same [`Document`] (SPEC §4.5 "Body format",
//! §7.6), which the UI turns into native widgets.
//!
//! * [`html_to_ast`] parses an HTML fragment with an allow-list of tags.
//! * [`markdown_to_ast`] parses Markdown with `pulldown-cmark` plus Reddit's
//!   extensions: spoilers (`>!text!<`), superscript (`^word`, `^(a few
//!   words)`) and `r/name` / `u/name` mentions.
//!
//! Hostile input cannot overflow the stack: nesting deeper than
//! [`MAX_DEPTH`] containers is flattened to plain text.

#![warn(missing_docs)]

mod html;
mod inline;
mod markdown;

use serde::{Deserialize, Serialize};

pub use html::html_to_ast;
pub use markdown::markdown_to_ast;

/// Deepest container nesting (block quotes, lists, emphasis, …) kept in a
/// [`Document`]. Content nested deeper is flattened to plain text.
pub const MAX_DEPTH: usize = 32;

/// A parsed post or comment body.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    /// Top-level blocks, in reading order.
    pub blocks: Vec<Block>,
}

/// A block-level node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Block {
    /// A paragraph of inline content.
    Paragraph(Vec<Inline>),
    /// A heading; `level` is 1 to 6.
    Heading {
        /// Heading level, 1 (largest) to 6.
        level: u8,
        /// Heading text.
        content: Vec<Inline>,
    },
    /// Preformatted code, whitespace preserved, without the final newline.
    CodeBlock {
        /// Language from a fenced block's info string or a
        /// `class="language-x"` attribute.
        lang: Option<String>,
        /// The code itself.
        text: String,
    },
    /// A block quote.
    BlockQuote(Vec<Block>),
    /// A bulleted or numbered list.
    List {
        /// `true` for a numbered list.
        ordered: bool,
        /// First number of an ordered list (defaults to 1); `None` when
        /// unordered.
        start: Option<u64>,
        /// The list items.
        items: Vec<ListItem>,
    },
    /// A table. `header` may be empty for HTML tables without header cells.
    Table {
        /// Header cells.
        header: Vec<TableCell>,
        /// Body rows.
        rows: Vec<TableRow>,
    },
    /// A horizontal rule.
    ThematicBreak,
}

/// One item of a [`Block::List`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItem {
    /// The item's content.
    pub blocks: Vec<Block>,
}

/// One body row of a [`Block::Table`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRow {
    /// The row's cells.
    pub cells: Vec<TableCell>,
}

/// One cell of a table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableCell {
    /// Cell text.
    pub content: Vec<Inline>,
    /// Horizontal alignment.
    pub align: Align,
}

/// Horizontal alignment of a table cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Align {
    /// No alignment given.
    #[default]
    None,
    /// Left aligned.
    Left,
    /// Centred.
    Center,
    /// Right aligned.
    Right,
}

/// An inline node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Inline {
    /// Plain text.
    Text(String),
    /// Emphasis (usually italic).
    Emphasis(Vec<Inline>),
    /// Strong emphasis (usually bold).
    Strong(Vec<Inline>),
    /// Struck-through text.
    Strikethrough(Vec<Inline>),
    /// Inline code.
    Code(String),
    /// A hyperlink. `url` is always absolute `http`, `https` or `mailto`.
    Link {
        /// Absolute target URL.
        url: String,
        /// Link text.
        content: Vec<Inline>,
    },
    /// An embedded image. `url` is always absolute `http` or `https`.
    Image {
        /// Absolute image URL.
        url: String,
        /// Alternative text.
        alt: String,
    },
    /// Superscript text.
    Superscript(Vec<Inline>),
    /// Spoiler text, hidden until revealed.
    Spoiler(Vec<Inline>),
    /// A bare subreddit mention; holds the name without the `r/` prefix.
    SubredditLink(String),
    /// A bare user mention; holds the name without the `u/` prefix.
    UserLink(String),
    /// A hard line break.
    LineBreak,
}

impl Document {
    /// `true` when the document has no blocks.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// The document as plain text, for accessible labels and row previews.
    ///
    /// Blocks are separated by newlines, list items are prefixed with `• `
    /// or their number, table cells are joined with ` | `, images give
    /// their alt text, and spoiler content is replaced by `[spoiler]` so a
    /// preview never reveals it.
    pub fn plain_text(&self) -> String {
        let mut lines = Vec::new();
        blocks_text(&self.blocks, &mut lines);
        lines.join("\n")
    }
}

fn blocks_text(blocks: &[Block], out: &mut Vec<String>) {
    for block in blocks {
        match block {
            Block::Paragraph(content) | Block::Heading { content, .. } => {
                out.push(inlines_text(content));
            }
            Block::CodeBlock { text, .. } => out.push(text.clone()),
            Block::BlockQuote(inner) => blocks_text(inner, out),
            Block::List { ordered, start, items } => {
                let first = start.unwrap_or(1);
                for (i, item) in items.iter().enumerate() {
                    let marker = if *ordered {
                        format!("{}. ", first.saturating_add(i as u64))
                    } else {
                        "• ".to_owned()
                    };
                    let mut inner = Vec::new();
                    blocks_text(&item.blocks, &mut inner);
                    out.push(format!("{marker}{}", inner.join("\n")));
                }
            }
            Block::Table { header, rows } => {
                if !header.is_empty() {
                    out.push(cells_text(header));
                }
                for row in rows {
                    out.push(cells_text(&row.cells));
                }
            }
            Block::ThematicBreak => {}
        }
    }
}

fn cells_text(cells: &[TableCell]) -> String {
    cells
        .iter()
        .map(|c| inlines_text(&c.content))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn inlines_text(inlines: &[Inline]) -> String {
    let mut s = String::new();
    push_inlines_text(inlines, &mut s);
    s
}

fn push_inlines_text(inlines: &[Inline], s: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Text(t) | Inline::Code(t) => s.push_str(t),
            Inline::Emphasis(c)
            | Inline::Strong(c)
            | Inline::Strikethrough(c)
            | Inline::Superscript(c)
            | Inline::Link { content: c, .. } => push_inlines_text(c, s),
            Inline::Image { alt, .. } => s.push_str(alt),
            Inline::Spoiler(_) => s.push_str("[spoiler]"),
            Inline::SubredditLink(name) => {
                s.push_str("r/");
                s.push_str(name);
            }
            Inline::UserLink(name) => {
                s.push_str("u/");
                s.push_str(name);
            }
            Inline::LineBreak => s.push('\n'),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_document() {
        let doc = Document::default();
        assert!(doc.is_empty());
        assert_eq!(doc.plain_text(), "");
    }

    #[test]
    fn plain_text_of_every_node() {
        let doc = Document {
            blocks: vec![
                Block::Heading {
                    level: 1,
                    content: vec![Inline::Text("Title".into())],
                },
                Block::Paragraph(vec![
                    Inline::Text("a ".into()),
                    Inline::Strong(vec![Inline::Text("b".into())]),
                    Inline::Text(" ".into()),
                    Inline::Code("c".into()),
                    Inline::LineBreak,
                    Inline::Spoiler(vec![Inline::Text("secret".into())]),
                    Inline::Text(" ".into()),
                    Inline::SubredditLink("rust".into()),
                    Inline::Text(" ".into()),
                    Inline::UserLink("spez".into()),
                    Inline::Image {
                        url: "https://i.redd.it/x.png".into(),
                        alt: "pic".into(),
                    },
                ]),
                Block::List {
                    ordered: true,
                    start: Some(3),
                    items: vec![
                        ListItem {
                            blocks: vec![Block::Paragraph(vec![Inline::Text("x".into())])],
                        },
                        ListItem {
                            blocks: vec![Block::Paragraph(vec![Inline::Text("y".into())])],
                        },
                    ],
                },
                Block::List {
                    ordered: false,
                    start: None,
                    items: vec![ListItem {
                        blocks: vec![Block::Paragraph(vec![Inline::Text("z".into())])],
                    }],
                },
                Block::ThematicBreak,
                Block::Table {
                    header: vec![
                        TableCell {
                            content: vec![Inline::Text("h1".into())],
                            align: Align::Left,
                        },
                        TableCell {
                            content: vec![Inline::Text("h2".into())],
                            align: Align::Right,
                        },
                    ],
                    rows: vec![TableRow {
                        cells: vec![
                            TableCell {
                                content: vec![Inline::Text("1".into())],
                                align: Align::Left,
                            },
                            TableCell {
                                content: vec![Inline::Text("2".into())],
                                align: Align::Right,
                            },
                        ],
                    }],
                },
                Block::BlockQuote(vec![Block::CodeBlock {
                    lang: None,
                    text: "fn main() {}".into(),
                }]),
            ],
        };
        assert!(!doc.is_empty());
        assert_eq!(
            doc.plain_text(),
            "Title\na b c\n[spoiler] r/rust u/spezpic\n3. x\n4. y\n• z\nh1 | h2\n1 | 2\nfn main() {}"
        );
    }

    #[test]
    fn serde_round_trip() {
        let doc = markdown_to_ast("# Hi\n\n*a* >!b!< ^c r/rust\n\n| x |\n|---|\n| 1 |\n");
        let json = serde_json::to_string(&doc).unwrap();
        let back: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(doc, back);
    }
}
