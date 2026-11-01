//! Reddit-flavoured Markdown to [`Document`].

use pulldown_cmark::{Alignment, CodeBlockKind, Event, LinkType, Options, Parser, Tag, TagEnd, TextMergeStream};

use crate::inline::{
    UrlUse, for_each_inlines, mentions, merge_text, prune_empty, safe_url, spoilers, superscripts, text_of,
};
use crate::{Align, Block, Document, Inline, ListItem, MAX_DEPTH, TableCell, TableRow};

/// Parses Reddit-flavoured Markdown (the composer preview and mock data).
///
/// CommonMark plus tables and `~~strikethrough~~`, with Reddit's spoilers
/// (`>!text!<`, also at the start of a line), superscript (`^word`,
/// `^(a few words)`) and bare `r/name` / `u/name` mentions. Raw HTML is
/// kept as literal text. Links and images keep only `http`, `https` (and,
/// for links, `mailto`) targets; root-relative links such as `/r/rust` are
/// made absolute on `https://www.reddit.com`.
pub fn markdown_to_ast(md: &str) -> Document {
    let source = escape_line_start_spoilers(md);
    let mut builder = Builder::default();
    for event in TextMergeStream::new(Parser::new_ext(&source, options())) {
        builder.event(event);
    }
    let mut doc = builder.finish();
    for_each_inlines(&mut doc.blocks, 0, &mut |v, depth| {
        merge_text(v);
        spoilers(v, depth);
        superscripts(v, depth);
        mentions(v);
        prune_empty(v);
        merge_text(v);
    });
    doc
}

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH
}

/// `>!spoiler!<` at the start of a line would otherwise open a block quote.
/// Escapes that `>` (outside code and HTML blocks) when the line also holds
/// the closing `!<`.
fn escape_line_start_spoilers(md: &str) -> std::borrow::Cow<'_, str> {
    if !md.contains(">!") {
        return md.into();
    }
    let protected: Vec<std::ops::Range<usize>> = Parser::new_ext(md, options())
        .into_offset_iter()
        .filter_map(|(event, range)| match event {
            Event::Start(Tag::CodeBlock(_) | Tag::HtmlBlock) | Event::Code(_) => Some(range),
            _ => None,
        })
        .collect();
    let mut inserts = Vec::new();
    let mut line_start = 0;
    for line in md.split_inclusive('\n') {
        let bytes = line.as_bytes();
        let mut p = 0;
        while p < bytes.len() && p < 3 && bytes[p] == b' ' {
            p += 1;
        }
        loop {
            if line[p..].starts_with(">!") {
                let at = line_start + p;
                if line[p + 2..].contains("!<") && !protected.iter().any(|r| r.contains(&at)) {
                    inserts.push(at);
                }
                break;
            } else if bytes.get(p) == Some(&b'>') {
                p += 1;
                if matches!(bytes.get(p), Some(b' ' | b'\t')) {
                    p += 1;
                }
            } else {
                break;
            }
        }
        line_start += line.len();
    }
    if inserts.is_empty() {
        return md.into();
    }
    let mut out = String::with_capacity(md.len() + inserts.len());
    let mut last = 0;
    for at in inserts {
        out.push_str(&md[last..at]);
        out.push('\\');
        last = at;
    }
    out.push_str(&md[last..]);
    out.into()
}

/// An inline container being built.
enum Span {
    Emphasis,
    Strong,
    Strikethrough,
    /// `None` when the target is not allowed; the text is kept.
    Link(Option<String>),
    /// `None` when the source is not allowed; the alt text is kept.
    Image(Option<String>),
}

/// One open container.
enum Frame {
    Root(Vec<Block>),
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<ListItem>,
    },
    Item(Vec<Block>),
    /// `implicit` paragraphs wrap the bare text of tight list items.
    Para {
        inlines: Vec<Inline>,
        implicit: bool,
    },
    Heading {
        level: u8,
        inlines: Vec<Inline>,
    },
    Code {
        lang: Option<String>,
        text: String,
    },
    Table {
        aligns: Vec<Align>,
        header: Vec<TableCell>,
        rows: Vec<TableRow>,
    },
    Row {
        cells: Vec<TableCell>,
        head: bool,
    },
    Cell(Vec<Inline>),
    Span {
        kind: Span,
        inlines: Vec<Inline>,
    },
    /// Content nested deeper than [`MAX_DEPTH`], kept as plain text.
    /// `nesting` counts the tags opened inside it.
    Flat {
        text: String,
        nesting: usize,
    },
}

