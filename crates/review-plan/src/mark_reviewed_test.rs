use crate::mark_reviewed::{MarkReviewedParams, NoCommonHistory, UnknownReviewBase, UnknownReviewBranch, mark_reviewed_core};
use git_ops::reviewed_refs::{REVIEWED_REFS, read_reviewed_pair};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const PREFIX: &str = "jane";

/// `master` at `base`, the virtual branch `jane/virtual/auth` one commit on top of it, and the local branch `auth` at
/// `base`, a name the virtual branch shadows
fn repo() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  let base = repo.create_commit("Add readme", "README.md", "readme\n");
  repo.git(&["branch", "auth"]);
  repo.git(&["switch", "-q", "-c", "jane/virtual/auth"]);
  let tip = repo.create_commit("(auth) Add login", "login.rs", "// login\n");
  repo.git(&["switch", "-q", "master"]);
  (repo, base, tip)
}

fn params(repo: &TestRepo, branch: &str) -> MarkReviewedParams {
  MarkReviewedParams {
    repository_path: repo.path_str().to_string(),
    branch: branch.to_string(),
    base: None,
    baseline_branch: Some("master".to_string()),
  }
}

#[test]
fn marks_the_virtual_branch_of_a_name_at_its_merge_base_with_the_baseline() {
  let (repo, base, tip) = repo();
  let marked = mark_reviewed_core(repo.git_executor(), PREFIX, &params(&repo, "auth")).unwrap();
  assert_eq!((marked.branch.as_str(), &marked.base, &marked.tip), ("jane/virtual/auth", &base, &tip));
  assert_eq!((marked.previous, marked.changed, marked.pruned), (None, true, 0));
  let pair = read_reviewed_pair(repo.git_executor(), repo.path_str(), "jane/virtual/auth").unwrap().unwrap();
  assert_eq!((pair.base, pair.tip), (base.clone(), tip.clone()));

  // the local branch by its full ref; the same mark again changes nothing
  let local = mark_reviewed_core(repo.git_executor(), PREFIX, &params(&repo, "refs/heads/auth")).unwrap();
  assert_eq!((local.branch.as_str(), &local.base, &local.tip), ("auth", &base, &base));
  let again = mark_reviewed_core(repo.git_executor(), PREFIX, &params(&repo, "auth")).unwrap();
  assert!(!again.changed);
  assert_eq!(again.previous.map(|pair| pair.tip), Some(tip));
}

#[test]
fn a_branch_stacked_on_another_is_marked_on_the_base_its_caller_names() {
  let (repo, base, parent_tip) = repo();
  repo.git(&["switch", "-q", "-c", "jane/virtual/ui", "jane/virtual/auth"]);
  let tip = repo.create_commit("(ui) Add form", "form.rs", "// form\n");
  repo.git(&["switch", "-q", "master"]);

  // the parent by its name, as the branch: its virtual branch, not the local `auth` at the baseline
  let stacked = MarkReviewedParams {
    base: Some("auth".to_string()),
    ..params(&repo, "ui")
  };
  let marked = mark_reviewed_core(repo.git_executor(), PREFIX, &stacked).unwrap();
  assert_eq!((&marked.base, &marked.tip), (&parent_tip, &tip));

  // without it, the baseline: the commits of the parent are reviewed too
  let whole = mark_reviewed_core(repo.git_executor(), PREFIX, &params(&repo, "ui")).unwrap();
  assert_eq!((&whole.base, whole.previous.map(|pair| pair.base)), (&base, Some(parent_tip)));
}

/// A base that names no commit, or one with no history in common with the tip, fails before the transaction: no ref
/// is written, also when the base reads as an option of git
#[test]
fn a_base_that_is_no_commit_or_shares_no_history_is_its_own_error_and_writes_nothing() {
  let (repo, _, _) = repo();
  let orphan = repo.git(&["commit-tree", "4b825dc642cb6eb9a060e54bf8d69288fbee4904", "-m", "Unrelated root"]);
  repo.git(&["branch", "unrelated", orphan.trim()]);
  let on = |base: &str| MarkReviewedParams {
    base: Some(base.to_string()),
    ..params(&repo, "auth")
  };

  for missing in ["nope", "--all", "master~5"] {
    let error = mark_reviewed_core(repo.git_executor(), PREFIX, &on(missing)).unwrap_err();
    assert_eq!(error.downcast_ref::<UnknownReviewBase>(), Some(&UnknownReviewBase(missing.to_string())), "{missing}");
  }
  for unrelated in ["unrelated", orphan.trim()] {
    let error = mark_reviewed_core(repo.git_executor(), PREFIX, &on(unrelated)).unwrap_err();
    let expected = NoCommonHistory {
      branch: "jane/virtual/auth".to_string(),
      base: unrelated.to_string(),
    };
    assert_eq!(error.downcast_ref::<NoCommonHistory>(), Some(&expected), "{unrelated}");
  }
  // so is a baseline with no history in common
  let baseline = MarkReviewedParams {
    baseline_branch: Some("unrelated".to_string()),
    ..params(&repo, "auth")
  };
  let error = mark_reviewed_core(repo.git_executor(), PREFIX, &baseline).unwrap_err();
  assert!(error.downcast_ref::<NoCommonHistory>().is_some(), "{error:#}");
  assert_eq!(repo.git(&["for-each-ref", REVIEWED_REFS]).trim(), "");
}

#[test]
fn an_unknown_branch_is_its_own_error() {
  let (repo, _, _) = repo();
  let error = mark_reviewed_core(repo.git_executor(), PREFIX, &params(&repo, "missing")).unwrap_err();
  assert_eq!(error.downcast_ref::<UnknownReviewBranch>(), Some(&UnknownReviewBranch("missing".to_string())));
}
