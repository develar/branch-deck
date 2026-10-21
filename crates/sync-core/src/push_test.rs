use crate::push::{PushBranchParams, PushBranchesOutcome, PushBranchesParams, PushOutcome, StackPushBranch, push_branch, push_branches};
use crate::remote_status::compute_remote_status_for_branch;
use crate::transfer_progress::TransferProgress;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use sync_types::RemoteStatusUpdate;
use sync_types::stack::BlockReason;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, disable_background_maintenance, refuse_every_push};

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
  remote_tip_of(remote, BRANCH)
}

fn remote_tip_of(remote: &TestRepo, branch: &str) -> Option<String> {
  rev_parse(remote, &format!("refs/heads/{branch}"))
}

/// Point the virtual branch at `id`, as a sync does
fn set_branch(local: &TestRepo, id: &str) {
  set_branch_of(local, BRANCH, id);
}

fn set_branch_of(local: &TestRepo, branch: &str, id: &str) {
  local
    .git_executor()
    .execute_command(&["update-ref", &format!("refs/heads/{branch}"), id], path(local))
    .unwrap();
}

/// Someone else commits `subject` onto the remote branch from another clone (creating the branch if it is not there)
fn push_from_elsewhere(remote: &TestRepo, subject: &str) -> String {
  push_from_elsewhere_to(remote, BRANCH, subject)
}