struct Builder {
    stack: Vec<Frame>,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            stack: vec![Frame::Root(Vec::new())],
        }
    }
}

fn heading_level(level: pulldown_cmark::HeadingLevel) -> u8 {
    level as u8
}

fn align(a: Alignment) -> Align {
    match a {
        Alignment::None => Align::None,
        Alignment::Left => Align::Left,
        Alignment::Center => Align::Center,
        Alignment::Right => Align::Right,
    }
}

/// Plain text of a block, for content flattened into inline context.
fn block_text(block: &Block) -> String {
    Document {
        blocks: vec![block.clone()],
    }
    .plain_text()
}

impl Builder {
    fn top(&mut self) -> &mut Frame {
        self.stack.last_mut().expect("root frame is never popped")
    }

    fn in_flat(&mut self) -> bool {
        matches!(self.top(), Frame::Flat { .. })
    }

    fn close_implicit_para(&mut self) {
        if matches!(self.top(), Frame::Para { implicit: true, .. }) {
            self.close_top();
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(s) => {
                if let Frame::Code { text, .. } = self.top() {
                    text.push_str(&s);
                } else {
                    self.push_inline(Inline::Text(s.into_string()));
                }
            }
            Event::Code(s) => self.push_inline(Inline::Code(s.into_string())),
            Event::InlineMath(s) | Event::DisplayMath(s) => {
                self.push_inline(Inline::Code(s.into_string()));
            }
            Event::Html(s) => {
                let line = s.trim_end_matches('\n').to_owned();
                if let Frame::Para { inlines, .. } = self.top()
                    && !inlines.is_empty()
                {
                    inlines.push(Inline::LineBreak);
                }
                self.push_inline(Inline::Text(line));
            }
            Event::InlineHtml(s) => self.push_inline(Inline::Text(s.into_string())),
            Event::FootnoteReference(s) => self.push_inline(Inline::Text(format!("[^{s}]"))),
            Event::SoftBreak => self.push_inline(Inline::Text(" ".to_owned())),
            Event::HardBreak => self.push_inline(Inline::LineBreak),
            Event::Rule => {
                if self.in_flat() {
                    self.push_inline(Inline::Text(" ".to_owned()));
                } else {
                    self.close_implicit_para();
                    self.deliver_block(Block::ThematicBreak);
                }
            }
            Event::TaskListMarker(done) => {
                self.push_inline(Inline::Text(if done { "[x] " } else { "[ ] " }.to_owned()));
            }
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        if let Frame::Flat { nesting, .. } = self.top() {
            *nesting += 1;
            return;
        }
        let is_inline = matches!(
            tag,
            Tag::Emphasis
                | Tag::Strong
                | Tag::Strikethrough
                | Tag::Superscript
                | Tag::Subscript
                | Tag::Link { .. }
                | Tag::Image { .. }
        );
        let is_block = !is_inline && !matches!(tag, Tag::Item | Tag::TableHead | Tag::TableRow | Tag::TableCell);
        if is_block {
            self.close_implicit_para();
        } else if is_inline {
            self.ensure_inline_context();
        }
        if self.stack.len() > MAX_DEPTH {
            self.stack.push(Frame::Flat {
                text: String::new(),
                nesting: 0,
            });
            return;
        }
        let frame = match tag {
            Tag::Paragraph | Tag::HtmlBlock => Frame::Para {
                inlines: Vec::new(),
                implicit: false,
            },
            Tag::Heading { level, .. } => Frame::Heading {
                level: heading_level(level),
                inlines: Vec::new(),
            },
            Tag::BlockQuote(_) => Frame::Quote(Vec::new()),
            Tag::CodeBlock(kind) => Frame::Code {
                lang: match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split_whitespace()
                        .next()
                        .map(|l| l.split(',').next().unwrap_or(l).to_owned())
                        .filter(|l| !l.is_empty()),
                    CodeBlockKind::Indented => None,
                },
                text: String::new(),
            },
            Tag::List(start) => Frame::List {
                start,
                items: Vec::new(),
            },
            Tag::Item => Frame::Item(Vec::new()),
            Tag::Table(aligns) => Frame::Table {
                aligns: aligns.into_iter().map(align).collect(),
                header: Vec::new(),
                rows: Vec::new(),
            },
            Tag::TableHead => Frame::Row {
                cells: Vec::new(),
                head: true,
            },
            Tag::TableRow => Frame::Row {
                cells: Vec::new(),
                head: false,
            },
            Tag::TableCell => Frame::Cell(Vec::new()),
            Tag::Emphasis => span(Span::Emphasis),
            Tag::Strong => span(Span::Strong),
            Tag::Strikethrough => span(Span::Strikethrough),
            Tag::Link {
                link_type, dest_url, ..
            } => {
                let dest = if link_type == LinkType::Email {
                    format!("mailto:{dest_url}")
                } else {
                    dest_url.into_string()
                };
                span(Span::Link(safe_url(&dest, UrlUse::Link)))
            }
            Tag::Image { dest_url, .. } => span(Span::Image(safe_url(&dest_url, UrlUse::Image))),
            // Extensions that are not enabled: keep their text only.
            _ => Frame::Flat {
                text: String::new(),
                nesting: 0,
            },
        };
        self.stack.push(frame);
    }

