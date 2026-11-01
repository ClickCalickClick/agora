//! Turns the `agora-markdown` document AST into native widgets (SPEC §7.6).

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use agora_core::Body;
use agora_markdown::{Align, Block, Document, Inline, ListItem, TableCell};
use gettextrs::gettext;
use gtk::glib;
use gtk::prelude::*;

/// Link scheme for spoilers inside a label: `agora-spoiler:N`.
const SPOILER_SCHEME: &str = "agora-spoiler:";

/// Deepest nesting the renderer builds widgets for; deeper content is
/// rendered as plain text so hostile input cannot build huge widget trees.
const MAX_NESTING: usize = 12;

/// Parses a body into the document AST.
pub fn parse_body(body: &Body) -> Document {
    match body {
        Body::Markdown(md) => agora_markdown::markdown_to_ast(md),
        Body::Html(html) => agora_markdown::html_to_ast(html),
    }
}

/// Called with every activated link (already absolute, or `agora:r/name`,
/// `agora:u/name` for Reddit name links).
pub type LinkHandler = Rc<dyn Fn(&str)>;

/// Renders a document into a vertical box of widgets.
pub fn render(doc: &Document, on_link: &LinkHandler) -> gtk::Box {
    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    root.add_css_class("markdown");
    for block in &doc.blocks {
        root.append(&render_block(block, on_link, 0));
    }
    root
}

fn render_block(block: &Block, on_link: &LinkHandler, nesting: usize) -> gtk::Widget {
    if nesting > MAX_NESTING {
        return paragraph(&[Inline::Text(block_text(block))], on_link).upcast();
    }
    match block {
        Block::Paragraph(inlines) => paragraph(inlines, on_link).upcast(),
        Block::Heading { level, content } => {
            let label = paragraph(content, on_link);
            label.add_css_class(match level {
                1 => "title-2",
                2 => "title-3",
                3 => "title-4",
                _ => "heading",
            });
            label.update_property(&[gtk::accessible::Property::Level(i32::from(*level))]);
            label.set_accessible_role(gtk::AccessibleRole::Heading);
            label.upcast()
        }
        Block::CodeBlock { text, .. } => {
            let label = gtk::Label::builder()
                .label(text.trim_end_matches('\n'))
                .xalign(0.0)
                .selectable(true)
                .css_classes(["monospace"])
                .build();
            let scroller = gtk::ScrolledWindow::builder()
                .child(&label)
                .vscrollbar_policy(gtk::PolicyType::Never)
                .propagate_natural_height(true)
                .css_classes(["code-block"])
                .build();
            scroller.upcast()
        }
        Block::BlockQuote(blocks) => {
            let quote = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(8)
                .build();
            quote.add_css_class("blockquote");
            for b in blocks {
                quote.append(&render_block(b, on_link, nesting + 1));
            }
            quote.upcast()
        }
        Block::List { ordered, start, items } => list(*ordered, start.unwrap_or(1), items, on_link, nesting),
        Block::Table { header, rows } => table(header, rows.iter().map(|r| r.cells.as_slice()), on_link),
        Block::ThematicBreak => gtk::Separator::new(gtk::Orientation::Horizontal).upcast(),
    }
}

fn list(ordered: bool, start: u64, items: &[ListItem], on_link: &LinkHandler, nesting: usize) -> gtk::Widget {
    let list = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();
    for (i, item) in items.iter().enumerate() {
        let row = gtk::Box::builder().spacing(8).build();
        let marker = if ordered {
            format!("{}.", start + i as u64)
        } else {
            "•".to_string()
        };
        let marker = gtk::Label::builder()
            .label(marker)
            .yalign(0.0)
            .xalign(1.0)
            .width_chars(2)
            .build();
        marker.add_css_class("list-marker");
        row.append(&marker);
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .hexpand(true)
            .build();
        for b in &item.blocks {
            content.append(&render_block(b, on_link, nesting + 1));
        }
        row.append(&content);
        list.append(&row);
    }
    list.upcast()
}

fn table<'a>(header: &[TableCell], rows: impl Iterator<Item = &'a [TableCell]>, on_link: &LinkHandler) -> gtk::Widget {
    let grid = gtk::Grid::builder().column_spacing(18).row_spacing(6).build();
    grid.add_css_class("md-table");
    let place = |cells: &[TableCell], row: i32, bold: bool| {
        for (col, cell) in cells.iter().enumerate() {
            let label = paragraph(&cell.content, on_link);
            label.set_xalign(match cell.align {
                Align::Center => 0.5,
                Align::Right => 1.0,
                _ => 0.0,
            });
            label.set_wrap(false);
            if bold {
                label.add_css_class("heading");
            }
            grid.attach(&label, col as i32, row, 1, 1);
        }
    };
    place(header, 0, true);
    for (i, cells) in rows.enumerate() {
        place(cells, i as i32 + 1, false);
    }
    gtk::ScrolledWindow::builder()
        .child(&grid)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .build()
        .upcast()
}

