use crate::branch_review::{BranchReviewCommit, GetBranchReviewParams, get_branch_review};
use pretty_assertions::assert_eq;
use test_utils::git_test_utils::TestRepo;

fn params(repo: &TestRepo, chain: &[&str]) -> GetBranchReviewParams {
  GetBranchReviewParams {
    repository_path: repo.path().to_str().unwrap().to_string(),
    branch_prefix: "user".to_string(),
    baseline_branch: "baseline".to_string(),
    chain: chain.iter().map(ToString::to_string).collect(),
  }
}

fn ids(commits: &[BranchReviewCommit]) -> Vec<&str> {
  commits.iter().map(|commit| commit.id.as_str()).collect()
}

/// `baseline` at B, then c1 (the root `auth`) and c2, c3 (its child `ui`) as virtual branches
fn stacked_repo() -> (TestRepo, [String; 4]) {
  let repo = TestRepo::new();
  let base = repo.create_commit("Base", "base.txt", "base\n");
  repo.create_branch_at("baseline", &base).unwrap();
  let c1 = repo.create_commit("(auth) token", "auth.txt", "auth\n");
  let c2 = repo.create_commit("(ui) form", "ui.txt", "form\n");
  let c3 = repo.create_commit("(ui) button", "ui.txt", "form\nbutton\n");
  repo.create_branch_at("user/virtual/auth", &c1).unwrap();
  repo.create_branch_at("user/virtual/ui", &c3).unwrap();
  (repo, [base, c1, c2, c3])
}

#[test]
fn splits_a_stack_at_the_heads_of_the_parents() {
  let (repo, [base, c1, c2, c3]) = stacked_repo();
  let review = get_branch_review(repo.git_executor(), &params(&repo, &["auth", "ui"])).unwrap();
  let segments: Vec<(&str, &str, &str, Vec<&str>)> = review
    .segments
    .iter()
    .map(|segment| (segment.name.as_str(), segment.base.as_str(), segment.head.as_str(), ids(&segment.commits)))
    .collect();
  assert_eq!(
    segments,
    vec![
      ("auth", base.as_str(), c1.as_str(), vec![c1.as_str()]),
      ("ui", c1.as_str(), c3.as_str(), vec![c3.as_str(), c2.as_str()])
    ]
  );
  assert_eq!(review.segments[1].commits[0].subject, "(ui) button");
  assert!(review.segments[1].commits[0].author_time > 0);
}

#[test]
fn a_branch_outside_a_stack_is_one_segment_from_the_baseline() {
  let (repo, [base, _, _, c3]) = stacked_repo();
  let review = get_branch_review(repo.git_executor(), &params(&repo, &["ui"])).unwrap();
  assert_eq!(review.segments.len(), 1);
  assert_eq!((review.segments[0].base.as_str(), review.segments[0].head.as_str()), (base.as_str(), c3.as_str()));
  assert_eq!(review.segments[0].commits.len(), 3);
}

#[test]
fn a_child_not_built_on_the_head_of_its_parent_needs_a_sync() {
  let (repo, _) = stacked_repo();
  // the parent was rebuilt elsewhere: its head is no longer below the child
  repo
    .git_executor()
    .execute_command(&["update-ref", "refs/heads/user/virtual/auth", "refs/heads/baseline"], repo.path().to_str().unwrap())
    .unwrap();
  let error = get_branch_review(repo.git_executor(), &params(&repo, &["auth", "ui"])).unwrap_err();
  assert_eq!(error.to_string(), "ui isn't built on the head of auth: sync first");
  let error = get_branch_review(repo.git_executor(), &params(&repo, &["missing", "ui"])).unwrap_err();
  assert_eq!(error.to_string(), "missing has no virtual branch yet: sync first");
}
