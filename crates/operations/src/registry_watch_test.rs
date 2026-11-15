use crate::registry::registry_test::{IdleHost, view_operations};
use crate::registry::{Registry, RegistryFiles};
use crate::registry_file::registry_file_test::{id, record, safe_push, session, sleeping_child};
use crate::registry_file::{ProcessIdentity, ProcessRegistry};
use crate::registry_watch::{Wake, Watches, touch_stamp, woken};
use domain_types::error::HolderTool;
use domain_types::fixtures::land_intent;
use domain_types::operation::{Origin, RunningOperation};
use domain_types::patronus::PatronusPushMode;
use domain_types::session::ViewPatch;
use notify::event::{EventKind, Flag, ModifyKind};
use pretty_assertions::assert_eq;
use repository_model::session::{OneClock, SessionHost, SessionKey, Sessions, Subscriber, SubscriberGone};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use sync_utils::clock::TestClock;
use sync_utils::dir_watch::{WatchConfig, WatchedDir};
use sync_utils::process_scan::{host_name, process_started};
use sync_utils::timestamp::unix_now;
use tokio::sync::mpsc;

/// The end of a wait for what the watch reads: a run that reads it never reaches it, and one that reads nothing fails
/// there, naming what it waited for, instead of at the timeout of the runner
const DEADLINE: Duration = Duration::from_secs(30);

/// A test of the guarantee: the watch tells no event (as fseventsd at 100 % CPU told none), and the task looks this often
const LOOKS_ONLY: WatchConfig = WatchConfig {
  recheck: Some(Duration::from_millis(50)),
  events: false,
};

/// A test of the events of the system: no look without one
const EVENTS_ONLY: WatchConfig = WatchConfig { recheck: None, events: true };

/// A test of the events it gives the task itself: neither an event of the system nor a look
const GIVEN_EVENTS: WatchConfig = WatchConfig { recheck: None, events: false };

/// A window: each patch it gets wakes the test
struct Window(mpsc::UnboundedSender<()>);

impl Subscriber for Window {
  fn send(&self, _patch: &ViewPatch) -> Result<(), SubscriberGone> {
    self.0.send(()).map_err(|_gone| SubscriberGone)
  }
}

/// The registry of the sessions of `host` whose repository is `common_dir`, its watches learning of a change as `config`
/// says, the key of its session (prefix `user`) and the patches of a window subscribed to it through the registry
async fn watched(host: Arc<dyn SessionHost>, common_dir: &Path, config: WatchConfig) -> (Registry, SessionKey, mpsc::UnboundedReceiver<()>) {
  let sessions = Arc::new(Sessions::new(tokio::runtime::Handle::current(), host, Arc::new(OneClock(Arc::new(TestClock::new())))));
  let dir = common_dir.to_path_buf();
  let registry = Registry::watched_as(sessions, RegistryFiles::new(HolderTool::App, move |_key| Some(dir.clone())).unflushed(), config);
  let key = session("user");
  let (patches, patched) = mpsc::unbounded_channel();
  registry.subscribe(&key, Box::new(Window(patches)), None).await.unwrap();
  (registry, key, patched)
}

/// The journal of the registry of `common_dir` with one more line, as an operation of `bd` that ended writes it
fn journal_line(common_dir: &Path) {
  use std::io::Write as _;
  let path = crate::journal::journal_path_in(&common_dir.join("branch-deck"));
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  let mut journal = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
  writeln!(journal, "{{}}").unwrap();
}

/// The sessions of an idle host and a listener of the journals, whose calls the receiver hears
fn sessions_hearing_journals() -> (Arc<Sessions>, crate::registry::JournalListener, mpsc::UnboundedReceiver<()>) {
  let sessions = Arc::new(Sessions::new(
    tokio::runtime::Handle::current(),
    Arc::new(IdleHost::default()),
    Arc::new(OneClock(Arc::new(TestClock::new()))),
  ));
  let (told, heard) = mpsc::unbounded_channel();
  let listener: crate::registry::JournalListener = Arc::new(move |_keys: &[SessionKey]| told.send(()).unwrap());
  (sessions, listener, heard)
}

/// The operations of the view once `done` holds of them, read again after each patch; past `DEADLINE` the test fails
/// with `missing`, what read nothing
async fn after_patches(
  registry: &Registry,
  key: &SessionKey,
  patched: &mut mpsc::UnboundedReceiver<()>,
  missing: &str,
  done: impl Fn(&[RunningOperation]) -> bool,
) -> Vec<RunningOperation> {
  let waited = tokio::time::timeout(DEADLINE, async {
    loop {
      let operations = view_operations(registry.sessions(), key).await;
      if done(&operations) {
        return operations;
      }
      patched.recv().await.unwrap();
    }
  })
  .await;
  waited.unwrap_or_else(|_elapsed| panic!("{missing} within {DEADLINE:?}"))
}

