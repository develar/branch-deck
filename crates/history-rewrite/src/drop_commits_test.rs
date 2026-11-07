use crate::drop_commits::{DropCommitsParams, DropSelectionRefused, DropUndoToken, drop_commits_core, undo_drop_core};
use crate::repository_lock::{LockWait, RewriteOptions};
use anyhow::Result;
use domain_types::rewrite::{DropCommitsResult, RemovedCommit, UndoneDrop};
use git_ops::rewrite_guard::HeadMoved;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, baseline_repo};

fn params(repo: &TestRepo, commits: &[&str], dry_run: bool) -> DropCommitsParams {
  DropCommitsParams {
    repository_path: repo.path_str().to_string(),
    commit_ids: commits.iter().map(|id| (*id).to_string()).collect(),
    dry_run,
  }
}

fn drop(repo: &TestRepo, commits: &[&str], dry_run: bool) -> Result<DropCommitsResult> {
  drop_commits_core(repo.git_executor(), &params(repo, commits, dry_run), &RewriteOptions::default())
}

fn subjects(repo: &TestRepo) -> String {
  repo.git(&["log", "--format=%s"])
}

/// `Initial commit`, `(auth) Add a` (adds `a.txt`), `(ui) Add b`, `(auth) Add c`
fn repo_with_branches() -> (TestRepo, [String; 3]) {
  let repo = baseline_repo();
  let a = repo.create_commit("(auth) Add a", "a.txt", "a\n");
  let b = repo.create_commit("(ui) Add b", "b.txt", "b\n");
  let c = repo.create_commit("(auth) Add c", "c.txt", "c\n");
  (repo, [a, b, c])
}

#[test]
fn a_drop_takes_the_commits_and_their_files_out_and_names_what_the_next_sync_builds_anew() -> Result<()> {
  let (repo, [a, _, _]) = repo_with_branches();
  let head = repo.head();

  // the dry run says what the drop does and changes nothing
  let checked = drop(&repo, &[&a[..8]], true)?;
  assert_eq!(repo.head(), head);
  assert_eq!(repo.git(&["for-each-ref", "--format=%(refname)", "refs/branch-deck/"]), "");
  assert_eq!((checked.new_head.as_str(), checked.undo_token.as_deref()), (head.as_str(), None));

  let result = drop(&repo, &[&a[..8]], false)?;
  assert_eq!(subjects(&repo), "(auth) Add c\n(ui) Add b\nInitial commit");
  assert!(!repo.path().join("a.txt").exists());
  assert_eq!(repo.git(&["status", "--porcelain"]), "");
  let dropped = [RemovedCommit {
    commit: a,
    subject: "(auth) Add a".to_string(),
    branch: Some("auth".to_string()),
  }];
  assert_eq!(result.dropped, dropped);
  assert_eq!(checked.dropped, dropped);
  assert!(result.files_changed);
  // the later commits lost the file from their trees: both branches are built anew; auth keeps a commit
  assert_eq!(result.rebuilt_branches, ["auth", "ui"]);
  assert_eq!(result.emptied_branches, [] as [String; 0]);
  assert_eq!(result.previous_head, head);
  assert_eq!(result.new_head, repo.head());
  assert_eq!(result.undo_token, Some(format!("drop:{head}..{}", repo.head())));

  let token = DropUndoToken::parse(result.undo_token.as_deref().unwrap()).expect("a drop token");
  assert_eq!(
    undo_drop_core(repo.git_executor(), repo.path_str(), &token, LockWait::default())?,
    UndoneDrop { already_undone: false }
  );
  assert_eq!(repo.head(), head);
  assert!(repo.path().join("a.txt").exists());
  assert_eq!(
    undo_drop_core(repo.git_executor(), repo.path_str(), &token, LockWait::default())?,
    UndoneDrop { already_undone: true }
  );
  Ok(())
}

#[cfg(any(unix, windows))]
#[test]
fn dropping_a_commit_signs_the_replayed_survivors() {
  #[cfg(windows)]
  if sync_utils::child_process::windowless_command("ssh-keygen").arg("-?").output().is_err() {
    return;
  }

  let (repo, [dropped, ..]) = repo_with_branches();
  let base = repo.git(&["rev-parse", "HEAD~3"]);
  repo.enable_ssh_signing();

  let result = drop(&repo, &[&dropped], false).unwrap();

  assert_eq!(repo.git(&["log", "--format=%G?", &format!("{base}..{}", result.new_head)]).trim(), "G\nG");
  for commit in repo.git(&["rev-list", &format!("{base}..{}", result.new_head)]).lines() {
    repo.git(&["verify-commit", commit]);
  }
}