    fn end(&mut self, tag: TagEnd) {
        if let Frame::Flat { nesting, text } = self.top()
            && *nesting > 0
        {
            *nesting -= 1;
            let inline = matches!(
                tag,
                TagEnd::Emphasis
                    | TagEnd::Strong
                    | TagEnd::Strikethrough
                    | TagEnd::Superscript
                    | TagEnd::Subscript
                    | TagEnd::Link
                    | TagEnd::Image
            );
            if !inline && !text.ends_with(' ') {
                text.push(' ');
            }
            return;
        }
        self.close_implicit_para();
        self.close_top();
    }

    /// Pops the top frame and hands its result to the new top.
    fn close_top(&mut self) {
        let frame = self.stack.pop().expect("frame to close");
        match frame {
            Frame::Root(blocks) => self.stack.push(Frame::Root(blocks)),
            Frame::Quote(blocks) => self.deliver_block(Block::BlockQuote(blocks)),
            Frame::List { start, items } => self.deliver_block(Block::List {
                ordered: start.is_some(),
                start,
                items,
            }),
            Frame::Item(blocks) => {
                if let Frame::List { items, .. } = self.top() {
                    items.push(ListItem { blocks });
                } else {
                    for block in blocks {
                        self.deliver_block(block);
                    }
                }
            }
            Frame::Para { inlines, .. } => {
                if !inlines.is_empty() {
                    self.deliver_block(Block::Paragraph(inlines));
                }
            }
            Frame::Heading { level, inlines } => self.deliver_block(Block::Heading {
                level,
                content: inlines,
            }),
            Frame::Code { lang, mut text } => {
                if text.ends_with('\n') {
                    text.pop();
                }
                self.deliver_block(Block::CodeBlock { lang, text });
            }
            Frame::Table { header, rows, .. } => self.deliver_block(Block::Table { header, rows }),
            Frame::Row { cells, head } => {
                if let Frame::Table { header, rows, .. } = self.top() {
                    if head {
                        *header = cells;
                    } else {
                        rows.push(TableRow { cells });
                    }
                }
            }
            Frame::Cell(content) => {
                let n = self.stack.len();
                let align = match (self.stack.get(n.wrapping_sub(2)), self.stack.last()) {
                    (Some(Frame::Table { aligns, .. }), Some(Frame::Row { cells, .. })) => {
                        aligns.get(cells.len()).copied().unwrap_or_default()
                    }
                    _ => Align::None,
                };
                if let Frame::Row { cells, .. } = self.top() {
                    cells.push(TableCell { content, align });
                } else {
                    for inline in content {
                        self.push_inline(inline);
                    }
                }
            }
            Frame::Span { kind, inlines } => match kind {
                Span::Emphasis => self.push_inline(Inline::Emphasis(inlines)),
                Span::Strong => self.push_inline(Inline::Strong(inlines)),
                Span::Strikethrough => self.push_inline(Inline::Strikethrough(inlines)),
                Span::Link(Some(url)) => self.push_inline(Inline::Link { url, content: inlines }),
                Span::Image(Some(url)) => self.push_inline(Inline::Image {
                    url,
                    alt: text_of(&inlines),
                }),
                Span::Link(None) => {
                    for inline in inlines {
                        self.push_inline(inline);
                    }
                }
                Span::Image(None) => self.push_inline(Inline::Text(text_of(&inlines))),
            },
            Frame::Flat { text, .. } => self.deliver_flat_text(text.trim().to_owned()),
        }
    }

