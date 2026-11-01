//! Shared types for Agora.
//!
//! This crate has no GTK or WebKit dependency. It defines the Reddit models the
//! UI works with, the error type, the [`Backend`] trait that is the only way the
//! UI reaches Reddit (SPEC §4.3), and [`MockBackend`], an in-memory backend with
//! sample data used for UI work and tests (`AGORA_MOCK=1`).

#![warn(missing_docs)]

mod backend;
mod error;
pub mod mock;
mod model;

pub use backend::{Backend, BackendEvent, BackendFuture, ListingRequest, SearchRequest, ThreadRequest};
pub use error::{AgoraError, BackendResult};
pub use mock::MockBackend;
pub use model::*;
