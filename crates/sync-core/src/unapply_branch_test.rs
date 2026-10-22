use crate::regroup_test::{commits, path, repo, sync};
use crate::sync::{SyncOptions, sync_branches};
use crate::unapply_branch::{UnapplyBranchParams, UnapplyBranchResult, UnapplyRefusal, UnapplyUndoToken, UndoneUnapply, unapply_branch_core, undo_unapply_core};
use crate::update_project::UndoBlocked;
use anyhow::Result;
use domain_types::{SyncEvent, UnappliedBranchInfo};
use git_ops::rewrite_guard::HeadMoved;
use git_ops::unapply::MoveBlocked;
use history_rewrite::repository_lock::{DEFAULT_LOCK_TIMEOUT, RewriteOptions};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use test_utils::sync_fixtures::TestReporter;

pub(crate) const PREFIX: &str = "test-prefix";
const VIRTUAL: &str = "test-prefix/virtual/feature";

/// Unapplies `branch` (branch prefix `test-prefix`), checked against `expected` when given
pub(crate) fn unapply(repo: &TestRepo, branch: &str, expected: Option<Vec<String>>) -> Result<UnapplyBranchResult> {
  let params = UnapplyBranchParams {
    repository_path: path(repo).to_string(),
    branch_name: branch.to_string(),
    branch_prefix: PREFIX.to_string(),
    original_commit_ids: expected,
  };
  unapply_branch_core(repo.git_executor(), params, &RewriteOptions::default())
}

fn refusal(repo: &TestRepo, branch: &str, expected: Option<Vec<String>>) -> UnapplyRefusal {
  let error = unapply(repo, branch, expected).expect_err("unapply should be refused");
  error.downcast_ref::<UnapplyRefusal>().cloned().unwrap_or_else(|| panic!("not a refusal: {error:#}"))
}

/// `(feature) Add file0`, `(other) Add file1`, `(feature) Add file2`, `(different) Add file3`, synced; returns them
fn synced_repo() -> (TestRepo, Vec<String>) {
  let repo = repo(false);
  let ids = commits(&repo, &["(feature) Add file0", "(other) Add file1", "(feature) Add file2", "(different) Add file3"]);
  sync(&repo);
  (repo, ids)
}

/// `git status --porcelain` including untracked files, without trimming the leading status column
fn status(repo: &TestRepo) -> String {
  let output = repo.git_executor().cmd(&["status", "--porcelain"], repo.path_str()).bytes().unwrap();
  String::from_utf8_lossy(&output).trim_end().to_string()
}

fn refs(repo: &TestRepo) -> String {
  repo.git(&["for-each-ref", "--format=%(refname) %(objectname)"])
}

fn subjects(repo: &TestRepo) -> Vec<String> {
  repo.git(&["log", "--reverse", "--format=%s"]).lines().map(str::to_string).collect()
}

#[test]
fn unapply_takes_the_commits_of_the_branch_out_of_head_with_their_files() -> Result<()> {
  let (repo, ids) = synced_repo();
  let tip = repo.rev_parse(VIRTUAL).map_err(anyhow::Error::msg)?;

  let result = unapply(&repo, "feature", Some(vec![ids[2].clone(), ids[0].clone()]))?;

  assert_eq!(result.unapplied_branch_name, "test-prefix/unapplied/feature");
  assert_eq!(result.commits_removed, [ids[0].clone(), ids[2].clone()]);
  assert_eq!(subjects(&repo), ["Initial commit", "(other) Add file1", "(different) Add file3"]);
  assert!(!repo.path().join("file0.txt").exists());
  assert!(!repo.path().join("file2.txt").exists());
  assert_eq!(status(&repo), "");
  assert!(!repo.branch_exists(VIRTUAL));
  assert_eq!(repo.rev_parse(&result.unapplied_branch_name).map_err(anyhow::Error::msg)?, tip);
  assert_eq!(repo.git(&["log", "-g", "-1", "--format=%gs", "refs/heads/master"]), "branch-deck: unapply");

  // the next sync builds the other branches and leaves the unapplied one as it is
  sync(&repo);
  assert!(!repo.branch_exists(VIRTUAL));
  assert_eq!(repo.rev_parse(&result.unapplied_branch_name).map_err(anyhow::Error::msg)?, tip);
  Ok(())
}

