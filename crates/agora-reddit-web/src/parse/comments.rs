//! `post_with_comments` and `more_comments`: the `<shreddit-comment>` tree.
//!
//! Comments nest inside each other, and many sit in `<template>` elements. The
//! HTML parser keeps a template's content in a fragment node under it, so the
//! walk steps through fragments as well as elements.

use agora_core::{AgoraError, Body, Comment, CommentId, CommentNode, CommentState, MoreToken, PostId};
use ego_tree::NodeRef;
use scraper::{ElementRef, Html, Node};
use serde::{Deserialize, Serialize};

use super::*;
use crate::parse::listing::vote_state;

/// What `more_comments` needs: the partial's `src` and its hidden `cursor`.
/// Encoded as the opaque [`MoreToken`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct More {
    /// `/svc/shreddit/more-comments/r/{sub}/{post}?sort=…&startingDepth=…`.
    pub src: String,
    /// The hidden form field.
    pub cursor: String,
}

impl More {
    /// Reads a token back.
    pub fn from_token(token: &MoreToken) -> Result<Self, AgoraError> {
        serde_json::from_str(token.as_str()).map_err(|e| AgoraError::Parse(e.to_string()))
    }

    fn to_token(&self) -> MoreToken {
        MoreToken::new(serde_json::to_string(self).expect("strings serialize"))
    }
}

/// Parses a comment tree or a more-comments response.
pub fn parse(html: &str, post: &PostId) -> Vec<CommentNode> {
    let doc = Html::parse_document(html);
    walk(*doc.root_element(), None, 0, post)
}

/// The nodes directly under `el`, stopping at each nested comment.
fn walk(node: NodeRef<'_, Node>, parent: Option<&CommentId>, depth: u32, post: &PostId) -> Vec<CommentNode> {
    let mut out = Vec::new();
    for child in node.children() {
        if child.value().is_fragment() {
            out.extend(walk(child, parent, depth, post));
        }
        let Some(child) = ElementRef::wrap(child) else {
            continue;
        };
        match child.value().name() {
            "shreddit-comment" => out.extend(comment_node(child, post)),
            "faceplate-partial" if child.value().classes().any(|c| c == "more-comments-partial") => {
                out.extend(more_node(child, parent, depth));
            }
            "a" if attr(&child, "slot") == Some("more-comments-permalink") => {
                out.extend(continue_node(child, parent, depth));
            }
            // Ads hide in templates too, and composers hold no comments.
            "shreddit-comments-page-ad" | "shreddit-comment-tree-ads" | "script" | "style" | "svg" => {}
            _ => out.extend(walk(*child, parent, depth, post)),
        }
    }
    out
}

fn comment_node(el: ElementRef<'_>, post: &PostId) -> Option<CommentNode> {
    let id = attr(&el, "thingid").or_else(|| attr(&el, "thing-id"))?;
    let id = CommentId::new(id);
    let depth = attr(&el, "depth").and_then(|d| d.parse().ok()).unwrap_or(0);
    let author = attr(&el, "author");
    let body_html = own_slot(el).map(|b| b.inner_html()).unwrap_or_default();
    let body_text = own_slot(el).map(|b| text_of(&b)).unwrap_or_default();
    let state = match body_text.as_str() {
        "[deleted]" => CommentState::Deleted,
        "[removed]" => CommentState::Removed,
        _ if flag(&el, &["is-comment-deleted", "deleted"]) => CommentState::Deleted,
        _ if flag(&el, &["is-comment-removed", "removed"]) => CommentState::Removed,
        _ => CommentState::Normal,
    };
    let hidden_score = flag(&el, &["is-score-hidden"]);
    let comment = Comment {
        post_id: attr(&el, "postid")
            .or_else(|| attr(&el, "post-id"))
            .map_or_else(|| post.clone(), PostId::new),
        parent: attr(&el, "parentid")
            .or_else(|| attr(&el, "parent-id"))
            .filter(|p| p.starts_with("t1_"))
            .map(CommentId::new),
        author: username(author),
        body: Body::Html(if state == CommentState::Normal {
            body_html
        } else {
            String::new()
        }),
        created: attr(&el, "created").and_then(parse_timestamp).unwrap_or(0),
        score: if hidden_score {
            None
        } else {
            attr(&el, "score").and_then(|s| s.parse().ok())
        },
        vote: vote_state(&el),
        saved: flag(&el, &["is-saved", "saved"]),
        is_submitter: flag(&el, &["is-op"]),
        pinned: flag(&el, &["stickied"]),
        edited: flag(&el, &["is-edited", "edited"]),
        state,
        depth,
        permalink: attr(&el, "permalink").unwrap_or_default().to_string(),
        id: id.clone(),
    };
    Some(CommentNode::Comment {
        replies: walk(*el, Some(&id), depth + 1, post),
        comment,
    })
}

