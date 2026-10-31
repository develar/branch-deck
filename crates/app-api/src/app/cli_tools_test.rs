use crate::app::cli_tools::{CliToolsState, read_once};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// What the reads of a test did, in order
type Events = Arc<Mutex<Vec<&'static str>>>;

/// A find of the tools that tells `events` it ran as `name`, and `found` once it did
fn find_as(events: &Events, name: &'static str, found: Option<Arc<tokio::sync::Notify>>) -> impl FnOnce() -> Vec<cli_tools::status::FoundTool> + Send + 'static {
  let events = Arc::clone(events);
  move || {
    events.lock().unwrap().push(name);
    if let Some(found) = found {
      found.notify_one();
    }
    Vec::new()
  }
}

/// The tools are found while GitHub is asked for their releases. Two windows that ask while that read runs (it may
/// have started before what they ask about: an action ended, an install in a terminal) share one read after it: the
/// CLIs run twice, never three times, and the second time after the first read ended
#[tokio::test]
async fn the_tools_are_found_while_the_releases_are_asked_and_windows_that_ask_meanwhile_share_the_read_after_it() {
  let state = CliToolsState::default();
  let events: Events = Arc::default();
  let found = Arc::new(tokio::sync::Notify::new());
  let (answer, releases) = tokio::sync::oneshot::channel::<HashMap<_, _>>();
  let first_releases = {
    let events = Arc::clone(&events);
    async move {
      let releases = releases.await.unwrap_or_default();
      events.lock().unwrap().push("the first read ended");
      releases
    }
  };
  let first = read_once(&state, first_releases, find_as(&events, "first", Some(Arc::clone(&found))), || None);
  let second = read_once(&state, async { HashMap::new() }, find_as(&events, "second", None), || None);
  let third = read_once(&state, async { HashMap::new() }, find_as(&events, "third", None), || None);
  // the releases of the first read answer once it found the tools, and once both windows asked
  let releases_after_the_find = async {
    found.notified().await;
    answer.send(HashMap::new()).unwrap();
  };
  let (first, second, third, ()) = tokio::join!(first, second, third, releases_after_the_find);
  assert_eq!((first.unwrap(), second.unwrap(), third.unwrap()), (Vec::new(), Vec::new(), Vec::new()));
  // either window leads the read after the first, the other shares it
  let events = events.lock().unwrap().clone();
  assert_eq!(events[..2], ["first", "the first read ended"], "{events:?}");
  assert!(matches!(events[2..], ["second" | "third"]), "one read after the first, shared: {events:?}");
}
