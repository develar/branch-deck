use crate::process_budget_test::sync_events;
use crate::remote_status::forget_statuses;
use crate::stack_branch::forget_builds;
use crate::sync::{SyncOptions, sync_branches};
use domain_types::stack::{BlockReason, BranchStack, ParentSource, ParentState, StackBranchConflict, StackBranchState, StackBranchUpdate, StackMemberBlock};
use domain_types::{BranchError, BranchSyncStatus, GroupedBranchInfo, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::empty_cache;
use test_utils::sync_fixtures::TestReporter;

fn repo_path(test_repo: &TestRepo) -> &str {
  test_repo.path().to_str().unwrap()
}

/// A repository with an initial commit and stacked branches on
fn stacked_repo() -> TestRepo {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.set_config("branchdeck.stacks", "true").unwrap();
  test_repo
}

/// Syncs with the `test` branch prefix and returns the events
async fn sync(test_repo: &TestRepo) -> Vec<SyncEvent> {
  let progress = TestReporter::new();
  sync_branches(test_repo.git_executor(), repo_path(test_repo), "test", progress.clone(), SyncOptions::default())
    .await
    .unwrap();
  progress.get_events()
}

fn grouped(events: &[SyncEvent]) -> BTreeMap<String, GroupedBranchInfo> {
  events
    .iter()
    .find_map(|event| match event {
      SyncEvent::BranchesGrouped { branches, .. } => Some(branches.iter().map(|branch| (branch.name.clone(), branch.clone())).collect()),
      _ => None,
    })
    .expect("BranchesGrouped is sent")
}

#[tokio::test]
async fn without_stacks_branches_grouped_has_no_stack() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(docs) Document", "docs.md", "docs");
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");

  let flat = grouped(&sync(&test_repo).await);

  assert_eq!(flat.keys().collect::<Vec<_>>(), ["auth/login", "docs"]);
  assert!(flat.values().all(|branch| branch.stack.is_none()));
  assert!(test_repo.branch_exists("test/virtual/auth/login"));
}

#[tokio::test]
async fn branches_grouped_carries_the_stack() {
  let test_repo = stacked_repo();
  test_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");

  let stacked = grouped(&sync(&test_repo).await);

  assert_eq!(stacked.keys().collect::<Vec<_>>(), ["auth", "login"]);
  assert_eq!(stacked["auth"].stack, Some(BranchStack::default()));
  assert_eq!(
    stacked["login"].stack,
    Some(BranchStack {
      declared_parent: Some("auth".to_string()),
      parent_source: Some(ParentSource::Prefix),
      parent_state: Some(ParentState::Active),
      effective_base: Some("auth".to_string()),
      ..BranchStack::default()
    })
  );
}

#[tokio::test]
async fn an_unapplied_parent_is_reported_before_the_sync_starts() {
  let test_repo = stacked_repo();
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  let head = test_repo.head();
  test_repo.create_branch_at("test/unapplied/auth", &head).unwrap();

  let branches = grouped(&sync(&test_repo).await);

  let stack = branches["login"].stack.clone().unwrap();
  assert_eq!(stack.parent_state, Some(ParentState::Unapplied));
  assert_eq!(stack.blocked_by, None);
  assert!(stack.pr_base_changed);
  // built on the baseline
  assert_eq!(rev(&test_repo, "test/virtual/login~1"), rev(&test_repo, "master~1"));
}

/// The final status of each branch and its error text
fn final_statuses(events: &[SyncEvent]) -> BTreeMap<String, (BranchSyncStatus, Option<String>)> {
  let mut statuses = BTreeMap::new();
  for event in events {
    if let SyncEvent::BranchStatusUpdate { branch_name, status, error } = event
      && *status != BranchSyncStatus::AnalyzingConflict
    {
      let error = error.as_ref().map(|error| match error {
        BranchError::Generic(text) => text.clone(),
        BranchError::MergeConflict(_) => "merge conflict".to_string(),
      });
      statuses.insert(branch_name.clone(), (*status, error));
    }
  }
  statuses
}

fn status(status: BranchSyncStatus) -> (BranchSyncStatus, Option<String>) {
  (status, None)
}

fn rev(test_repo: &TestRepo, revision: &str) -> String {
  test_repo.rev_parse(revision).unwrap()
}