/// The comment's own `[slot=comment]`, not one of a nested reply's.
fn own_slot(el: ElementRef<'_>) -> Option<ElementRef<'_>> {
    el.select(&sel("[slot=comment]")).find(|slot| {
        slot.ancestors()
            .filter_map(ElementRef::wrap)
            .find(|a| a.value().name() == "shreddit-comment")
            .is_some_and(|a| a.id() == el.id())
    })
}

fn more_node(el: ElementRef<'_>, parent: Option<&CommentId>, depth: u32) -> Option<CommentNode> {
    let src = attr(&el, "src")?.to_string();
    let cursor = el
        .select(&sel("input[name=cursor]"))
        .next()
        .and_then(|i| attr(&i, "value"))?
        .to_string();
    let depth = query_param(&src, "startingDepth")
        .and_then(|d| d.parse().ok())
        .unwrap_or(depth);
    Some(CommentNode::More {
        parent: parent.cloned(),
        depth,
        count: first_number(&text_of(&el)).unwrap_or(0),
        token: More { src, cursor }.to_token(),
    })
}

fn continue_node(el: ElementRef<'_>, parent: Option<&CommentId>, depth: u32) -> Option<CommentNode> {
    let href = attr(&el, "href")?;
    let id = comment_id_from_path(href).or_else(|| parent.cloned())?;
    Some(CommentNode::Continue { parent: id, depth })
}

/// `t1_abc` from `/r/x/comments/1x/comment/abc/`.
pub fn comment_id_from_path(path: &str) -> Option<CommentId> {
    let mut parts = reply::path_of(path).split('/');
    parts.find(|p| *p == "comment")?;
    let id = parts.next().filter(|p| !p.is_empty())?;
    Some(CommentId::new(format!("t1_{id}")))
}

fn query_param<'a>(url: &'a str, name: &str) -> Option<&'a str> {
    url.split_once('?')?
        .1
        .split('&')
        .find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == name).map(|(_, v)| v))
}