#[test]
fn unapply_keeps_unrelated_work_staged_and_unstaged() -> Result<()> {
  let (repo, _) = synced_repo();
  std::fs::write(repo.path().join("README.md"), "staged edit")?;
  repo.git(&["add", "README.md"]);
  std::fs::write(repo.path().join("file1.txt"), "unstaged edit")?;
  std::fs::write(repo.path().join("notes.txt"), "untracked")?;

  unapply(&repo, "feature", None)?;

  assert_eq!(status(&repo), "M  README.md\n M file1.txt\n?? notes.txt");
  assert_eq!(repo.git(&["show", ":README.md"]), "staged edit");
  Ok(())
}

#[test]
fn unapply_refuses_a_local_change_on_a_path_it_writes_and_changes_nothing() -> Result<()> {
  let (repo, _) = synced_repo();
  std::fs::write(repo.path().join("file2.txt"), "unstaged over the dropped file")?;
  let refs_before = refs(&repo);

  let error = unapply(&repo, "feature", None).expect_err("unapply should be refused");
  let blocked = error.downcast_ref::<MoveBlocked>().unwrap_or_else(|| panic!("not blocked: {error:#}"));
  assert_eq!(blocked.paths, ["file2.txt"]);
  assert_eq!(refs(&repo), refs_before);

  // staged, the same
  repo.git(&["add", "file2.txt"]);
  let error = unapply(&repo, "feature", None).expect_err("unapply should be refused");
  assert!(error.downcast_ref::<MoveBlocked>().is_some(), "{error:#}");
  assert_eq!(refs(&repo), refs_before);
  assert_eq!(repo.git(&["show", ":file2.txt"]), "unstaged over the dropped file");
  Ok(())
}

#[test]
fn unapply_conflict_changes_nothing() -> Result<()> {
  let (repo, _) = synced_repo();
  // a later commit depends on file0.txt
  repo.create_commit("(other) Edit file0", "file0.txt", "edited");
  sync(&repo);
  std::fs::write(repo.path().join("README.md"), "work in progress")?;
  let refs_before = refs(&repo);
  let status_before = status(&repo);

  let error = unapply(&repo, "feature", None).expect_err("unapply should conflict");

  assert!(error.to_string().contains("conflicts with later commits"), "{error:#}");
  assert_eq!(refs(&repo), refs_before);
  assert_eq!(status(&repo), status_before);
  Ok(())
}

#[test]
fn unapply_refuses_an_unknown_or_changed_branch_one_not_built_and_one_checked_out() -> Result<()> {
  let (repo, ids) = synced_repo();
  assert_eq!(refusal(&repo, "ghost", None), UnapplyRefusal::UnknownBranch("ghost".to_string()));
  assert_eq!(refusal(&repo, "feature", Some(vec![ids[0].clone()])), UnapplyRefusal::BranchChanged("feature".to_string()));

  // a new commit that no sync copied yet
  repo.create_commit("(feature) Add file4", "file4.txt", "4");
  assert!(matches!(refusal(&repo, "feature", None), UnapplyRefusal::NotBuilt { reason, .. } if reason == "a commit has no copy"));
  sync(&repo);

  let other = tempfile::tempdir()?;
  let worktree = other.path().join("feature");
  repo.git(&["worktree", "add", "--quiet", worktree.to_str().unwrap(), VIRTUAL]);
  assert!(matches!(refusal(&repo, "feature", None), UnapplyRefusal::CheckedOut { branch, .. } if branch == VIRTUAL));
  repo.git(&["worktree", "remove", "--force", worktree.to_str().unwrap()]);

  repo.git(&["branch", "-D", VIRTUAL]);
  assert!(matches!(refusal(&repo, "feature", None), UnapplyRefusal::NotBuilt { reason, .. } if reason == "it does not exist"));
  Ok(())
}