/// A wrapping, selectable label for inline content, with spoilers that reveal
/// on click and links routed through `on_link`.
fn paragraph(inlines: &[Inline], on_link: &LinkHandler) -> gtk::Label {
    let label = gtk::Label::builder()
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .xalign(0.0)
        .selectable(true)
        .build();
    let revealed: Rc<RefCell<HashSet<usize>>> = Rc::default();
    let inlines: Rc<[Inline]> = inlines.into();
    let set_markup = {
        let label = label.downgrade();
        let revealed = revealed.clone();
        let inlines = inlines.clone();
        move || {
            if let Some(label) = label.upgrade() {
                let mut out = String::new();
                let mut spoilers = 0;
                markup(&inlines, &revealed.borrow(), &mut spoilers, &mut out, 0);
                label.set_markup(&out);
            }
        }
    };
    set_markup();
    let on_link = on_link.clone();
    label.connect_activate_link(move |_, uri| {
        if let Some(n) = uri.strip_prefix(SPOILER_SCHEME).and_then(|n| n.parse::<usize>().ok()) {
            revealed.borrow_mut().insert(n);
            set_markup();
        } else {
            on_link(uri);
        }
        glib::Propagation::Stop
    });
    label
}

fn markup(inlines: &[Inline], revealed: &HashSet<usize>, spoilers: &mut usize, out: &mut String, depth: usize) {
    for inline in inlines {
        if depth > MAX_NESTING {
            out.push_str(&glib::markup_escape_text(&inline_text(inline)));
            continue;
        }
        match inline {
            Inline::Text(t) => out.push_str(&glib::markup_escape_text(t)),
            Inline::Emphasis(c) => wrap(out, "<i>", "</i>", |o| markup(c, revealed, spoilers, o, depth + 1)),
            Inline::Strong(c) => wrap(out, "<b>", "</b>", |o| markup(c, revealed, spoilers, o, depth + 1)),
            Inline::Strikethrough(c) => wrap(out, "<s>", "</s>", |o| markup(c, revealed, spoilers, o, depth + 1)),
            Inline::Superscript(c) => wrap(out, "<sup>", "</sup>", |o| markup(c, revealed, spoilers, o, depth + 1)),
            Inline::Code(t) => {
                out.push_str("<tt>");
                out.push_str(&glib::markup_escape_text(t));
                out.push_str("</tt>");
            }
            Inline::Link { url, content } => {
                out.push_str(&format!("<a href=\"{}\">", glib::markup_escape_text(url)));
                // Pango links cannot nest; a spoiler inside a link shows as text.
                out.push_str(&glib::markup_escape_text(
                    &content.iter().map(inline_text).collect::<String>(),
                ));
                out.push_str("</a>");
            }
            Inline::Image { url, alt } => {
                let text = if alt.is_empty() { gettext("Image") } else { alt.clone() };
                out.push_str(&format!(
                    "<a href=\"{}\">{}</a>",
                    glib::markup_escape_text(url),
                    glib::markup_escape_text(&format!("🖼 {text}"))
                ));
            }
            Inline::SubredditLink(name) => {
                out.push_str(&format!(
                    "<a href=\"agora:r/{0}\">r/{0}</a>",
                    glib::markup_escape_text(name)
                ));
            }
            Inline::UserLink(name) => {
                out.push_str(&format!(
                    "<a href=\"agora:u/{0}\">u/{0}</a>",
                    glib::markup_escape_text(name)
                ));
            }
            Inline::LineBreak => out.push('\n'),
            Inline::Spoiler(c) => {
                let n = *spoilers;
                *spoilers += 1;
                if revealed.contains(&n) {
                    wrap(out, "<span bgalpha=\"12%\" background=\"#808080\">", "</span>", |o| {
                        markup(c, revealed, spoilers, o, depth + 1)
                    });
                } else {
                    out.push_str(&format!(
                        "<a href=\"{SPOILER_SCHEME}{n}\" title=\"{}\"><span underline=\"none\" bgalpha=\"35%\" background=\"#808080\">\u{2002}{}\u{2002}</span></a>",
                        glib::markup_escape_text(&gettext("Show spoiler")),
                        glib::markup_escape_text(&gettext("Spoiler")),
                    ));
                }
            }
        }
    }
}

fn wrap(out: &mut String, open: &str, close: &str, f: impl FnOnce(&mut String)) {
    out.push_str(open);
    f(out);
    out.push_str(close);
}

fn inline_text(inline: &Inline) -> String {
    match inline {
        Inline::Text(t) | Inline::Code(t) => t.clone(),
        Inline::Emphasis(c)
        | Inline::Strong(c)
        | Inline::Strikethrough(c)
        | Inline::Superscript(c)
        | Inline::Spoiler(c)
        | Inline::Link { content: c, .. } => c.iter().map(inline_text).collect(),
        Inline::Image { alt, .. } => alt.clone(),
        Inline::SubredditLink(n) => format!("r/{n}"),
        Inline::UserLink(n) => format!("u/{n}"),
        Inline::LineBreak => "\n".into(),
    }
}

fn block_text(block: &Block) -> String {
    Document {
        blocks: vec![block.clone()],
    }
    .plain_text()
}
