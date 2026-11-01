//! Helpers for store tests: a context on `MockBackend` with in-memory settings.

use std::rc::Rc;
use std::time::{Duration, Instant};

use agora_core::MockBackend;
use gtk::{gio, glib};

use super::Context;

/// A mock context. Settings use the memory backend and the schema `build.rs`
/// compiled, so tests never touch the user's dconf.
pub fn context(backend: MockBackend) -> Rc<Context> {
    // SAFETY: set before any GSettings use in this test process; tests that
    // race here all write the same values.
    unsafe {
        std::env::set_var("GSETTINGS_BACKEND", "memory");
        std::env::set_var("GSETTINGS_SCHEMA_DIR", crate::config::DEV_SCHEMA_DIR);
    }
    let settings = gio::Settings::new(crate::config::SCHEMA_ID);
    Context::new(Rc::new(backend), settings, true)
}

/// Runs `f` with a fresh main context as the thread default, so tasks a test
/// spawns never outlive it on another test's thread.
pub fn with_main_context<R>(f: impl FnOnce() -> R) -> R {
    glib::MainContext::new().with_thread_default(f).expect("main context")
}

/// Iterates the thread-default main context until `done` holds (5 s limit).
pub fn run_until(done: impl Fn() -> bool) {
    let ctx = glib::MainContext::ref_thread_default();
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < Duration::from_secs(5), "timed out");
        ctx.iteration(false);
    }
}