/// A process of this host that runs until it is killed, as the records name it
fn agent(child: &std::process::Child) -> ProcessIdentity {
  ProcessIdentity {
    pid: child.id(),
    started: process_started(child.id()),
    host: host_name().to_string(),
  }
}

/// The Safe Push of auth that `bd` (a child that sleeps) runs, recorded in `file`; the child goes with `end`
fn bd_runs_auth(file: &ProcessRegistry) -> std::process::Child {
  let child = sleeping_child();
  file.insert_of_another_process(record("bd", &agent(&child), &safe_push("auth"), unix_now())).unwrap();
  child
}

fn end(mut child: std::process::Child) {
  child.kill().unwrap();
  child.wait().unwrap();
}

/// A host whose runs of a sync the test hears
fn heard_host() -> (Arc<IdleHost>, mpsc::UnboundedReceiver<()>) {
  let (synced, syncs) = mpsc::unbounded_channel();
  let host = IdleHost {
    runs: Some(synced),
    ..IdleHost::default()
  };
  (Arc::new(host), syncs)
}

/// A window that watches a repository where `bd` runs the Safe Push of auth, with a host whose syncs the test hears
struct WhileBdRuns {
  directory: tempfile::TempDir,
  file: ProcessRegistry,
  child: std::process::Child,
  host: Arc<IdleHost>,
  syncs: mpsc::UnboundedReceiver<()>,
  registry: Registry,
  key: SessionKey,
  patched: mpsc::UnboundedReceiver<()>,
}

/// `WhileBdRuns`, its watch learning of a change as `config` says, once the first look of the subscription showed the
/// Safe Push of auth
async fn watched_while_bd_runs(config: WatchConfig) -> WhileBdRuns {
  let directory = tempfile::tempdir().unwrap();
  let file = ProcessRegistry::of(directory.path()).unflushed();
  let child = bd_runs_auth(&file);
  let (host, syncs) = heard_host();
  let (registry, key, mut patched) = watched(Arc::clone(&host) as Arc<dyn SessionHost>, directory.path(), config).await;
  after_patches(&registry, &key, &mut patched, "the subscription read no registry", |operations| operations.len() == 1).await;
  WhileBdRuns {
    directory,
    file,
    child,
    host,
    syncs,
    registry,
    key,
    patched,
  }
}

/// An event of the system that names `path`
fn modified(path: &Path) -> notify::Event {
  notify::Event::new(EventKind::Modify(ModifyKind::Any)).add_path(path.to_path_buf())
}

/// While a window shows a session, the operations registry of its repository is watched: the records of the other
/// processes of its branch prefix are its operations of other processes, of any worktree, the ones of another prefix and
/// of this process are not; a record that leaves the file leaves the view. The guarantee: the watch tells no event here,
/// and the look of the task every `RECHECK` reads each change
#[tokio::test(flavor = "multi_thread")]
async fn a_window_shows_the_operations_another_process_runs_under_its_prefix_at_the_next_look_with_no_event_of_the_watch() {
  let directory = tempfile::tempdir().unwrap();
  let file = ProcessRegistry::of(directory.path()).unflushed();
  let mut child = sleeping_child();
  let agent = agent(&child);
  let mut elsewhere = record("bd-elsewhere", &agent, &safe_push("api"), unix_now());
  elsewhere.session.worktree = "/another/worktree".to_string();
  file.insert_of_another_process(elsewhere).unwrap();
  let mut other_prefix = record("bd-other", &agent, &safe_push("docs"), unix_now());
  other_prefix.session = session("other");
  file.insert_of_another_process(other_prefix).unwrap();

  // the first read of the subscription
  let (registry, key, mut patched) = watched(Arc::new(IdleHost::default()), directory.path(), LOOKS_ONLY).await;
  let missing = "no look read the registry";
  let shown = after_patches(&registry, &key, &mut patched, missing, |operations| operations.len() == 1).await;
  assert_eq!(
    shown.iter().map(|operation| (operation.id.0.as_str(), operation.origin.clone())).collect::<Vec<_>>(),
    [(
      "bd-elsewhere",
      Origin::OtherProcess {
        tool: HolderTool::Bd,
        pid: child.id()
      }
    )]
  );

  // a change of the file: a record of this process shows only as the session's own operation would
  let own = file.admit(&id("own"), HolderTool::App, &key, &safe_push("login")).unwrap();
  file.insert_of_another_process(record("bd", &agent, &safe_push("auth"), unix_now())).unwrap();
  let shown = after_patches(&registry, &key, &mut patched, missing, |operations| operations.len() == 2).await;
  let auth = shown.iter().find(|operation| operation.id == id("bd")).unwrap();
  assert_eq!(auth.intent, land_intent("auth", PatronusPushMode::RebaseAutosquash));
  drop(own);

  file.remove(&id("bd")).unwrap();
  file.remove(&id("bd-elsewhere")).unwrap();
  after_patches(&registry, &key, &mut patched, missing, <[RunningOperation]>::is_empty).await;
  child.kill().unwrap();
  child.wait().unwrap();
}

