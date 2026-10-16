use crate::push::{PushBranchParams, PushOutcome, push_branch};
use crate::remote_status::compute_remote_status_for_branch;
use crate::transfer_progress::TransferProgress;
use pretty_assertions::assert_eq;
use sync_types::RemoteStatusUpdate;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, disable_background_maintenance};

const BRANCH: &str = "prefix/virtual/feature";

fn path(repo: &TestRepo) -> &str {
  repo.path().to_str().unwrap()
}

/// A bare `origin` with `master`, and a clone of it
fn local_with_bare_remote() -> (TestRepo, TestRepo) {
  let remote = TestRepo::new_empty();
  remote
    .git_executor()
    .execute_command(&["init", "--bare", "--initial-branch=master"], path(&remote))
    .unwrap();
  disable_background_maintenance(remote.path());
  let local = TestRepo::new();
  local.create_commit("Initial commit", "README.md", "# Test");
  local.add_remote("origin", path(&remote)).unwrap();
  local.push("origin", "master").unwrap();
  (remote, local)
}

fn rev_parse(repo: &TestRepo, reference: &str) -> Option<String> {
  let (id, exit_code) = repo
    .git_executor()
    .execute_command_with_status(&["rev-parse", "--verify", "--quiet", reference], path(repo))
    .unwrap();
  (exit_code == 0).then_some(id)
}

fn remote_tip(remote: &TestRepo) -> Option<String> {
  rev_parse(remote, &format!("refs/heads/{BRANCH}"))
}

/// Point the virtual branch at `id`, as a sync does
fn set_branch(local: &TestRepo, id: &str) {
  local
    .git_executor()
    .execute_command(&["update-ref", &format!("refs/heads/{BRANCH}"), id], path(local))
    .unwrap();
}

/// Someone else commits `subject` onto the remote branch from another clone (creating the branch if it is not there)
fn push_from_elsewhere(remote: &TestRepo, subject: &str) -> String {
  let other = TestRepo::new_empty();
  other.clone_from(remote.path()).unwrap();
  if remote_tip(remote).is_some() {
    other.checkout(BRANCH).unwrap();
  } else {
    other.checkout_new_branch(BRANCH).unwrap();
  }
  other.set_config("user.name", "Other").unwrap();
  other.set_config("user.email", "other@example.com").unwrap();
  let id = other.create_commit(subject, &format!("{subject}.txt"), subject);
  other.push("origin", BRANCH).unwrap();
  id
}

/// The remote status the branch row shows
fn shown_status(local: &TestRepo) -> RemoteStatusUpdate {
  compute_remote_status_for_branch(local.git_executor(), path(local), "origin", BRANCH, "feature", Some("test@example.com"), "master", None).unwrap()
}

/// The push button's push, leased on `expected`
fn push(local: &TestRepo, expected: Option<&str>) -> PushOutcome {
  push_reporting(local, expected, &mut |_| {})
}

/// `push`, with how far it is
fn push_reporting(local: &TestRepo, expected: Option<&str>, on_progress: &mut dyn FnMut(TransferProgress)) -> PushOutcome {
  let params = PushBranchParams {
    repository_path: path(local).to_string(),
    branch_prefix: "prefix".to_string(),
    branch_name: "feature".to_string(),
    my_email: Some("test@example.com".to_string()),
    baseline_branch: "master".to_string(),
    parent_branch: None,
    expected_remote: expected.map(str::to_string),
    tip: None,
  };
  push_branch(local.git_executor(), &params, on_progress).unwrap()
}

/// The push button's push, leased on the remote head of the status the row shows
fn push_as_shown(local: &TestRepo) -> PushOutcome {
  push(local, shown_status(local).remote_tip.as_deref())
}

fn pushed(outcome: PushOutcome) -> RemoteStatusUpdate {
  match outcome {
    PushOutcome::Pushed { remote_status } => remote_status,
    PushOutcome::RemoteMoved { remote_status } => panic!("the remote moved: {remote_status:?}"),
  }
}