#[tokio::test]
async fn builds_each_branch_on_its_parent() {
  let test_repo = stacked_repo();
  // the child's commit comes before every commit of its parent
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  test_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  test_repo.create_commit("(auth) Add session", "session.rs", "// session");
  test_repo.create_commit("(login/oauth) Add OAuth", "oauth.rs", "// oauth");
  test_repo.create_commit("(docs) Document", "docs.md", "docs");

  let events = sync(&test_repo).await;

  let statuses = final_statuses(&events);
  for branch in ["auth", "login", "oauth", "docs"] {
    assert_eq!(statuses[branch], status(BranchSyncStatus::Created), "{branch}");
  }
  assert_eq!(rev(&test_repo, "test/virtual/login~1"), rev(&test_repo, "test/virtual/auth"));
  assert_eq!(rev(&test_repo, "test/virtual/oauth~1"), rev(&test_repo, "test/virtual/login"));
  assert_eq!(rev(&test_repo, "test/virtual/docs~1"), rev(&test_repo, "master~5"));
  assert_eq!(
    test_repo.log(&["--format=%s", "master~5..test/virtual/oauth"]).unwrap().lines().collect::<Vec<_>>(),
    ["Add OAuth", "Add form", "Add session", "Add auth"]
  );
  assert!(grouped(&events)["login"].stack.as_ref().unwrap().child_precedes_parent);

  // nothing changed: every commit is reused
  let statuses_again = final_statuses(&sync(&test_repo).await);
  assert!(statuses_again.values().all(|(status, _)| *status == BranchSyncStatus::Unchanged), "{statuses_again:?}");

  // a new parent commit moves the children onto the parent's new head
  test_repo.create_commit("(auth) Add logout", "logout.rs", "// logout");
  let statuses = final_statuses(&sync(&test_repo).await);
  assert_eq!(statuses["auth"], status(BranchSyncStatus::Updated));
  assert_eq!(statuses["login"], status(BranchSyncStatus::Updated));
  assert_eq!(statuses["oauth"], status(BranchSyncStatus::Updated));
  assert_eq!(statuses["docs"], status(BranchSyncStatus::Unchanged));
  assert_eq!(rev(&test_repo, "test/virtual/login~1"), rev(&test_repo, "test/virtual/auth"));
  assert_eq!(rev(&test_repo, "test/virtual/oauth~1"), rev(&test_repo, "test/virtual/login"));
}

#[tokio::test]
async fn a_parent_conflict_blocks_its_descendants() {
  let test_repo = stacked_repo();
  // auth changes a file that only an unassigned commit creates, so its copy onto the base conflicts
  test_repo.create_commit("Add config", "config.txt", "a");
  test_repo.create_commit("(auth) Change config", "config.txt", "b");
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  test_repo.create_commit("(login/oauth) Add OAuth", "oauth.rs", "// oauth");
  test_repo.create_commit("(docs) Document", "docs.md", "docs");

  let events = sync(&test_repo).await;

  let statuses = final_statuses(&events);
  assert_eq!(statuses["auth"].0, BranchSyncStatus::MergeConflict);
  assert_eq!(
    statuses["login"],
    (BranchSyncStatus::Blocked, Some("Blocked: parent auth has a merge conflict".to_string()))
  );
  assert_eq!(statuses["oauth"], (BranchSyncStatus::Blocked, Some("Blocked: parent login is blocked".to_string())));
  assert_eq!(statuses["docs"], status(BranchSyncStatus::Created));
  assert!(!test_repo.branch_exists("test/virtual/login"));
  assert!(!test_repo.branch_exists("test/virtual/oauth"));
  let blocked: Vec<&str> = events
    .iter()
    .filter_map(|event| match event {
      SyncEvent::CommitsBlocked { branch_name, .. } => Some(branch_name.as_str()),
      _ => None,
    })
    .collect();
  assert!(blocked.contains(&"login") && blocked.contains(&"oauth"), "{blocked:?}");
}

#[tokio::test]
async fn a_problem_is_not_built_and_a_missing_parent_is_passed() {
  let test_repo = stacked_repo();
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  let looped = test_repo.create_commit("(a/a) Loop", "a.rs", "// a");
  test_repo.create_commit("(docs) Document", "docs.md", "docs");

  let statuses = final_statuses(&sync(&test_repo).await);

  assert_eq!(statuses["login"], status(BranchSyncStatus::Created));
  assert_eq!(
    statuses["a"],
    (
      BranchSyncStatus::Error,
      Some(format!("A prefix makes the branch its own parent: {}; fix: `bd unstack a`", &looped[..8]))
    )
  );
  assert_eq!(statuses["docs"], status(BranchSyncStatus::Created));
  assert_eq!(rev(&test_repo, "test/virtual/login~1"), rev(&test_repo, "master~3"));
  assert!(!test_repo.branch_exists("test/virtual/a"));
}

