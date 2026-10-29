use crate::interdiff::{CommitChanges, PairState, RangeScope};
use crate::test_support::{mark, plan_of};
use crate::{ReviewPlan, ReviewPlanParams, ReviewSubject, SinceReviewedOfRange, UnknownRevision};
use git_ops::landed_pairing::PathLines;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// `master` at `base`, and the checked out virtual branch `jane/virtual/auth` with two commits on it, each at its own
/// second, so their identities differ and a copy keeps them
fn repo() -> (TestRepo, String, [String; 2]) {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_timestamp("Add readme", "README.md", "readme\n", Some(1_700_000_000));
  repo.git(&["switch", "-q", "-c", "jane/virtual/auth"]);
  let login = repo.create_commit_with_timestamp("(auth) Add login", "login.rs", "fn login() {}\n", Some(1_700_000_100));
  let logout = repo.create_commit_with_timestamp("(auth) Add logout", "logout.rs", "fn logout() {}\n", Some(1_700_000_200));
  (repo, base, [login, logout])
}

const SESSION: &str = "(auth) Add session";

/// A third commit on the branch, after the second
fn commit_session(repo: &TestRepo) -> String {
  repo.create_commit_with_timestamp(SESSION, "session.rs", "fn session() {}\n", Some(1_700_000_300))
}

fn plan(repo: &TestRepo, since_reviewed: bool) -> ReviewPlan {
  let params = ReviewPlanParams {
    repository_path: repo.path_str().to_string(),
    subject: ReviewSubject::Branch {
      name: "auth".to_string(),
      base: None,
      baseline_branch: Some("master".to_string()),
    },
    since_reviewed,
  };
  plan_of(repo, None, &params).unwrap()
}

/// A commit of the interdiff: its id, the reviewed id, its subject, its state
type Row<'a> = (Option<&'a str>, Option<&'a str>, &'a str, PairState);

/// A reviewed commit that is still on the branch
fn kept<'a>(id: &'a str, subject: &'a str) -> Row<'a> {
  (Some(id), Some(id), subject, PairState::Unchanged)
}

fn added<'a>(id: &'a str, subject: &'a str) -> Row<'a> {
  (Some(id), None, subject, PairState::Added)
}

fn commits(plan: &ReviewPlan) -> Vec<Row<'_>> {
  plan
    .interdiff
    .as_ref()
    .unwrap()
    .commits
    .iter()
    .map(|commit| (commit.id.as_deref(), commit.reviewed_id.as_deref(), commit.subject.as_str(), commit.state))
    .collect()
}

/// The scope of the interdiff, which each test checks is there by its `commits` first
fn scope(plan: &ReviewPlan) -> Option<(&str, &str)> {
  plan.interdiff.as_ref()?.scope.as_ref().map(|RangeScope { base, head }| (base.as_str(), head.as_str()))
}

#[test]
fn a_branch_never_marked_has_no_interdiff() {
  let (repo, base, [_, logout]) = repo();
  let plan = plan(&repo, true);
  assert_eq!(
    (plan.branch.as_deref(), plan.base.as_str(), plan.head.as_str(), plan.interdiff),
    (Some("jane/virtual/auth"), base.as_str(), logout.as_str(), None)
  );
}

/// Commits appended after the mark: the reviewed ones unchanged, the new one added, and the scope the plain range of
/// the new ones, `<first added>^..head`
#[test]
fn commits_added_after_the_mark_are_the_range_on_top() {
  let (repo, _, [login, logout]) = repo();
  mark(&repo);
  let session = commit_session(&repo);

  let whole = plan(&repo, false);
  assert_eq!(
    commits(&whole),
    [kept(&login, "(auth) Add login"), kept(&logout, "(auth) Add logout"), added(&session, SESSION)]
  );
  assert_eq!(scope(&whole), Some((logout.as_str(), session.as_str())));
  let interdiff = whole.interdiff.as_ref().unwrap();
  assert_eq!((interdiff.reviewed_tip.as_str(), interdiff.commits[2].changes.as_ref()), (logout.as_str(), None));

  let since = plan(&repo, true);
  assert_eq!(commits(&since), [added(&session, SESSION)]);
  assert_eq!(scope(&since), Some((logout.as_str(), session.as_str())));
}

