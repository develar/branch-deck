use crate::branch_review::{BranchReviewMemo, GetBranchReviewParams, get_branch_review};
use pretty_assertions::assert_eq;
use sync_types::review_request::ReviewCommit;
use test_utils::git_test_utils::TestRepo;

fn params(repo: &TestRepo, chain: &[&str]) -> GetBranchReviewParams {
  GetBranchReviewParams {
    repository_path: repo.path().to_str().unwrap().to_string(),
    branch_prefix: "user".to_string(),
    baseline_branch: "baseline".to_string(),
    chain: chain.iter().map(ToString::to_string).collect(),
  }
}

fn ids(commits: &[ReviewCommit]) -> Vec<&str> {
  commits.iter().map(|commit| commit.id.as_str()).collect()
}

/// `baseline` at B, then c1 (the root `auth`) and c2, c3 (its child `ui`) as virtual branches
fn stacked_repo() -> (TestRepo, [String; 4]) {
  let repo = TestRepo::new();
  let base = repo.create_commit("Base", "base.txt", "base\n");
  repo.create_branch_at("baseline", &base).unwrap();
  let c1 = repo.create_commit("(auth) token", "auth.txt", "auth\n");
  let c2 = repo.create_commit("(ui) form", "ui.txt", "form\n");
  let c3 = repo.create_commit("(ui) button\n\nThe button submits the form.\nIt has \u{1e} and no other NUL.", "ui.txt", "form\nbutton\n");
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
  let button = &review.segments[1].commits[0];
  assert_eq!(button.subject, "(ui) button");
  assert_eq!(button.message, "(ui) button\n\nThe button submits the form.\nIt has \u{1e} and no other NUL.");
  assert_eq!(button.author, "Test User");
  assert!(button.author_time > 0);
  assert!(button.committer_time > 0);
  assert_eq!(review.segments[1].commits[1].message, "(ui) form");
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

fn git(repo: &TestRepo, args: &[&str]) -> String {
  repo.git_executor().execute_command(args, repo.path().to_str().unwrap()).unwrap()
}

fn commit_count(memo: &BranchReviewMemo, repo: &TestRepo, chain: &[&str]) -> usize {
  memo
    .get(repo.git_executor(), &params(repo, chain))
    .unwrap()
    .segments
    .iter()
    .map(|segment| segment.commits.len())
    .sum()
}

#[test]
fn a_memoised_review_runs_no_log_for_the_same_tips() {
  let (repo, [_, _, c2, c3]) = stacked_repo();
  let memo = BranchReviewMemo::new();
  let first = memo.get(repo.git_executor(), &params(&repo, &["auth", "ui"])).unwrap();
  assert_eq!(*first, get_branch_review(repo.git_executor(), &params(&repo, &["auth", "ui"])).unwrap());

  // a replacement changes what the log reads but no tip: the memo answers without running it
  git(&repo, &["replace", &c3, &c2]);
  assert_eq!(get_branch_review(repo.git_executor(), &params(&repo, &["ui"])).unwrap().segments[0].commits.len(), 2);
  let again = memo.get(repo.git_executor(), &params(&repo, &["auth", "ui"])).unwrap();
  assert!(std::sync::Arc::ptr_eq(&first, &again));
  // another chain of the same branches is another review
  assert_eq!(commit_count(&memo, &repo, &["ui"]), 2);
}

#[test]
fn a_moved_tip_or_baseline_is_read_again() {
  let (repo, [_, c1, _, c3]) = stacked_repo();
  let memo = BranchReviewMemo::new();
  assert_eq!(commit_count(&memo, &repo, &["ui"]), 3);

  // the baseline moved (a fetch): the commits it now has are no longer the branch's
  git(&repo, &["update-ref", "refs/heads/baseline", &c1]);
  assert_eq!(commit_count(&memo, &repo, &["ui"]), 2);

  // the branch moved (a sync)
  let c4 = repo.create_commit("(ui) label", "ui.txt", "form\nbutton\nlabel\n");
  git(&repo, &["update-ref", "refs/heads/user/virtual/ui", &c4, &c3]);
  assert_eq!(commit_count(&memo, &repo, &["ui"]), 3);

  // a remote baseline is found by its short name
  git(&repo, &["update-ref", "refs/remotes/origin/main", &c1]);
  let remote = GetBranchReviewParams {
    baseline_branch: "origin/main".to_string(),
    ..params(&repo, &["ui"])
  };
  assert_eq!(memo.get(repo.git_executor(), &remote).unwrap().segments[0].commits.len(), 3);
  git(&repo, &["update-ref", "refs/remotes/origin/main", &c3]);
  assert_eq!(memo.get(repo.git_executor(), &remote).unwrap().segments[0].commits.len(), 1);
}