    /// Opens an implicit paragraph when inline content arrives directly in
    /// a block container (bare text of a tight list item).
    fn ensure_inline_context(&mut self) {
        if matches!(self.top(), Frame::Root(_) | Frame::Quote(_) | Frame::Item(_)) {
            self.stack.push(Frame::Para {
                inlines: Vec::new(),
                implicit: true,
            });
        }
    }

    fn push_inline(&mut self, inline: Inline) {
        self.ensure_inline_context();
        match self.top() {
            Frame::Para { inlines, .. }
            | Frame::Heading { inlines, .. }
            | Frame::Cell(inlines)
            | Frame::Span { inlines, .. } => inlines.push(inline),
            Frame::Flat { text, .. } => text.push_str(&text_of(std::slice::from_ref(&inline))),
            Frame::Code { text, .. } => text.push_str(&text_of(std::slice::from_ref(&inline))),
            // pulldown-cmark never puts inline content directly in these.
            Frame::Root(_)
            | Frame::Quote(_)
            | Frame::Item(_)
            | Frame::List { .. }
            | Frame::Table { .. }
            | Frame::Row { .. } => {}
        }
    }

    fn deliver_block(&mut self, block: Block) {
        match self.top() {
            Frame::Root(blocks) | Frame::Quote(blocks) | Frame::Item(blocks) => blocks.push(block),
            Frame::List { items, .. } => items.push(ListItem { blocks: vec![block] }),
            Frame::Flat { text, .. } => {
                text.push_str(&block_text(&block));
                text.push(' ');
            }
            Frame::Para { .. } | Frame::Heading { .. } | Frame::Cell(_) | Frame::Span { .. } => {
                let text = block_text(&block);
                self.push_inline(Inline::Text(text));
            }
            Frame::Code { text, .. } => text.push_str(&block_text(&block)),
            Frame::Table { .. } | Frame::Row { .. } => {}
        }
    }

    /// Hands the text of a flattened subtree to its container.
    fn deliver_flat_text(&mut self, text: String) {
        if text.is_empty() {
            return;
        }
        match self.top() {
            Frame::List { items, .. } => items.push(ListItem {
                blocks: vec![Block::Paragraph(vec![Inline::Text(text)])],
            }),
            Frame::Row { cells, .. } => cells.push(TableCell {
                content: vec![Inline::Text(text)],
                align: Align::None,
            }),
            Frame::Table { rows, .. } => rows.push(TableRow {
                cells: vec![TableCell {
                    content: vec![Inline::Text(text)],
                    align: Align::None,
                }],
            }),
            _ => self.push_inline(Inline::Text(text)),
        }
    }

    fn finish(mut self) -> Document {
        while self.stack.len() > 1 {
            self.close_top();
        }
        match self.stack.pop() {
            Some(Frame::Root(blocks)) => Document { blocks },
            _ => Document::default(),
        }
    }
}

