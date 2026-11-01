//! HTML body fragments (from reddit.com partials) to [`Document`].

use html5ever::tendril::TendrilSink;
use html5ever::{ParseOpts, QualName, local_name, ns, parse_fragment};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use crate::inline::{
    UrlUse, collapse_ws, for_each_inlines, is_html_space, mentions, merge_text, safe_url, trim_block_whitespace,
};
use crate::{Align, Block, Document, Inline, ListItem, MAX_DEPTH, TableCell, TableRow};

/// DOM nesting followed recursively; deeper subtrees are read as plain text
/// without recursion. Transparent wrappers (`div`, `span`, custom
/// elements) do not add AST depth, so this bounds them separately.
const MAX_DOM_DEPTH: usize = 128;

/// Parses an HTML body fragment, such as the inner HTML of a comment's
/// `<div slot="comment">`.
///
/// Allowed tags: `p`, `a`, `em`/`i`, `strong`/`b`, `del`/`s`/`strike`,
/// `code`, `pre` (with `<code class="language-x">`), `blockquote`,
/// `ul`/`ol`/`li`, tables, `sup`, `h1`–`h6`, `hr`, `br`, `img` and spoiler
/// spans (`class="md-spoiler-text"` or a `*-spoiler` element). Any other
/// element keeps only its content; `script`, `style`, `template`, `svg`
/// and form controls are dropped entirely. Whitespace collapses as in a
/// browser except inside `pre`. Bare `r/name` / `u/name` text becomes a
/// mention. Links keep only `http`, `https` and `mailto` targets (images
/// `http` and `https`), and root-relative links become absolute on
/// `https://www.reddit.com`.
pub fn html_to_ast(html: &str) -> Document {
    let dom = parse_fragment(
        RcDom::default(),
        ParseOpts::default(),
        QualName::new(None, ns!(html), local_name!("div")),
        Vec::new(),
        false,
    )
    .one(html);
    let mut acc = BlockAcc::default();
    block_children(&dom.document, &mut acc, 0, 0);
    let mut doc = Document { blocks: acc.finish() };
    for_each_inlines(&mut doc.blocks, 0, &mut |v, _| {
        mentions(v);
        merge_text(v);
    });
    doc
}

/// Collects blocks, wrapping loose inline content in paragraphs.
#[derive(Default)]
struct BlockAcc {
    blocks: Vec<Block>,
    inlines: Vec<Inline>,
}

impl BlockAcc {
    fn flush(&mut self) {
        let inlines = finish_inlines(std::mem::take(&mut self.inlines));
        if !inlines.is_empty() {
            self.blocks.push(Block::Paragraph(inlines));
        }
    }

    fn push(&mut self, block: Block) {
        self.flush();
        self.blocks.push(block);
    }

    fn finish(mut self) -> Vec<Block> {
        self.flush();
        self.blocks
    }
}

/// Whitespace, merging and pruning for one block's inline content.
fn finish_inlines(mut v: Vec<Inline>) -> Vec<Inline> {
    merge_text(&mut v);
    trim_block_whitespace(&mut v);
    v
}

fn tag_name(node: &Handle) -> Option<&str> {
    match &node.data {
        NodeData::Element { name, .. } => Some(name.local.as_ref()),
        _ => None,
    }
}

fn attr(node: &Handle, key: &str) -> Option<String> {
    match &node.data {
        NodeData::Element { attrs, .. } => attrs
            .borrow()
            .iter()
            .find(|a| a.name.local.as_ref() == key)
            .map(|a| a.value.to_string()),
        _ => None,
    }
}

fn has_class(node: &Handle, class: &str) -> bool {
    attr(node, "class").is_some_and(|c| c.split(is_html_space).any(|t| t == class))
}

/// Elements dropped with all their content.
fn is_dropped(node: &Handle) -> bool {
    match &node.data {
        NodeData::Element { name, .. } => matches!(
            name.local.as_ref(),
            "script"
                | "style"
                | "template"
                | "svg"
                | "noscript"
                | "head"
                | "title"
                | "meta"
                | "link"
                | "iframe"
                | "object"
                | "embed"
                | "button"
                | "input"
                | "select"
                | "textarea"
        ),
        NodeData::Comment { .. } | NodeData::Doctype { .. } | NodeData::ProcessingInstruction { .. } => true,
        NodeData::Document | NodeData::Text { .. } => false,
    }
}