/// `stacked_repo` with a bare `origin` that has the baseline: (the repository, the remote)
fn stacked_repo_with_remote() -> (TestRepo, TestRepo) {
  let test_repo = stacked_repo();
  let remote = TestRepo::new_empty();
  remote.git_executor().execute_command(&["init", "--bare"], repo_path(&remote)).unwrap();
  test_repo.add_remote("origin", repo_path(&remote)).unwrap();
  test_repo.push("origin", "master").unwrap();
  (test_repo, remote)
}

#[tokio::test]
async fn the_unpushed_commits_of_a_child_exclude_its_parent() {
  let (test_repo, _remote) = stacked_repo_with_remote();
  test_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  sync(&test_repo).await;
  test_repo.push("origin", "test/virtual/login").unwrap();

  // the parent and the child get a commit each: login is copied onto the parent's new head
  test_repo.create_commit("(auth) Add session", "session.rs", "// session");
  test_repo.create_commit("(login) Validate form", "validate.rs", "// validate");
  let events = sync(&test_repo).await;

  let login = events
    .iter()
    .find_map(|event| match event {
      SyncEvent::RemoteStatusUpdate(update) if update.branch_name == "login" => Some(update.clone()),
      _ => None,
    })
    .expect("login has a remote status");
  assert!(login.remote_exists);
  // only the new commit is login's: the copy is patch-equivalent to the pushed one, the session commit is the parent's
  assert_eq!(login.unpushed_commits, [test_repo.rev_parse("test/virtual/login").unwrap()]);
  assert_eq!(login.my_unpushed_count, 1);
}

/// Syncs with the `test` branch prefix, expecting a failure: its error and the events sent before
async fn failed_sync(test_repo: &TestRepo) -> (anyhow::Error, Vec<SyncEvent>) {
  let progress = TestReporter::new();
  let error = sync_branches(test_repo.git_executor(), repo_path(test_repo), "test", progress.clone(), SyncOptions::default())
    .await
    .unwrap_err();
  (error, progress.get_events())
}

/// Stacks off in a repository with stacked prefixes: `(auth/login)` is the flat branch `auth/login`, whose ref
/// `test/virtual/auth/login` cannot exist beside `test/virtual/auth` (a directory/file conflict of refs), so the sync
/// fails. Which of the two fails depends on the order the branches are written in, the other one is written
#[tokio::test]
async fn without_stacks_a_parent_and_its_child_prefix_fail_the_sync() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");

  let (error, events) = failed_sync(&test_repo).await;

  assert_eq!(grouped(&events).keys().collect::<Vec<_>>(), ["auth", "auth/login"]);
  let message = format!("{error:#}");
  assert!(message.starts_with("Failed to sync branch auth"), "{message}");
  assert!(message.contains("refs/heads/test/virtual/auth"), "{message}");
  assert!(!(test_repo.branch_exists("test/virtual/auth") && test_repo.branch_exists("test/virtual/auth/login")));
}

/// Turning stacks off after a stacked sync: `auth` stays, `login` is archived, and the flat `auth/login` fails the sync
#[tokio::test]
async fn turning_stacks_off_fails_the_sync_on_the_flat_child() {
  let test_repo = stacked_repo();
  test_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  sync(&test_repo).await;
  test_repo.set_config("branchdeck.stacks", "false").unwrap();

  let (error, events) = failed_sync(&test_repo).await;

  let message = format!("{error:#}");
  assert!(message.starts_with("Failed to sync branch auth/login: Failed to update refs (sync)"), "{message}");
  assert!(
    message.contains("cannot lock ref 'refs/heads/test/virtual/auth/login': 'refs/heads/test/virtual/auth' exists"),
    "{message}"
  );
  assert_eq!(final_statuses(&events), BTreeMap::from([("auth".to_string(), status(BranchSyncStatus::Unchanged))]));
  assert!(test_repo.branch_exists("test/virtual/auth"));
  assert!(!test_repo.branch_exists("test/virtual/login"));
  assert_eq!(test_repo.list_branches("test/archived/*/login").unwrap().len(), 1);
}