/// An amend keeps the identity of the commit and changes its lines: `rewritten`, with what each version changes that
/// the other does not, and no plain range
#[test]
fn an_amended_commit_is_rewritten_with_its_lines() {
  let (repo, _, [login, logout]) = repo();
  mark(&repo);
  std::fs::write(repo.path().join("logout.rs"), "fn logout() { clear(); }\n").unwrap();
  let amended = repo.commit_with(&["--amend", "--no-edit", "-a"], &[("GIT_COMMITTER_DATE", "1700000300 +0000")]);

  let whole = plan(&repo, false);
  assert_eq!(
    commits(&whole),
    [
      kept(&login, "(auth) Add login"),
      (Some(amended.as_str()), Some(logout.as_str()), "(auth) Add logout", PairState::Rewritten),
    ]
  );
  let lines = |line: &str| {
    vec![PathLines {
      path: "logout.rs".to_string(),
      lines: vec![line.to_string()],
    }]
  };
  assert_eq!(
    whole.interdiff.as_ref().unwrap().commits[1].changes,
    Some(CommitChanges {
      new_only: lines("+fn logout() { clear(); }"),
      reviewed_only: lines("+fn logout() {}"),
    })
  );
  assert_eq!(scope(&whole), None);

  let since = plan(&repo, true);
  assert_eq!(
    commits(&since),
    [(Some(amended.as_str()), Some(logout.as_str()), "(auth) Add logout", PairState::Rewritten)]
  );
}

/// A move of the baseline copies every commit with the same changes: all `unchanged`, and the scope is empty
#[test]
fn a_baseline_move_alone_leaves_every_commit_unchanged() {
  let (repo, _, [login, logout]) = repo();
  mark(&repo);
  repo.git(&["switch", "-q", "master"]);
  let moved = repo.create_commit_with_timestamp("Change readme", "README.md", "readme 2\n", Some(1_700_000_400));
  repo.git(&["switch", "-q", "jane/virtual/auth"]);
  repo.rebase("master").unwrap();
  let tip = repo.head();

  let whole = plan(&repo, false);
  assert_eq!(whole.base, moved);
  let states: Vec<(Option<&str>, &str, PairState)> = commits(&whole).into_iter().map(|(_, reviewed, subject, state)| (reviewed, subject, state)).collect();
  assert_eq!(
    states,
    [
      (Some(login.as_str()), "(auth) Add login", PairState::Unchanged),
      (Some(logout.as_str()), "(auth) Add logout", PairState::Unchanged),
    ]
  );
  assert_ne!(whole.interdiff.as_ref().unwrap().commits[1].id.as_deref(), Some(logout.as_str()));
  assert_eq!(scope(&whole), Some((tip.as_str(), tip.as_str())));
  assert_eq!(commits(&plan(&repo, true)), []);
}

/// A reword changes the identity but not the lines: paired by its lines, `unchanged`. A commit taken out of the branch
/// is `dropped`, listed after the commits of the branch, and a commit added in its place leaves no plain range
#[test]
fn a_reworded_commit_pairs_by_its_lines_and_a_dropped_one_is_listed() {
  let (repo, _, [login, logout]) = repo();
  mark(&repo);
  let reworded = repo.commit_with(&["--amend", "-m", "(auth) Add sign out"], &[]);
  assert_eq!(
    commits(&plan(&repo, false))[1],
    (Some(reworded.as_str()), Some(logout.as_str()), "(auth) Add sign out", PairState::Unchanged)
  );

  repo.git(&["reset", "-q", "--hard", &login]);
  let session = commit_session(&repo);
  let whole = plan(&repo, false);
  assert_eq!(
    commits(&whole),
    [
      kept(&login, "(auth) Add login"),
      added(&session, SESSION),
      (None, Some(logout.as_str()), "(auth) Add logout", PairState::Dropped),
    ]
  );
  assert_eq!(scope(&whole), None);
}

/// A range has no reviewed pair: no interdiff, and `since_reviewed` or a rev that names no commit is the caller's error
#[test]
fn a_range_has_no_interdiff() {
  let (repo, base, [_, logout]) = repo();
  let params = |base: &str, since_reviewed: bool| ReviewPlanParams {
    repository_path: repo.path_str().to_string(),
    subject: ReviewSubject::Range {
      base: base.to_string(),
      head: "jane/virtual/auth".to_string(),
    },
    since_reviewed,
  };
  let plan = plan_of(&repo, None, &params("master", false)).unwrap();
  assert_eq!((plan.branch, plan.base, plan.head, plan.interdiff), (None, base, logout, None));

  let error = plan_of(&repo, None, &params("master", true)).unwrap_err();
  assert!(error.downcast_ref::<SinceReviewedOfRange>().is_some(), "{error:?}");
  let error = plan_of(&repo, None, &params("--all", false)).unwrap_err();
  assert_eq!(error.downcast_ref::<UnknownRevision>(), Some(&UnknownRevision("--all".to_string())));
}
