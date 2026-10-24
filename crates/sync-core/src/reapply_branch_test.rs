use crate::reapply_branch::{ReapplyBranchParams, ReapplyBranchResult, ReapplyRefusal, ReapplyUndoToken, UndoneReapply, reapply_branch_core, undo_reapply_core};
use crate::regroup_test::{commits, path, sync};
use crate::unapply_branch_test::{PREFIX, unapply};
use anyhow::Result;
use git_ops::rewrite_guard::HeadMoved;
use git_ops::unapply::MoveBlocked;
use history_rewrite::repository_lock::{DEFAULT_LOCK_TIMEOUT, LockWait, RewriteOptions, lock_repository};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, baseline_repo};

const UNAPPLIED: &str = "test-prefix/unapplied/feature";

/// `(feature) Add file0`, `(other) Add file1`, `(feature) Add file2`, synced, then `feature` unapplied
fn unapplied_repo() -> TestRepo {
  let repo = baseline_repo(false);
  commits(&repo, &["(feature) Add file0", "(other) Add file1", "(feature) Add file2"]);
  sync(&repo);
  unapply(&repo, "feature", None).unwrap();
  repo
}

fn reapply(repo: &TestRepo, name: &str, dry_run: bool) -> Result<ReapplyBranchResult> {
  let params = ReapplyBranchParams {
    repository_path: path(repo).to_string(),
    branch_name: name.to_string(),
    dry_run,
  };
  reapply_branch_core(repo.git_executor(), &params, PREFIX, &RewriteOptions::default())
}

fn refusal(repo: &TestRepo, name: &str) -> ReapplyRefusal {
  let error = reapply(repo, name, false).expect_err("reapply should be refused");
  error.downcast_ref::<ReapplyRefusal>().cloned().unwrap_or_else(|| panic!("not a refusal: {error:#}"))
}

fn subjects(repo: &TestRepo) -> Vec<String> {
  repo.git(&["log", "--reverse", "--format=%s"]).lines().map(str::to_string).collect()
}

fn refs(repo: &TestRepo) -> String {
  repo.git(&["for-each-ref", "--format=%(refname) %(objectname)"])
}

fn undo(repo: &TestRepo, token: &str) -> Result<UndoneReapply> {
  undo_reapply_core(
    repo.git_executor(),
    path(repo),
    &ReapplyUndoToken::parse(token).expect("a reapply token"),
    LockWait::default(),
  )
}

#[test]
fn reapply_puts_the_commits_back_on_top_with_their_prefix_and_files_and_the_sync_builds_the_branch_again() -> Result<()> {
  let repo = unapplied_repo();
  let head = repo.head();
  let author = repo.git(&["log", "-1", "--format=%an <%ae> %at", UNAPPLIED]);

  let result = reapply(&repo, "feature", false)?;
  assert_eq!((result.branch.as_str(), result.unapplied_branch_name.as_str()), ("feature", UNAPPLIED));
  assert_eq!(result.prefix_restored, 2);
  assert_eq!((result.previous_head.as_str(), result.new_head.as_str()), (head.as_str(), repo.head().as_str()));
  assert_eq!(
    result.reapplied.iter().map(|commit| commit.subject.as_str()).collect::<Vec<_>>(),
    ["(feature) Add file0", "(feature) Add file2"]
  );
  assert_eq!(subjects(&repo), ["Initial commit", "(other) Add file1", "(feature) Add file0", "(feature) Add file2"]);
  assert!(repo.path().join("file0.txt").exists() && repo.path().join("file2.txt").exists());
  assert_eq!(repo.git(&["status", "--porcelain"]), "");
  assert!(!repo.branch_exists(UNAPPLIED));
  assert_eq!(repo.git(&["log", "-1", "--format=%an <%ae> %at"]), author, "the author stays");
  assert_eq!(repo.git(&["log", "-g", "-1", "--format=%gs", "refs/heads/master"]), "branch-deck: reapply");

  sync(&repo);
  assert_eq!(
    repo.git(&["log", "--reverse", "--format=%s", "test-prefix/virtual/feature", "^master~2"]),
    "Add file0\nAdd file2"
  );
  Ok(())
}

#[test]
fn an_issue_branch_comes_back_without_a_prefix() -> Result<()> {
  let repo = baseline_repo(false);
  commits(&repo, &["ABC-1 Fix the parser", "(other) Add file1"]);
  sync(&repo);
  unapply(&repo, "ABC-1", None)?;
  let result = reapply(&repo, "ABC-1", false)?;
  assert_eq!(result.prefix_restored, 0);
  assert_eq!(subjects(&repo).last().map(String::as_str), Some("ABC-1 Fix the parser"));
  Ok(())
}