/// The stack branch updates of a sync, by root
fn stack_updates(events: &[SyncEvent]) -> BTreeMap<String, StackBranchUpdate> {
  events
    .iter()
    .filter_map(|event| match event {
      SyncEvent::StackBranchUpdate(update) => Some((update.root.clone(), update.as_ref().clone())),
      _ => None,
    })
    .collect()
}

/// The roots of the stack branches `BranchesGrouped` announced
fn announced_stacks(events: &[SyncEvent]) -> Vec<String> {
  events
    .iter()
    .find_map(|event| match event {
      SyncEvent::BranchesGrouped { stack_branches, .. } => Some(stack_branches.clone()),
      _ => None,
    })
    .expect("BranchesGrouped is sent")
}

/// `auth` with two children, `login` and `session`, a commit of `docs` between them, and `oauth` on `login`; the
/// commits oldest first: auth, login, docs, session, oauth, auth
fn forking_repo() -> TestRepo {
  let test_repo = stacked_repo();
  test_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  test_repo.create_commit("(docs) Document", "docs.md", "docs");
  test_repo.create_commit("(auth/session) Add session", "session.rs", "// session");
  test_repo.create_commit("(login/oauth) Add OAuth", "oauth.rs", "// oauth");
  test_repo.create_commit("(auth) Add logout", "logout.rs", "// logout");
  test_repo
}

#[tokio::test]
async fn a_forking_stack_builds_its_commits_in_history_order_as_one_branch() {
  let test_repo = forking_repo();

  let progress = TestReporter::new();
  let report = sync_branches(test_repo.git_executor(), repo_path(&test_repo), "test", progress.clone(), SyncOptions::default())
    .await
    .unwrap();
  let events = progress.get_events();

  assert_eq!(announced_stacks(&events), ["auth"]);
  let tip = rev(&test_repo, "test/stack/auth");
  let update = &stack_updates(&events)["auth"];
  assert_eq!(
    (
      update.branch_name.as_str(),
      update.members.as_slice(),
      update.commit_count,
      update.state,
      update.tip.as_deref()
    ),
    (
      "test/stack/auth",
      &["auth".to_string(), "login".to_string(), "session".to_string(), "oauth".to_string()][..],
      5,
      StackBranchState::Built,
      Some(tip.as_str())
    )
  );
  assert_eq!(report.stack_branches, std::slice::from_ref(update));
  // on the base of the sync, the commits of every member in the order of the history, the prefixes stripped
  assert_eq!(rev(&test_repo, "test/stack/auth~5"), rev(&test_repo, "master~6"));
  assert_eq!(
    test_repo.log(&["--format=%s", "master~6..test/stack/auth"]).unwrap().lines().collect::<Vec<_>>(),
    ["Add logout", "Add OAuth", "Add session", "Add form", "Add auth"]
  );
  let mut files = test_repo
    .git(&["ls-tree", "--name-only", "test/stack/auth"])
    .lines()
    .map(str::to_string)
    .collect::<Vec<_>>();
  files.sort_unstable();
  assert_eq!(files, ["README.md", "auth.rs", "login.rs", "logout.rs", "oauth.rs", "session.rs"]);
  // a copy is the one a virtual branch of the same commits on the same parent gets, the same commit
  assert_eq!(rev(&test_repo, "test/stack/auth~4"), rev(&test_repo, "test/virtual/auth~1"));
  assert_eq!(test_repo.log(&["-1", "--format=%cn", "test/stack/auth"]).unwrap().trim(), "branch-deck");
}

#[tokio::test]
async fn the_stack_branch_writes_no_mapping_notes_and_sends_no_branch_events() {
  let test_repo = forking_repo();

  let events = sync(&test_repo).await;

  // one note per original, naming the copy in the virtual branch of its own branch
  let branch_of = |subject: &str| match subject {
    "(auth) Add auth" | "(auth) Add logout" => "auth",
    "(auth/login) Add form" => "login",
    "(docs) Document" => "docs",
    "(auth/session) Add session" => "session",
    _ => "oauth",
  };
  let originals = test_repo.log(&["--format=%H %s", "master~6..master"]).unwrap();
  for line in originals.lines() {
    let (original, subject) = line.split_once(' ').unwrap();
    let note = test_repo.show_note(original).unwrap();
    let copy = note.lines().find_map(|line| line.strip_prefix("v-commit-v1:")).expect("a mapping note").trim().to_string();
    let virtual_branch = format!("test/virtual/{}", branch_of(subject));
    let status = test_utils::git_test_utils::git_command()
      .args(["merge-base", "--is-ancestor", &copy, &virtual_branch])
      .current_dir(test_repo.path())
      .status()
      .unwrap();
    assert!(status.success(), "the note of {subject} names {copy}, a commit of {virtual_branch}");
  }
  // every branch event names a grouped branch; the stack branch has its own event only
  let grouped = grouped(&events);
  for event in &events {
    let branch = match event {
      SyncEvent::CommitSynced { branch_name, .. }
      | SyncEvent::CommitError { branch_name, .. }
      | SyncEvent::CommitsBlocked { branch_name, .. }
      | SyncEvent::BranchStatusUpdate { branch_name, .. } => branch_name,
      SyncEvent::RemoteStatusUpdate(update) => &update.branch_name,
      _ => continue,
    };
    assert!(grouped.contains_key(branch), "{event:?}");
  }
  assert_eq!(events.iter().filter(|event| matches!(event, SyncEvent::StackBranchUpdate(_))).count(), 1);
  assert_eq!(
    final_statuses(&events).values().map(|(status, _)| *status).collect::<Vec<_>>(),
    [BranchSyncStatus::Created; 5]
  );
}

