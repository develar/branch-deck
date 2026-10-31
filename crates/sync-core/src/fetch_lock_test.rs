//! Two fetches of the baseline of one repository, in two threads of this process (as the app's Update Project beside
//! its Fetch; two processes are `bd-update`'s `update_test.rs`), against a remote whose upload-pack holds the first
//! run until the test lets it go: the fake `remote.origin.uploadpack` of the clone, which counts its runs, so the test
//! knows the first fetch holds the lock and waits at the remote before the second one starts

use crate::baseline::{BaselineUpstream, fetch_baseline};
use crate::transfer_progress::TransferProgress;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::LockHolder;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use test_utils::held_upload_pack::{FAIL, HeldUploadPack, SERVE};
use test_utils::process_budget::processes;

/// The upstream, its clone, and the upload-pack of the clone's remote
struct Remote {
  upstream: TestRepo,
  clone: TestRepo,
  upload_pack: HeldUploadPack,
}

impl Remote {
  /// A clone of an upstream with one commit, and one more commit upstream that a fetch brings; the first fetch waits at
  /// the remote until `release`, then does `first`
  fn new(first: &str) -> Self {
    let upstream = TestRepo::new();
    upstream.create_commit("Initial", "shared.txt", "one\n");
    let clone = TestRepo::new_empty();
    clone.clone_from(upstream.path()).unwrap();
    upstream.create_commit("Upstream change", "shared.txt", "two\n");
    let upload_pack = HeldUploadPack::install(&clone, "origin", first);
    Self { upstream, clone, upload_pack }
  }

  fn path(&self) -> &str {
    self.clone.path().to_str().unwrap()
  }

  fn tracking_tip(&self, branch: &str) -> String {
    self.clone.git(&["rev-parse", &format!("refs/remotes/origin/{branch}")])
  }
}

/// `origin/master` of the clone
fn master() -> BaselineUpstream {
  upstream_of("master")
}

/// The branch `branch` of the remote, into `refs/remotes/origin/<branch>`
fn upstream_of(branch: &str) -> BaselineUpstream {
  BaselineUpstream {
    remote: "origin".to_string(),
    remote_ref: format!("refs/heads/{branch}"),
    tracking_ref: format!("refs/remotes/origin/{branch}"),
    name: format!("origin/{branch}"),
  }
}

/// A fetch of `upstream` in `remote` through `git`, its progress sent to `progress`
fn fetch(remote: &Remote, git: &GitCommandExecutor, upstream: &BaselineUpstream, progress: &mpsc::Sender<TransferProgress>) -> anyhow::Result<Option<String>> {
  fetch_baseline(git, remote.path(), upstream, &mut |line| progress.send(line).unwrap(), &|| false)
}

/// The fetch of `remote` that holds the lock, in `scope`, and one through `waiter` that starts once the first one
/// waits at the remote: the progress of the waiter until its first line, which names whom it waits for, then the
/// release of the remote; both results with every progress line of the waiter
fn beside_a_held_fetch(remote: &Remote, holder: &GitCommandExecutor, waiter: &GitCommandExecutor, upstream: &BaselineUpstream) -> FetchesBeside {
  let (holder_progress, _holder_lines) = mpsc::channel();
  let (waiter_progress, lines) = mpsc::channel();
  std::thread::scope(|scope| {
    let held = scope.spawn(move || fetch(remote, holder, &master(), &holder_progress));
    remote.upload_pack.wait_held(|| !held.is_finished());
    // the sender goes with the thread, so the lines end once it ended
    let waiting = scope.spawn(move || fetch(remote, waiter, upstream, &waiter_progress));
    let first = lines.recv().expect("the waiter tells whom it waits for");
    remote.upload_pack.release();
    let (held, waited) = (held.join().unwrap(), waiting.join().unwrap());
    FetchesBeside {
      held,
      waited,
      lines: std::iter::once(first).chain(lines.iter()).map(|progress| progress.line).collect(),
    }
  })
}

struct FetchesBeside {
  held: anyhow::Result<Option<String>>,
  waited: anyhow::Result<Option<String>>,
  lines: Vec<String>,
}

/// Every line names the holder of the lock, this process
fn assert_names_this_process(lines: &[String], upstream: &str) {
  let pid = format!("(pid {}, for ", std::process::id());
  assert_ne!(lines.len(), 0);
  for line in lines {
    assert!(
      line.starts_with(&format!("Fetching {upstream} with `")) && line.contains(&pid) && line.ends_with('…'),
      "{line}"
    );
  }
}

/// A fetch of the tracking ref that another fetch writes waits for it and takes its result: one transfer, both Ok, the
/// ref where the one fetch left it; the waiter runs no fetch, only the reads of the ref before and after the wait and
/// its ancestry, and its progress names the holder while it waits
#[test]
fn a_fetch_beside_another_fetch_of_its_tracking_ref_takes_its_result() {
  let remote = Remote::new(SERVE);
  let (holder, waiter) = (
    GitCommandExecutor::new().with_process_stats("holder"),
    GitCommandExecutor::new().with_process_stats("waiter"),
  );
  let fetched = beside_a_held_fetch(&remote, &holder, &waiter, &master());
  assert_eq!((fetched.held.unwrap(), fetched.waited.unwrap()), (None, None), "fast-forwards");
  assert_eq!(remote.upload_pack.runs(), 1, "one transfer");
  assert_eq!(remote.tracking_tip("master"), remote.upstream.head());
  assert_names_this_process(&fetched.lines, "origin/master");
  assert_eq!(processes(&holder), [("fetch".to_string(), 1), ("rev-parse".to_string(), 1)]);
  assert_eq!(processes(&waiter), [("merge-base".to_string(), 1), ("rev-parse".to_string(), 3)]);
}