/// Reddit spoiler markup. Old Reddit and the RTJSON renderer use
/// `<span class="md-spoiler-text">`; spoiler custom elements are accepted
/// too.
fn is_spoiler(node: &Handle) -> bool {
    let Some(tag) = tag_name(node) else {
        return false;
    };
    tag == "spoiler"
        || tag.ends_with("-spoiler")
        || tag.ends_with("-spoiler-text")
        || has_class(node, "md-spoiler-text")
        || has_class(node, "spoiler")
}

/// Block-level elements that are not on the allow-list: their content is
/// kept, and they separate the inline content around them like a browser.
fn is_blockish(tag: &str) -> bool {
    matches!(
        tag,
        "div"
            | "figure"
            | "figcaption"
            | "section"
            | "article"
            | "aside"
            | "header"
            | "footer"
            | "main"
            | "nav"
            | "address"
            | "details"
            | "summary"
            | "dl"
            | "dt"
            | "dd"
            | "center"
            | "form"
            | "fieldset"
            | "li"
            | "caption"
    )
}

/// The text of a subtree, read without recursion. `raw` keeps whitespace
/// (for `pre`); otherwise it is collapsed.
fn text_content(node: &Handle, raw: bool) -> String {
    let mut out = String::new();
    let mut stack: Vec<Handle> = vec![node.clone()];
    while let Some(n) = stack.pop() {
        match &n.data {
            NodeData::Text { contents } => out.push_str(&contents.borrow()),
            NodeData::Element { .. } if is_dropped(&n) => {}
            NodeData::Element { .. } => {
                match tag_name(&n) {
                    Some("br") => out.push('\n'),
                    Some(tag) if !raw && is_block_tag(tag) => out.push(' '),
                    _ => {}
                }
                stack.extend(n.children.borrow().iter().rev().cloned());
            }
            NodeData::Document => stack.extend(n.children.borrow().iter().rev().cloned()),
            _ => {}
        }
    }
    if raw { out } else { collapse_ws(&out) }
}

fn is_block_tag(tag: &str) -> bool {
    is_blockish(tag)
        || matches!(
            tag,
            "p" | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "pre"
                | "blockquote"
                | "ul"
                | "ol"
                | "table"
                | "tr"
                | "td"
                | "th"
                | "hr"
        )
}

/// Flattened text of a subtree that is too deep to keep as structure.
fn flat(node: &Handle) -> Inline {
    Inline::Text(text_content(node, false))
}

/// Walks the children of `node` in block context. `depth` is the AST depth
/// of the container being filled; `dom` the DOM recursion depth.
fn block_children(node: &Handle, acc: &mut BlockAcc, depth: usize, dom: usize) {
    for child in node.children.borrow().iter() {
        match &child.data {
            NodeData::Text { contents } => {
                acc.inlines.push(Inline::Text(collapse_ws(&contents.borrow())));
            }
            NodeData::Element { .. } => block_element(child, acc, depth, dom + 1),
            _ => {}
        }
    }
}

/// Blocks inside a new container at `depth`.
fn blocks_of(node: &Handle, depth: usize, dom: usize) -> Vec<Block> {
    let mut acc = BlockAcc::default();
    block_children(node, &mut acc, depth, dom);
    acc.finish()
}

