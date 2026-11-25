use crate::registry::registry_test::{IdleHost, view_operations};
use crate::registry::{Registry, RegistryFiles};
use crate::registry_file::registry_file_test::{id, record, safe_push, session, sleeping_child};
use crate::registry_file::{ProcessIdentity, ProcessRegistry};
use crate::registry_watch::{Wake, Watches, seam, touch_stamp, woken};
use domain_types::error::HolderTool;
use domain_types::fixtures::land_intent;
use domain_types::operation::{Origin, RunningOperation};
use domain_types::patronus::PatronusPushMode;
use domain_types::session::ViewPatch;
use git_ops::lease_slot::STATE_DIR;
use notify::event::{EventKind, Flag, ModifyKind};
use pretty_assertions::assert_eq;
use repository_model::session::{OneClock, SessionHost, SessionKey, Sessions, Subscriber, SubscriberGone};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use sync_utils::clock::TestClock;
use sync_utils::dir_watch::{self, WatchConfig, WatchedDir};
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

/// How long the probe of a test of the events of the system waits for the event of its write before it writes again
const PROBE_EVERY: Duration = Duration::from_millis(100);

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
  let path = crate::journal::journal_path_in(&common_dir.join(STATE_DIR));
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  let mut journal = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
  writeln!(journal, "{{}}").unwrap();
}

/// The sessions of an idle host on a clock of its own
fn idle_sessions() -> Arc<Sessions> {
  Arc::new(Sessions::new(
    tokio::runtime::Handle::current(),
    Arc::new(IdleHost::default()),
    Arc::new(OneClock(Arc::new(TestClock::new()))),
  ))
}

/// The sessions of an idle host and a listener of the journals, whose calls the receiver hears
fn sessions_hearing_journals() -> (Arc<Sessions>, crate::registry::JournalListener, mpsc::UnboundedReceiver<()>) {
  let sessions = idle_sessions();
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

/// The Safe Push of auth that `bd` (a child that sleeps) runs, recorded in `file`; the child is killed as it drops, also
/// when the test fails
fn bd_runs_auth(file: &ProcessRegistry) -> test_utils::killed::Killed {
  let child = sleeping_child();
  file.insert_of_another_process(record("bd", &agent(&child), &safe_push("auth"), unix_now())).unwrap();
  child
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
  file: ProcessRegistry,
  child: test_utils::killed::Killed,
  host: Arc<IdleHost>,
  syncs: mpsc::UnboundedReceiver<()>,
  registry: Registry,
  key: SessionKey,
  patched: mpsc::UnboundedReceiver<()>,
  /// Last: the watch, the sessions and the child go before the directory
  directory: tempfile::TempDir,
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
    file,
    child,
    host,
    syncs,
    registry,
    key,
    patched,
    directory,
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
  drop(child);
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
  let dir = WatchedDir::new(&directory.path().join(STATE_DIR));

  file.insert_of_another_process(record("bd-docs", &agent(&child), &safe_push("docs"), unix_now())).unwrap();
  assert_eq!(view_operations(registry.sessions(), &key).await.len(), 1, "nothing reads without an event");
  woken(&wake, &dir, &Ok(modified(&directory.path().join(STATE_DIR).join(".operations.json.tmpX4"))));
  let missing = "the event read no registry";
  after_patches(&registry, &key, &mut patched, missing, |operations| operations.len() == 2).await;
  assert_eq!(wake.stamps(), 0, "the stamp is as it was: the look asked no sync");

  touch_stamp(directory.path());
  woken(&wake, &dir, &Ok(notify::Event::new(EventKind::Other).set_flag(Flag::Rescan)));
  tokio::time::timeout(DEADLINE, syncs.recv())
    .await
    .unwrap_or_else(|_elapsed| panic!("the lost event synced nothing for the stamp within {DEADLINE:?}"))
    .unwrap();
  drop(child);
}

/// The event path of the system: the watch tells that a record left the registry, with no look of the task. The test
/// begins once the stream told a write of its own (a probe, another record, written again until the window shows it):
/// the watcher starts on a thread of its own, and `FSEvents` drops the changes of a stream's first moments, which only
/// a look finds. It fails with "no watch event" where the watch runs behind the writes (`FSEvents` on a loaded
/// machine), which the looks of the app stand in for
#[tokio::test(flavor = "multi_thread")]
async fn the_watch_of_the_system_tells_that_a_record_left_the_registry_with_no_look() {
  let directory = tempfile::tempdir().unwrap();
  let file = ProcessRegistry::of(directory.path()).unflushed();
  let child = bd_runs_auth(&file);
  let (registry, key, mut patched) = watched(Arc::new(IdleHost::default()), directory.path(), EVENTS_ONLY).await;
  // the first read of the subscription, the one read without an event
  after_patches(&registry, &key, &mut patched, "the subscription read no registry", |operations| operations.len() == 1).await;
  let missing = "no watch event: the watch of the system runs behind the writes";
  let deadline = tokio::time::Instant::now() + DEADLINE;
  'live: for probe in 1_u32.. {
    let record = record(&format!("probe{probe}"), &agent(&child), &safe_push("probe"), unix_now());
    file.insert_of_another_process(record).unwrap();
    loop {
      if view_operations(registry.sessions(), &key).await.len() > 1 {
        break 'live;
      }
      if tokio::time::timeout(PROBE_EVERY, patched.recv()).await.is_err() {
        assert!(tokio::time::Instant::now() < deadline, "{missing} within {DEADLINE:?}");
        continue 'live;
      }
    }
  }

  file.remove(&id("bd")).unwrap();
  after_patches(&registry, &key, &mut patched, missing, |operations| {
    operations.iter().all(|operation| operation.id != id("bd"))
  })
  .await;
  drop(child);
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
  // no read is due by the age of the last one: the focus reads of itself
  seam::freeze();
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

  looks_past(&wake, 0).await;
  let reads = wake.reads();
  journal_line(directory.path());
  touch_stamp(directory.path());
  registry.reread(&key).await;
  tokio::time::timeout(DEADLINE, heard.recv())
    .await
    .unwrap_or_else(|_elapsed| panic!("the focus told no change of the journal within {DEADLINE:?}"))
    .unwrap();
  assert_eq!(wake.stamps(), 1, "the focus asked the syncs of the stamp");
  assert_eq!(wake.reads(), reads + 1, "the focus read the registry, whose stat is as it was");
  drop(child);
}

