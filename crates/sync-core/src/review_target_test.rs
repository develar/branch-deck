use crate::review_target::{ReviewMemo, get_review};
use domain_types::review::{GetReviewParams, ReviewTarget, TargetReview};
use domain_types::review_request::ReviewCommit;
use pretty_assertions::assert_eq;
use test_utils::git_test_utils::TestRepo;

/// The prefix of the branches of `stacked_repo`, which the host resolved
const PREFIX: &str = "user";

fn target_params(repo: &TestRepo, target: ReviewTarget) -> GetReviewParams {
  GetReviewParams {
    repository_path: repo.path().to_str().unwrap().to_string(),
    baseline_branch: "baseline".to_string(),
    target,
  }
}

fn params(repo: &TestRepo, chain: &[&str]) -> GetReviewParams {
  target_params(
    repo,
    ReviewTarget::Branch {
      chain: chain.iter().map(ToString::to_string).collect(),
    },
  )
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
  let review = get_review(repo.git_executor(), PREFIX, &params(&repo, &["auth", "ui"])).unwrap();
  let segments: Vec<(&str, &str, &str, Vec<&str>)> = review
    .segments()
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
  let button = &review.segments()[1].commits[0];
  assert_eq!(button.subject, "(ui) button");
  assert_eq!(button.message, "(ui) button\n\nThe button submits the form.\nIt has \u{1e} and no other NUL.");
  assert_eq!(button.author, "Test User");
  assert!(button.author_time > 0);
  assert!(button.committer_time > 0);
  assert_eq!(review.segments()[1].commits[1].message, "(ui) form");
}

#[test]
fn a_branch_outside_a_stack_is_one_segment_from_the_baseline() {
  let (repo, [base, _, _, c3]) = stacked_repo();
  let review = get_review(repo.git_executor(), PREFIX, &params(&repo, &["ui"])).unwrap();
  assert_eq!(review.segments().len(), 1);
  assert_eq!((review.segments()[0].base.as_str(), review.segments()[0].head.as_str()), (base.as_str(), c3.as_str()));
  assert_eq!(review.segments()[0].commits.len(), 3);
}

#[test]
fn a_child_not_built_on_the_head_of_its_parent_needs_a_sync() {
  let (repo, _) = stacked_repo();
  // the parent was rebuilt elsewhere: its head is no longer below the child
  repo
    .git_executor()
    .execute_command(&["update-ref", "refs/heads/user/virtual/auth", "refs/heads/baseline"], repo.path().to_str().unwrap())
    .unwrap();
  let error = get_review(repo.git_executor(), PREFIX, &params(&repo, &["auth", "ui"])).unwrap_err();
  assert_eq!(error.to_string(), "ui isn't built on the head of auth: sync first");
  let error = get_review(repo.git_executor(), PREFIX, &params(&repo, &["missing", "ui"])).unwrap_err();
  assert_eq!(error.to_string(), "missing has no virtual branch yet: sync first");
}

fn git(repo: &TestRepo, args: &[&str]) -> String {
  repo.git_executor().execute_command(args, repo.path().to_str().unwrap()).unwrap()
}

fn commit_count(memo: &ReviewMemo, repo: &TestRepo, chain: &[&str]) -> usize {
  memo
    .get(repo.git_executor(), PREFIX, &params(repo, chain))
    .unwrap()
    .segments()
    .iter()
    .map(|segment| segment.commits.len())
    .sum()
}

#[test]
fn a_memoised_review_runs_no_log_for_the_same_tips() {
  let (repo, [_, _, c2, c3]) = stacked_repo();
  let memo = ReviewMemo::new();
  let first = memo.get(repo.git_executor(), PREFIX, &params(&repo, &["auth", "ui"])).unwrap();
  assert_eq!(*first, get_review(repo.git_executor(), PREFIX, &params(&repo, &["auth", "ui"])).unwrap());

  // a replacement changes what the log reads but no tip: the memo answers without running it
  git(&repo, &["replace", &c3, &c2]);
  assert_eq!(get_review(repo.git_executor(), PREFIX, &params(&repo, &["ui"])).unwrap().segments()[0].commits.len(), 2);
  let again = memo.get(repo.git_executor(), PREFIX, &params(&repo, &["auth", "ui"])).unwrap();
  assert!(std::sync::Arc::ptr_eq(&first, &again));
  // another chain of the same branches is another review
  assert_eq!(commit_count(&memo, &repo, &["ui"]), 2);
}

#[test]
fn a_moved_tip_or_baseline_is_read_again() {
  let (repo, [_, c1, _, c3]) = stacked_repo();
  let memo = ReviewMemo::new();
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
  let remote = GetReviewParams {
    baseline_branch: "origin/main".to_string(),
    ..params(&repo, &["ui"])
  };
  assert_eq!(memo.get(repo.git_executor(), PREFIX, &remote).unwrap().segments()[0].commits.len(), 3);
  git(&repo, &["update-ref", "refs/remotes/origin/main", &c3]);
  assert_eq!(memo.get(repo.git_executor(), PREFIX, &remote).unwrap().segments()[0].commits.len(), 1);
}