fn span(kind: Span) -> Frame {
    Frame::Span {
        kind,
        inlines: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Inline {
        Inline::Text(s.into())
    }

    fn para(md: &str) -> Vec<Inline> {
        match markdown_to_ast(md).blocks.as_slice() {
            [Block::Paragraph(inlines)] => inlines.clone(),
            other => panic!("expected one paragraph for {md:?}, got {other:?}"),
        }
    }

    fn depth_of(blocks: &[Block]) -> usize {
        fn inl(v: &[Inline]) -> usize {
            v.iter()
                .map(|i| match i {
                    Inline::Emphasis(c)
                    | Inline::Strong(c)
                    | Inline::Strikethrough(c)
                    | Inline::Superscript(c)
                    | Inline::Spoiler(c)
                    | Inline::Link { content: c, .. } => 1 + inl(c),
                    _ => 0,
                })
                .max()
                .unwrap_or(0)
        }
        blocks
            .iter()
            .map(|b| match b {
                Block::Paragraph(c) | Block::Heading { content: c, .. } => 1 + inl(c),
                Block::BlockQuote(inner) => 1 + depth_of(inner),
                Block::List { items, .. } => 2 + items.iter().map(|i| depth_of(&i.blocks)).max().unwrap_or(0),
                Block::Table { header, rows } => {
                    2 + header
                        .iter()
                        .chain(rows.iter().flat_map(|r| r.cells.iter()))
                        .map(|c| inl(&c.content))
                        .max()
                        .unwrap_or(0)
                }
                _ => 1,
            })
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn empty_input() {
        assert!(markdown_to_ast("").is_empty());
        assert!(markdown_to_ast("   \n\n  \n").is_empty());
    }

    #[test]
    fn paragraphs_and_breaks() {
        let doc = markdown_to_ast("one\ntwo  \nthree\n\nfour");
        assert_eq!(
            doc.blocks,
            vec![
                Block::Paragraph(vec![t("one two"), Inline::LineBreak, t("three")]),
                Block::Paragraph(vec![t("four")]),
            ]
        );
    }

    #[test]
    fn headings() {
        let doc = markdown_to_ast("# One\n### Three *em*\n###### Six");
        assert_eq!(
            doc.blocks,
            vec![
                Block::Heading {
                    level: 1,
                    content: vec![t("One")]
                },
                Block::Heading {
                    level: 3,
                    content: vec![t("Three "), Inline::Emphasis(vec![t("em")])]
                },
                Block::Heading {
                    level: 6,
                    content: vec![t("Six")]
                },
            ]
        );
    }

    #[test]
    fn emphasis_strong_strike_code() {
        assert_eq!(
            para("*a* **b** ~~c~~ `d` ***e***"),
            vec![
                Inline::Emphasis(vec![t("a")]),
                t(" "),
                Inline::Strong(vec![t("b")]),
                t(" "),
                Inline::Strikethrough(vec![t("c")]),
                t(" "),
                Inline::Code("d".into()),
                t(" "),
                Inline::Emphasis(vec![Inline::Strong(vec![t("e")])]),
            ]
        );
    }

    #[test]
    fn nested_emphasis() {
        assert_eq!(
            para("**bold *both* bold**"),
            vec![Inline::Strong(vec![
                t("bold "),
                Inline::Emphasis(vec![t("both")]),
                t(" bold")
            ])]
        );
    }

    #[test]
    fn code_blocks() {
        let doc = markdown_to_ast("```rust ignore\nfn main() {}\n  x\n```\n\n    indented\n    code\n");
        assert_eq!(
            doc.blocks,
            vec![
                Block::CodeBlock {
                    lang: Some("rust".into()),
                    text: "fn main() {}\n  x".into()
                },
                Block::CodeBlock {
                    lang: None,
                    text: "indented\ncode".into()
                },
            ]
        );
        let doc = markdown_to_ast("```\n>!not a spoiler!< ^no r/rust\n```");
        assert_eq!(
            doc.blocks,
            vec![Block::CodeBlock {
                lang: None,
                text: ">!not a spoiler!< ^no r/rust".into()
            }]
        );
    }

    #[test]
    fn block_quotes() {
        let doc = markdown_to_ast("> quoted\n>\n> > nested");
        assert_eq!(
            doc.blocks,
            vec![Block::BlockQuote(vec![
                Block::Paragraph(vec![t("quoted")]),
                Block::BlockQuote(vec![Block::Paragraph(vec![t("nested")])]),
            ])]
        );
    }

    #[test]
    fn lists() {
        let doc = markdown_to_ast("- a\n- b\n  - c\n\n3. x\n4. y\n");
        assert_eq!(
            doc.blocks,
            vec![
                Block::List {
                    ordered: false,
                    start: None,
                    items: vec![
                        ListItem {
                            blocks: vec![Block::Paragraph(vec![t("a")])]
                        },
                        ListItem {
                            blocks: vec![
                                Block::Paragraph(vec![t("b")]),
                                Block::List {
                                    ordered: false,
                                    start: None,
                                    items: vec![ListItem {
                                        blocks: vec![Block::Paragraph(vec![t("c")])]
                                    }]
                                }
                            ]
                        },
                    ]
                },
                Block::List {
                    ordered: true,
                    start: Some(3),
                    items: vec![
                        ListItem {
                            blocks: vec![Block::Paragraph(vec![t("x")])]
                        },
                        ListItem {
                            blocks: vec![Block::Paragraph(vec![t("y")])]
                        },
                    ]
                },
            ]
        );
    }

    #[test]
    fn loose_list_items() {
        let doc = markdown_to_ast("1. a\n\n   more\n2. b\n");
        let Block::List { items, .. } = &doc.blocks[0] else {
            panic!("{doc:?}")
        };
        assert_eq!(
            items[0].blocks,
            vec![Block::Paragraph(vec![t("a")]), Block::Paragraph(vec![t("more")])]
        );
    }

    #[test]
    fn tables() {
        let doc = markdown_to_ast("| a | b | c |\n|:--|:-:|--:|\n| 1 | *2* | 3 |\n| 4 |");
        let cell = |s: &str, align| TableCell {
            content: if s.is_empty() { vec![] } else { vec![t(s)] },
            align,
        };
        assert_eq!(
            doc.blocks,
            vec![Block::Table {
                header: vec![
                    cell("a", Align::Left),
                    cell("b", Align::Center),
                    cell("c", Align::Right)
                ],
                rows: vec![
                    TableRow {
                        cells: vec![
                            cell("1", Align::Left),
                            TableCell {
                                content: vec![Inline::Emphasis(vec![t("2")])],
                                align: Align::Center
                            },
                            cell("3", Align::Right),
                        ]
                    },
                    TableRow {
                        cells: vec![cell("4", Align::Left), cell("", Align::Center), cell("", Align::Right)]
                    },
                ]
            }]
        );
    }

    #[test]
    fn thematic_break() {
        assert_eq!(
            markdown_to_ast("a\n\n---\n\nb").blocks,
            vec![
                Block::Paragraph(vec![t("a")]),
                Block::ThematicBreak,
                Block::Paragraph(vec![t("b")])
            ]
        );
    }

    #[test]
    fn links_and_images() {
        assert_eq!(
            para("[x](https://e.com) <https://a.b> <me@x.org> ![alt *t*](https://i.redd.it/p.png)"),
            vec![
                Inline::Link {
                    url: "https://e.com".into(),
                    content: vec![t("x")]
                },
                t(" "),
                Inline::Link {
                    url: "https://a.b".into(),
                    content: vec![t("https://a.b")]
                },
                t(" "),
                Inline::Link {
                    url: "mailto:me@x.org".into(),
                    content: vec![t("me@x.org")]
                },
                t(" "),
                Inline::Image {
                    url: "https://i.redd.it/p.png".into(),
                    alt: "alt t".into()
                },
            ]
        );
        assert_eq!(
            para("[sub](/r/rust)"),
            vec![Inline::Link {
                url: "https://www.reddit.com/r/rust".into(),
                content: vec![t("sub")]
            }]
        );
    }

    #[test]
    fn unsafe_urls_become_text() {
        assert_eq!(
            para("[click *me*](javascript:alert(1)) ![pic](data:image/png;base64,AAA) [rel](foo.html)"),
            vec![t("click "), Inline::Emphasis(vec![t("me")]), t(" pic rel")]
        );
        assert_eq!(para("![](javascript:x) end"), vec![t(" end")]);
    }

    #[test]
    fn raw_html_is_text() {
        assert_eq!(para("a <b>bold</b> c"), vec![t("a <b>bold</b> c")]);
        assert_eq!(
            markdown_to_ast("<div>\n<script>alert(1)</script>\n</div>").blocks,
            vec![Block::Paragraph(vec![
                t("<div>"),
                Inline::LineBreak,
                t("<script>alert(1)</script>"),
                Inline::LineBreak,
                t("</div>")
            ])]
        );
    }

    #[test]
    fn spoilers_inline() {
        assert_eq!(
            para("a >!secret!< b"),
            vec![t("a "), Inline::Spoiler(vec![t("secret")]), t(" b")]
        );
        assert_eq!(
            para(">!x *y* z!< and >!w!<"),
            vec![
                Inline::Spoiler(vec![t("x "), Inline::Emphasis(vec![t("y")]), t(" z")]),
                t(" and "),
                Inline::Spoiler(vec![t("w")]),
            ]
        );
        assert_eq!(
            para("*>!in emphasis!<*"),
            vec![Inline::Emphasis(vec![Inline::Spoiler(vec![t("in emphasis")])])]
        );
    }

    #[test]
    fn spoiler_at_line_start_is_not_a_quote() {
        assert_eq!(
            para(">!spoiler!< text"),
            vec![Inline::Spoiler(vec![t("spoiler")]), t(" text")]
        );
        assert_eq!(
            markdown_to_ast("> >!quoted spoiler!<").blocks,
            vec![Block::BlockQuote(vec![Block::Paragraph(vec![Inline::Spoiler(vec![
                t("quoted spoiler")
            ])])])]
        );
        // Without a closing `!<` it stays a quote.
        assert_eq!(
            markdown_to_ast(">!not closed").blocks,
            vec![Block::BlockQuote(vec![Block::Paragraph(vec![t("!not closed")])])]
        );
    }

    #[test]
    fn spoiler_edge_cases() {
        assert_eq!(
            para("a `>!code!<` b"),
            vec![t("a "), Inline::Code(">!code!<".into()), t(" b")]
        );
        assert_eq!(para("unclosed >!spoiler"), vec![t("unclosed >!spoiler")]);
        assert_eq!(para("empty >!!< here"), vec![t("empty >!!< here")]);
        assert_eq!(para("close !< first"), vec![t("close !< first")]);
        // Delimiters split by a code span still match around it.
        assert_eq!(
            para(">!a `b` c!<"),
            vec![Inline::Spoiler(vec![t("a "), Inline::Code("b".into()), t(" c")])]
        );
    }

    #[test]
    fn superscripts() {
        assert_eq!(
            para("x^2 and ^(two words) end"),
            vec![
                t("x"),
                Inline::Superscript(vec![t("2")]),
                t(" and "),
                Inline::Superscript(vec![t("two words")]),
                t(" end"),
            ]
        );
        assert_eq!(
            para("^^double"),
            vec![Inline::Superscript(vec![Inline::Superscript(vec![t("double")])])]
        );
        assert_eq!(para("^(a (b) c)"), vec![Inline::Superscript(vec![t("a (b) c")])]);
        assert_eq!(
            para("^(a ^(b))"),
            vec![Inline::Superscript(vec![t("a "), Inline::Superscript(vec![t("b")])])]
        );
        assert_eq!(
            para("^(with *em* inside)"),
            vec![Inline::Superscript(vec![
                t("with "),
                Inline::Emphasis(vec![t("em")]),
                t(" inside")
            ])]
        );
    }

    #[test]
    fn superscript_edge_cases() {
        assert_eq!(para("end ^"), vec![t("end ^")]);
        assert_eq!(para("a ^ b"), vec![t("a ^ b")]);
        assert_eq!(para("^()"), vec![Inline::Superscript(vec![t("()")])]);
        assert_eq!(
            para("^(unclosed words"),
            vec![Inline::Superscript(vec![t("(unclosed")]), t(" words")]
        );
        assert_eq!(para("`a^b`"), vec![Inline::Code("a^b".into())]);
        assert_eq!(
            para("[a^b](https://x.y)"),
            vec![Inline::Link {
                url: "https://x.y".into(),
                content: vec![t("a"), Inline::Superscript(vec![t("b")])]
            }]
        );
    }

    #[test]
    fn mentions_in_markdown() {
        assert_eq!(
            para("ask r/rust or /u/spez"),
            vec![
                t("ask "),
                Inline::SubredditLink("rust".into()),
                t(" or "),
                Inline::UserLink("spez".into())
            ]
        );
        assert_eq!(para("`r/rust`"), vec![Inline::Code("r/rust".into())]);
        assert_eq!(
            para("[r/rust](https://x.y/r/rust)"),
            vec![Inline::Link {
                url: "https://x.y/r/rust".into(),
                content: vec![t("r/rust")]
            }]
        );
        assert_eq!(
            para("**r/gnome**"),
            vec![Inline::Strong(vec![Inline::SubredditLink("gnome".into())])]
        );
        assert_eq!(
            para(">!u/spoiled!<"),
            vec![Inline::Spoiler(vec![Inline::UserLink("spoiled".into())])]
        );
        assert_eq!(
            para("see https://reddit.com/r/rust"),
            vec![t("see https://reddit.com/r/rust")]
        );
    }

    #[test]
    fn escaped_markers_stay_literal() {
        assert_eq!(para(r"\*not em\*"), vec![t("*not em*")]);
    }

    #[test]
    fn deep_quotes_are_flattened() {
        let md = format!("{}deep", "> ".repeat(500));
        let doc = markdown_to_ast(&md);
        assert!(
            depth_of(&doc.blocks) <= MAX_DEPTH + 2,
            "depth {}",
            depth_of(&doc.blocks)
        );
        assert_eq!(doc.plain_text(), "deep");
    }

    #[test]
    fn deep_lists_are_flattened() {
        let mut md = String::new();
        for i in 0..100 {
            md.push_str(&"  ".repeat(i));
            md.push_str(&format!("- item {i}\n"));
        }
        let doc = markdown_to_ast(&md);
        assert!(
            depth_of(&doc.blocks) <= MAX_DEPTH + 2,
            "depth {}",
            depth_of(&doc.blocks)
        );
        let text = doc.plain_text();
        assert!(text.contains("item 0") && text.contains("item 99"), "{text}");
    }

    #[test]
    fn deep_inlines_are_flattened() {
        let md = format!("{}x{}", "*a ".repeat(2000), " b*".repeat(2000));
        let doc = markdown_to_ast(&md);
        assert!(
            depth_of(&doc.blocks) <= MAX_DEPTH + 2,
            "depth {}",
            depth_of(&doc.blocks)
        );
        assert!(doc.plain_text().contains('x'));

        let md = format!("{}x", "^".repeat(10_000));
        let doc = markdown_to_ast(&md);
        assert!(
            depth_of(&doc.blocks) <= MAX_DEPTH + 2,
            "depth {}",
            depth_of(&doc.blocks)
        );

        let md = format!("{}x{}", "[".repeat(5000), "](https://a.b)".repeat(5000));
        let doc = markdown_to_ast(&md);
        assert!(depth_of(&doc.blocks) <= MAX_DEPTH + 2);
    }

    #[test]
    fn hostile_inputs_do_not_panic() {
        for md in [
            ">!",
            "!<",
            ">!!<",
            "^",
            "^(",
            "^)",
            "r/",
            "/u/",
            "| |\n|-|",
            "\u{0}",
            "*",
            "~~",
            "> >! !< <",
            "```",
            "<",
            "]]]]",
            "\\",
            "^(>!a)!<",
            ">!^(a!<)",
        ] {
            let _ = markdown_to_ast(md);
        }
    }
}
