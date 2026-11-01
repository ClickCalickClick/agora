//! Invented sample data for `MockBackend`. Deterministic: the same build gives
//! the same posts and threads every time.

use std::collections::HashMap;

use super::{MOCK_NOW, MOCK_USER, Tree};
use crate::model::*;

/// Invented usernames used by the sample data.
pub const USERS: &[&str] = &[
    "quiet_otter",
    "pixel_gardener",
    "stonecutter",
    "late_tram",
    "fernwood",
    "byte_baker",
    "harbor_lights",
    "mossy_keys",
    "tea_and_tabs",
    "northbound",
    "kettle_logic",
    "paper_kite",
    "slow_river",
    "lamplighter",
    "copper_finch",
    MOCK_USER,
];

/// Small deterministic generator (SplitMix64).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

pub(super) fn domain_of(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_string()
}

struct SubSpec {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    members: u64,
    subscribed: bool,
    topics: &'static [&'static str],
}

const SUBS: &[SubSpec] = &[
    SubSpec {
        name: "gnome",
        title: "GNOME",
        description: "News, questions and screenshots about the GNOME desktop and its apps.",
        members: 182_000,
        subscribed: true,
        topics: &[
            "the new sidebar",
            "fractional scaling",
            "a libadwaita app I wrote",
            "Files search",
            "the Settings redesign",
            "my extension list",
            "GNOME on a tablet",
            "accent colors",
        ],
    },
    SubSpec {
        name: "rust",
        title: "The Rust Programming Language",
        description: "A place for all things related to the Rust programming language.",
        members: 389_000,
        subscribed: true,
        topics: &[
            "async traits",
            "error handling",
            "gtk-rs",
            "compile times",
            "a borrow checker puzzle",
            "my first crate",
            "editions",
            "unsafe blocks",
        ],
    },
    SubSpec {
        name: "linux",
        title: "Linux",
        description: "All things Linux and GNU/Linux, the kernel and the systems built on it.",
        members: 1_200_000,
        subscribed: true,
        topics: &[
            "a new kernel release",
            "Wayland",
            "my distro hop",
            "power usage on laptops",
            "filesystems",
            "Flatpak",
            "a ten-year-old ThinkPad",
            "PipeWire",
        ],
    },
    SubSpec {
        name: "photography",
        title: "Photography",
        description: "Talk about the craft of photography: gear, technique and the photos themselves.",
        members: 640_000,
        subscribed: true,
        topics: &[
            "golden hour",
            "a used prime lens",
            "editing raw files",
            "street photography",
            "film again",
            "composition",
            "a foggy morning",
            "tripods",
        ],
    },
    SubSpec {
        name: "Cooking",
        title: "Cooking",
        description: "Recipes, techniques and questions from home cooks.",
        members: 920_000,
        subscribed: true,
        topics: &[
            "sourdough",
            "cast iron",
            "a weeknight soup",
            "knife skills",
            "braising",
            "spice blends",
            "meal prep",
            "a failed custard",
        ],
    },
    SubSpec {
        name: "AskHistorians",
        title: "Ask Historians",
        description: "Questions about history, answered in depth.",
        members: 2_100_000,
        subscribed: false,
        topics: &[
            "medieval markets",
            "ancient libraries",
            "letters home",
            "old maps",
            "the printing press",
            "early railways",
        ],
    },
    SubSpec {
        name: "MechanicalKeyboards",
        title: "Mechanical Keyboards",
        description: "Builds, switches, keycaps and layouts.",
        members: 1_000_000,
        subscribed: false,
        topics: &[
            "a first build",
            "tactile switches",
            "split layouts",
            "keycap profiles",
            "lubing",
            "a 40% board",
        ],
    },
    SubSpec {
        name: "gardening",
        title: "Gardening",
        description: "Grow things, show things and ask about the things that won't grow.",
        members: 700_000,
        subscribed: true,
        topics: &[
            "tomatoes",
            "compost",
            "a shady corner",
            "seed saving",
            "raised beds",
            "winter prep",
        ],
    },
];

const TITLE_FORMS: &[&str] = &[
    "What do you all think about {}?",
    "Finally figured out {}",
    "A short write-up on {}",
    "Question about {} — am I doing this wrong?",
    "{} after one year: thoughts",
    "TIL something about {}",
    "Small win today with {}",
    "Is anyone else struggling with {}?",
    "Beginner's notes on {}",
    "Here's my take on {}, with some caveats that turned out to matter more than I expected when I started",
];