#[test]
fn test_unapply_avoids_a_ref_below_the_name() -> Result<()> {
  let (repo, _) = synced_repo();
  // `test-prefix/unapplied/feature` is a directory, so no ref can have that name
  repo.create_branch_at("test-prefix/unapplied/feature/old", &repo.head()).map_err(anyhow::Error::msg)?;

  let result = unapply(&repo, "feature", None)?;

  assert_eq!(result.unapplied_branch_name, "test-prefix/unapplied/feature-1");
  assert!(!repo.branch_exists(VIRTUAL));
  Ok(())
}

#[test]
fn test_unapply_avoids_a_ref_above_the_name() -> Result<()> {
  let repo = repo(false);
  // a flat name with a slash, as without stacked branches
  commits(&repo, &["(feature/x) Add file1"]);
  sync(&repo);
  // `test-prefix/unapplied/feature` is a file, so `test-prefix/unapplied/feature/x` can't be created
  repo.create_branch_at("test-prefix/unapplied/feature", &repo.head()).map_err(anyhow::Error::msg)?;

  let result = unapply(&repo, "feature/x", None)?;

  assert_eq!(result.unapplied_branch_name, "test-prefix/unapplied/feature-1/x");
  assert!(!repo.branch_exists("test-prefix/virtual/feature/x"));
  Ok(())
}

#[test]
fn test_unapply_drops_the_commits_from_the_current_branch_of_a_clone() -> Result<()> {
  // with a remote, the baseline is `origin/master`: the commits must still leave the current branch, `master`
  let upstream = TestRepo::new();
  upstream.create_commit("Initial commit", "initial.txt", "initial content");
  let repo = TestRepo::new_empty();
  repo.clone_from(upstream.path()).map_err(anyhow::Error::msg)?;
  let kept = repo.create_commit("(other) Add other", "other.txt", "other");
  let dropped = repo.create_commit("(feature) Add file1", "file1.txt", "content 1");
  sync(&repo);

  unapply(&repo, "feature", None)?;

  let head_commits = repo.log(&["--format=%H", "master"]).map_err(anyhow::Error::msg)?;
  assert!(!head_commits.contains(&dropped), "the unapplied commit must leave master");
  assert!(head_commits.contains(&kept), "the other commits must stay");
  assert!(!repo.branch_exists("origin/master"), "no local branch named like the baseline");
  Ok(())
}

fn undo(repo: &TestRepo, token: &str) -> Result<UndoneUnapply> {
  undo_unapply_core(
    repo.git_executor(),
    path(repo),
    &UnapplyUndoToken::parse(token).expect("an unapply token"),
    DEFAULT_LOCK_TIMEOUT,
  )
}

#[test]
fn the_undo_of_an_unapply_brings_the_commits_their_files_and_the_virtual_branch_back() -> Result<()> {
  let (repo, _) = synced_repo();
  let (head, tip) = (repo.head(), repo.rev_parse(VIRTUAL).map_err(anyhow::Error::msg)?);
  let result = unapply(&repo, "feature", None)?;
  assert_eq!((result.previous_head.as_str(), result.new_head.as_str()), (head.as_str(), repo.head().as_str()));
  assert_eq!(
    UnapplyUndoToken::parse(&result.undo_token).map(|token| token.to_string()).as_deref(),
    Some(result.undo_token.as_str())
  );

  assert_eq!(undo(&repo, &result.undo_token)?, UndoneUnapply { already_undone: false });
  assert_eq!(repo.head(), head);
  assert_eq!(std::fs::read_to_string(repo.path().join("file2.txt"))?, "(feature) Add file2");
  assert_eq!(status(&repo), "");
  assert_eq!(repo.rev_parse(VIRTUAL).map_err(anyhow::Error::msg)?, tip);
  assert!(!repo.branch_exists(&result.unapplied_branch_name));
  // the next sync reuses the copies: the virtual branch stays where it was
  sync(&repo);
  assert_eq!(repo.rev_parse(VIRTUAL).map_err(anyhow::Error::msg)?, tip);

  // again: nothing to do, no error
  assert_eq!(undo(&repo, &result.undo_token)?, UndoneUnapply { already_undone: true });
  Ok(())
}