fn block_element(node: &Handle, acc: &mut BlockAcc, depth: usize, dom: usize) {
    if is_spoiler(node) {
        inline_element(node, &mut acc.inlines, depth + 1, dom);
        return;
    }
    if is_dropped(node) {
        return;
    }
    let tag = tag_name(node).unwrap_or_default();
    if dom >= MAX_DOM_DEPTH || depth >= MAX_DEPTH {
        if is_block_tag(tag) {
            acc.inlines.push(Inline::Text(" ".into()));
        }
        acc.inlines.push(flat(node));
        if is_block_tag(tag) {
            acc.inlines.push(Inline::Text(" ".into()));
        }
        return;
    }
    match tag {
        "p" => {
            acc.flush();
            let inlines = finish_inlines(inline_children(node, depth + 1, dom));
            if !inlines.is_empty() {
                acc.push(Block::Paragraph(inlines));
            }
        }
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = tag[1..].parse().unwrap_or(1);
            let content = finish_inlines(inline_children(node, depth + 1, dom));
            acc.push(Block::Heading { level, content });
        }
        "pre" => acc.push(code_block(node)),
        "blockquote" => {
            let inner = blocks_of(node, depth + 1, dom);
            acc.push(Block::BlockQuote(inner));
        }
        "ul" | "ol" => acc.push(list(node, tag == "ol", depth, dom)),
        "table" => acc.push(table(node, depth, dom)),
        "hr" => acc.push(Block::ThematicBreak),
        "br" => acc.inlines.push(Inline::LineBreak),
        "a" | "em" | "i" | "strong" | "b" | "del" | "s" | "strike" | "code" | "sup" | "img" => {
            inline_element(node, &mut acc.inlines, depth + 1, dom);
        }
        _ if is_blockish(tag) => {
            acc.flush();
            block_children(node, acc, depth, dom);
            acc.flush();
        }
        _ => block_children(node, acc, depth, dom),
    }
}

/// Walks the children of `node` in inline context; `depth` is the AST depth
/// of the inline sequence being filled.
fn inline_children(node: &Handle, depth: usize, dom: usize) -> Vec<Inline> {
    let mut out = Vec::new();
    push_inline_children(node, &mut out, depth, dom);
    out
}

fn push_inline_children(node: &Handle, out: &mut Vec<Inline>, depth: usize, dom: usize) {
    for child in node.children.borrow().iter() {
        match &child.data {
            NodeData::Text { contents } => out.push(Inline::Text(collapse_ws(&contents.borrow()))),
            NodeData::Element { .. } => inline_element(child, out, depth, dom + 1),
            _ => {}
        }
    }
}

fn inline_element(node: &Handle, out: &mut Vec<Inline>, depth: usize, dom: usize) {
    let spoiler = is_spoiler(node);
    if !spoiler && is_dropped(node) {
        return;
    }
    let tag = tag_name(node).unwrap_or_default();
    if dom >= MAX_DOM_DEPTH || depth >= MAX_DEPTH {
        out.push(flat(node));
        return;
    }
    let children = || inline_children(node, depth + 1, dom);
    if spoiler {
        out.push(Inline::Spoiler(children()));
        return;
    }
    match tag {
        "em" | "i" => out.push(Inline::Emphasis(children())),
        "strong" | "b" => out.push(Inline::Strong(children())),
        "del" | "s" | "strike" => out.push(Inline::Strikethrough(children())),
        "sup" => out.push(Inline::Superscript(children())),
        "code" => out.push(Inline::Code(text_content(node, false))),
        "pre" => out.push(Inline::Code(text_content(node, true))),
        "br" => out.push(Inline::LineBreak),
        "a" => {
            let content = children();
            match attr(node, "href").and_then(|h| safe_url(&h, UrlUse::Link)) {
                Some(url) => out.push(Inline::Link { url, content }),
                None => out.extend(content),
            }
        }
        "img" => {
            let alt = collapse_ws(&attr(node, "alt").unwrap_or_default()).trim().to_owned();
            match attr(node, "src").and_then(|s| safe_url(&s, UrlUse::Image)) {
                Some(url) => out.push(Inline::Image { url, alt }),
                None => out.push(Inline::Text(alt)),
            }
        }
        _ => {
            let block = is_block_tag(tag);
            if block {
                out.push(Inline::Text(" ".into()));
            }
            push_inline_children(node, out, depth, dom);
            if block {
                out.push(Inline::Text(" ".into()));
            }
        }
    }
}