#[test]
fn a_dry_run_changes_nothing_and_says_what_the_reapply_would_do() -> Result<()> {
  let repo = unapplied_repo();
  let before = refs(&repo);
  let result = reapply(&repo, "feature", true)?;
  assert_eq!((result.reapplied.len(), result.undo_token.as_deref()), (2, None));
  assert_eq!(refs(&repo), before);
  assert!(!repo.path().join("file0.txt").exists());
  Ok(())
}

#[test]
fn reapply_refuses_an_unknown_branch_a_conflict_and_local_changes_in_the_way_and_changes_nothing() -> Result<()> {
  let repo = unapplied_repo();
  assert_eq!(refusal(&repo, "ghost"), ReapplyRefusal::UnknownBranch("test-prefix/unapplied/ghost".to_string()));

  std::fs::write(repo.path().join("file0.txt"), "in the way")?;
  let before = refs(&repo);
  let error = reapply(&repo, "feature", false).expect_err("blocked");
  assert_eq!(
    error.downcast_ref::<MoveBlocked>().map(|blocked| blocked.paths.clone()),
    Some(vec!["file0.txt".to_string()])
  );
  assert_eq!(refs(&repo), before);
  std::fs::remove_file(repo.path().join("file0.txt"))?;

  // HEAD wrote the file the branch adds, in another way
  repo.create_commit("(other) Own file2", "file2.txt", "theirs");
  let before = refs(&repo);
  assert!(matches!(refusal(&repo, "feature"), ReapplyRefusal::Conflict { files, .. } if files == ["file2.txt"]));
  assert_eq!(refs(&repo), before);
  Ok(())
}

#[test]
fn the_undo_of_a_reapply_takes_the_commits_out_again_and_keeps_the_unapplied_branch() -> Result<()> {
  let repo = unapplied_repo();
  let (head, tip) = (repo.head(), repo.rev_parse(UNAPPLIED).map_err(anyhow::Error::msg)?);
  let token = reapply(&repo, "feature", false)?.undo_token.expect("a token");
  assert_eq!(ReapplyUndoToken::parse(&token).map(|parsed| parsed.to_string()).as_deref(), Some(token.as_str()));

  assert_eq!(undo(&repo, &token)?, UndoneReapply { already_undone: false });
  assert_eq!(repo.head(), head);
  assert!(!repo.path().join("file0.txt").exists());
  assert_eq!(repo.rev_parse(UNAPPLIED).map_err(anyhow::Error::msg)?, tip);
  assert_eq!(undo(&repo, &token)?, UndoneReapply { already_undone: true });

  // a reapply again, then a commit: its undo is stale
  let token = reapply(&repo, "feature", false)?.undo_token.expect("a token");
  repo.create_commit("(other) More", "more.txt", "more");
  let error = undo(&repo, &token).expect_err("stale");
  assert!(error.downcast_ref::<HeadMoved>().is_some(), "{error:#}");
  Ok(())
}

#[test]
fn a_reapply_token_names_full_ids_and_the_unapplied_branch() {
  let id = |c: char| c.to_string().repeat(40);
  let text = format!("reapply:{}..{}:{}:jane/unapplied/a-1/b", id('1'), id('2'), id('3'));
  assert_eq!(ReapplyUndoToken::parse(&text).map(|token| token.to_string()), Some(text));
  for bad in [
    format!("reapply:{}..{}:{}", id('1'), id('2'), id('3')),
    format!("reapply:{}..{}:{}:jane/unapplied/a", &id('1')[..12], id('2'), id('3')),
    format!("unapply:{}..{}:{}:a:b", id('1'), id('2'), id('3')),
  ] {
    assert_eq!(ReapplyUndoToken::parse(&bad), None, "{bad}");
  }
}

#[test]
fn reapply_waits_for_the_repository_lock_and_fails_with_locked() {
  let repo = unapplied_repo();
  let _held = lock_repository(repo.git_executor(), path(&repo), DEFAULT_LOCK_TIMEOUT).unwrap();
  let options = RewriteOptions {
    lock_wait: LockWait::Bounded(std::time::Duration::from_millis(100)),
    ..RewriteOptions::default()
  };
  let params = ReapplyBranchParams {
    repository_path: path(&repo).to_string(),
    branch_name: "feature".to_string(),
    dry_run: false,
  };
  let error = reapply_branch_core(repo.git_executor(), &params, PREFIX, &options).expect_err("locked");
  assert!(error.downcast_ref::<history_rewrite::repository_lock::RepositoryLocked>().is_some(), "{error:#}");
  assert!(repo.branch_exists(UNAPPLIED));
}
