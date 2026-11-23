//! The local socket of the backend (openspec/specs/repository/backend-protocol/spec.md, "Socket"): the key of a build,
//! the owner-only directory, the peer of another user, `backend.lock` and its stale socket, `showMain` of a second start

use crate::host::{Host, HostHooks};
use crate::socket::{Listener, SocketError, connect, key, same_user, show_main};
use app_api::context::AppContext;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const BUILD: &str = "0.0.0-socket-test";

/// A directory of its own for the key of a test, short enough for `sun_path` (under `/tmp`, not the `$TMPDIR` of
/// macOS), its name unique, as the name of the pipe on Windows is
fn scratch() -> (tempfile::TempDir, PathBuf) {
  #[cfg(unix)]
  let tmp = tempfile::Builder::new().prefix("bdh").tempdir_in("/tmp").unwrap();
  #[cfg(not(unix))]
  let tmp = tempfile::Builder::new().prefix("bdh").tempdir().unwrap();
  let name = format!("branch-deck-{}", tmp.path().file_name().unwrap().to_string_lossy());
  let dir = tmp.path().join(name);
  (tmp, dir)
}

/// The context of a test repository of the e2e server, the state that keeps it with it
async fn context() -> (Arc<test_server::state::AppState>, Arc<AppContext>) {
  let (_app, state) = test_server::create_route_test_app_with_state();
  let request = serde_json::from_value(json!({ "template": "simple" })).unwrap();
  let created = test_server::create_repository(axum::extract::State(Arc::clone(&state)), axum::Json(request))
    .await
    .unwrap_or_else(|status| panic!("no test repository: {status}"));
  let context = Arc::clone(&state.repositories.get(&created.0.id).unwrap().context);
  (state, context)
}

/// One request of a client on `stream`: the answer of its id
/// A backend of the fixtures listening in `dir`, and a client connected to it
async fn serving(dir: &Path) -> BufReader<crate::socket::Stream> {
  let (_state, ctx) = context().await;
  let listener = Listener::bind(dir).await.unwrap();
  tokio::spawn(listener.serve(Host::of_build(ctx, BUILD.to_owned(), HostHooks::default())));
  BufReader::new(connect(dir).await.unwrap())
}

/// The answer of a ping of the id `id`
async fn ping<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(stream: &mut BufReader<S>, id: u32) -> Value {
  ask(stream, &json!({ "jsonrpc": "2.0", "id": id, "method": "ping", "params": { "channel": "ahp-root://" } })).await
}

async fn ask<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(stream: &mut BufReader<S>, request: &Value) -> Value {
  let mut line = request.to_string();
  line.push('\n');
  stream.get_mut().write_all(line.as_bytes()).await.unwrap();
  stream.get_mut().flush().await.unwrap();
  loop {
    let mut answer = String::new();
    assert!(stream.read_line(&mut answer).await.unwrap() > 0, "the backend closed the connection");
    let answer: Value = serde_json::from_str(&answer).unwrap();
    if answer["id"] == request["id"] {
      return answer;
    }
  }
}

/// A dev build and a release of one user have two keys: another executable, or another data directory, is another
/// backend; one build has one key, of 16 hex digits
#[test]
fn a_dev_build_beside_a_release_has_its_own_key() {
  let data = Path::new("/Users/u/Library/Application Support/org.develar.branch-deck");
  let release = key(data, Path::new("/Applications/Branch Deck.app/Contents/MacOS/branch-deck"));
  let dev = key(data, Path::new("/Users/u/projects/branch-deck/target/debug/branch-deck"));
  assert_ne!(release, dev);
  assert_ne!(
    release,
    key(Path::new("/tmp/other-data"), Path::new("/Applications/Branch Deck.app/Contents/MacOS/branch-deck"))
  );
  assert_eq!(release, key(data, Path::new("/Applications/Branch Deck.app/Contents/MacOS/branch-deck")));
  assert_eq!(release.len(), 16);
  assert!(release.chars().all(|digit| digit.is_ascii_hexdigit()));
  // FNV-1a of the bytes, stable across releases of Rust: a change of the hash is another backend for every user
  assert_eq!(key(Path::new("a"), Path::new("b")), "e5d29919042666b2");
}