/// A change made after the watcher started and before the task of the watch took its first look is told at a look, with
/// no event: `FSEvents` may drop it (a write in the first moments of a stream), and a first look that already held it
/// would tell no change at any look after. On a runtime of one thread the task runs only after the test yields, so the
/// writes land in that window every time (the test of bd-host-heavy, with the stamp beside the journal)
#[tokio::test]
async fn a_change_between_the_start_of_the_watcher_and_the_first_look_is_seen() {
  let directory = tempfile::tempdir().unwrap();
  let (sessions, listener, mut heard) = sessions_hearing_journals();
  let watches = Watches::new(Some(listener), LOOKS_ONLY);
  let registry = ProcessRegistry::of(directory.path());
  let _watch = watches.watch(&sessions, &registry, &session("user"));

  // the journal of an operation of `bd` that ended right after the watcher started, and its stamp
  journal_line(directory.path());
  touch_stamp(directory.path());
  tokio::time::timeout(DEADLINE, heard.recv())
    .await
    .unwrap_or_else(|_elapsed| panic!("no look told the journal written before the first look within {DEADLINE:?}"))
    .unwrap();
  let wake = watches.wake_of(&registry).expect("the watch of the registry");
  assert_eq!(wake.stamps(), 1, "the look that told the journal asked the syncs of the stamp");
}

/// A repository that goes away while a window watches it (its git directory is deleted, as a test deletes its
/// repository): the looks of a `RECHECK` close its sessions, so the window's subscription ends, and ask no sync, though
/// the stamp is gone too; a sync of it would run git in a directory that is gone (on macOS a git whose directory went
/// away under it scans the parent for it: 7 s in a temporary directory of 122 000 entries, 18 s under load)
#[tokio::test(flavor = "multi_thread")]
async fn a_look_that_finds_the_repository_gone_closes_its_sessions_and_syncs_nothing() {
  let directory = tempfile::tempdir().unwrap();
  let common_dir = directory.path().join("repo/.git");
  std::fs::create_dir_all(&common_dir).unwrap();
  touch_stamp(&common_dir);
  let (registry, key, mut patched) = watched(Arc::new(IdleHost::default()), &common_dir, LOOKS_ONLY).await;
  let wake = registry.watch_wake(&key).await.expect("the window watches the registry");

  let taken = wake.looks_taken();
  std::fs::remove_dir_all(directory.path().join("repo")).unwrap();
  // the second look that ended since began once the first ended, 50 ms on, after the removal, and found the directory
  // missing; a `RECHECK` on, so does the next and closes the sessions. Had it begun before the removal ended, the span
  // counts from a later look and a real `RECHECK` closes them, within `DEADLINE` still
  looks_past(&wake, taken + 1).await;
  seam::advance(dir_watch::RECHECK);
  let ended = tokio::time::timeout(DEADLINE, async { while patched.recv().await.is_some() {} }).await;
  ended.unwrap_or_else(|_elapsed| panic!("no look closed the session of the repository that went away within {DEADLINE:?}"));
  assert_eq!(wake.stamps(), 0, "no look asked a sync of the repository that went away");
}