/// The subcommands `git` counted
fn subcommands(git: &GitCommandExecutor) -> Vec<String> {
  git.process_stats().unwrap().by_command().into_keys().collect()
}

/// The build of the same commits on the same base is remembered: by the process, and by the object cache for a new
/// process; without either the stack branch is built again, to the same commit, and the ref doesn't move
#[tokio::test]
async fn an_unchanged_stack_reuses_its_stack_branch_without_a_git_process() {
  let test_repo = forking_repo();
  let (_dir, cache) = empty_cache();
  sync_events(&GitCommandExecutor::new(), &test_repo, &cache).await;
  let tip = rev(&test_repo, "test/stack/auth");

  for new_process in [false, true] {
    if new_process {
      forget_builds();
      forget_statuses();
    }
    let git = GitCommandExecutor::new().with_process_stats("test");
    let events = sync_events(&git, &test_repo, &cache).await;
    let update = &stack_updates(&events)["auth"];
    assert_eq!((update.state, update.tip.as_deref()), (StackBranchState::Built, Some(tip.as_str())));
    let counted = subcommands(&git);
    for writer in ["fast-import", "merge-tree", "update-ref"] {
      assert!(!counted.contains(&writer.to_string()), "{writer} in {counted:?}");
    }
  }

  // a remembered tip counts only while the ref is at it: a deleted ref (its commits may be pruned since) is built again
  test_repo.git(&["update-ref", "-d", "refs/heads/test/stack/auth"]);
  let git = GitCommandExecutor::new().with_process_stats("test");
  let events = sync_events(&git, &test_repo, &cache).await;
  assert_eq!(stack_updates(&events)["auth"].tip.as_deref(), Some(tip.as_str()));
  assert_eq!(rev(&test_repo, "test/stack/auth"), tip);
  assert!(subcommands(&git).contains(&"fast-import".to_string()));

  // a new process without the object cache builds it again, the same commit
  forget_builds();
  let git = GitCommandExecutor::new().with_process_stats("test");
  let events = sync_events(&git, &test_repo, &ObjectCache::disabled()).await;
  assert_eq!(stack_updates(&events)["auth"].tip.as_deref(), Some(tip.as_str()));
  assert_eq!(rev(&test_repo, "test/stack/auth"), tip);
  let counted = subcommands(&git);
  assert!(counted.contains(&"fast-import".to_string()) && !counted.contains(&"update-ref".to_string()), "{counted:?}");

  // a new commit of a member rebuilds it
  test_repo.create_commit("(session) Expire sessions", "expire.rs", "// expire");
  let update = stack_updates(&sync_events(&GitCommandExecutor::new(), &test_repo, &cache).await)["auth"].clone();
  assert_eq!(update.state, StackBranchState::Built);
  assert_ne!(update.tip.as_deref(), Some(tip.as_str()));
  assert_eq!(rev(&test_repo, "test/stack/auth~1"), tip);
  assert_eq!(update.commit_count, 6);
}

