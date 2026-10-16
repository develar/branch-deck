use crate::robot_commits::robot_commit_map;
use crate::sync::{SyncOptions, sync_branches};
use pretty_assertions::assert_eq;
use sync_test_utils::TestReporter;
use sync_types::robot_failure::{RobotCommitMap, RobotCommitState};
use test_utils::git_test_utils::TestRepo;

use RobotCommitState::{Added, Changed, Landed, Rebased, Removed, Tested, Unknown};

/// Ten numbered lines with `changed` lines replaced: a change near another one shifts its context
fn lines(changed: &[(usize, &str)]) -> String {
  (1..=10)
    .map(|number| {
      changed
        .iter()
        .find(|(line, _)| *line == number)
        .map_or_else(|| format!("line {number}"), |(_, text)| (*text).to_string())
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// A commit authored (and committed) at `time`: the same subject and time make the same identity after a rewrite
fn commit(repo: &TestRepo, subject: &str, file: &str, content: &str, time: i64) -> String {
  repo.create_commit_with_timestamp(subject, file, content, Some(1_700_000_000 + time))
}

async fn sync(repo: &TestRepo) {
  sync_branches(repo.git_executor(), repo.path_str(), "test", TestReporter::new(), SyncOptions::default())
    .await
    .unwrap();
}

/// `master` with `shared.txt`, and the checked-out `feature` with a docs commit and two `(auth)` commits, synced
struct Setup {
  repo: TestRepo,
  add_token: String,
  fix_login: String,
}

async fn setup() -> Setup {
  let repo = TestRepo::new();
  commit(&repo, "Initial commit", "shared.txt", &lines(&[]), 900);
  repo.checkout_new_branch("feature").unwrap();
  commit(&repo, "(docs) Document", "docs.md", "docs", 999);
  let add_token = commit(&repo, "(auth) Add token", "auth.txt", "token", 1000);
  let fix_login = commit(&repo, "(auth) Fix login", "shared.txt", &lines(&[(5, "login fixed")]), 1001);
  sync(&repo).await;
  Setup { repo, add_token, fix_login }
}

fn virtual_tip(repo: &TestRepo, name: &str) -> String {
  repo.git(&["rev-parse", &format!("refs/heads/test/virtual/{name}")])
}

fn map_of(repo: &TestRepo, name: &str, source: &str, ancestors: &[&str]) -> RobotCommitMap {
  let ancestors: Vec<String> = ancestors.iter().map(|ancestor| (*ancestor).to_string()).collect();
  robot_commit_map(repo.git_executor(), repo.path_str(), "test", name, source, "master", &ancestors).unwrap()
}

/// `subject state local id` per commit, newest first
fn describe(map: &RobotCommitMap) -> Vec<(String, RobotCommitState, Option<String>)> {
  map.commits.iter().map(|commit| (commit.subject.clone(), commit.state, commit.local_id.clone())).collect()
}

fn row(subject: &str, state: RobotCommitState, local_id: Option<&str>) -> (String, RobotCommitState, Option<String>) {
  (subject.to_string(), state, local_id.map(str::to_string))
}

/// Rewrites `feature` as its commits on `onto` (a rebase, an amend or a drop, keeping the identities)
fn rebuild(repo: &TestRepo, onto: &str, commits: &[(&str, &str, &str, i64)]) -> Vec<String> {
  repo.git(&["reset", "--hard", onto]);
  commits.iter().map(|(subject, file, content, time)| commit(repo, subject, file, content, *time)).collect()
}

#[tokio::test]
async fn an_unchanged_branch_is_tested_by_its_local_commits() {
  let Setup { repo, add_token, fix_login } = setup().await;
  let source = virtual_tip(&repo, "auth");

  let map = map_of(&repo, "auth", &source, &[]);

  assert_eq!(
    describe(&map),
    [row("(auth) Fix login", Tested, Some(&fix_login)), row("(auth) Add token", Tested, Some(&add_token))]
  );
  assert!(!map.source_missing);
  assert!(!map.virtual_behind_head);
}

#[tokio::test]
async fn a_commit_after_the_robot_is_added() {
  let Setup { repo, add_token, fix_login } = setup().await;
  let source = virtual_tip(&repo, "auth");
  let rename = commit(&repo, "(auth) Rename option", "option.txt", "option", 1002);
  sync(&repo).await;

  assert_eq!(
    describe(&map_of(&repo, "auth", &source, &[])),
    [
      row("(auth) Rename option", Added, Some(&rename)),
      row("(auth) Fix login", Tested, Some(&fix_login)),
      row("(auth) Add token", Tested, Some(&add_token)),
    ]
  );
}

#[tokio::test]
async fn an_amended_commit_is_changed() {
  let Setup { repo, add_token, .. } = setup().await;
  let source = virtual_tip(&repo, "auth");
  let base = repo.git(&["rev-parse", "HEAD~1"]);
  let amended = rebuild(&repo, &base, &[("(auth) Fix login", "shared.txt", &lines(&[(5, "login fixed again")]), 1001)]);
  sync(&repo).await;

  assert_eq!(
    describe(&map_of(&repo, "auth", &source, &[])),
    [row("(auth) Fix login", Changed, Some(&amended[0])), row("(auth) Add token", Tested, Some(&add_token))]
  );
}

#[tokio::test]
async fn a_rebuild_on_a_baseline_that_moves_the_context_is_rebased() {
  let Setup { repo, .. } = setup().await;
  let source = virtual_tip(&repo, "auth");
  // master changes line 3, within the context of the change of line 5: the patch id with context differs
  repo.git(&["checkout", "master"]);
  let master = commit(&repo, "Change line 3", "shared.txt", &lines(&[(3, "master line")]), 1100);
  repo.git(&["checkout", "feature"]);
  let rebased = rebuild(
    &repo,
    &master,
    &[
      ("(docs) Document", "docs.md", "docs", 999),
      ("(auth) Add token", "auth.txt", "token", 1000),
      ("(auth) Fix login", "shared.txt", &lines(&[(3, "master line"), (5, "login fixed")]), 1001),
    ],
  );
  sync(&repo).await;

  let map = map_of(&repo, "auth", &source, &[]);

  assert_eq!(
    describe(&map),
    [
      row("(auth) Fix login", Rebased, Some(&rebased[2])),
      // another file: the same patch id with context
      row("(auth) Add token", Tested, Some(&rebased[1])),
    ]
  );
  assert!(!map.virtual_behind_head);
}

#[tokio::test]
async fn a_dropped_commit_is_removed() {
  let Setup { repo, .. } = setup().await;
  let source = virtual_tip(&repo, "auth");
  let base = repo.git(&["rev-parse", "HEAD~2"]);
  let rebuilt = rebuild(&repo, &base, &[("(auth) Fix login", "shared.txt", &lines(&[(5, "login fixed")]), 1001)]);
  sync(&repo).await;

  assert_eq!(
    describe(&map_of(&repo, "auth", &source, &[])),
    [row("(auth) Fix login", Tested, Some(&rebuilt[0])), row("Add token", Removed, None)]
  );
}

#[tokio::test]
async fn a_commit_in_the_baseline_landed() {
  let Setup { repo, .. } = setup().await;
  let source = virtual_tip(&repo, "auth");
  // a robot merged the virtual copy of "Add token": its author, author time and message without the prefix
  let landed_copy = repo.git(&["rev-parse", &format!("{source}~1")]);
  repo.git(&["checkout", "master"]);
  repo.git(&["cherry-pick", &landed_copy]);
  let master = repo.git(&["rev-parse", "HEAD"]);
  repo.git(&["checkout", "feature"]);
  let rebuilt = rebuild(
    &repo,
    &master,
    &[
      ("(docs) Document", "docs.md", "docs", 999),
      ("(auth) Fix login", "shared.txt", &lines(&[(5, "login fixed")]), 1001),
    ],
  );
  sync(&repo).await;

  assert_eq!(
    describe(&map_of(&repo, "auth", &source, &[])),
    [row("(auth) Fix login", Tested, Some(&rebuilt[1])), row("Add token", Landed, None)]
  );
}

#[tokio::test]
async fn the_commits_of_a_stack_parent_are_listed_with_their_branch() {
  let repo = TestRepo::new();
  commit(&repo, "Initial commit", "README.md", "# Test", 900);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  repo.checkout_new_branch("feature").unwrap();
  commit(&repo, "(auth) Add auth", "auth.rs", "// auth", 1000);
  commit(&repo, "(auth/login) Add form", "login.rs", "// login", 1001);
  sync(&repo).await;
  let source = virtual_tip(&repo, "login");
  // the parent's commit changes after the robot of the child built the whole stack
  let rebuilt = rebuild(
    &repo,
    "master",
    &[("(auth) Add auth", "auth.rs", "// auth v2", 1000), ("(auth/login) Add form", "login.rs", "// login", 1001)],
  );
  sync(&repo).await;

  let map = map_of(&repo, "login", &source, &["auth"]);
  assert_eq!(
    describe(&map),
    [row("(auth/login) Add form", Tested, Some(&rebuilt[1])), row("(auth) Add auth", Changed, Some(&rebuilt[0]))]
  );
  let branches: Vec<Option<&str>> = map.commits.iter().map(|commit| commit.branch.as_deref()).collect();
  assert_eq!(branches, [Some("login"), Some("auth")]);
  assert!(!map.virtual_behind_head);
}

#[tokio::test]
async fn a_missing_source_revision_lists_the_local_commits_as_unknown() {
  let Setup { repo, add_token, fix_login } = setup().await;

  let map = map_of(&repo, "auth", &"0".repeat(40), &[]);

  assert!(map.source_missing);
  assert_eq!(
    describe(&map),
    [row("(auth) Fix login", Unknown, Some(&fix_login)), row("(auth) Add token", Unknown, Some(&add_token))]
  );
}

#[tokio::test]
async fn local_commits_after_the_last_sync_are_reported() {
  let Setup { repo, add_token, fix_login } = setup().await;
  let source = virtual_tip(&repo, "auth");
  commit(&repo, "(auth) Rename option", "option.txt", "option", 1002);

  let map = map_of(&repo, "auth", &source, &[]);

  assert!(map.virtual_behind_head);
  assert_eq!(
    describe(&map),
    [row("(auth) Fix login", Tested, Some(&fix_login)), row("(auth) Add token", Tested, Some(&add_token))]
  );

  // amended and not synced: the virtual copy is the old version of the local commit
  let base = repo.git(&["rev-parse", "HEAD~2"]);
  let amended = rebuild(&repo, &base, &[("(auth) Fix login", "shared.txt", &lines(&[(5, "login fixed again")]), 1001)]);
  let map = map_of(&repo, "auth", &source, &[]);
  assert!(map.virtual_behind_head);
  assert_eq!(
    describe(&map),
    [row("(auth) Fix login", Changed, Some(&amended[0])), row("(auth) Add token", Tested, Some(&add_token))]
  );
}
