use crate::app::cli_tools::{CliToolsState, RunningGuard, read_once};
use crate::events::CLI_TOOLS_RUNNING_EVENT;
use crate::transport::{AppEvents, WindowId};
use domain_types::cli_tools::{CliTool, CliToolAction, CliToolRunning, CliToolsRunning};
use domain_types::error::ErrorCode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
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

/// The snapshots of `cli-tools-running` every window heard, in order
#[derive(Default)]
struct Told(Mutex<Vec<CliToolsRunning>>);

impl AppEvents for Told {
  fn emit(&self, event: &str, payload: Value) {
    assert_eq!(event, CLI_TOOLS_RUNNING_EVENT);
    self.0.lock().unwrap().push(serde_json::from_value(payload).unwrap());
  }

  fn emit_to(&self, window: &WindowId, event: &str, _payload: Value) {
    panic!("{event} went to {window} alone");
  }
}

impl Told {
  /// What each snapshot heard says: its version and the line of each action
  fn heard(&self) -> Vec<(u32, Vec<(CliTool, String)>)> {
    let snapshots = self.0.lock().unwrap();
    snapshots
      .iter()
      .map(|snapshot| (snapshot.version, snapshot.actions.iter().map(|it| (it.tool, it.line.clone())).collect()))
      .collect()
  }
}

fn start(state: &CliToolsState, events: &Arc<dyn AppEvents>, tool: CliTool) -> Result<RunningGuard, ErrorCode> {
  RunningGuard::start(state, events, tool, CliToolAction::Update, Arc::new(AtomicBool::new(false))).map_err(|e| e.code)
}

/// Every window hears an action start, each line of it and its end, the whole of the actions each time at a newer
/// version; a window that opens meanwhile reads the same (`cli_tools_running`). A second action of the tool is refused
/// while the first runs, as the window that asks did not hear of it yet, and runs once it ended
#[test]
fn every_window_hears_the_action_of_a_tool_start_write_its_lines_and_end_and_a_second_one_is_refused_meanwhile() {
  let state = CliToolsState::default();
  let told = Arc::new(Told::default());
  let events: Arc<dyn AppEvents> = Arc::<Told>::clone(&told);
  let guard = start(&state, &events, CliTool::Youtrack).unwrap();
  guard.progress("==> Upgrading youtrack", None);
  assert_eq!(
    state.running_now(),
    CliToolsRunning {
      version: 2,
      actions: vec![CliToolRunning {
        tool: CliTool::Youtrack,
        action: CliToolAction::Update,
        line: "==> Upgrading youtrack".to_owned(),
        url: None,
      }],
    }
  );
  assert_eq!(start(&state, &events, CliTool::Youtrack).err(), Some(ErrorCode::CliToolBusy));
  let other = start(&state, &events, CliTool::Teamcity).unwrap();
  drop(guard);
  drop(other);
  assert_eq!(
    told.heard(),
    vec![
      (1, vec![(CliTool::Youtrack, String::new())]),
      (2, vec![(CliTool::Youtrack, "==> Upgrading youtrack".to_owned())]),
      (3, vec![(CliTool::Teamcity, String::new()), (CliTool::Youtrack, "==> Upgrading youtrack".to_owned())]),
      (4, vec![(CliTool::Teamcity, String::new())]),
      (5, vec![]),
    ]
  );
  // the tool is free once its action ended
  drop(start(&state, &events, CliTool::Youtrack).unwrap());
}

/// An action that panics ends in every window too: the tool is not left busy until the app restarts
#[test]
fn an_action_that_panics_frees_its_tool_in_every_window() {
  let state = CliToolsState::default();
  let told = Arc::new(Told::default());
  let events: Arc<dyn AppEvents> = Arc::<Told>::clone(&told);
  let guard = start(&state, &events, CliTool::Teamcity).unwrap();
  let panicked = std::thread::spawn(move || {
    let _guard = guard;
    panic!("the action failed hard");
  })
  .join();
  assert!(panicked.is_err());
  assert_eq!(state.running_now().actions, vec![]);
  assert_eq!(told.heard().last(), Some(&(2, vec![])));
  drop(start(&state, &events, CliTool::Teamcity).unwrap());
}
