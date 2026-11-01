use futures::executor::block_on;

use super::*;

fn first_thread(backend: &MockBackend, post: &str) -> Vec<CommentNode> {
    block_on(backend.post_with_comments(ThreadRequest {
        post: PostId::new(post),
        sort: CommentSort::Best,
        focus: None,
    }))
    .unwrap()
    .1
}

#[test]
fn home_pages_through_all_posts() {
    let b = MockBackend::new();
    let mut after = None;
    let mut seen = 0;
    let mut pages = 0;
    loop {
        let page = block_on(b.listing(ListingRequest {
            feed: Feed::Home,
            sort: Sort::Hot,
            after,
        }))
        .unwrap();
        assert!(page.items.len() <= PAGE_SIZE);
        seen += page.items.len();
        pages += 1;
        after = page.after;
        if after.is_none() {
            break;
        }
    }
    assert!(pages >= 3, "sample data should span several pages");
    assert!(seen > 50);
}

#[test]
fn subreddit_feed_and_not_found() {
    let b = MockBackend::new();
    let page = block_on(b.listing(ListingRequest {
        feed: Feed::Subreddit(SubredditName::new("GNOME")),
        sort: Sort::New,
        after: None,
    }))
    .unwrap();
    assert!(page.items.iter().all(|p| p.subreddit.as_str() == "gnome"));
    let err = block_on(b.listing(ListingRequest {
        feed: Feed::Subreddit(SubredditName::new("doesnotexist")),
        sort: Sort::New,
        after: None,
    }))
    .unwrap_err();
    assert_eq!(err, AgoraError::NotFound);
}

fn walk(nodes: &[CommentNode], f: &mut impl FnMut(&CommentNode)) {
    for n in nodes {
        f(n);
        if let CommentNode::Comment { replies, .. } = n {
            walk(replies, f);
        }
    }
}

#[test]
fn long_thread_has_more_and_continue_rows() {
    let b = MockBackend::new();
    let nodes = first_thread(&b, "t3_showcase7");
    let (mut more, mut cont) = (Vec::new(), 0);
    walk(&nodes, &mut |n| match n {
        CommentNode::More { token, .. } => more.push(token.clone()),
        CommentNode::Continue { .. } => cont += 1,
        CommentNode::Comment { .. } => {}
    });
    assert!(!more.is_empty(), "expected more-replies rows");
    assert!(cont > 0, "expected continue-thread rows");
    let loaded = block_on(b.more_comments(PostId::new("t3_showcase7"), more[0].clone())).unwrap();
    assert!(!loaded.is_empty());
}

#[test]
fn continue_thread_focuses_comment() {
    let b = MockBackend::new();
    let nodes = first_thread(&b, "t3_showcase7");
    let mut target = None;
    walk(&nodes, &mut |n| {
        if let CommentNode::Continue { parent, .. } = n {
            target.get_or_insert(parent.clone());
        }
    });
    let target = target.unwrap();
    let (_, focused) = block_on(b.post_with_comments(ThreadRequest {
        post: PostId::new("t3_showcase7"),
        sort: CommentSort::Best,
        focus: Some(target.clone()),
    }))
    .unwrap();
    match &focused[0] {
        CommentNode::Comment { comment, replies } => {
            assert_eq!(comment.id, target);
            assert!(!replies.is_empty());
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn vote_changes_score_once() {
    let b = MockBackend::new();
    let id = PostId::new("t3_showcase1");
    let score = |b: &MockBackend| {
        block_on(b.post_with_comments(ThreadRequest {
            post: id.clone(),
            sort: CommentSort::Best,
            focus: None,
        }))
        .unwrap()
        .0
        .score
    };
    let before = score(&b);
    block_on(b.vote(Thing::Post(id.clone()), VoteState::Up)).unwrap();
    block_on(b.vote(Thing::Post(id.clone()), VoteState::Up)).unwrap();
    assert_eq!(score(&b), before + 1);
    block_on(b.vote(Thing::Post(id.clone()), VoteState::Down)).unwrap();
    assert_eq!(score(&b), before - 1);
}

#[test]
fn scripted_failures_and_events() {
    let b = MockBackend::new().with_failures(FailureScript::from_spec(
        "listing=network, whoami=expired, bogus=network",
    ));
    let err = block_on(b.listing(ListingRequest {
        feed: Feed::Home,
        sort: Sort::Best,
        after: None,
    }))
    .unwrap_err();
    assert!(matches!(err, AgoraError::Network(_)));
    assert_eq!(block_on(b.whoami()).unwrap_err(), AgoraError::SessionExpired);
    assert_eq!(b.events().try_recv().unwrap(), BackendEvent::SessionExpired);
}

#[test]
fn signed_out_session_expires() {
    let b = MockBackend::new();
    b.set_signed_in(false);
    assert_eq!(block_on(b.whoami()).unwrap(), None);
    assert_eq!(block_on(b.subscriptions()).unwrap_err(), AgoraError::SessionExpired);
}

#[test]
fn sample_data_is_deterministic() {
    let a = first_thread(&MockBackend::new(), "t3_gen3");
    let b = first_thread(&MockBackend::new(), "t3_gen3");
    assert_eq!(a, b);
}