fn push_from_elsewhere_to(remote: &TestRepo, branch: &str, subject: &str) -> String {
  let other = TestRepo::new_empty();
  other.clone_from(remote.path()).unwrap();
  if remote_tip_of(remote, branch).is_some() {
    other.checkout(branch).unwrap();
  } else {
    other.checkout_new_branch(branch).unwrap();
  }
  other.set_config("user.name", "Other").unwrap();
  other.set_config("user.email", "other@example.com").unwrap();
  let id = other.create_commit(subject, &format!("{subject}.txt"), subject);
  other.push("origin", branch).unwrap();
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
  push_branch(local.git_executor(), &params, &HashMap::new(), on_progress).unwrap()
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
  push_branch(local.git_executor(), &params, &HashMap::new(), &mut |_| {}).unwrap_err();
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
  assert_eq!(remote_tip(&remote), Some(rewritten));
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
  assert_eq!(remote_tip(&remote), Some(first), "their commit is dropped");
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
  assert_eq!(remote_tip(&remote), Some(rebuilt));
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
  refuse_every_push(remote.path());
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
  let error = push_branch(local.git_executor(), &params, &HashMap::new(), &mut |_| {}).unwrap_err();
  assert!(error.to_string().contains("The remote rejected the push"), "{error}");
  assert_eq!(remote_tip(&remote), Some(first));
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
  push_branch(local.git_executor(), &params, &HashMap::new(), &mut |_| {})
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
  push_up_to(&local, Some(&second), &second).unwrap_err();
  push_up_to(&local, Some(&second), &first).unwrap_err();
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
  push_up_to(&local, None, &baseline).unwrap_err();
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

const AUTH: &str = "prefix/virtual/auth";
const LOGIN: &str = "prefix/virtual/login";

/// The app's push of a stack: `auth`, then `login` built on it, each leased on its `expected` remote head and pushed
/// up to its `tip` (Push Up to Here) when one is given
fn push_stack(local: &TestRepo, expected: [Option<&str>; 2], tips: [Option<&str>; 2]) -> PushBranchesOutcome {
  let branch = |name: &str, parent: Option<&str>, expected: Option<&str>, tip: Option<&str>| StackPushBranch {
    branch_name: name.to_string(),
    parent_branch: parent.map(str::to_string),
    expected_remote: expected.map(str::to_string),
    tip: tip.map(str::to_string),
    whole_stack: false,
    unapplied: false,
  };
  let params = PushBranchesParams {
    repository_path: path(local).to_string(),
    branch_prefix: "prefix".to_string(),
    my_email: Some("test@example.com".to_string()),
    baseline_branch: "origin/master".to_string(),
    branches: vec![branch("auth", None, expected[0], tips[0]), branch("login", Some("auth"), expected[1], tips[1])],
  };
  push_branches(local.git_executor(), &params, &HashMap::new(), &mut |_| {}).unwrap()
}

/// The remote statuses of a push of `params` that no sync refuses and whose remote branches are where it expects them
fn stack_pushed_as_is(local: &TestRepo, params: &PushBranchesParams) -> Vec<RemoteStatusUpdate> {
  stack_pushed(push_branches(local.git_executor(), params, &HashMap::new(), &mut |_| {}).unwrap())
}

fn stack_pushed(outcome: PushBranchesOutcome) -> Vec<RemoteStatusUpdate> {
  match outcome {
    PushBranchesOutcome::Pushed { remote_statuses } => remote_statuses,
    PushBranchesOutcome::RemoteMoved { branch_name, .. } => panic!("the remote of {branch_name} moved"),
  }
}

/// `auth` of one commit and `login` of one commit built on it, as the sync leaves a stack; returns their tips
fn stack(local: &TestRepo) -> (String, String) {
  let auth = local.create_commit("(auth) Add auth", "auth.txt", "auth");
  let login = local.create_commit("(auth/login) Add login", "login.txt", "login");
  set_branch_of(local, AUTH, &auth);
  set_branch_of(local, LOGIN, &login);
  (auth, login)
}

/// A branch the last sync did not build keeps the ref of an older build, of commits it may no longer have: the push of
/// it, alone or in a stack, pushes nothing, and says why
#[test]
fn a_push_of_a_branch_the_last_sync_did_not_build_is_refused() {
  let (remote, local) = local_with_bare_remote();
  stack(&local);
  let params = PushBranchesParams {
    repository_path: path(&local).to_string(),
    branch_prefix: "prefix".to_string(),
    my_email: Some("test@example.com".to_string()),
    baseline_branch: "origin/master".to_string(),
    branches: ["auth", "login"]
      .map(|name| StackPushBranch {
        branch_name: name.to_string(),
        parent_branch: (name == "login").then(|| "auth".to_string()),
        expected_remote: None,
        tip: None,
        whole_stack: false,
        unapplied: false,
      })
      .into(),
  };
  let not_built = HashMap::from([("auth".to_string(), BlockReason::Conflict), ("login".to_string(), BlockReason::Blocked)]);

  let error = push_branches(local.git_executor(), &params, &not_built, &mut |_| {}).unwrap_err();
  assert_eq!(
    error.to_string(),
    "Nothing was pushed: auth is not synced (a merge conflict), login is not synced (its parent is not built)"
  );
  let one = PushBranchParams {
    repository_path: path(&local).to_string(),
    branch_prefix: "prefix".to_string(),
    branch_name: "login".to_string(),
    my_email: None,
    baseline_branch: "origin/master".to_string(),
    parent_branch: Some("auth".to_string()),
    expected_remote: None,
    tip: None,
  };
  let error = push_branch(local.git_executor(), &one, &not_built, &mut |_| {}).unwrap_err();
  assert_eq!(error.to_string(), "Nothing was pushed: login is not synced (its parent is not built)");
  assert_eq!((remote_tip_of(&remote, AUTH), remote_tip_of(&remote, LOGIN)), (None, None));

  // once a sync built them, the same push goes
  stack_pushed_as_is(&local, &params);
  assert!(remote_tip_of(&remote, LOGIN).is_some());
}

/// The stack branch of the whole stack goes as one more branch of a push: `{prefix}/stack/<root>`, leased like a
/// virtual branch, its remote status computed against the baseline (it has no parent)
#[test]
fn the_whole_stack_pushes_its_stack_branch() {
  let (remote, local) = local_with_bare_remote();
  let (auth, login) = stack(&local);
  set_branch_of(&local, "prefix/stack/auth", &login);
  let params = |expected: Option<String>| PushBranchesParams {
    repository_path: path(&local).to_string(),
    branch_prefix: "prefix".to_string(),
    my_email: Some("test@example.com".to_string()),
    baseline_branch: "origin/master".to_string(),
    branches: vec![StackPushBranch {
      branch_name: "auth".to_string(),
      parent_branch: None,
      expected_remote: expected,
      tip: None,
      whole_stack: true,
      unapplied: false,
    }],
  };

  let statuses = stack_pushed_as_is(&local, &params(None));
  assert_eq!(remote_tip_of(&remote, "prefix/stack/auth"), Some(login.clone()));
  assert_eq!(remote_tip_of(&remote, AUTH), None, "only the stack branch is pushed");
  let [status] = statuses.as_slice() else { panic!("{statuses:?}") };
  assert_eq!(
    (status.remote_tip.as_deref(), status.unpushed_commits.len(), status.commits_behind),
    (Some(login.as_str()), 0, 0)
  );

  // its lease holds as a virtual branch's: a rebuilt stack branch pushed as if it had no remote branch is refused
  set_branch_of(&local, "prefix/stack/auth", &auth);
  let moved = push_branches(local.git_executor(), &params(None), &HashMap::new(), &mut |_| {}).unwrap();
  assert!(
    matches!(moved, PushBranchesOutcome::RemoteMoved { ref branch_name, whole_stack: true, .. } if branch_name == "auth"),
    "{moved:?}"
  );
  assert_eq!(remote_tip_of(&remote, "prefix/stack/auth"), Some(login));
}

/// Push with Children of a root takes the stack branch of its whole stack along, after the branches: its virtual
/// branch and its stack branch both carry the name of the root, so the outcome of a moved stack branch says which one
/// moved (`whole_stack`), and nothing of the push went
#[test]
fn the_stack_branch_goes_with_the_branches_and_a_moved_one_is_named_as_the_stack_branch() {
  let (remote, local) = local_with_bare_remote();
  let (auth, login) = stack(&local);
  set_branch_of(&local, "prefix/stack/auth", &login);
  let branch = |name: &str, parent: Option<&str>, expected: Option<&str>, whole_stack: bool| StackPushBranch {
    branch_name: name.to_string(),
    parent_branch: parent.map(str::to_string),
    expected_remote: expected.map(str::to_string),
    tip: None,
    whole_stack,
    unapplied: false,
  };
  let params = |stack_lease: Option<&str>| PushBranchesParams {
    repository_path: path(&local).to_string(),
    branch_prefix: "prefix".to_string(),
    my_email: Some("test@example.com".to_string()),
    baseline_branch: "origin/master".to_string(),
    branches: vec![
      branch("auth", None, None, false),
      branch("login", Some("auth"), None, false),
      branch("auth", None, stack_lease, true),
    ],
  };
  // someone pushed the stack branch elsewhere: its lease of no remote branch refuses the whole push
  let theirs = push_from_elsewhere_to(&remote, "prefix/stack/auth", "Their stack");
  let PushBranchesOutcome::RemoteMoved {
    branch_name,
    whole_stack,
    remote_statuses,
  } = push_branches(local.git_executor(), &params(None), &HashMap::new(), &mut |_| {}).unwrap()
  else {
    panic!("pushed over the remote stack branch");
  };
  assert_eq!((branch_name.as_str(), whole_stack), ("auth", true));
  assert_eq!((remote_tip_of(&remote, AUTH), remote_tip_of(&remote, LOGIN)), (None, None), "nothing was pushed");
  assert_eq!(remote_statuses[2].remote_tip.as_deref(), Some(theirs.as_str()));

  // leased on the fetched head, the branches and the stack branch go in one push
  let statuses = stack_pushed_as_is(&local, &params(Some(&theirs)));
  assert_eq!(
    (remote_tip_of(&remote, AUTH), remote_tip_of(&remote, LOGIN), remote_tip_of(&remote, "prefix/stack/auth")),
    (Some(auth), Some(login.clone()), Some(login))
  );
  assert_eq!(statuses.len(), 3);
}

#[test]
fn a_stack_pushes_its_branches_in_one_push() {
  let (remote, local) = local_with_bare_remote();
  let (auth, login) = stack(&local);

  let statuses = stack_pushed(push_stack(&local, [None, None], [None, None]));
  assert_eq!((remote_tip_of(&remote, AUTH), remote_tip_of(&remote, LOGIN)), (Some(auth.clone()), Some(login.clone())));
  let tips: Vec<(&str, Option<&str>)> = statuses.iter().map(|status| (status.branch_name.as_str(), status.remote_tip.as_deref())).collect();
  assert_eq!(tips, [("auth", Some(auth.as_str())), ("login", Some(login.as_str()))]);
  assert!(
    statuses.iter().all(|status| status.unpushed_commits.is_empty() && status.commits_behind == 0),
    "{statuses:?}"
  );
}

/// A stack is pushed whole or not at all: someone else's commit on the remote branch of `login` refuses the push of
/// `auth` too. Both are fetched, the outcome names `login` and says what each remote branch has now, and the next push
/// with the fetched head replaces it
#[test]
fn a_moved_lease_refuses_every_branch_and_names_it() {
  let (remote, local) = local_with_bare_remote();
  let (auth, login) = stack(&local);
  stack_pushed(push_stack(&local, [None, None], [None, None]));
  let theirs = push_from_elsewhere_to(&remote, LOGIN, "Their fix");
  // a commit more in each, `login` rebuilt on the new `auth` as a sync does
  local.reset_hard(&auth).unwrap();
  let more_auth = local.create_commit("(auth) More auth", "more-auth.txt", "more");
  local.create_commit("(auth/login) Add login", "login.txt", "login");
  let more_login = local.create_commit("(auth/login) More login", "more-login.txt", "more");
  set_branch_of(&local, AUTH, &more_auth);
  set_branch_of(&local, LOGIN, &more_login);

  let PushBranchesOutcome::RemoteMoved {
    branch_name,
    whole_stack,
    remote_statuses,
  } = push_stack(&local, [Some(&auth), Some(&login)], [None, None])
  else {
    panic!("pushed over someone else's commit");
  };
  assert_eq!((branch_name.as_str(), whole_stack), ("login", false));
  assert_eq!(
    (remote_tip_of(&remote, AUTH), remote_tip_of(&remote, LOGIN)),
    (Some(auth.clone()), Some(theirs.clone())),
    "nothing was pushed"
  );
  assert_eq!(
    rev_parse(&local, &format!("refs/remotes/origin/{LOGIN}")),
    Some(theirs.clone()),
    "the branches were fetched"
  );
  let [auth_status, login_status] = remote_statuses.as_slice() else {
    panic!("{remote_statuses:?}");
  };
  assert_eq!(
    (auth_status.remote_tip.as_deref(), auth_status.unpushed_commits.as_slice()),
    (Some(auth.as_str()), [more_auth.clone()].as_slice())
  );
  assert_eq!((login_status.remote_tip.as_deref(), login_status.commits_behind), (Some(theirs.as_str()), 1));
  assert_eq!(dropped_subjects(login_status), ["Their fix"]);

  // the user saw their commit and pushes over it
  stack_pushed(push_stack(&local, [Some(&auth), Some(&theirs)], [None, None]));
  assert_eq!((remote_tip_of(&remote, AUTH), remote_tip_of(&remote, LOGIN)), (Some(more_auth), Some(more_login)));
}

/// Push Up to Here in a stack: `auth` goes whole, `login` up to the commit, in the same push
#[test]
fn a_stack_push_up_to_a_commit_pushes_the_branches_below_whole() {
  let (remote, local) = local_with_bare_remote();
  let (auth, first_login) = stack(&local);
  let second_login = local.create_commit("(auth/login) More login", "more-login.txt", "more");
  set_branch_of(&local, LOGIN, &second_login);

  let statuses = stack_pushed(push_stack(&local, [None, None], [None, Some(&first_login)]));
  assert_eq!((remote_tip_of(&remote, AUTH), remote_tip_of(&remote, LOGIN)), (Some(auth), Some(first_login)));
  assert_eq!(statuses[1].unpushed_commits, [second_login]);
}

/// An unapplied branch is pushed under its own name, `{prefix}/unapplied/<name>`, leased as any other: the remote keeps a
/// copy, the remote branch of its virtual branch stays as it was
#[test]
fn an_unapplied_branch_is_pushed_under_its_own_name() {
  let (remote, local) = local_with_bare_remote();
  let tip = local.create_commit("Add docs", "docs.md", "docs");
  set_branch_of(&local, "prefix/unapplied/docs", &tip);
  let params = |expected: Option<&str>| PushBranchesParams {
    repository_path: path(&local).to_string(),
    branch_prefix: "prefix".to_string(),
    my_email: None,
    baseline_branch: "origin/master".to_string(),
    branches: vec![StackPushBranch {
      branch_name: "docs".to_string(),
      parent_branch: None,
      expected_remote: expected.map(str::to_string),
      tip: None,
      whole_stack: false,
      unapplied: true,
    }],
  };

  let statuses = stack_pushed_as_is(&local, &params(None));
  assert_eq!(remote_tip_of(&remote, "prefix/unapplied/docs"), Some(tip.clone()));
  assert_eq!(remote_tip_of(&remote, "prefix/virtual/docs"), None);
  assert_eq!(
    (statuses[0].branch_name.as_str(), statuses[0].remote_tip.as_deref(), statuses[0].unpushed_commits.len()),
    ("docs", Some(tip.as_str()), 0)
  );

  // a second push without the lease of the remote branch is refused: someone may have pushed it since
  let more = local.create_commit("More docs", "more.md", "more");
  set_branch_of(&local, "prefix/unapplied/docs", &more);
  assert!(matches!(
    push_branches(local.git_executor(), &params(None), &HashMap::new(), &mut |_| {}).unwrap(),
    PushBranchesOutcome::RemoteMoved { .. }
  ));
  stack_pushed_as_is(&local, &params(Some(&tip)));
  assert_eq!(remote_tip_of(&remote, "prefix/unapplied/docs"), Some(more));
}