/// The fetch waited for was a forced update (the upstream was force-pushed): the waiter returns the old tip too, as its
/// own fetch would have, so its update check leaves out the commits the force-push removed
#[test]
fn a_fetch_that_joins_a_forced_update_returns_the_old_tip() {
  let remote = Remote::new(SERVE);
  let old_tip = remote.tracking_tip("master");
  remote.upstream.git(&["reset", "--hard", "HEAD~1"]);
  remote.upstream.git(&["commit", "--amend", "--allow-empty", "-m", "Rewritten"]);
  let git = GitCommandExecutor::new();
  let fetched = beside_a_held_fetch(&remote, &git, &git, &master());
  assert_eq!((fetched.held.unwrap(), fetched.waited.unwrap()), (Some(old_tip.clone()), Some(old_tip)));
  assert_eq!(remote.upload_pack.runs(), 1);
  assert_eq!(remote.tracking_tip("master"), remote.upstream.head());
}

/// The fetch waited for ended well, but the remote had nothing new, so the ref did not move: the waiter joins it all the
/// same, by the outcome the holder wrote under the lock, instead of a transfer of its own
#[test]
fn a_fetch_beside_another_that_found_nothing_new_takes_its_result() {
  let remote = Remote::new(SERVE);
  let tip = remote.tracking_tip("master");
  remote.upstream.git(&["reset", "--hard", "HEAD~1"]);
  let git = GitCommandExecutor::new();
  let fetched = beside_a_held_fetch(&remote, &git, &git, &master());
  assert_eq!((fetched.held.unwrap(), fetched.waited.unwrap()), (None, None));
  assert_eq!(remote.upload_pack.runs(), 1, "one transfer");
  assert_eq!(remote.tracking_tip("master"), tip);
}

/// The user stops the app's wait for the fetch of another process: the wait ends with `LockWaitCanceled`, no transfer
#[test]
fn a_canceled_wait_for_another_fetch_ends_without_a_transfer() {
  let remote = Remote::new(SERVE);
  remote.upload_pack.release();
  let common_dir = PathBuf::from(remote.clone.git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]));
  let _other = crate::fetch_lock::hold_fetch_as_another_process(&common_dir, "refs/remotes/origin/master", "bd update").unwrap();
  let error = fetch_baseline(&GitCommandExecutor::new(), remote.path(), &master(), &mut |_| {}, &|| true).unwrap_err();
  assert!(error.downcast_ref::<history_rewrite::repository_lock::LockWaitCanceled>().is_some(), "{error:#}");
  assert_eq!(remote.upload_pack.runs(), 0);
}

/// The fetch waited for failed (the network, the remote) and left the ref where it was: the waiter fetches itself, once
/// the lock is free, and gets the upstream; the failure stays the other one's
#[test]
fn a_fetch_that_waited_for_one_that_failed_fetches_itself() {
  let remote = Remote::new(FAIL);
  let git = GitCommandExecutor::new();
  let fetched = beside_a_held_fetch(&remote, &git, &git, &master());
  fetched.held.unwrap_err();
  assert_eq!(fetched.waited.unwrap(), None);
  assert_eq!(remote.upload_pack.runs(), 2, "the failed fetch, then the waiter's own");
  assert_eq!(remote.tracking_tip("master"), remote.upstream.head());
}

/// One lock for every fetch of the baseline: a fetch of another tracking ref (the baseline of another worktree) waits
/// for the holder, its progress naming it, then fetches its own ref, which the holder did not fetch
#[test]
fn a_fetch_of_another_tracking_ref_waits_and_fetches_itself() {
  let remote = Remote::new(SERVE);
  remote.upstream.git(&["branch", "release", "HEAD~1"]);
  let git = GitCommandExecutor::new();
  let fetched = beside_a_held_fetch(&remote, &git, &git, &upstream_of("release"));
  assert_eq!((fetched.held.unwrap(), fetched.waited.unwrap()), (None, None));
  assert_eq!(remote.upload_pack.runs(), 2);
  assert_names_this_process(&fetched.lines, "origin/release");
  assert_eq!(remote.tracking_tip("release"), remote.upstream.git(&["rev-parse", "release"]));
  assert_eq!(remote.tracking_tip("master"), remote.upstream.head());
}

/// A holder killed in its fetch frees the lock with its process and leaves its record behind: the record names a
/// process that runs no more (another start time of the pid), so the next fetch neither waits nor joins, and fetches at
/// once; its own record replaces the stale one and goes with its lock
#[test]
fn a_fetch_after_a_killed_holder_fetches_at_once() {
  let remote = Remote::new(SERVE);
  remote.upload_pack.release();
  let git = GitCommandExecutor::new();
  let common_dir = PathBuf::from(remote.clone.git(&["rev-parse", "--path-format=absolute", "--git-common-dir"])).join("branch-deck");
  fs::create_dir_all(&common_dir).unwrap();
  let record = common_dir.join("fetch.lock.holder");
  let killed = LockHolder {
    command: "bd update".to_string(),
    pid: std::process::id(),
    process_started: Some(1),
    since: 0,
    subject: Some("refs/remotes/origin/master".to_string()),
  };
  fs::write(&record, serde_json::to_vec(&killed).unwrap()).unwrap();

  let (progress, lines) = mpsc::channel();
  assert_eq!(fetch(&remote, &git, &master(), &progress).unwrap(), None);
  drop(progress);
  assert!(lines.iter().all(|progress| !progress.line.starts_with("Fetching")), "no wait");
  assert_eq!(remote.upload_pack.runs(), 1);
  assert_eq!(remote.tracking_tip("master"), remote.upstream.head());
  assert!(!record.exists(), "the record went with the lock");
}