/// A commit and its revert leave HEAD's files as they are: no file moves, and the branches after the pair keep their
/// copies; a branch left without a commit is named
#[test]
fn a_commit_and_its_revert_drop_without_a_file_moving_and_empty_their_branch() -> Result<()> {
  let repo = baseline_repo();
  let a = repo.create_commit("(scratch) Add a", "a.txt", "a\n");
  repo.create_commit("(ui) Add b", "b.txt", "b\n");
  repo.git(&["revert", "--no-edit", &a]);
  let revert = repo.head();
  repo.create_commit("(docs) Add d", "d.txt", "d\n");
  let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);

  let result = drop(&repo, &[&revert, &a], false)?;
  assert_eq!(subjects(&repo), "(docs) Add d\n(ui) Add b\nInitial commit");
  assert_eq!(repo.git(&["rev-parse", "HEAD^{tree}"]), tree);
  assert!(!result.files_changed);
  assert_eq!(
    result.dropped.iter().map(|commit| commit.commit.as_str()).collect::<Vec<_>>(),
    [a.as_str(), revert.as_str()]
  );
  // the commit between the pair lost the file from its trees, the one after it kept them
  assert_eq!(result.rebuilt_branches, ["scratch", "ui"]);
  assert_eq!(result.emptied_branches, ["scratch"]);
  Ok(())
}

#[test]
fn a_drop_refuses_commits_that_are_not_ahead_and_another_head() {
  let (repo, [a, ..]) = repo_with_branches();
  let error = drop(&repo, &["0000000"], false).unwrap_err();
  assert_eq!(
    error.downcast_ref::<DropSelectionRefused>(),
    Some(&DropSelectionRefused::NotAhead(vec!["0000000".to_string()]))
  );
  let error = drop(&repo, &[], false).unwrap_err();
  assert_eq!(error.downcast_ref::<DropSelectionRefused>(), Some(&DropSelectionRefused::NoCommit));

  let options = RewriteOptions {
    expect_head: Some(a.clone()),
    ..RewriteOptions::default()
  };
  let error = drop_commits_core(repo.git_executor(), &params(&repo, &[&a], false), &options).unwrap_err();
  assert!(error.downcast_ref::<HeadMoved>().is_some(), "{error:#}");
}

#[test]
fn a_drop_token_names_two_full_ids() {
  let previous = "1".repeat(40);
  let new = "2".repeat(40);
  let token = DropUndoToken::parse(&format!("drop:{previous}..{new}")).expect("a token");
  assert_eq!(token.to_string(), format!("drop:{previous}..{new}"));
  assert_eq!(DropUndoToken::parse("drop:1111..2222"), None);
  assert_eq!(DropUndoToken::parse(&format!("drop:{previous}..{previous}")), None);
  assert_eq!(DropUndoToken::parse(&format!("{previous}..{new}")), None);
  assert!(DropUndoToken::is_drop_token("drop:x"));
}

/// A drop and its undo read and compute before the repository lock (the commits ahead, the grouping, the replay, the
/// count of the new history, the dry run of the files; the reflog and the dry run of the undo); under it only the
/// witness of HEAD and the guarded move of HEAD and the files
#[cfg(unix)]
#[test]
fn a_drop_and_its_undo_lock_only_the_witness_and_the_move() -> Result<()> {
  let (repo, [a, ..]) = repo_with_branches();
  let span = test_utils::lock_span::LockSpan::new(&repo.path().join(".git"));
  let result = drop_commits_core(span.executor(), &params(&repo, &[&a], false), &RewriteOptions::default())?;
  assert_eq!(subjects(&repo), "(auth) Add c\n(ui) Add b\nInitial commit");
  assert_eq!(span.commands_under_the_lock(), MOVE_UNDER_THE_LOCK, "{:#?}", span.commands());
  span.assert_before_the_lock(&["replay", "log"]);

  let token = DropUndoToken::parse(result.undo_token.as_deref().unwrap()).expect("a drop token");
  let undo_span = test_utils::lock_span::LockSpan::new(&repo.path().join(".git"));
  let undone = undo_drop_core(undo_span.executor(), repo.path_str(), &token, LockWait::default())?;
  assert_eq!((undone, repo.head()), (UndoneDrop { already_undone: false }, result.previous_head));
  assert_eq!(undo_span.commands_under_the_lock(), MOVE_UNDER_THE_LOCK, "{:#?}", undo_span.commands());
  undo_span.assert_before_the_lock(&["log"]);
  Ok(())
}

/// What a move that the files follow runs under the repository lock: the witness (`for-each-ref`), then the guarded
/// move (the transaction with the mark, the check of the checked out branch, the files, the index, the mark deleted)
#[cfg(unix)]
const MOVE_UNDER_THE_LOCK: [&str; 5] = ["for-each-ref", "read-tree", "symbolic-ref", "update-index", "update-ref"];