/// A directory of the key of another mode: the backend opens no socket in it, the client does not connect to it
#[cfg(unix)]
#[tokio::test]
async fn a_directory_of_another_mode_is_refused_by_both_sides() {
  use std::os::unix::fs::PermissionsExt;
  let (_tmp, dir) = scratch();
  std::fs::create_dir_all(&dir).unwrap();
  std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
  let bound = Listener::bind(&dir).await;
  assert!(matches!(bound, Err(SocketError::Directory { .. })), "{bound:?}");
  assert!(!dir.join(crate::socket::SOCKET_FILE).exists(), "no socket in a refused directory");
  let connected = connect(&dir).await;
  assert!(matches!(connected, Err(SocketError::Directory { .. })), "{:?}", connected.err());
}

/// The check of the peer: a process of another uid is closed before a message is read, either way. A connection of
/// another user cannot be made without a second account (root), so the decision is tested on the uids it compares
#[test]
fn a_peer_of_another_user_is_refused() {
  assert!(same_user(501, 501));
  assert!(!same_user(502, 501), "another uid");
  assert!(!same_user(0, 501), "root is another user too");
}

/// A socket file that no backend listens on (its backend died): the backend that takes the lock removes it and listens
#[cfg(unix)]
#[tokio::test]
async fn a_stale_socket_is_removed_under_the_lock_and_the_backend_listens() {
  use std::os::unix::fs::DirBuilderExt;
  let (_tmp, dir) = scratch();
  std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
  let socket = dir.join(crate::socket::SOCKET_FILE);
  drop(std::os::unix::net::UnixListener::bind(&socket).unwrap());
  assert!(socket.exists(), "the socket file of a backend that went");
  assert!(std::os::unix::net::UnixStream::connect(&socket).is_err(), "no backend listens on it");

  let mut stream = serving(&dir).await;
  assert_eq!(ping(&mut stream, 1).await, json!({ "jsonrpc": "2.0", "id": 1, "result": {} }));
}

/// One backend per key: a second one cannot take the lock while the first holds it, and takes it once the first went
#[tokio::test]
async fn a_second_backend_cannot_take_the_lock_while_the_first_holds_it() {
  let (_tmp, dir) = scratch();
  let first = Listener::bind(&dir).await.unwrap();
  let second = Listener::bind_waiting(&dir, Duration::from_millis(300)).await;
  assert!(matches!(second, Err(SocketError::Held { .. })), "{second:?}");
  drop(first);
  Listener::bind_waiting(&dir, Duration::from_secs(2)).await.unwrap();
}

/// A second start of the app while its backend runs: it sends `showMain`, which reaches the app's hook (it builds and
/// shows the main window), and answers; the second process then exits
#[tokio::test]
async fn a_second_start_of_the_app_shows_the_main_window_of_the_backend() {
  let (_tmp, dir) = scratch();
  let (_state, ctx) = context().await;
  let shown = Arc::new(AtomicUsize::new(0));
  let hooks = HostHooks {
    show_main: Box::new({
      let shown = Arc::clone(&shown);
      move || {
        shown.fetch_add(1, Ordering::SeqCst);
      }
    }),
    ..HostHooks::default()
  };
  let listener = Listener::bind(&dir).await.unwrap();
  tokio::spawn(listener.serve(Host::of_build(ctx, BUILD.to_owned(), hooks)));
  show_main(&dir, BUILD).await.unwrap();
  assert_eq!(shown.load(Ordering::SeqCst), 1);
  let other = show_main(&dir, "0.0.0-another-build").await;
  assert!(matches!(other, Err(SocketError::Refused { .. })), "another build is refused: {other:?}");
  assert_eq!(shown.load(Ordering::SeqCst), 1);
}

/// A message goes round the socket: a ping of a client of the backend is answered on its connection
#[tokio::test]
async fn a_ping_goes_round_the_socket() {
  let (_tmp, dir) = scratch();
  let mut stream = serving(&dir).await;
  for id in 1..=2 {
    assert_eq!(ping(&mut stream, id).await, json!({ "jsonrpc": "2.0", "id": id, "result": {} }));
  }
}