/// `bd` touches the stamp of the repository after a mutation: the session a window shows syncs, without a focus. The
/// guarantee: the watch tells no event here, and the look of the task syncs once the stat of the stamp changed, never
/// at a look that finds it as it was
#[tokio::test(flavor = "multi_thread")]
async fn a_stamp_of_bd_syncs_the_session_a_window_shows_at_the_next_look_with_no_event_of_the_watch() {
  let WhileBdRuns {
    directory,
    file,
    child,
    host,
    mut syncs,
    registry,
    key,
    mut patched,
  } = watched_while_bd_runs(LOOKS_ONLY).await;
  let wake = registry.watch_wake(&key).await.expect("the window watches the registry");
  let missing = "no look read the registry";
  // `brd sync` marked the worktree synced, which the session read as never synced when it opened: a sync for a stamp
  // runs from now. A look reads that the record left, finds the stamp as it was and syncs nothing
  host.marked.store(true, std::sync::atomic::Ordering::SeqCst);
  file.remove(&id("bd")).unwrap();
  after_patches(&registry, &key, &mut patched, missing, <[RunningOperation]>::is_empty).await;
  assert_eq!(wake.stamps(), 0, "no look asked a sync before the stamp");
  assert!(syncs.try_recv().is_err(), "no sync before the stamp");

  // then `brd` touched the stamp: the session syncs of itself
  touch_stamp(directory.path());
  tokio::time::timeout(DEADLINE, syncs.recv())
    .await
    .unwrap_or_else(|_elapsed| panic!("no look synced for the stamp within {DEADLINE:?}"))
    .unwrap();
  assert_eq!(wake.stamps(), 1, "one look asked the syncs of the stamp");
  end(child);
}

/// The event path, with the events the test gives: an event of any name of the directory (the temporary file of a
/// rename over the registry) reads the registry at once, and a lost event (`MUST_SCAN_SUBDIRS`) syncs for a stamp that
/// changed, before any look; without an event nothing reads
#[tokio::test(flavor = "multi_thread")]
async fn an_event_of_any_name_of_the_directory_reads_the_registry_and_syncs_for_the_stamp_before_the_next_look() {
  // the first look of the subscription showed the Safe Push of auth: no look comes after it without an event
  let WhileBdRuns {
    directory,
    file,
    child,
    host,
    mut syncs,
    registry,
    key,
    mut patched,
  } = watched_while_bd_runs(GIVEN_EVENTS).await;
  // `brd sync` marked the worktree synced after the session opened: any sync for a stamp runs from now
  host.marked.store(true, std::sync::atomic::Ordering::SeqCst);
  let wake = registry.watch_wake(&key).await.expect("the window watches the registry");
  let dir = WatchedDir::new(&directory.path().join("branch-deck"));

  file.insert_of_another_process(record("bd-docs", &agent(&child), &safe_push("docs"), unix_now())).unwrap();
  assert_eq!(view_operations(registry.sessions(), &key).await.len(), 1, "nothing reads without an event");
  woken(&wake, &dir, &Ok(modified(&directory.path().join("branch-deck/.operations.json.tmpX4"))));
  let missing = "the event read no registry";
  after_patches(&registry, &key, &mut patched, missing, |operations| operations.len() == 2).await;
  assert_eq!(wake.stamps(), 0, "the stamp is as it was: the look asked no sync");

  touch_stamp(directory.path());
  woken(&wake, &dir, &Ok(notify::Event::new(EventKind::Other).set_flag(Flag::Rescan)));
  tokio::time::timeout(DEADLINE, syncs.recv())
    .await
    .unwrap_or_else(|_elapsed| panic!("the lost event synced nothing for the stamp within {DEADLINE:?}"))
    .unwrap();
  end(child);
}

