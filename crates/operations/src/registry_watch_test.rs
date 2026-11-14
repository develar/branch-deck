use crate::registry::registry_test::{IdleHost, view_operations};
use crate::registry::{Registry, RegistryFiles};
use crate::registry_file::registry_file_test::{id, record, safe_push, session, sleeping_child};
use crate::registry_file::{ProcessIdentity, ProcessRegistry};
use crate::registry_watch::{Wake, touch_stamp, woken};
use domain_types::error::HolderTool;
use domain_types::fixtures::land_intent;
use domain_types::operation::{Origin, RunningOperation};
use domain_types::patronus::PatronusPushMode;
use domain_types::session::ViewPatch;
use pretty_assertions::assert_eq;
use repository_model::session::{OneClock, SessionHost, SessionKey, Sessions, Subscriber, SubscriberGone};
use std::path::Path;
use std::sync::Arc;
use sync_utils::clock::TestClock;
use sync_utils::process_scan::{host_name, process_started};
use sync_utils::timestamp::unix_now;
use tokio::sync::mpsc;

/// A window: each patch it gets wakes the test
struct Window(mpsc::UnboundedSender<()>);

impl Subscriber for Window {
  fn send(&self, _patch: &ViewPatch) -> Result<(), SubscriberGone> {
    self.0.send(()).map_err(|_gone| SubscriberGone)
  }
}

/// The registry of the sessions of `host` whose repository is `common_dir`, the key of its session (prefix `user`) and
/// the patches of a window subscribed to it through the registry
async fn watched(host: Arc<dyn SessionHost>, common_dir: &Path) -> (Registry, SessionKey, mpsc::UnboundedReceiver<()>) {
  let sessions = Arc::new(Sessions::new(tokio::runtime::Handle::current(), host, Arc::new(OneClock(Arc::new(TestClock::new())))));
  let dir = common_dir.to_path_buf();
  let registry = Registry::new(sessions, RegistryFiles::new(HolderTool::App, move |_key| Some(dir.clone())).unflushed());
  let key = session("user");
  let (patches, patched) = mpsc::unbounded_channel();
  registry.subscribe(&key, Box::new(Window(patches)), None).await.unwrap();
  (registry, key, patched)
}

/// The operations of the view once `done` holds of them, read again after each patch
async fn after_patches(registry: &Registry, key: &SessionKey, patched: &mut mpsc::UnboundedReceiver<()>, done: impl Fn(&[RunningOperation]) -> bool) -> Vec<RunningOperation> {
  loop {
    let operations = view_operations(registry.sessions(), key).await;
    if done(&operations) {
      return operations;
    }
    patched.recv().await.unwrap();
  }
}

/// A process of this host that runs until it is killed, as the records name it
fn agent(child: &std::process::Child) -> ProcessIdentity {
  ProcessIdentity {
    pid: child.id(),
    started: process_started(child.id()),
    host: host_name().to_string(),
  }
}

/// While a window shows a session, the operations registry of its repository is watched: the records of the other
/// processes of its branch prefix are its operations of other processes, of any worktree, the ones of another prefix and
/// of this process are not; a record that leaves the file leaves the view
#[tokio::test(flavor = "multi_thread")]
async fn a_window_shows_the_operations_another_process_runs_under_its_prefix_while_it_watches() {
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
  let (registry, key, mut patched) = watched(Arc::new(IdleHost::default()), directory.path()).await;
  let shown = after_patches(&registry, &key, &mut patched, |operations| operations.len() == 1).await;
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
  let shown = after_patches(&registry, &key, &mut patched, |operations| operations.len() == 2).await;
  let auth = shown.iter().find(|operation| operation.id == id("bd")).unwrap();
  assert_eq!(auth.intent, land_intent("auth", PatronusPushMode::RebaseAutosquash));
  drop(own);

  file.remove(&id("bd")).unwrap();
  file.remove(&id("bd-elsewhere")).unwrap();
  after_patches(&registry, &key, &mut patched, <[RunningOperation]>::is_empty).await;
  child.kill().unwrap();
  child.wait().unwrap();
}

/// `bd` touches the stamp of the repository after a mutation: the session a window shows syncs, without a focus. The
/// watch is seen to run first: a record that leaves the file leaves the view only through it
#[tokio::test(flavor = "multi_thread")]
async fn a_stamp_of_bd_syncs_the_session_a_window_shows() {
  let directory = tempfile::tempdir().unwrap();
  let file = ProcessRegistry::of(directory.path()).unflushed();
  let mut child = sleeping_child();
  file.insert_of_another_process(record("bd", &agent(&child), &safe_push("auth"), unix_now())).unwrap();
  let (synced, mut syncs) = mpsc::unbounded_channel();
  let host = Arc::new(IdleHost {
    runs: Some(synced),
    ..IdleHost::default()
  });
  let (registry, key, mut patched) = watched(Arc::clone(&host) as Arc<dyn SessionHost>, directory.path()).await;
  after_patches(&registry, &key, &mut patched, |operations| operations.len() == 1).await;
  file.remove(&id("bd")).unwrap();
  after_patches(&registry, &key, &mut patched, <[RunningOperation]>::is_empty).await;

  assert!(syncs.try_recv().is_err(), "no sync before the stamp");
  // the mutation was `brd sync`, which marked the worktree synced: the session, which read it as never synced when it
  // opened, syncs of itself now
  host.marked.store(true, std::sync::atomic::Ordering::SeqCst);
  touch_stamp(directory.path());
  // a broken rule syncs nothing: the test fails in seconds, not at the timeout of the runner
  tokio::time::timeout(std::time::Duration::from_secs(20), syncs.recv())
    .await
    .expect("a sync after the stamp")
    .unwrap();
  child.kill().unwrap();
  child.wait().unwrap();
}

/// A watcher that lost events under load (`FSEvents` and inotify tell an overflow of their queue as one event that names
/// no file) or failed wakes the read of the registry, the syncs of a stamp and the journal: the stamp of `bd` may have
/// been among the lost events, and a session would never sync for it
#[test]
fn lost_events_wake_the_registry_the_stamp_and_the_journal() {
  use notify::event::{EventKind, Flag};
  use std::sync::atomic::Ordering;
  let woke = |event: notify::Result<notify::Event>| {
    let wake = Wake::default();
    woken(&wake, event);
    [&wake.registry, &wake.stamp, &wake.journal].map(|flag| flag.load(Ordering::SeqCst))
  };
  assert_eq!(woke(Ok(notify::Event::new(EventKind::Other).set_flag(Flag::Rescan))), [true; 3]);
  assert_eq!(woke(Err(notify::Error::generic("the stream failed"))), [true; 3]);
  let stamp = notify::Event::new(EventKind::Any).add_path(std::path::PathBuf::from("/repo/.git/branch-deck/stamp"));
  assert_eq!(woke(Ok(stamp)), [false, true, false], "an event of a file wakes what it names");
}