fn code_block(node: &Handle) -> Block {
    let code_child = node
        .children
        .borrow()
        .iter()
        .find(|c| tag_name(c) == Some("code"))
        .cloned();
    let lang = [Some(node), code_child.as_ref()]
        .into_iter()
        .flatten()
        .filter_map(|n| attr(n, "class"))
        .flat_map(|class| {
            class
                .split(is_html_space)
                .filter_map(|t| t.strip_prefix("language-").or_else(|| t.strip_prefix("lang-")))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .find(|l| !l.is_empty());
    let mut text = text_content(node, true);
    if text.ends_with('\n') {
        text.pop();
    }
    Block::CodeBlock { lang, text }
}

fn list(node: &Handle, ordered: bool, depth: usize, dom: usize) -> Block {
    let start = ordered.then(|| {
        attr(node, "start")
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(1)
    });
    let mut items: Vec<ListItem> = Vec::new();
    let mut loose = BlockAcc::default();
    for child in node.children.borrow().iter() {
        if tag_name(child) == Some("li") {
            let pending = loose.finish();
            if !pending.is_empty() {
                items.push(ListItem { blocks: pending });
            }
            loose = BlockAcc::default();
            let blocks = if depth + 2 >= MAX_DEPTH || dom + 1 >= MAX_DOM_DEPTH {
                let text = text_content(child, false).trim().to_owned();
                if text.is_empty() {
                    Vec::new()
                } else {
                    vec![Block::Paragraph(vec![Inline::Text(text)])]
                }
            } else {
                blocks_of(child, depth + 2, dom + 1)
            };
            items.push(ListItem { blocks });
        } else {
            // Content outside `<li>` (invalid but common) becomes its own item.
            match &child.data {
                NodeData::Text { contents } => {
                    loose.inlines.push(Inline::Text(collapse_ws(&contents.borrow())));
                }
                NodeData::Element { .. } => block_element(child, &mut loose, depth + 2, dom + 1),
                _ => {}
            }
        }
    }
    let pending = loose.finish();
    if !pending.is_empty() {
        items.push(ListItem { blocks: pending });
    }
    Block::List { ordered, start, items }
}

fn cell_align(node: &Handle) -> Align {
    let from_style = attr(node, "style").and_then(|style| {
        style.split(';').find_map(|decl| {
            let (k, v) = decl.split_once(':')?;
            (k.trim().eq_ignore_ascii_case("text-align")).then(|| v.trim().to_ascii_lowercase())
        })
    });
    match from_style
        .or_else(|| attr(node, "align").map(|a| a.trim().to_ascii_lowercase()))
        .as_deref()
    {
        Some("left" | "start") => Align::Left,
        Some("center") => Align::Center,
        Some("right" | "end") => Align::Right,
        _ => Align::None,
    }
}

fn table(node: &Handle, depth: usize, dom: usize) -> Block {
    // (cells, all header cells, inside thead)
    let mut rows: Vec<(Vec<TableCell>, bool, bool)> = Vec::new();
    // Sections (thead/tbody/tfoot) are walked without recursion.
    let mut stack: Vec<(Handle, bool)> = vec![(node.clone(), false)];
    let mut sections = Vec::new();
    while let Some((n, in_head)) = stack.pop() {
        sections.push((n.clone(), in_head));
        for child in n.children.borrow().iter().rev() {
            match tag_name(child) {
                Some("thead") => stack.push((child.clone(), true)),
                Some("tbody" | "tfoot") => stack.push((child.clone(), in_head)),
                _ => {}
            }
        }
    }
    for (section, in_head) in sections {
        for tr in section.children.borrow().iter().filter(|c| tag_name(c) == Some("tr")) {
            let mut cells = Vec::new();
            let mut all_th = true;
            for cell in tr.children.borrow().iter() {
                let tag = tag_name(cell);
                if !matches!(tag, Some("td" | "th")) {
                    continue;
                }
                all_th &= tag == Some("th");
                let content = if depth + 2 >= MAX_DEPTH || dom + 3 >= MAX_DOM_DEPTH {
                    vec![flat(cell)]
                } else {
                    inline_children(cell, depth + 2, dom + 3)
                };
                cells.push(TableCell {
                    content: finish_inlines(content),
                    align: cell_align(cell),
                });
            }
            rows.push((cells, all_th && !tr.children.borrow().is_empty(), in_head));
        }
    }
    let mut header = Vec::new();
    let has_head = rows
        .first()
        .is_some_and(|(cells, all_th, in_head)| !cells.is_empty() && (*in_head || *all_th));
    let mut rows = rows.into_iter();
    if has_head && let Some((cells, ..)) = rows.next() {
        header = cells;
    }
    Block::Table {
        header,
        rows: rows.map(|(cells, ..)| TableRow { cells }).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Inline {
        Inline::Text(s.into())
    }

    fn para(html: &str) -> Vec<Inline> {
        match html_to_ast(html).blocks.as_slice() {
            [Block::Paragraph(inlines)] => inlines.clone(),
            other => panic!("expected one paragraph for {html:?}, got {other:?}"),
        }
    }

    #[test]
    fn empty_input() {
        assert!(html_to_ast("").is_empty());
        assert!(html_to_ast("  \n\t ").is_empty());
        assert!(html_to_ast("<p>   </p><div> </div>").is_empty());
        assert!(html_to_ast("<!-- comment -->").is_empty());
    }

    #[test]
    fn paragraphs_and_inline_formatting() {
        let doc = html_to_ast(
            "<p>a <em>b</em> <i>c</i> <strong>d</strong> <b>e</b> <del>f</del> <s>g</s> \
             <code>h  i</code> x<sup>2</sup></p><p>second<br>line</p>",
        );
        assert_eq!(
            doc.blocks,
            vec![
                Block::Paragraph(vec![
                    t("a "),
                    Inline::Emphasis(vec![t("b")]),
                    t(" "),
                    Inline::Emphasis(vec![t("c")]),
                    t(" "),
                    Inline::Strong(vec![t("d")]),
                    t(" "),
                    Inline::Strong(vec![t("e")]),
                    t(" "),
                    Inline::Strikethrough(vec![t("f")]),
                    t(" "),
                    Inline::Strikethrough(vec![t("g")]),
                    t(" "),
                    Inline::Code("h i".into()),
                    t(" x"),
                    Inline::Superscript(vec![t("2")]),
                ]),
                Block::Paragraph(vec![t("second"), Inline::LineBreak, t("line")]),
            ]
        );
    }

    #[test]
    fn headings_and_rule() {
        let doc = html_to_ast("<h1>One</h1><h3>Three</h3><hr><h6>Six</h6>");
        assert_eq!(
            doc.blocks,
            vec![
                Block::Heading {
                    level: 1,
                    content: vec![t("One")]
                },
                Block::Heading {
                    level: 3,
                    content: vec![t("Three")]
                },
                Block::ThematicBreak,
                Block::Heading {
                    level: 6,
                    content: vec![t("Six")]
                },
            ]
        );
    }

    #[test]
    fn code_blocks_keep_whitespace() {
        let doc = html_to_ast(
            "<pre><code class=\"language-rust\">fn main() {\n    r/x  ^y\n}\n</code></pre>\
             <pre>plain\n  text</pre>",
        );
        assert_eq!(
            doc.blocks,
            vec![
                Block::CodeBlock {
                    lang: Some("rust".into()),
                    text: "fn main() {\n    r/x  ^y\n}".into()
                },
                Block::CodeBlock {
                    lang: None,
                    text: "plain\n  text".into()
                },
            ]
        );
    }

    #[test]
    fn block_quotes() {
        let doc = html_to_ast("<blockquote><p>q</p><blockquote>inner</blockquote></blockquote>");
        assert_eq!(
            doc.blocks,
            vec![Block::BlockQuote(vec![
                Block::Paragraph(vec![t("q")]),
                Block::BlockQuote(vec![Block::Paragraph(vec![t("inner")])]),
            ])]
        );
    }

    #[test]
    fn lists() {
        let doc = html_to_ast("<ul><li>a</li><li><p>b</p><ol start=\"7\"><li>c</li></ol></li></ul><ol><li>x</li></ol>");
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
                                    ordered: true,
                                    start: Some(7),
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
                    start: Some(1),
                    items: vec![ListItem {
                        blocks: vec![Block::Paragraph(vec![t("x")])]
                    }]
                },
            ]
        );
    }

    #[test]
    fn tables_with_alignment() {
        let doc = html_to_ast(
            "<table><thead><tr><th align=\"left\">a</th><th style=\"text-align: center\">b</th>\
             <th style=\"color:red; TEXT-ALIGN:right\">c</th></tr></thead>\
             <tbody><tr><td>1</td><td><em>2</em></td><td align=\"right\">3</td></tr></tbody></table>",
        );
        assert_eq!(
            doc.blocks,
            vec![Block::Table {
                header: vec![
                    TableCell {
                        content: vec![t("a")],
                        align: Align::Left
                    },
                    TableCell {
                        content: vec![t("b")],
                        align: Align::Center
                    },
                    TableCell {
                        content: vec![t("c")],
                        align: Align::Right
                    },
                ],
                rows: vec![TableRow {
                    cells: vec![
                        TableCell {
                            content: vec![t("1")],
                            align: Align::None
                        },
                        TableCell {
                            content: vec![Inline::Emphasis(vec![t("2")])],
                            align: Align::None
                        },
                        TableCell {
                            content: vec![t("3")],
                            align: Align::Right
                        },
                    ]
                }]
            }]
        );
    }

    #[test]
    fn table_without_header() {
        let doc = html_to_ast("<table><tr><td>1</td></tr><tr><td>2</td></tr></table>");
        let Block::Table { header, rows } = &doc.blocks[0] else {
            panic!("{doc:?}")
        };
        assert!(header.is_empty());
        assert_eq!(rows.len(), 2);
        // A first row of only <th> is the header even without <thead>.
        let doc = html_to_ast("<table><tr><th>h</th></tr><tr><td>1</td></tr></table>");
        let Block::Table { header, rows } = &doc.blocks[0] else {
            panic!("{doc:?}")
        };
        assert_eq!(header.len(), 1);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn links_and_images() {
        assert_eq!(
            para(
                "<p><a href=\"https://e.com\">x</a> <a href=\"/r/gnome\">r/gnome</a> \
                 <a href=\"mailto:a@b.c\">mail</a> <img src=\"https://i.redd.it/a.png\" alt=\"pic\"></p>"
            ),
            vec![
                Inline::Link {
                    url: "https://e.com".into(),
                    content: vec![t("x")]
                },
                t(" "),
                Inline::Link {
                    url: "https://www.reddit.com/r/gnome".into(),
                    content: vec![t("r/gnome")]
                },
                t(" "),
                Inline::Link {
                    url: "mailto:a@b.c".into(),
                    content: vec![t("mail")]
                },
                t(" "),
                Inline::Image {
                    url: "https://i.redd.it/a.png".into(),
                    alt: "pic".into()
                },
            ]
        );
        assert_eq!(
            para("<a href=\"https://e.com\"></a>"),
            vec![Inline::Link {
                url: "https://e.com".into(),
                content: vec![t("https://e.com")]
            }]
        );
    }

    #[test]
    fn unsafe_urls_become_text() {
        assert_eq!(
            para(
                "<a href=\"javascript:alert(1)\">click <b>me</b></a> \
                 <a href=\"  JaVaScRiPt:x\">two</a> <a>nohref</a> <a href=\"page.html\">rel</a> \
                 <img src=\"data:image/png;base64,AA\" alt=\"img\"><img src=\"javascript:x\">"
            ),
            vec![t("click "), Inline::Strong(vec![t("me")]), t(" two nohref rel img"),]
        );
    }

    #[test]
    fn spoilers() {
        assert_eq!(
            para("<p>a <span class=\"md-spoiler-text\" title=\"Spoiler\">hidden <em>x</em></span> b</p>"),
            vec![
                t("a "),
                Inline::Spoiler(vec![t("hidden "), Inline::Emphasis(vec![t("x")])]),
                t(" b")
            ]
        );
        assert_eq!(
            para("<p><shreddit-spoiler>s</shreddit-spoiler></p>"),
            vec![Inline::Spoiler(vec![t("s")])]
        );
        // Spoiler span directly in block context.
        assert_eq!(
            para("<span class=\"md-spoiler-text\">top</span>"),
            vec![Inline::Spoiler(vec![t("top")])]
        );
    }

    #[test]
    fn whitespace_collapses_like_a_browser() {
        assert_eq!(
            para("<p>\n   a \n\t b   <em> c </em>  d\n</p>"),
            vec![t("a b "), Inline::Emphasis(vec![t("c ")]), t("d")]
        );
        assert_eq!(para("<p>a <br> b</p>"), vec![t("a"), Inline::LineBreak, t("b")]);
        assert_eq!(para("<p>&nbsp;x&nbsp;</p>"), vec![t("\u{a0}x\u{a0}")]);
    }

    #[test]
    fn unknown_tags_keep_text() {
        assert_eq!(
            para("<p><span class=\"x\"><font>kept</font> <u>under</u> <custom-el>c</custom-el></span></p>"),
            vec![t("kept under c")]
        );
        let doc = html_to_ast("before<div>in div</div>after<figure>fig</figure>");
        assert_eq!(
            doc.blocks,
            vec![
                Block::Paragraph(vec![t("before")]),
                Block::Paragraph(vec![t("in div")]),
                Block::Paragraph(vec![t("after")]),
                Block::Paragraph(vec![t("fig")]),
            ]
        );
    }

    #[test]
    fn dropped_elements() {
        assert_eq!(
            para(
                "<p>a<script>alert(1)</script><style>p{}</style><template><p>t</p></template>\
                 <svg><text>svg text</text></svg><button>Reply</button>b</p>"
            ),
            vec![t("ab")]
        );
        assert!(html_to_ast("<script>x</script><style>y</style>").is_empty());
    }

    #[test]
    fn mentions_in_text_only() {
        assert_eq!(
            para("<p>see r/rust and /u/spez, <a href=\"https://x.y\">r/notme</a> <code>r/code</code></p>"),
            vec![
                t("see "),
                Inline::SubredditLink("rust".into()),
                t(" and "),
                Inline::UserLink("spez".into()),
                t(", "),
                Inline::Link {
                    url: "https://x.y".into(),
                    content: vec![t("r/notme")]
                },
                t(" "),
                Inline::Code("r/code".into()),
            ]
        );
    }

    #[test]
    fn no_markdown_extensions_in_html() {
        assert_eq!(para("<p>&gt;!not!&lt; x^2</p>"), vec![t(">!not!< x^2")]);
    }

    #[test]
    fn entities_are_decoded() {
        assert_eq!(para("<p>haven&#39;t &amp; &lt;b&gt;</p>"), vec![t("haven't & <b>")]);
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
    fn deep_nesting_is_flattened() {
        for (open, close) in [
            ("<blockquote>", "</blockquote>"),
            ("<ul><li>", "</li></ul>"),
            ("<em>", "</em>"),
            ("<div>", "</div>"),
            ("<span>", "</span>"),
            ("<custom-x>", "</custom-x>"),
            ("<table><tr><td>", "</td></tr></table>"),
        ] {
            let html = format!("{}deep{}", open.repeat(1000), close.repeat(1000));
            let doc = html_to_ast(&html);
            assert!(
                depth_of(&doc.blocks) <= MAX_DEPTH + 2,
                "{open}: {}",
                depth_of(&doc.blocks)
            );
            assert!(doc.plain_text().ends_with("deep"), "{open}");
        }
    }

    #[test]
    fn hostile_inputs_do_not_panic() {
        for html in [
            "<",
            "</p>",
            "<table><td>",
            "<li>orphan",
            "<ol start=\"-1\"><li>x</ol>",
            "<ol start=\"99999999999999999999999\"><li>x</ol>",
            "<pre></pre>",
            "<a href>",
            "<img>",
            "<h7>x</h7>",
            "<math><mi>x</mi></math>",
            "\u{0}",
        ] {
            let _ = html_to_ast(html);
        }
    }
}
