//! Another build (`connection.rs`; openspec/specs/repository/backend-protocol/spec.md, "Socket: The stale side yields to
//! a client of another build"): the stale side yields, stale as the hooks of the app tell it (`Executable::is_stale`)

use crate::corpus_test::Fixture;
use crate::host::{Host, HostHooks};
use crate::in_process::InProcess;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

fn initialize(build: &str) -> Value {
  json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
    "channel": "ahp-root://", "protocolVersions": ["1.0.0"], "clientId": "bridge", "clientInfo": { "name": "branch-deck-ide", "build": build },
    "_meta": { "x-branch-deck": { "version": "1.0.0", "capabilities": { "hostRequests": ["openWindow"] } } },
  } })
}

/// What a stale host did: it exited, or it told Restart to Update
#[derive(Default)]
struct Yielded {
  exited: AtomicBool,
  restart_told: AtomicBool,
}

/// A host of the old build, stale, with a window or without one
fn stale_host(fixture: &Fixture, has_window: bool, yielded: &Arc<Yielded>) -> Arc<Host> {
  let (exited, told) = (Arc::clone(yielded), Arc::clone(yielded));
  let hooks = HostHooks {
    stale: Box::new(|| true),
    has_window: Box::new(move || has_window),
    exit: Box::new(move || exited.exited.store(true, Ordering::SeqCst)),
    restart_to_update: Box::new(move || told.restart_told.store(true, Ordering::SeqCst)),
    ..HostHooks::default()
  };
  Host::of_build(Arc::clone(&fixture.ctx), "0.0.1-old".to_owned(), hooks)
}

async fn answer_of(link: &InProcess, request: &Value) -> Value {
  link.send(request);
  let frame = tokio::time::timeout(Duration::from_secs(10), link.next()).await.unwrap().unwrap();
  serde_json::from_str(&frame.into_text()).unwrap()
}

/// The app updated in place, no window: a bridge of the new build gets `otherBuild` with `exits: true`, and the backend
/// exits after the answer
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stale_backend_without_a_window_exits_after_its_answer() {
  let fixture = Fixture::new().await;
  let yielded = Arc::new(Yielded::default());
  let host = stale_host(&fixture, false, &yielded);
  let link = InProcess::client(&host);
  let answer = answer_of(&link, &initialize("0.0.2-new")).await;
  assert_eq!(answer["error"]["code"], -32005);
  assert_eq!(answer["error"]["data"], json!({ "reason": "otherBuild", "build": "0.0.1-old", "exits": true }));
  let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
  while !yielded.exited.load(Ordering::SeqCst) {
    assert!(tokio::time::Instant::now() < deadline, "the stale backend exits");
    tokio::time::sleep(Duration::from_millis(20)).await;
  }
  assert!(!yielded.restart_told.load(Ordering::SeqCst));
}

/// The app updated in place, a window open: `otherBuild` with `exits: false`, the backend stays and tells Restart to
/// Update, and the bridge runs its own backend
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stale_backend_with_a_window_stays_and_tells_restart_to_update() {
  let fixture = Fixture::new().await;
  let yielded = Arc::new(Yielded::default());
  let host = stale_host(&fixture, true, &yielded);
  let link = InProcess::client(&host);
  let answer = answer_of(&link, &initialize("0.0.2-new")).await;
  assert_eq!(answer["error"]["data"]["exits"], false, "{answer:#}");
  assert!(yielded.restart_told.load(Ordering::SeqCst));
  tokio::time::sleep(Duration::from_millis(300)).await;
  assert!(!yielded.exited.load(Ordering::SeqCst), "a backend with a window stays");
}