/// The whole stack is its stack branch, one part named by the root: every commit of the stack on the baseline. Without
/// a stack branch (a straight chain has none) the review says so
#[test]
fn the_whole_stack_is_one_part_of_its_stack_branch() {
  let (repo, [base, c1, c2, c3]) = stacked_repo();
  let whole = |root: &str| target_params(&repo, ReviewTarget::WholeStack { root: root.to_string() });
  let error = get_review(repo.git_executor(), PREFIX, &whole("auth")).unwrap_err();
  assert!(error.to_string().contains("no stack branch"), "{error:#}");

  repo.create_branch_at("user/stack/auth", &c3).unwrap();
  let review = get_review(repo.git_executor(), PREFIX, &whole("auth")).unwrap();
  let [segment] = review.segments() else { panic!("{review:?}") };
  assert_eq!((segment.name.as_str(), segment.base.as_str(), segment.head.as_str()), ("auth", base.as_str(), c3.as_str()));
  assert_eq!(ids(&segment.commits), [c3.as_str(), c2.as_str(), c1.as_str()]);
}

fn kept(repo: &TestRepo, name: &str) -> GetReviewParams {
  target_params(repo, ReviewTarget::Kept { name: name.to_string() })
}

fn one_segment(review: &TargetReview) -> (&str, &str, &str, Vec<&str>) {
  let [segment] = review.segments() else { panic!("{review:?}") };
  (segment.name.as_str(), segment.base.as_str(), segment.head.as_str(), ids(&segment.commits))
}

/// A kept branch is one part, its own commits from where it left the baseline, named without its namespace (and the
/// date of an archived one); a name of another namespace or prefix is refused, a deleted branch says so
#[test]
fn a_kept_branch_is_one_part_of_its_own_commits() {
  let (repo, [base, c1, _, c3]) = stacked_repo();
  repo.create_branch_at("user/unapplied/feature-1/docs", &c3).unwrap();
  repo.create_branch_at("user/archived/2026-10-01/auth", &c1).unwrap();
  // the baseline moved on since the branch left it: the review is the branch's own commits, on its own base
  let later = git(&repo, &["commit-tree", "-p", &base, "-m", "Later", &format!("{base}^{{tree}}")]);
  git(&repo, &["update-ref", "refs/heads/baseline", later.trim()]);

  let unapplied = get_review(repo.git_executor(), PREFIX, &kept(&repo, "user/unapplied/feature-1/docs")).unwrap();
  let (name, from, head, commits) = one_segment(&unapplied);
  assert_eq!((name, from, head, commits.len()), ("feature-1/docs", base.as_str(), c3.as_str(), 3));
  let archived = get_review(repo.git_executor(), PREFIX, &kept(&repo, "user/archived/2026-10-01/auth")).unwrap();
  assert_eq!(one_segment(&archived), ("auth", base.as_str(), c1.as_str(), vec![c1.as_str()]));

  for name in ["user/virtual/auth", "other/unapplied/docs", "user/archived/auth"] {
    let error = get_review(repo.git_executor(), PREFIX, &kept(&repo, name)).unwrap_err();
    assert_eq!(error.to_string(), format!("{name} is no unapplied or archived branch of user"));
  }
  let error = get_review(repo.git_executor(), PREFIX, &kept(&repo, "user/unapplied/gone")).unwrap_err();
  assert_eq!(error.to_string(), "gone no longer exists");
}

#[test]
fn a_memoised_kept_branch_is_read_again_only_when_its_tip_moves() {
  let (repo, [_, c1, _, c3]) = stacked_repo();
  repo.create_branch_at("user/unapplied/docs", &c1).unwrap();
  let memo = ReviewMemo::new();
  let first = memo.get(repo.git_executor(), PREFIX, &kept(&repo, "user/unapplied/docs")).unwrap();
  assert!(std::sync::Arc::ptr_eq(
    &first,
    &memo.get(repo.git_executor(), PREFIX, &kept(&repo, "user/unapplied/docs")).unwrap()
  ));
  git(&repo, &["update-ref", "refs/heads/user/unapplied/docs", &c3, &c1]);
  let moved = memo.get(repo.git_executor(), PREFIX, &kept(&repo, "user/unapplied/docs")).unwrap();
  assert_eq!(one_segment(&moved).3.len(), 3);
}

/// A list of commits is read by its ids in the order of the list, each a commit of its own; the memo keeps it
#[test]
fn a_list_of_commits_keeps_its_order() {
  let (repo, [_, c1, _, c3]) = stacked_repo();
  let list = target_params(
    &repo,
    ReviewTarget::Commits {
      ids: vec![c1.clone(), c3.clone()],
    },
  );
  let review = get_review(repo.git_executor(), PREFIX, &list).unwrap();
  let TargetReview::Commits { commits } = &review else { panic!("{review:?}") };
  assert_eq!(ids(commits), [c1.as_str(), c3.as_str()]);
  assert_eq!(commits[1].subject, "(ui) button");
  assert_eq!(review.segments(), []);

  let memo = ReviewMemo::new();
  let first = memo.get(repo.git_executor(), PREFIX, &list).unwrap();
  assert!(std::sync::Arc::ptr_eq(&first, &memo.get(repo.git_executor(), PREFIX, &list).unwrap()));
  let empty = target_params(&repo, ReviewTarget::Commits { ids: Vec::new() });
  assert_eq!(get_review(repo.git_executor(), PREFIX, &empty).unwrap_err().to_string(), "No commit to review");
}
