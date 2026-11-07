//! Agora's real backend: a hidden WebKit session on reddit.com (SPEC §4).
//!
//! `scheduler` and `parse` need no display, so plain `cargo test` covers them.

mod backend;
mod bridge;
pub mod parse;
mod quiet;
pub mod scheduler;

pub use backend::{Config, RedditWebBackend, SelfTestLine};