/// The count of the finished looks of `wake` once it passed `taken`
async fn looks_past(wake: &Wake, taken: u64) {
  let waited = tokio::time::timeout(DEADLINE, async {
    while wake.looks_taken() <= taken {
      tokio::task::yield_now().await;
    }
  })
  .await;
  waited.unwrap_or_else(|_elapsed| panic!("no look finished within {DEADLINE:?}"));
}

/// An event of `leases.json`, another file of the state directory of `common_dir`, woke `wake`, and its look finished
async fn look_of_another_file(wake: &Wake, common_dir: &Path) {
  let state_dir = common_dir.join(STATE_DIR);
  let taken = wake.looks_taken();
  woken(wake, &WatchedDir::new(&state_dir), &Ok(modified(&state_dir.join("leases.json"))));
  looks_past(wake, taken).await;
}

/// One look asked of the watch of the session of `key`, and taken
async fn look_once(registry: &Registry, key: &SessionKey, wake: &Wake) {
  let taken = wake.looks_taken();
  registry.look(key).await;
  looks_past(wake, taken).await;
}

/// The git directory missing at one look and back at the next (an external disk, a network share away for a moment)
/// keeps the sessions, and so do two looks in a row that find it missing within milliseconds (the burst of events of a
/// volume that unmounts, a focus right after an event); missing still at a look a `RECHECK` after the first that missed
/// it (the repository was deleted) closes them, and the window ends
#[tokio::test(flavor = "multi_thread")]
async fn the_sessions_close_once_the_repository_is_missing_for_a_recheck_not_at_one_look_or_a_burst() {
  // the looks are as far apart as the test moves the clock, also on a runner so loaded that they are seconds apart
  seam::freeze();
  let directory = tempfile::tempdir().unwrap();
  let (registry, key, mut patched) = watched(Arc::new(IdleHost::default()), directory.path(), GIVEN_EVENTS).await;
  let wake = registry.watch_wake(&key).await.expect("the window watches the registry");
  // the first look of the subscription
  looks_past(&wake, 0).await;

  seam::script_stats(&[std::io::ErrorKind::NotFound]);
  look_once(&registry, &key, &wake).await;
  assert!(registry.sessions().is_open(&key), "missing at one look");
  look_once(&registry, &key, &wake).await;
  assert!(registry.sessions().is_open(&key), "back at the next");

  seam::script_stats(&[std::io::ErrorKind::NotFound, std::io::ErrorKind::NotFound, std::io::ErrorKind::NotFound]);
  look_once(&registry, &key, &wake).await;
  look_once(&registry, &key, &wake).await;
  assert!(registry.sessions().is_open(&key), "missing at two looks with no time between them");
  seam::advance(dir_watch::RECHECK);
  look_once(&registry, &key, &wake).await;
  assert!(!registry.sessions().is_open(&key), "missing still a RECHECK after the first look that missed it");
  let ended = tokio::time::timeout(DEADLINE, async { while patched.recv().await.is_some() {} }).await;
  ended.unwrap_or_else(|_elapsed| panic!("the window of the closed session did not end within {DEADLINE:?}"));
}

/// Another error of the stat of the git directory (`EACCES`, `EIO` of a volume that is away) keeps the sessions at every
/// look, which reads nothing then, and the next look that can tell reads the registry
#[tokio::test(flavor = "multi_thread")]
async fn a_stat_error_other_than_not_found_keeps_the_sessions_and_the_next_look_asks_again() {
  let directory = tempfile::tempdir().unwrap();
  let file = ProcessRegistry::of(directory.path()).unflushed();
  let (registry, key, mut patched) = watched(Arc::new(IdleHost::default()), directory.path(), GIVEN_EVENTS).await;
  let wake = registry.watch_wake(&key).await.expect("the window watches the registry");
  looks_past(&wake, 0).await;

  let child = bd_runs_auth(&file);
  seam::script_stats(&[std::io::ErrorKind::PermissionDenied, std::io::ErrorKind::Other, std::io::ErrorKind::PermissionDenied]);
  for _ in 0..3 {
    look_once(&registry, &key, &wake).await;
    assert!(registry.sessions().is_open(&key), "an error of the stat closes nothing");
  }
  assert_eq!(view_operations(registry.sessions(), &key).await, Vec::new(), "a look that can't tell reads nothing");
  look_once(&registry, &key, &wake).await;
  after_patches(&registry, &key, &mut patched, "the look after the errors read no registry", |operations| {
    operations.len() == 1
  })
  .await;
  drop(child);
}