const SENTENCES: &[&str] = &[
    "I spent most of the weekend on this and wanted to write it down before I forget.",
    "The short version is that it works, but not the way I expected.",
    "Happy to answer questions if anyone is trying the same thing.",
    "There are a few rough edges, mostly around the defaults.",
    "I tried three approaches before landing on this one.",
    "Credit to the people in last week's thread for the pointers.",
    "It took longer than it should have, mostly because I skipped the documentation.",
    "Curious whether this matches what other people see.",
    "Not sure this is the best way, but it is the simplest one I found.",
    "The difference was bigger than I expected.",
];

const REPLIES: &[&str] = &[
    "This matches what I saw too.",
    "Thanks for writing this up, it is clearer than the docs.",
    "Did you try changing the default first? That fixed it for me.",
    "I disagree a little: it depends a lot on the setup.",
    "Same here. It took me a week to notice.",
    "Great post. Saving this for later.",
    "Could you share more details about the second step?",
    "Honestly the simplest answer is usually right with this one.",
    "I had the opposite experience, but I think we were on different versions.",
    "This is the kind of post I come here for.",
    "Small correction: the setting moved in the latest release.",
    "*Ha*, I did exactly the same thing last month.",
    "Agreed. The **first** option is the one I'd recommend to most people.",
    "Here is what worked for me:\n\n1. Start with the defaults\n2. Change one thing at a time\n3. Write down what you changed",
    "> It took longer than it should have\n\nStory of my life.",
    "Spoiler for anyone following along: >!it was the cable the whole time!<",
];