/// Whether a node exists anywhere under this parsed fragment; used by tests.
#[cfg(test)]
fn count(nodes: &[CommentNode]) -> usize {
    nodes
        .iter()
        .map(|n| match n {
            CommentNode::Comment { replies, .. } => 1 + count(replies),
            _ => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::tests::fixture;

    fn post() -> PostId {
        PostId::new("t3_test")
    }

    fn first_comment(nodes: &[CommentNode]) -> &Comment {
        nodes
            .iter()
            .find_map(|n| match n {
                CommentNode::Comment { comment, .. } => Some(comment),
                _ => None,
            })
            .expect("a comment")
    }

    #[test]
    fn recorded_tree_has_every_comment_once_and_nested() {
        let html = fixture("post_comments.html");
        let nodes = parse(&html, &post());
        assert_eq!(count(&nodes), html.matches("<shreddit-comment ").count());
        let first = first_comment(&nodes);
        assert!(first.id.as_str().starts_with("t1_"));
        assert_eq!(first.depth, 0);
        assert_eq!(first.parent, None);
        assert!(first.created > 1_700_000_000);
        assert!(first.permalink.contains("/comment/"));
        let Body::Html(body) = &first.body else {
            panic!("HTML body expected")
        };
        assert!(body.contains("<p>"), "{body}");
        fn check(nodes: &[CommentNode], parent: Option<&CommentId>, depth: u32) {
            for n in nodes {
                if let CommentNode::Comment { comment, replies } = n {
                    assert_eq!(comment.parent.as_ref(), parent, "{}", comment.id);
                    assert_eq!(comment.depth, depth, "{}", comment.id);
                    check(replies, Some(&comment.id), depth + 1);
                }
            }
        }
        check(&nodes, None, 0);
    }

    #[test]
    fn a_body_is_not_mixed_with_its_replies() {
        let html = r#"<shreddit-comment thingId="t1_a" depth="0">
                <div slot="comment"><p>parent</p></div>
                <shreddit-comment thingId="t1_b" depth="1" parentId="t1_a">
                  <div slot="comment"><p>child</p></div>
                </shreddit-comment>
            </shreddit-comment>"#;
        let nodes = parse(html, &post());
        assert_eq!(first_comment(&nodes).body, Body::Html("<p>parent</p>".into()));
        let CommentNode::Comment { replies, .. } = &nodes[0] else {
            panic!()
        };
        assert_eq!(first_comment(replies).body, Body::Html("<p>child</p>".into()));
    }

    #[test]
    fn template_content_is_read() {
        let html = r#"<template><shreddit-comment thingId="t1_a" depth="0"></shreddit-comment></template>"#;
        assert_eq!(count(&parse(html, &post())), 1);
    }

    #[test]
    fn ads_are_not_comments() {
        let html = r#"<shreddit-comment-tree-ads><template>
            <shreddit-comments-page-ad><shreddit-comment thingId="t1_ad"></shreddit-comment></shreddit-comments-page-ad>
            </template></shreddit-comment-tree-ads>"#;
        assert_eq!(count(&parse(html, &post())), 0);
    }

    #[test]
    fn more_row_keeps_what_the_request_needs() {
        let html = r#"<shreddit-comment thingId="t1_a" depth="2">
              <faceplate-partial class="x more-comments-partial" method="post"
                src="/svc/shreddit/more-comments/r/s/t3_x?sort=CONFIDENCE&amp;startingDepth=3">
                <input type="hidden" name="cursor" value="abc=">
                <button>12 more replies</button>
              </faceplate-partial>
            </shreddit-comment>"#;
        let nodes = parse(html, &post());
        let CommentNode::Comment { replies, .. } = &nodes[0] else {
            panic!()
        };
        let CommentNode::More {
            parent,
            depth,
            count,
            token,
        } = &replies[0]
        else {
            panic!("{replies:?}")
        };
        assert_eq!(parent.as_ref().unwrap().as_str(), "t1_a");
        assert_eq!(*depth, 3);
        assert_eq!(*count, 12);
        let more = More::from_token(token).unwrap();
        assert_eq!(more.cursor, "abc=");
        assert!(more.src.ends_with("startingDepth=3"));
    }

    #[test]
    fn continue_link_targets_the_comment_it_names() {
        let html = r#"<shreddit-comment thingId="t1_a" depth="9">
              <a slot="more-comments-permalink" href="/r/x/comments/1x/comment/pgqlp9p/?force-legacy-sct=1">Continue</a>
            </shreddit-comment>"#;
        let nodes = parse(html, &post());
        let CommentNode::Comment { replies, .. } = &nodes[0] else {
            panic!()
        };
        assert_eq!(
            replies[0],
            CommentNode::Continue {
                parent: CommentId::new("t1_pgqlp9p"),
                depth: 10
            }
        );
    }

    #[test]
    fn recorded_more_comments_keep_their_absolute_depth() {
        let nodes = parse(&fixture("more_comments.html"), &post());
        let first = first_comment(&nodes);
        assert_eq!(first.depth, 3);
        assert_eq!(first.parent.as_ref().unwrap().as_str(), "t1_pgqlp9p");
        assert_eq!(first.post_id.as_str(), "t3_1x5uo7s");
    }

    #[test]
    fn deleted_and_hidden() {
        let html = r#"<shreddit-comment thingId="t1_a" depth="0" author="[deleted]" is-score-hidden score="5">
               <div slot="comment"><p>[deleted]</p></div></shreddit-comment>"#;
        let c = parse(html, &post());
        let c = first_comment(&c);
        assert_eq!(c.author, None);
        assert_eq!(c.state, CommentState::Deleted);
        assert_eq!(c.body, Body::Html(String::new()));
        assert_eq!(c.score, None);
    }
}