/// A commit of the stack that conflicts with the commits of the stack before it keeps the stack branch from being
/// built: the ref keeps the tip of the last build, which is stale, and the update names the commit and its files. The
/// member it belongs to conflicts too; the stack branch's own conflict is what it says
#[tokio::test]
async fn a_conflict_keeps_the_old_ref_stale_and_says_why() {
  let test_repo = forking_repo();
  sync(&test_repo).await;
  let tip = rev(&test_repo, "test/stack/auth");

  // `session` changes a file that only an unassigned commit creates
  test_repo.create_commit("Add config", "config.txt", "a");
  let changed = test_repo.create_commit("(session) Change config", "config.txt", "b");
  let events = sync(&test_repo).await;

  assert_eq!(final_statuses(&events)["session"].0, BranchSyncStatus::MergeConflict);
  let update = &stack_updates(&events)["auth"];
  assert_eq!(update.state, StackBranchState::Conflict);
  assert_eq!(
    update.conflict,
    Some(StackBranchConflict {
      commit: changed,
      files: vec!["config.txt".to_string()],
    })
  );
  assert_eq!((update.tip.as_deref(), update.commit_count), (Some(tip.as_str()), 6));
  assert_eq!(rev(&test_repo, "test/stack/auth"), tip);
}

/// A member that can't be built blocks the stack branch, though its commits build in the order of the history: here the
/// child's commit creates the file its parent's later commit changes, so the parent conflicts on its own. The ref is
/// not written, and the update names the member and why
#[tokio::test]
async fn a_member_that_fails_blocks_the_stack_branch() {
  let test_repo = stacked_repo();
  test_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  test_repo.create_commit("(auth/login) Add config", "config.txt", "a");
  test_repo.create_commit("(auth) Change config", "config.txt", "b");
  test_repo.create_commit("(auth/session) Add session", "session.rs", "// session");

  let events = sync(&test_repo).await;

  assert_eq!(final_statuses(&events)["auth"].0, BranchSyncStatus::MergeConflict);
  let update = &stack_updates(&events)["auth"];
  assert_eq!((update.state, update.tip.as_deref()), (StackBranchState::Blocked, None));
  assert_eq!(
    update.blocked_by,
    Some(StackMemberBlock {
      member: "auth".to_string(),
      reason: BlockReason::Conflict,
    })
  );
  assert!(!test_repo.branch_exists("test/stack/auth"));
}

/// A stack that stops forking loses its stack branch, which a delete with compare-and-swap removes; one with a remote
/// branch stays, stale and without members, for the user to decide on
#[tokio::test]
async fn a_stack_that_no_longer_forks_loses_its_stack_branch_unless_it_is_on_the_remote() {
  let (test_repo, _remote) = stacked_repo_with_remote();
  test_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  let straight = test_repo.head();
  test_repo.create_commit("(auth/session) Add session", "session.rs", "// session");
  let forking = test_repo.head();
  sync(&test_repo).await;
  assert!(test_repo.branch_exists("test/stack/auth"));

  // without a remote branch: deleted
  test_repo.reset_hard(&straight).unwrap();
  let events = sync(&test_repo).await;
  assert!(!test_repo.branch_exists("test/stack/auth"));
  assert_eq!(announced_stacks(&events), Vec::<String>::new());
  assert!(stack_updates(&events).is_empty());

  // with one: kept
  test_repo.reset_hard(&forking).unwrap();
  sync(&test_repo).await;
  let pushed = rev(&test_repo, "test/stack/auth");
  test_repo.push("origin", "test/stack/auth").unwrap();
  test_repo.reset_hard(&straight).unwrap();
  let events = sync(&test_repo).await;
  assert_eq!(rev(&test_repo, "test/stack/auth"), pushed);
  assert_eq!(announced_stacks(&events), ["auth"]);
  let update = &stack_updates(&events)["auth"];
  assert_eq!(
    (update.state, update.tip.as_deref(), update.members.len()),
    (StackBranchState::Stale, Some(pushed.as_str()), 0)
  );
  assert_eq!(
    update.error.as_deref(),
    Some("Not rebuilt: auth heads no stack that forks; the ref stays for its remote branch")
  );
  let remote_status = update.remote.as_ref().expect("its remote status");
  assert!(remote_status.remote_exists && remote_status.unpushed_commits.is_empty());
}

#[tokio::test]
async fn a_straight_stack_has_none() {
  let test_repo = stacked_repo();
  test_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  test_repo.create_commit("(login/oauth) Add OAuth", "oauth.rs", "// oauth");
  test_repo.create_commit("(docs) Document", "docs.md", "docs");

  let events = sync(&test_repo).await;

  assert_eq!(announced_stacks(&events), Vec::<String>::new());
  assert!(stack_updates(&events).is_empty());
  assert_eq!(test_repo.list_branches("test/stack/*").unwrap(), Vec::<String>::new());
}