#[test]
fn the_undo_of_an_unapply_is_refused_after_a_new_commit_and_by_a_local_change_on_its_paths() -> Result<()> {
  let (repo, _) = synced_repo();
  let result = unapply(&repo, "feature", None)?;
  std::fs::write(repo.path().join("file0.txt"), "in the way")?;
  let error = undo(&repo, &result.undo_token).expect_err("blocked");
  assert_eq!(
    error.downcast_ref::<UndoBlocked>().map(|blocked| blocked.paths.clone()),
    Some(vec!["file0.txt".to_string()])
  );
  std::fs::remove_file(repo.path().join("file0.txt"))?;

  repo.create_commit("(other) More", "more.txt", "more");
  let error = undo(&repo, &result.undo_token).expect_err("stale");
  assert!(error.downcast_ref::<HeadMoved>().is_some(), "{error:#}");
  assert!(repo.branch_exists(&result.unapplied_branch_name));
  Ok(())
}

#[test]
fn an_unapply_token_names_full_ids_and_both_branches() {
  let id = |c: char| c.to_string().repeat(40);
  let text = format!("unapply:{}..{}:{}:jane/virtual/a/b:jane/unapplied/a-1/b", id('1'), id('2'), id('3'));
  let token = UnapplyUndoToken::parse(&text).unwrap();
  assert_eq!(
    (token.virtual_branch.as_str(), token.unapplied_branch.as_str()),
    ("jane/virtual/a/b", "jane/unapplied/a-1/b")
  );
  assert_eq!(token.to_string(), text);
  for bad in [
    format!("unapply:{}..{}:{}:jane/virtual/a", id('1'), id('2'), id('3')),
    format!("unapply:{}..{}:{}:jane/virtual/a:jane/unapplied/a", &id('1')[..12], id('2'), id('3')),
    format!("unapply:{}..{}:{}:jane/virtual/a:jane/unapplied/a", id('1'), id('1'), id('3')),
    format!("pick:{}..{}", id('1'), id('2')),
  ] {
    assert_eq!(UnapplyUndoToken::parse(&bad), None, "{bad}");
  }
}

/// The unapplied branches of a sync's `UnappliedBranchesFound`
fn unapplied_of_sync(repo: &TestRepo) -> Vec<UnappliedBranchInfo> {
  let progress = TestReporter::new();
  tokio::runtime::Runtime::new()
    .unwrap()
    .block_on(sync_branches(repo.git_executor(), path(repo), PREFIX, progress.clone(), SyncOptions::default()))
    .unwrap();
  let found: Vec<Vec<UnappliedBranchInfo>> = progress
    .get_events()
    .into_iter()
    .filter_map(|event| match event {
      SyncEvent::UnappliedBranchesFound { branches } => Some(branches),
      _ => None,
    })
    .collect();
  assert_eq!(found.len(), 1, "one event per sync");
  found.into_iter().next().unwrap()
}

#[test]
fn a_sync_reports_the_unapplied_branches_with_the_remote_status_of_a_pushed_one() -> Result<()> {
  let (repo, _) = synced_repo();
  assert_eq!(unapplied_of_sync(&repo), [], "the event comes empty too, so the list clears");
  let result = unapply(&repo, "feature", None)?;
  let tip = repo.rev_parse(&result.unapplied_branch_name).map_err(anyhow::Error::msg)?;

  let found = unapplied_of_sync(&repo);
  assert_eq!(found.len(), 1);
  assert_eq!(
    (
      found[0].name.as_str(),
      found[0].branch.as_str(),
      found[0].tip.as_str(),
      found[0].checked_out,
      &found[0].remote
    ),
    ("test-prefix/unapplied/feature", "feature", tip.as_str(), false, &None)
  );

  // its remote branch, as a push leaves it
  repo.git(&["update-ref", "refs/remotes/origin/test-prefix/unapplied/feature", &tip]);
  let found = unapplied_of_sync(&repo);
  let remote = found[0].remote.as_ref().expect("the remote status of the pushed branch");
  assert_eq!(
    (remote.remote_exists, remote.remote_tip.as_deref(), remote.unpushed_commits.len()),
    (true, Some(tip.as_str()), 0)
  );
  Ok(())
}