/// The event path of the system: the watch tells that a record left the registry, with no look of the task. It fails
/// with "no watch event" where the watch runs behind the writes (`FSEvents` on a loaded machine), which the looks of the
/// app stand in for
#[tokio::test(flavor = "multi_thread")]
async fn the_watch_of_the_system_tells_that_a_record_left_the_registry_with_no_look() {
  let directory = tempfile::tempdir().unwrap();
  let file = ProcessRegistry::of(directory.path()).unflushed();
  let child = bd_runs_auth(&file);
  let (registry, key, mut patched) = watched(Arc::new(IdleHost::default()), directory.path(), EVENTS_ONLY).await;
  // the first read of the subscription, the one read without an event
  after_patches(&registry, &key, &mut patched, "the subscription read no registry", |operations| operations.len() == 1).await;

  file.remove(&id("bd")).unwrap();
  after_patches(
    &registry,
    &key,
    &mut patched,
    "no watch event: the watch of the system runs behind the writes",
    <[RunningOperation]>::is_empty,
  )
  .await;
  end(child);
}

/// Every event that may be a change of the registry, the stamp or the journal asks the task for a look: an event of one
/// of them or of another name of the directory (a temporary file renamed over one), of the directory itself, a lost
/// event (`FSEvents` and inotify tell an overflow of their queue as one event that names no file) and an error; an event
/// below the directory (the notes of a lease) does not
#[test]
fn an_event_that_may_be_a_change_of_the_directory_asks_for_a_look() {
  let directory = tempfile::tempdir().unwrap();
  let dir = WatchedDir::new(directory.path());
  let looks = |event: notify::Result<notify::Event>| {
    let wake = Wake::default();
    woken(&wake, &dir, &event);
    wake.looks()
  };
  for event in [
    Ok(modified(&directory.path().join("stamp"))),
    Ok(modified(&directory.path().join(".journal.jsonl.tmp"))),
    Ok(modified(directory.path())),
    Ok(notify::Event::new(EventKind::Other).set_flag(Flag::Rescan)),
    Err(notify::Error::generic("the stream failed")),
  ] {
    assert!(looks(event), "a look");
  }
  assert!(!looks(Ok(modified(&directory.path().join("notes/lease/gate.log")))), "below the directory");
}

/// A window that gains the focus looks at once at the registry, the stamp and the journal: the journal of an operation of
/// `bd` that ended tells Activity, and the stamp syncs, though no event and no look came (until 2026-11-15 a focus read
/// the registry alone)
#[tokio::test(flavor = "multi_thread")]
async fn a_focus_looks_at_the_journal_and_the_stamp_too() {
  let directory = tempfile::tempdir().unwrap();
  let (sessions, listener, mut heard) = sessions_hearing_journals();
  let dir = directory.path().to_path_buf();
  let files = RegistryFiles::new(HolderTool::App, move |_key| Some(dir.clone())).unflushed();
  let registry = Registry::watched_as(sessions, files.on_journal(move |keys| listener(keys)), GIVEN_EVENTS);
  let key = session("user");
  let child = bd_runs_auth(&ProcessRegistry::of(directory.path()).unflushed());
  let (patches, mut patched) = mpsc::unbounded_channel();
  registry.subscribe(&key, Box::new(Window(patches)), None).await.unwrap();
  let wake = registry.watch_wake(&key).await.expect("the window watches the registry");
  // the first look of the subscription, whose stats came before its patch
  after_patches(&registry, &key, &mut patched, "the subscription read no registry", |operations| operations.len() == 1).await;

  journal_line(directory.path());
  touch_stamp(directory.path());
  registry.reread(&key).await;
  tokio::time::timeout(DEADLINE, heard.recv())
    .await
    .unwrap_or_else(|_elapsed| panic!("the focus told no change of the journal within {DEADLINE:?}"))
    .unwrap();
  assert_eq!(wake.stamps(), 1, "the focus asked the syncs of the stamp");
  end(child);
}

/// A change right after the watch started is a change at the first look: the first look is taken before the watcher,
/// as `FSEvents` drops the changes of the first moments of a stream. The task of the watch runs only once the test
/// awaits (one thread), so a first look of the task would find the line already there
#[tokio::test]
async fn a_change_right_after_the_watch_started_is_a_change_at_the_first_look() {
  let directory = tempfile::tempdir().unwrap();
  let (sessions, listener, mut heard) = sessions_hearing_journals();
  let watches = Watches::new(Some(listener), GIVEN_EVENTS);
  let registry = ProcessRegistry::of(directory.path());
  let _watch = watches.watch(&sessions, &registry, &session("user"));

  journal_line(directory.path());
  watches.look(&registry);
  tokio::time::timeout(DEADLINE, heard.recv())
    .await
    .unwrap_or_else(|_elapsed| panic!("the first look found no change of the journal within {DEADLINE:?}"))
    .unwrap();
}