/// Hand-written posts that exercise every Markdown feature and row badge.
fn showcase(rng: &mut Rng) -> Vec<Post> {
    let mut posts = Vec::new();
    let mut add = |id: &str,
                   sub: &str,
                   author: Option<&str>,
                   title: &str,
                   kind: PostKind,
                   body: Option<&str>,
                   hours: i64,
                   score: i64,
                   comments: u64,
                   f: &dyn Fn(&mut Post)| {
        let mut p = Post {
            id: PostId::new(format!("t3_{id}")),
            subreddit: SubredditName::new(sub),
            author: author.map(Username::new),
            title: title.into(),
            kind,
            body: body.map(|b| Body::Markdown(b.into())),
            permalink: format!("/r/{sub}/comments/{id}/"),
            created: MOCK_NOW - hours * 3600 - rng.below(3000) as i64,
            score,
            comment_count: comments,
            vote: VoteState::None,
            saved: false,
            flair: None,
            nsfw: false,
            spoiler: false,
            pinned: false,
            locked: false,
            oc: false,
            edited: false,
            thumbnail: None,
        };
        f(&mut p);
        posts.push(p);
    };

    add("showcase1", "gnome", Some("pixel_gardener"), "A tour of the formatting a post can use", PostKind::Text, Some(
"Reddit posts support more formatting than people expect. Here is **bold**, *italic*, ~~struck out~~ and `inline code`, plus a [link to GNOME](https://www.gnome.org).

## Lists

- Plain bullets
- With *nested* items:
  1. first
  2. second
- And a last one

## Quotes and code

> Quiet places make for good conversations.
>
> > Quotes can nest, too.

```rust
fn main() {
    println!(\"Hello from a code block\");
}
```

## A table

| Widget | Purpose | Since |
|:-------|:-------:|------:|
| AdwSidebar | Navigation | 1.9 |
| AdwSpinner | Loading | 1.6 |
| AdwToggleGroup | Choices | 1.7 |

Superscript works like 10^th or ^(a few words), spoilers hide >!the ending!<, and names link: r/rust and u/quiet_otter.

---

That's the tour."), 5, 412, 64, &|p| { p.flair = Some("Guide".into()); p.oc = true; });

    add(
        "showcase2",
        "gnome",
        Some("stonecutter"),
        "Weekly thread: share your desktop",
        PostKind::Text,
        Some("Post a screenshot and tell us what you changed. Be kind, and say which extensions you use."),
        30,
        88,
        41,
        &|p| {
            p.pinned = true;
            p.flair = Some("Weekly".into());
        },
    );

    add(
        "showcase3",
        "linux",
        Some("late_tram"),
        "An article on how the kernel schedules work across CPU cores",
        PostKind::Link {
            url: "https://example.org/articles/scheduler".into(),
            domain: "example.org".into(),
        },
        None,
        9,
        1290,
        233,
        &|_| {},
    );

    add(
        "showcase4",
        "photography",
        Some("fernwood"),
        "The ending of the documentary we talked about last week",
        PostKind::Text,
        Some(
            "Discussing the ending, so marked as a spoiler.\n\nThe last scene was filmed on the same hill as the first one, which I only noticed on a second viewing.",
        ),
        20,
        57,
        19,
        &|p| {
            p.spoiler = true;
        },
    );

    add(
        "showcase5",
        "rust",
        Some("byte_baker"),
        "This thread is locked, but you can still read it",
        PostKind::Text,
        Some("Moderators locked this after the discussion went in circles. Reading still works; replying does not."),
        70,
        34,
        12,
        &|p| {
            p.locked = true;
            p.edited = true;
        },
    );

    add(
        "showcase6",
        "Cooking",
        None,
        "[deleted]",
        PostKind::Text,
        Some(""),
        40,
        3,
        7,
        &|_| {},
    );

    add(
        "showcase7",
        "rust",
        Some("mossy_keys"),
        "A very long thread to test loading more replies and deep nesting",
        PostKind::Text,
        Some(
            "Reply here with anything. This thread exists so the comment tree has deep branches and \"more replies\" rows.",
        ),
        12,
        230,
        380,
        &|p| {
            p.flair = Some("Discussion".into());
        },
    );

    add(
        "showcase8",
        "gnome",
        Some(MOCK_USER),
        "My own post, so the You badge shows",
        PostKind::Text,
        Some("Testing how my own posts look."),
        2,
        5,
        2,
        &|p| {
            p.saved = true;
        },
    );

    add(
        "showcase9",
        "linux",
        Some("kettle_logic"),
        "An extremely long title to check wrapping, because some people write their entire question in the title and then leave the body empty, which is fine but tests the three-line limit nicely",
        PostKind::Text,
        None,
        15,
        18,
        4,
        &|_| {},
    );

    posts
}

pub(super) fn build() -> (Vec<Subreddit>, Vec<Post>, HashMap<PostId, Vec<Tree>>) {
    let mut rng = Rng(0xA90A);
    let subreddits: Vec<Subreddit> = SUBS
        .iter()
        .enumerate()
        .map(|(i, s)| Subreddit {
            id: format!("t5_mock{i}"),
            name: SubredditName::new(s.name),
            title: Some(s.title.into()),
            description: Some(s.description.into()),
            icon_url: None,
            banner_url: None,
            members: Some(s.members),
            subscribed: s.subscribed,
            nsfw: false,
            created: Some(MOCK_NOW - (5 + i as i64) * 365 * 86_400),
            rules: vec![
                Rule {
                    title: "Be kind".into(),
                    description: Some(Body::Markdown("Disagree with ideas, not people.".into())),
                },
                Rule {
                    title: "Stay on topic".into(),
                    description: Some(Body::Markdown(format!("Posts must be about {}.", s.title))),
                },
                Rule {
                    title: "No spam or self-promotion".into(),
                    description: None,
                },
            ],
        })
        .collect();

    let mut posts = showcase(&mut rng);
    let mut n = 0;
    for spec in SUBS {
        for _ in 0..14 {
            n += 1;
            let topic = *rng.pick(spec.topics);
            let title = rng.pick(TITLE_FORMS).replace("{}", topic);
            let title = capitalize(&title);
            let author = *rng.pick(&USERS[..USERS.len() - 1]);
            let link = rng.chance(20);
            let kind = if link {
                let url = format!("https://example.com/{}/{}", spec.name.to_lowercase(), n);
                PostKind::Link {
                    domain: domain_of(&url),
                    url,
                }
            } else {
                PostKind::Text
            };
            let body = (!link).then(|| {
                let count = 1 + rng.below(4) as usize;
                let paras: Vec<String> = (0..count)
                    .map(|_| {
                        (0..2 + rng.below(3))
                            .map(|_| *rng.pick(SENTENCES))
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .collect();
                Body::Markdown(paras.join("\n\n"))
            });
            let id = format!("gen{n}");
            let hours = rng.below(24 * 20) as i64;
            let score = (rng.below(1000) as i64).pow(2) / 600 + rng.below(20) as i64 - 3;
            posts.push(Post {
                id: PostId::new(format!("t3_{id}")),
                subreddit: SubredditName::new(spec.name),
                author: Some(Username::new(author)),
                title,
                kind,
                body,
                permalink: format!("/r/{}/comments/{id}/", spec.name),
                created: MOCK_NOW - hours * 3600 - rng.below(3600) as i64,
                score,
                comment_count: 0,
                vote: VoteState::None,
                saved: rng.chance(4),
                flair: rng
                    .chance(30)
                    .then(|| rng.pick(&["Question", "Discussion", "Help", "Showcase"]).to_string()),
                nsfw: false,
                spoiler: false,
                pinned: false,
                locked: false,
                oc: rng.chance(8),
                edited: rng.chance(10),
                thumbnail: None,
            });
        }
    }

    let mut threads = HashMap::new();
    for post in &mut posts {
        let (target, max_depth) = if post.id.as_str() == "t3_showcase7" {
            (380, 14)
        } else if post.comment_count > 0 {
            (post.comment_count as usize, 6)
        } else {
            (rng.below(30) as usize, 5)
        };
        let mut budget = target;
        let mut trees = Vec::new();
        let mut index = 0;
        while budget > 0 {
            trees.push(gen_tree(&mut rng, post, None, 0, max_depth, &mut budget, &mut index));
        }
        post.comment_count = target as u64;
        threads.insert(post.id.clone(), trees);
    }
    (subreddits, posts, threads)
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn gen_tree(
    rng: &mut Rng,
    post: &Post,
    parent: Option<&CommentId>,
    depth: u32,
    max_depth: u32,
    budget: &mut usize,
    index: &mut u64,
) -> Tree {
    *budget = budget.saturating_sub(1);
    *index += 1;
    let id = CommentId::new(format!("t1_{}c{}", &post.id.as_str()[3..], index));
    let state = match rng.below(40) {
        0 => CommentState::Deleted,
        1 => CommentState::Removed,
        _ => CommentState::Normal,
    };
    let author = match state {
        CommentState::Normal => {
            if rng.chance(12) {
                post.author.clone()
            } else {
                Some(Username::new(*rng.pick(USERS)))
            }
        }
        _ => None,
    };
    let body = match state {
        CommentState::Normal => {
            let mut text = rng.pick(REPLIES).to_string();
            if rng.chance(25) {
                text.push_str("\n\n");
                text.push_str(rng.pick(SENTENCES));
            }
            text
        }
        _ => String::new(),
    };
    let is_submitter = author.is_some() && author == post.author;
    let mut comment = Comment {
        id: id.clone(),
        post_id: post.id.clone(),
        parent: parent.cloned(),
        author,
        body: Body::Markdown(body),
        created: post.created + 600 + rng.below(40_000) as i64,
        score: Some(rng.below(200) as i64 - 10),
        vote: VoteState::None,
        saved: false,
        is_submitter,
        pinned: false,
        edited: rng.chance(6),
        state,
        depth,
    };
    if comment.created > MOCK_NOW {
        comment.created = MOCK_NOW - 60;
    }
    if rng.chance(5) {
        comment.score = None;
    }
    let mut replies = Vec::new();
    if depth < max_depth {
        let wanted = match depth {
            0 => rng.below(4),
            _ => rng.below(3),
        };
        for _ in 0..wanted {
            if *budget == 0 {
                break;
            }
            replies.push(gen_tree(rng, post, Some(&id), depth + 1, max_depth, budget, index));
        }
    }
    Tree { comment, replies }
}

/// Sample notifications.
pub(super) fn notifications() -> Vec<Notification> {
    vec![
        Notification {
            id: "n1".into(),
            kind: NotificationKind::CommentReply,
            title: "u/quiet_otter replied to your comment in r/gnome".into(),
            body: Some("Thanks, that fixed it!".into()),
            permalink: "/r/gnome/comments/showcase8/".into(),
            created: MOCK_NOW - 1800,
            read: false,
        },
        Notification {
            id: "n2".into(),
            kind: NotificationKind::Mention,
            title: "u/fernwood mentioned you in r/photography".into(),
            body: Some("u/sample_reader took the photo I mean.".into()),
            permalink: "/r/photography/comments/showcase4/".into(),
            created: MOCK_NOW - 7200,
            read: true,
        },
    ]
}
