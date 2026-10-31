use crate::app::cli_tools::{CliToolsState, read_once};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// The tools are found while GitHub is asked for their releases, and a second read meanwhile gets the answer of the
/// first: the CLIs run once
#[tokio::test]
async fn the_tools_are_found_while_the_releases_are_asked_and_a_read_meanwhile_joins() {
  let state = CliToolsState::default();
  let finds = Arc::new(AtomicU32::new(0));
  let found = Arc::new(tokio::sync::Notify::new());
  let (answer, releases) = tokio::sync::oneshot::channel::<HashMap<_, _>>();
  let first = read_once(
    &state,
    async { releases.await.unwrap_or_default() },
    {
      let (finds, found) = (Arc::clone(&finds), Arc::clone(&found));
      move || {
        finds.fetch_add(1, Ordering::SeqCst);
        found.notify_one();
        Vec::new()
      }
    },
    || None,
  );
  let second = read_once(
    &state,
    async { HashMap::new() },
    {
      let finds = Arc::clone(&finds);
      move || {
        finds.fetch_add(1, Ordering::SeqCst);
        Vec::new()
      }
    },
    || None,
  );
  let releases_after_the_find = async {
    found.notified().await;
    answer.send(HashMap::new()).unwrap();
  };
  let (first, second, ()) = tokio::join!(first, second, releases_after_the_find);
  assert_eq!((first.unwrap(), second.unwrap()), (Vec::new(), Vec::new()));
  assert_eq!(finds.load(Ordering::SeqCst), 1, "the second read joined the first");
}