/// A look stats the registry and reads it only when its stat changed since the last look (`Fingerprints`), when a
/// session joined the watch, or once its last read is a `RECHECK` old: a look of the file as it was, and one of an event
/// of another file of the directory (the leases, a lock), read nothing. A write renamed over the file is a change though
/// its size and its time are as they were (its file id is new); a session that joins reads the file the watch read
/// before; and the read a `RECHECK` on finds that the process of a record ended, though the file is as it was
#[tokio::test(flavor = "multi_thread")]
async fn a_look_reads_the_registry_only_once_it_changed_a_session_joined_or_its_last_read_is_a_recheck_old() {
  // the age of the last read is what the test moves the clock by, however slow the runner is
  seam::freeze();
  let WhileBdRuns {
    directory,
    child,
    registry,
    key,
    mut patched,
    ..
  } = watched_while_bd_runs(GIVEN_EVENTS).await;
  let wake = registry.watch_wake(&key).await.expect("the window watches the registry");
  looks_past(&wake, 0).await;
  assert_eq!(wake.reads(), 1, "the subscription read the registry");

  look_once(&registry, &key, &wake).await;
  look_of_another_file(&wake, directory.path()).await;
  assert_eq!(wake.reads(), 1, "a look of the registry as it was reads nothing, at an event of another file too");

  // the record of another id, of the same size, renamed over the file with the time of the file before
  let state_dir = directory.path().join(STATE_DIR);
  let path = state_dir.join("operations.json");
  let before = std::fs::read_to_string(&path).unwrap();
  let written = before.replace(r#""id": "bd""#, r#""id": "bx""#);
  assert_ne!(written, before, "the record of the process of the CLI in {before}");
  let temporary = state_dir.join(".operations.json.tmp-test");
  std::fs::write(&temporary, &written).unwrap();
  let time = std::fs::metadata(&path).unwrap().modified().unwrap();
  std::fs::File::options().write(true).open(&temporary).unwrap().set_modified(time).unwrap();
  sync_utils::fs_retry::rename(&temporary, &path).unwrap();
  look_once(&registry, &key, &wake).await;
  assert_eq!(wake.reads(), 2, "a rename over the registry is a change, whatever its size and its time");
  let missing = "the look of the renamed registry read no record";
  after_patches(&registry, &key, &mut patched, missing, |operations| {
    operations.iter().any(|operation| operation.id == id("bx"))
  })
  .await;

  // a window of another worktree of the prefix joins the watch: its session shows the record from its first read
  let joined = SessionKey {
    worktree: "/another/worktree".to_string(),
    ..key.clone()
  };
  let (patches, mut joined_patched) = mpsc::unbounded_channel();
  let taken = wake.looks_taken();
  registry.subscribe(&joined, Box::new(Window(patches)), None).await.unwrap();
  let missing = "the session that joined the watch read no registry";
  after_patches(&registry, &joined, &mut joined_patched, missing, |operations| operations.len() == 1).await;
  looks_past(&wake, taken).await;
  assert_eq!(wake.reads(), 3, "the session that joined read the registry as it was");

  // the process of the record ended and left the file as it was: a read a `RECHECK` after the last one tells it
  drop(child);
  look_once(&registry, &key, &wake).await;
  assert_eq!(wake.reads(), 3, "a look of the registry as it was reads nothing within a RECHECK of the last read");
  seam::advance(dir_watch::RECHECK);
  look_once(&registry, &key, &wake).await;
  assert_eq!(wake.reads(), 4, "a look a RECHECK after the last read reads the registry");
  let missing = "the read a RECHECK on kept the record of a process that ended";
  after_patches(&registry, &key, &mut patched, missing, <[RunningOperation]>::is_empty).await;
}

/// A change no stat tells (the process of a record ended, its file as it was) shows at most a `RECHECK` after the last
/// read, also when the look of an event came between and read nothing: the wait for the next look ends when the read is
/// due, not a `RECHECK` after the last look (a read at 0, an event at 1.8 s, the next look at 3.8 s). The clock of the
/// runtime is paused, so the time the record left is the time of the timers, however slow the runner is
#[tokio::test(start_paused = true)]
async fn a_change_no_stat_tells_shows_a_recheck_after_the_last_read_though_a_look_of_an_event_came_between() {
  let config = WatchConfig {
    recheck: Some(dir_watch::RECHECK),
    events: false,
  };
  let WhileBdRuns {
    directory,
    child,
    registry,
    key,
    mut patched,
    ..
  } = watched_while_bd_runs(config).await;
  let wake = registry.watch_wake(&key).await.expect("the window watches the registry");
  looks_past(&wake, 0).await;
  let read = tokio::time::Instant::now();
  assert_eq!(wake.reads(), 1, "the subscription read the registry");

  // the process of the record ended, and an event of another file of the directory came before the read is due
  drop(child);
  tokio::time::advance(dir_watch::RECHECK * 9 / 10).await;
  look_of_another_file(&wake, directory.path()).await;
  assert_eq!(wake.reads(), 1, "the look of the event, within a RECHECK of the read, read nothing");

  let missing = "no read dropped the record of the process that ended";
  after_patches(&registry, &key, &mut patched, missing, <[RunningOperation]>::is_empty).await;
  let left = read.elapsed();
  assert!(left <= dir_watch::RECHECK, "the record of the process that ended left {left:?} after the last read");
}

/// A read of the registry that failed is taken again at the next look, though the stat it followed is the last one:
/// the record written before it shows then, with no change of the file and no read due by the age of the last one
#[tokio::test(flavor = "multi_thread")]
async fn a_read_of_the_registry_that_failed_is_taken_again_at_the_next_look() {
  seam::freeze();
  let WhileBdRuns {
    file,
    child,
    registry,
    key,
    mut patched,
    directory: _directory,
    ..
  } = watched_while_bd_runs(GIVEN_EVENTS).await;
  let wake = registry.watch_wake(&key).await.expect("the window watches the registry");
  looks_past(&wake, 0).await;

  file.insert_of_another_process(record("bd-docs", &agent(&child), &safe_push("docs"), unix_now())).unwrap();
  seam::script_reads(&[std::io::ErrorKind::PermissionDenied]);
  look_once(&registry, &key, &wake).await;
  assert_eq!((wake.reads(), view_operations(registry.sessions(), &key).await.len()), (2, 1), "the read failed");

  look_once(&registry, &key, &wake).await;
  assert_eq!(wake.reads(), 3, "the look after the failed read read the registry again");
  let missing = "the look after the failed read showed nothing of it";
  after_patches(&registry, &key, &mut patched, missing, |operations| operations.len() == 2).await;
}

/// A read that finds the operations of another process before the session of the window that watches is open (the
/// watch starts before the subscription opens the session) gives them to no session and records none as given, so the
/// read the subscription asks gives them once the session is open, though the file is as the first read found it
#[tokio::test(flavor = "multi_thread")]
async fn a_read_before_the_session_opened_records_nothing_as_given_so_the_subscription_shows_it() {
  seam::freeze();
  let directory = tempfile::tempdir().unwrap();
  let registry = ProcessRegistry::of(directory.path()).unflushed();
  let child = bd_runs_auth(&registry);
  let sessions = idle_sessions();
  let watches = Watches::new(None, GIVEN_EVENTS);
  let key = session("user");
  let _watch = watches.watch(&sessions, &registry, &key);
  let wake = watches.wake_of(&registry).expect("the watch of the registry");
  wake.look();
  looks_past(&wake, 0).await;
  assert_eq!(wake.reads(), 1, "the look before the session opened read the registry");

  let (patches, mut patched) = mpsc::unbounded_channel();
  sessions.subscribe_from(&key, Box::new(Window(patches)), None).await.unwrap();
  watches.read(&registry);
  let shown = tokio::time::timeout(DEADLINE, async {
    loop {
      if view_operations(&sessions, &key).await.len() == 1 {
        return;
      }
      patched.recv().await.unwrap();
    }
  })
  .await;
  shown.unwrap_or_else(|_elapsed| panic!("the read of the subscription showed no operation of the CLI within {DEADLINE:?}"));
  drop(child);
}