fn remote_moved(outcome: PushOutcome) -> RemoteStatusUpdate {
  match outcome {
    PushOutcome::RemoteMoved { remote_status } => remote_status,
    PushOutcome::Pushed { remote_status } => panic!("pushed: {remote_status:?}"),
  }
}

fn dropped_subjects(status: &RemoteStatusUpdate) -> Vec<&str> {
  status.dropped_commits.iter().map(|commit| commit.subject.as_str()).collect()
}

/// The branch is pushed once: the remote is at `tip`
fn first_push(remote: &TestRepo, local: &TestRepo) -> String {
  let tip = local.create_commit("(feature) Add feature", "feature.txt", "feature");
  set_branch(local, &tip);
  let status = pushed(push(local, None));
  assert_eq!(remote_tip(remote), Some(tip.clone()));
  assert_eq!((status.remote_tip.as_deref(), status.commits_behind), (Some(tip.as_str()), 0));
  assert!(status.unpushed_commits.is_empty(), "{:?}", status.unpushed_commits);
  tip
}

#[test]
fn a_branch_not_on_the_remote_is_created_when_expected_absent() {
  let (remote, local) = local_with_bare_remote();
  first_push(&remote, &local);
}

#[test]
fn a_push_reports_its_progress() {
  let (_remote, local) = local_with_bare_remote();
  let tip = local.create_commit("(feature) Add feature", "feature.txt", "feature");
  set_branch(&local, &tip);

  let mut progress = Vec::new();
  pushed(push_reporting(&local, None, &mut |it| progress.push(it)));
  assert!(progress.windows(2).all(|pair| pair[0].percent <= pair[1].percent), "{progress:?}");
  // the server may resolve deltas after the objects are written
  let last = progress.last().expect("no push progress");
  let last_stage_ended = last.line.starts_with("Writing objects: 100% ") || last.line.starts_with("Resolving deltas: 100% ");
  assert!(last_stage_ended, "{progress:?}");
  assert!(last.percent >= 95, "{progress:?}");
  assert!(last.transferred.is_some(), "{progress:?}");
}

#[test]
fn a_push_fails_without_the_local_branch() {
  let (remote, local) = local_with_bare_remote();

  let params = PushBranchParams {
    repository_path: path(&local).to_string(),
    branch_prefix: "prefix".to_string(),
    branch_name: "missing".to_string(),
    my_email: None,
    baseline_branch: "master".to_string(),
    parent_branch: None,
    expected_remote: None,
    tip: None,
  };
  assert!(push_branch(local.git_executor(), &params, &mut |_| {}).is_err());
  assert_eq!(rev_parse(&remote, "refs/heads/prefix/virtual/missing"), None);
}

#[test]
fn a_rebuilt_branch_replaces_the_remote_tip_it_saw() {
  let (remote, local) = local_with_bare_remote();
  first_push(&remote, &local);

  // the virtual branch is rebuilt: not a fast-forward of what was pushed
  local.reset_hard("origin/master").unwrap();
  let rewritten = local.create_commit("(feature) Add feature, reworded", "feature.txt", "feature");
  set_branch(&local, &rewritten);

  pushed(push_as_shown(&local));
  assert_eq!(remote_tip(&remote), Some(rewritten.clone()));
}

/// Someone else's fetched commit is shown as dropped, and the Force Push the user confirms replaces it, although Branch
/// Deck's last push is older
#[test]
fn a_force_push_drops_the_remote_commit_the_user_saw() {
  let (remote, local) = local_with_bare_remote();
  let first = first_push(&remote, &local);
  let theirs = push_from_elsewhere(&remote, "Their fix");
  local.fetch_prune("origin").unwrap();

  let shown = shown_status(&local);
  assert_eq!((shown.remote_tip.as_deref(), shown.commits_behind), (Some(theirs.as_str()), 1));
  assert_eq!(dropped_subjects(&shown), ["Their fix"]);
  assert_eq!(shown.dropped_commits[0].author, "Other");

  let status = pushed(push_as_shown(&local));
  assert_eq!(remote_tip(&remote), Some(first.clone()), "their commit is dropped");
  assert_eq!(status.commits_behind, 0);
}

