use crate::regroup_test::{commits, path, repo, sync};
use crate::unapply_branch::{UnapplyBranchParams, UnapplyBranchResult, UnapplyRefusal, unapply_branch_core};
use anyhow::Result;
use git_ops::unapply::MoveBlocked;
use history_rewrite::repository_lock::RewriteOptions;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const PREFIX: &str = "test-prefix";
const VIRTUAL: &str = "test-prefix/virtual/feature";

/// Unapplies `branch` (branch prefix `test-prefix`), checked against `expected` when given
fn unapply(repo: &TestRepo, branch: &str, expected: Option<Vec<String>>) -> Result<UnapplyBranchResult> {
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
