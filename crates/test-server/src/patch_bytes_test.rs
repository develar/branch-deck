//! What the session of the `branch_table_large` template sends a window: each `ViewPatch` as the channel of the app
//! carries it (`serde_json`), counted and measured, with the view at the subscription, for a first sync and for a sync
//! that changes nothing. A measurement, not a check (`tests/perf/branch-table.perf.ts` counts the same in the page):
//!
//! ```text
//! cargo nextest run -p test-server --run-ignored only -E 'test(/patch_bytes/)' --no-capture
//! ```

use crate::session_routes::TestSessionEnvironment;
use domain_types::session::{SyncReason, ViewPatch};
use git_executor::git_command_executor::GitCommandExecutor;
use repository_model::runtime::GitSessionHost;
use repository_model::session::{OneClock, SessionKey, Sessions, Subscriber, SubscriberGone};
use std::sync::{Arc, Mutex};
use sync_utils::clock::TestClock;

/// The count and the bytes of the patches a window got
#[derive(Clone, Default)]
struct MeasuringSubscriber {
  patches: Arc<Mutex<(usize, usize)>>,
}

impl MeasuringSubscriber {
  fn take(&self) -> (usize, usize) {
    std::mem::take(&mut *self.patches.lock().expect("the counts of a subscriber"))
  }
}

impl Subscriber for MeasuringSubscriber {
  fn send(&self, patch: &ViewPatch) -> Result<(), SubscriberGone> {
    let bytes = serde_json::to_string(patch).map_err(|_error| SubscriberGone)?.len();
    let mut patches = self.patches.lock().map_err(|_error| SubscriberGone)?;
    patches.0 += 1;
    patches.1 += bytes;
    Ok(())
  }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "a measurement: prints the patches of a session of 141 branches and their bytes"]
async fn patch_bytes_of_branch_table_large() {
  let directory = tempfile::TempDir::new().unwrap();
  test_utils::repo_template::templates::branch_table_large().build(directory.path()).unwrap();
  let key = SessionKey {
    worktree: directory.path().to_str().unwrap().to_owned(),
    prefix: "user-name".to_owned(),
  };
  // the environment of the server's sessions, without a server: the syncs read the issue links themselves
  let environment = TestSessionEnvironment { state: std::sync::Weak::new() };
  let host = GitSessionHost::new(GitCommandExecutor::new(), environment);
  let sessions = Sessions::new(tokio::runtime::Handle::current(), Arc::new(host), Arc::new(OneClock(Arc::new(TestClock::new()))));
  let subscriber = MeasuringSubscriber::default();
  let (_id, view) = sessions.subscribe(&key, Box::new(subscriber.clone())).await.expect("the session");
  println!("subscription: {} bytes", serde_json::to_string(&view).unwrap().len());

  for (title, reason) in [("first sync", SyncReason::Manual), ("resync", SyncReason::Manual)] {
    sessions.sync(&key, reason).wait().await.expect("the sync of the template");
    let (count, bytes) = subscriber.take();
    println!("{title}: {count} patches, {bytes} bytes");
  }
  let snapshot = sessions.get_snapshot(&key).await.expect("the session");
  println!("snapshot: {} bytes", serde_json::to_string(&snapshot).unwrap().len());
  assert_eq!(snapshot.order.len(), 141, "the branches of the template");
}