/// The user brought someone else's commit into the branch: a plain push with the head the user saw goes through,
/// although Branch Deck's last push is older
#[test]
fn a_plain_push_with_the_seen_head_replaces_a_fetched_remote_commit() {
  let (remote, local) = local_with_bare_remote();
  first_push(&remote, &local);
  let theirs = push_from_elsewhere(&remote, "Their fix");
  local.fetch_prune("origin").unwrap();
  local.cherry_pick(&theirs).unwrap();
  let rebuilt = local.head();
  set_branch(&local, &rebuilt);

  let shown = shown_status(&local);
  // their commit is in the branch by content: nothing is dropped, and the copy replaces it
  assert_eq!((shown.commits_behind, shown.fast_forward), (0, false));

  pushed(push_as_shown(&local));
  assert_eq!(remote_tip(&remote), Some(rebuilt.clone()));
}

/// A commit pushed after the user's last look is not overwritten: the push fetches it and returns the new status, and
/// the next push with the new head replaces it
#[test]
fn a_remote_that_moved_since_the_status_is_fetched_not_overwritten() {
  let (remote, local) = local_with_bare_remote();
  let first = first_push(&remote, &local);
  let seen = push_from_elsewhere(&remote, "Their fix");
  local.fetch_prune("origin").unwrap();
  let shown = shown_status(&local);
  let newer = push_from_elsewhere(&remote, "Their second fix");

  let status = remote_moved(push(&local, shown.remote_tip.as_deref()));
  assert_eq!(shown.remote_tip, Some(seen));
  assert_eq!(remote_tip(&remote), Some(newer.clone()), "the remote is unchanged");
  assert_eq!(status.remote_tip, Some(newer.clone()));
  assert_eq!(rev_parse(&local, &format!("refs/remotes/origin/{BRANCH}")), Some(newer), "the branch was fetched");
  assert_eq!(status.commits_behind, 2);
  assert_eq!(dropped_subjects(&status), ["Their second fix", "Their fix"]);

  pushed(push(&local, status.remote_tip.as_deref()));
  assert_eq!(remote_tip(&remote), Some(first));
}

#[test]
fn a_branch_created_on_the_remote_meanwhile_is_fetched_not_overwritten() {
  let (remote, local) = local_with_bare_remote();
  let tip = local.create_commit("(feature) Add feature", "feature.txt", "feature");
  set_branch(&local, &tip);
  let theirs = push_from_elsewhere(&remote, "Their start");

  let status = remote_moved(push(&local, None));
  assert_eq!(remote_tip(&remote), Some(theirs.clone()));
  assert_eq!((status.remote_exists, status.remote_tip.as_deref()), (true, Some(theirs.as_str())));
  assert_eq!(dropped_subjects(&status), ["Their start"]);
}

#[test]
fn a_branch_deleted_on_the_remote_is_shown_as_not_pushed() {
  let (remote, local) = local_with_bare_remote();
  let first = first_push(&remote, &local);
  remote
    .git_executor()
    .execute_command(&["update-ref", "-d", &format!("refs/heads/{BRANCH}")], path(&remote))
    .unwrap();

  let status = remote_moved(push(&local, Some(&first)));
  assert_eq!((status.remote_exists, status.remote_tip), (false, None));
  assert_eq!(rev_parse(&local, &format!("refs/remotes/origin/{BRANCH}")), None, "the remote-tracking ref is deleted");

  pushed(push_as_shown(&local));
  assert_eq!(remote_tip(&remote), Some(first));
}

