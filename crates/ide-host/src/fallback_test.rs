//! The fallback (`fallback.rs`; openspec/specs/repository/ide-embedding/spec.md, "Bridge"): the way back to a backend
//! of the bridge's build, and the ends of the bridge

use crate::backend::{Transport, handshake};
use crate::fallback::{Fallback, watch_for_own_backend_every};
use crate::server::{Backend, Connection, Ended};
use backend_host::host::{Host, HostHooks};
use backend_host::in_process::InProcess;
use backend_host::socket::Listener;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

const BUILD: &str = "0.0.0-bridge-test";

/// How long a test waits for the bridge
const WAIT: Duration = Duration::from_secs(20);

/// A directory of its own for the socket of a test, short enough for `sun_path`, its name unique, as the name of the
/// pipe on Windows is
fn scratch() -> (tempfile::TempDir, PathBuf) {
  let base = if cfg!(unix) { PathBuf::from("/tmp") } else { std::env::temp_dir() };
  let tmp = tempfile::Builder::new().prefix("bdb").tempdir_in(base).unwrap();
  let unique = tmp.path().file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
  let dir = tmp.path().join(format!("branch-deck-{unique}"));
  (tmp, dir)
}

/// The fallback finds a backend of its build once one listens on the socket of its key: a backend of another build
/// keeps it serving, one of its build ends its watch (the bridge then ends with 1, and the IDE starts it again to join)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_fallback_ends_once_a_backend_of_its_build_listens() {
  let app_data = tempfile::tempdir().unwrap();
  let own = Fallback::open(Some(app_data.path().to_path_buf())).unwrap();
  let (_tmp, dir) = scratch();
  let other = Listener::bind(&dir).await.unwrap();
  let other = tokio::spawn(other.serve(Host::of_build(Arc::clone(own.host().context()), "0.0.0-other".to_owned(), HostHooks::default())));
  let watch = tokio::spawn({
    let dir = dir.clone();
    async move { watch_for_own_backend_every(&dir, BUILD, Duration::from_millis(50)).await }
  });
  tokio::time::sleep(Duration::from_millis(400)).await;
  assert!(!watch.is_finished(), "a backend of another build is no way back");

  // the other backend goes, one of the bridge's build starts
  other.abort();
  let _ = other.await;
  tokio::time::sleep(Duration::from_millis(200)).await;
  let mine = Listener::bind(&dir).await.unwrap();
  tokio::spawn(mine.serve(Host::of_build(Arc::clone(own.host().context()), BUILD.to_owned(), HostHooks::default())));
  tokio::time::timeout(WAIT, watch).await.expect("the fallback finds the backend of its build").unwrap();
}

/// The backend ends the connection of a bridge: an unexpected end (`Ended::Backend`, the exit code 1, after which the
/// IDE starts the bridge again); the end of the input is the end the IDE asked for (`Ended::Input`, 0)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_end_of_the_backend_is_unexpected_and_an_end_of_the_input_is_asked_for() {
  let app_data = tempfile::tempdir().unwrap();
  let own = Fallback::open(Some(app_data.path().to_path_buf())).unwrap();
  for backend_first in [true, false] {
    let client = InProcess::client(own.host());
    let host_side = Arc::clone(client.connection());
    let (link, source) = handshake(Transport::InProcess(client), own.host().build()).await.unwrap();
    let (connection, mut out) = Connection::new();
    let (lines, incoming) = mpsc::unbounded_channel();
    let serving = tokio::spawn(connection.serve(async move { Backend { link, source } }, incoming));
    lines.send(json!({ "jsonrpc": "2.0", "id": 1, "method": "ping", "params": {} }).to_string()).unwrap();
    let answer = tokio::time::timeout(WAIT, out.recv()).await.unwrap().unwrap();
    assert_eq!(serde_json::from_str::<serde_json::Value>(&answer).unwrap()["result"], json!({}));
    if backend_first {
      host_side.close();
      assert_eq!(tokio::time::timeout(WAIT, serving).await.unwrap().unwrap(), Ended::Backend);
    } else {
      drop(lines);
      assert_eq!(tokio::time::timeout(WAIT, serving).await.unwrap().unwrap(), Ended::Input);
    }
  }
}
