//! The JS bridge: scripts, the `agora` message handler and calls into the
//! isolated world (SPEC §4.2). Arguments travel as typed values and are never
//! put into script text.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use agora_core::AgoraError;
use gtk::glib;
use serde::Deserialize;
use webkit6::prelude::*;
use webkit6::{UserContentInjectedFrames, UserContentManager, UserScript, UserScriptInjectionTime, WebView};

use crate::parse::reply::Reply;

/// The script world everything runs in.
pub const WORLD: &str = "agora";
/// How long the chunks of a body may take to arrive once its request ended.
const CHUNK_TIMEOUT: Duration = Duration::from_secs(60);

/// The bridge and every operation, compiled into the crate (SPEC §4.2).
const SCRIPTS: &[&str] = &[
    include_str!("../js/agora-bridge.js"),
    include_str!("../js/ops/whoami.js"),
    include_str!("../js/ops/listing.js"),
    include_str!("../js/ops/post_page.js"),
    include_str!("../js/ops/comments.js"),
    include_str!("../js/ops/more_comments.js"),
    include_str!("../js/ops/subreddit_info.js"),
    include_str!("../js/ops/subscriptions.js"),
    include_str!("../js/ops/user_profile.js"),
    include_str!("../js/ops/notifications.js"),
    include_str!("../js/ops/search.js"),
];

#[derive(Deserialize)]
struct Chunk {
    id: u32,
    data: String,
}

type Chunks = Rc<RefCell<HashMap<u32, async_channel::Sender<String>>>>;

/// Calls into the hidden view.
pub struct Bridge {
    view: WebView,
    chunks: Chunks,
    next_id: Cell<u32>,
}

impl Bridge {
    /// Adds the scripts and the handler to `manager`, for pages under `origin`.
    pub fn new(view: WebView, manager: &UserContentManager, origin: &str) -> Self {
        // WebKit's URL patterns have no port; only the bridge tests have one.
        let (scheme, host) = origin.split_once("://").unwrap_or(("https", origin));
        let host = host.split(':').next().unwrap_or(host);
        let allow = format!("{scheme}://{host}/*");
        manager.add_script(&UserScript::for_world(
            &SCRIPTS.join("\n"),
            UserContentInjectedFrames::TopFrame,
            UserScriptInjectionTime::Start,
            WORLD,
            &[allow.as_str()],
            &[],
        ));
        let chunks: Chunks = Rc::default();
        let for_handler = chunks.clone();
        manager.connect_script_message_received(Some(WORLD), move |_, value| {
            let Some(json) = value.to_json(0) else { return };
            let Ok(chunk) = serde_json::from_str::<Chunk>(&json) else {
                tracing::warn!("bridge sent a message that is not a chunk");
                return;
            };
            if let Some(sender) = for_handler.borrow().get(&chunk.id) {
                let _ = sender.try_send(chunk.data);
            }
        });
        manager.register_script_message_handler(WORLD, Some(WORLD));
        Self {
            view,
            chunks,
            next_id: Cell::new(1),
        }
    }

    /// Runs `op` and returns its response with the body reassembled.
    pub async fn call(&self, op: &str, args: &serde_json::Value) -> Result<Reply, AgoraError> {
        let id = self.next_id.get();
        self.next_id.set(id.wrapping_add(1));
        let (sender, receiver) = async_channel::unbounded();
        self.chunks.borrow_mut().insert(id, sender);
        let _forget = Forget(self.chunks.clone(), id);

        let arguments = glib::VariantDict::new(None);
        arguments.insert("op", op);
        arguments.insert("args", args.to_string());
        arguments.insert("id", id as i32);
        let value = self
            .view
            .call_async_javascript_function_future(
                "return agora.run(op, JSON.parse(args), id);",
                Some(&arguments.end()),
                Some(WORLD),
                None,
            )
            .await
            .map_err(|e| AgoraError::Network(e.message().to_string()))?;
        let json = value
            .to_json(0)
            .ok_or_else(|| AgoraError::Parse("the bridge returned no value".into()))?;
        let mut reply: Reply = serde_json::from_str(&json).map_err(|e| AgoraError::Parse(e.to_string()))?;
        for _ in 0..reply.chunks {
            let recv = std::pin::pin!(receiver.recv());
            let timeout = std::pin::pin!(glib::timeout_future(CHUNK_TIMEOUT));
            let next = futures::future::select(recv, timeout).await;
            match next {
                futures::future::Either::Left((Ok(data), _)) => reply.body.push_str(&data),
                _ => return Err(AgoraError::Network("a response body did not arrive".into())),
            }
        }
        Ok(reply)
    }
}

/// Forgets a call's chunk channel when the call ends, however it ends.
struct Forget(Chunks, u32);

impl Drop for Forget {
    fn drop(&mut self) {
        self.0.borrow_mut().remove(&self.1);
    }
}