/// The lease held, but the remote refused the push (a hook, a protected branch): an error
#[test]
fn a_push_the_remote_refuses_for_another_reason_is_an_error() {
  let (remote, local) = local_with_bare_remote();
  let first = first_push(&remote, &local);
  let hook = remote.path().join("hooks/pre-receive");
  std::fs::write(&hook, "#!/bin/sh\necho 'protected branch' >&2\nexit 1\n").unwrap();
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
  }
  let more = local.create_commit("(feature) More", "more.txt", "more");
  set_branch(&local, &more);

  let params = PushBranchParams {
    repository_path: path(&local).to_string(),
    branch_prefix: "prefix".to_string(),
    branch_name: "feature".to_string(),
    my_email: None,
    baseline_branch: "master".to_string(),
    parent_branch: None,
    expected_remote: Some(first.clone()),
    tip: None,
  };
  let error = push_branch(local.git_executor(), &params, &mut |_| {}).unwrap_err();
  assert!(error.to_string().contains("The remote rejected the push"), "{error}");
  assert_eq!(remote_tip(&remote), Some(first.clone()));
}

/// Push Up to Here: the push of the branch up to `tip`, leased on `expected`, against the baseline on the remote (the
/// commits of the branch are on the local `master`)
fn push_up_to(local: &TestRepo, expected: Option<&str>, tip: &str) -> anyhow::Result<PushOutcome> {
  let params = PushBranchParams {
    repository_path: path(local).to_string(),
    branch_prefix: "prefix".to_string(),
    branch_name: "feature".to_string(),
    my_email: Some("test@example.com".to_string()),
    baseline_branch: "origin/master".to_string(),
    parent_branch: None,
    expected_remote: expected.map(str::to_string),
    tip: Some(tip.to_string()),
  };
  push_branch(local.git_executor(), &params, &mut |_| {})
}

#[test]
fn a_push_up_to_a_commit_leaves_the_commits_after_it_unpushed() {
  let (remote, local) = local_with_bare_remote();
  let first = local.create_commit("(feature) Add the parser", "parser.txt", "parser");
  let second = local.create_commit("(feature) Add the printer", "printer.txt", "printer");
  let third = local.create_commit("(feature) Add the docs", "docs.txt", "docs");
  set_branch(&local, &third);

  let status = pushed(push_up_to(&local, None, &second).unwrap());
  assert_eq!(remote_tip(&remote), Some(second.clone()));
  assert_eq!(status.unpushed_commits, vec![third.clone()]);
  // the remote has the commit now: a push up to it, or to an older one, has nothing to send
  assert!(push_up_to(&local, Some(&second), &second).is_err());
  assert!(push_up_to(&local, Some(&second), &first).is_err());
  // a newer commit fast-forwards the remote
  pushed(push_up_to(&local, Some(&second), &third).unwrap());
  assert_eq!(remote_tip(&remote), Some(third));
}

#[test]
fn a_push_up_to_a_commit_refuses_one_outside_the_branch_and_a_branch_rewritten_since_its_push() {
  let (remote, local) = local_with_bare_remote();
  let baseline = rev_parse(&local, "origin/master").unwrap();
  let first = local.create_commit("(feature) Add the parser", "parser.txt", "parser");
  set_branch(&local, &first);
  // the baseline is not the branch's own
  assert!(push_up_to(&local, None, &baseline).is_err());
  pushed(push(&local, None));

  // rebuilt: the remote head is no older commit of the branch, so a part of it would rewind the pull request
  local.reset_hard("origin/master").unwrap();
  let reworded = local.create_commit("(feature) Add the parser, reworded", "parser.txt", "parser");
  let second = local.create_commit("(feature) Add the printer", "printer.txt", "printer");
  set_branch(&local, &second);
  let error = push_up_to(&local, Some(&first), &reworded).unwrap_err();
  assert!(error.to_string().contains("rewritten"), "{error:#}");
  assert_eq!(remote_tip(&remote), Some(first));
}
