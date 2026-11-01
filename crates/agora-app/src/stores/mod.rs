//! Stores: GObjects that own state and expose list models (SPEC §3.4,
//! contract in `docs/STORES.md`). Views bind to stores and never call the
//! backend themselves.

mod context;
mod feed;
mod objects;
mod session;
mod subscriptions;
#[cfg(test)]
mod test_util;
mod thread;
mod writes;

pub use context::Context;
pub use feed::FeedStore;
pub use objects::*;
pub use session::{SessionState, SessionStore};
pub use subscriptions::SubscriptionsStore;
pub use thread::ThreadStore;
pub use writes::Writes;
