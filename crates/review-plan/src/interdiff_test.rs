use crate::interdiff::{CommitChanges, MessageChange, PairState, RangeScope, message_change};
use crate::test_support::{mark, plan_of};
use crate::{ReviewPlan, ReviewPlanParams, ReviewSubject, SinceReviewedOfRange, UnknownRevision};
use git_ops::landed_pairing::PathLines;
use pretty_assertions::assert_eq;
use sync_utils::commit_body::prose_lines;
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
    concern_edges: crate::concerns::ConcernEdges::default(),
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

/// The scope of the interdiff, which each test checks is there by its `commits` first: its base, its head, and whether
/// its base is the reviewed change carried onto the base
fn scope(plan: &ReviewPlan) -> Option<(&str, &str, bool)> {
  plan
    .interdiff
    .as_ref()?
    .scope
    .as_ref()
    .map(|RangeScope { base, head, carried }| (base.as_str(), head.as_str(), *carried))
}

/// The paths of the files of `plan`
fn paths(plan: &ReviewPlan) -> Vec<&str> {
  plan.files.iter().map(|file| file.path.as_str()).collect()
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
  assert_eq!(scope(&whole), Some((logout.as_str(), session.as_str(), false)));
  let interdiff = whole.interdiff.as_ref().unwrap();
  assert_eq!((interdiff.reviewed_tip.as_str(), interdiff.commits[2].changes.as_ref()), (logout.as_str(), None));

  let since = plan(&repo, true);
  assert_eq!(commits(&since), [added(&session, SESSION)]);
  assert_eq!(scope(&since), Some((logout.as_str(), session.as_str(), false)));
}

/// An amend keeps the identity of the commit and changes its lines: `rewritten`, with what each version changes that
/// the other does not; on the same base the scope is the diff from the reviewed tip to the tip
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
  assert_eq!(scope(&whole), Some((logout.as_str(), amended.as_str(), true)));

  let since = plan(&repo, true);
  assert_eq!(
    commits(&since),
    [(Some(amended.as_str()), Some(logout.as_str()), "(auth) Add logout", PairState::Rewritten)]
  );
  assert_eq!(paths(&since), ["logout.rs"]);
}

/// A commit on `master` and the branch rebased onto it, as a sync onto a new baseline: the commit
fn move_the_baseline(repo: &TestRepo) -> String {
  repo.git(&["switch", "-q", "master"]);
  let moved = repo.create_commit_with_timestamp("Change readme", "README.md", "readme 2\n", Some(1_700_000_400));
  repo.git(&["switch", "-q", "jane/virtual/auth"]);
  repo.rebase("master").unwrap();
  moved
}

/// A rebase onto a moved baseline with an amend: the scope is the diff from the reviewed change carried onto the new base
/// to the tip, so it holds the amend alone, not the move of the baseline nor the reviewed lines
#[test]
fn after_a_rebase_with_an_amend_the_scope_holds_the_amend_alone() {
  let (repo, _, [_, logout]) = repo();
  mark(&repo);
  let moved = move_the_baseline(&repo);
  std::fs::write(repo.path().join("logout.rs"), "fn logout() { clear(); }\n").unwrap();
  let amended = repo.commit_with(&["--amend", "--no-edit", "-a"], &[("GIT_COMMITTER_DATE", "1700000500 +0000")]);

  let since = plan(&repo, true);
  assert_eq!(since.base, moved);
  assert_eq!(
    commits(&since),
    [(Some(amended.as_str()), Some(logout.as_str()), "(auth) Add logout", PairState::Rewritten)]
  );
  let (carried, head, true) = scope(&since).unwrap() else {
    panic!("no carried scope: {:?}", scope(&since))
  };
  assert_eq!(head, amended);
  assert_eq!(repo.git(&["rev-parse", &format!("{carried}^")]), moved);
  assert_eq!(repo.git(&["diff", "--name-only", carried, &amended]), "logout.rs");
  assert_eq!(paths(&since), ["logout.rs"]);
}

/// A move of the baseline copies every commit with the same changes: all `unchanged`, and the scope is empty
#[test]
fn a_baseline_move_alone_leaves_every_commit_unchanged() {
  let (repo, _, [login, logout]) = repo();
  mark(&repo);
  let moved = move_the_baseline(&repo);
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
  assert_eq!(scope(&whole), Some((tip.as_str(), tip.as_str(), false)));
  assert_eq!(commits(&plan(&repo, true)), []);
}

/// A reword changes the identity but not the lines: paired by its lines, `reworded`, its subject before kept. A commit
/// taken out of the branch
/// is `dropped`, listed after the commits of the branch, and a commit added in its place leaves no plain range: the
/// scope is the diff from the reviewed tip, which removes the dropped file and adds the new one
#[test]
fn a_reworded_commit_pairs_by_its_lines_and_a_dropped_one_is_listed() {
  let (repo, _, [login, logout]) = repo();
  mark(&repo);
  let reworded = repo.commit_with(&["--amend", "-m", "(auth) Add sign out"], &[]);
  assert_eq!(
    commits(&plan(&repo, false))[1],
    (Some(reworded.as_str()), Some(logout.as_str()), "(auth) Add sign out", PairState::Reworded)
  );
  assert_eq!(
    plan(&repo, false).interdiff.unwrap().commits[1].message_change,
    Some(MessageChange {
      subject_before: Some("Add logout".to_string()),
      hunks: Vec::new(),
    })
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
  assert_eq!(scope(&whole), Some((logout.as_str(), session.as_str(), true)));
  assert_eq!(paths(&plan(&repo, true)), ["session.rs", "logout.rs"]);
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
    concern_edges: crate::concerns::ConcernEdges::default(),
  };
  let plan = plan_of(&repo, None, &params("master", false)).unwrap();
  assert_eq!((plan.branch, plan.base, plan.head, plan.interdiff), (None, base, logout, None));

  let error = plan_of(&repo, None, &params("master", true)).unwrap_err();
  assert!(error.downcast_ref::<SinceReviewedOfRange>().is_some(), "{error:?}");
  let error = plan_of(&repo, None, &params("--all", false)).unwrap_err();
  assert_eq!(error.downcast_ref::<UnknownRevision>(), Some(&UnknownRevision("--all".to_string())));
}

/// What `change` says of a body: per hunk its place, the lines of the blocks it adds and of those it removes
fn hunk_words(change: &MessageChange) -> Vec<(u32, Vec<String>, Vec<String>)> {
  let words = |blocks: &[domain_types::commit_body::CommitBodyBlock]| -> Vec<String> { blocks.iter().map(|block| prose_lines(block).join(" ")).collect() };
  change
    .hunks
    .iter()
    .map(|hunk| (hunk.start, words(&hunk.added.blocks), words(&hunk.removed.blocks)))
    .collect()
}

const REVIEWED_BODY: &str = "(auth) Add logout\n\nThe logout clears the session.\n\nWhy: the token outlives the tab.\n\nTests: `logout_test` passes.";

/// The body is compared by its blocks: a lead rewritten is one hunk of a removed and an added paragraph in its place, a
/// new Look closely an added label and its paragraph where it stands, a removed section the blocks that stood before
/// the block it left; each run with the code of its lines
#[test]
fn the_body_of_a_message_changes_by_its_blocks() {
  let now = "(auth) Add logout\n\nThe logout clears the session and the `Cache`.\n\nLook closely: the cache is cleared first.\n\nTests: `logout_test` passes.";
  let change = message_change(REVIEWED_BODY, now).unwrap();
  assert_eq!(change.subject_before, None);
  assert_eq!(
    hunk_words(&change),
    [(
      0,
      vec![
        "The logout clears the session and the `Cache`.".to_string(),
        "Look closely".to_string(),
        "the cache is cleared first.".to_string()
      ],
      vec!["The logout clears the session.".to_string(), "Why".to_string(), "the token outlives the tab.".to_string()],
    ),]
  );
  // the code of the lines of each run, by its own blocks
  assert_eq!(change.hunks[0].added.code.iter().map(|line| line.block).collect::<Vec<_>>(), [0]);

  let shorter = "(auth) Add logout\n\nThe logout clears the session.\n\nWhy: the token outlives the tab.";
  assert_eq!(
    hunk_words(&message_change(REVIEWED_BODY, shorter).unwrap()),
    [(3, vec![], vec!["Tests".to_string(), "`logout_test` passes.".to_string()])]
  );
}

/// A trailer added, a paragraph wrapped anew, a blank line more and a prefix changed say the same: no change
#[test]
fn a_new_trailer_or_a_rewrap_changes_no_message() {
  let now =
    "(login) Add logout\n\nThe logout clears\nthe session.\n\n\nWhy: the token outlives the tab.\n\nTests: `logout_test` passes.\n\nCo-Authored-By: Jane <jane@example.com>";
  assert_eq!(message_change(REVIEWED_BODY, now), None);
}

/// An amend of the code and of the body: `rewritten`, with its lines and with the blocks of its body that changed
#[test]
fn a_commit_amended_with_a_new_body_is_rewritten_with_its_added_and_removed_blocks() {
  let (repo, _, [_, logout]) = repo();
  repo.git(&["commit", "-q", "--amend", "-m", REVIEWED_BODY]);
  let reviewed = repo.head();
  mark(&repo);
  std::fs::write(repo.path().join("logout.rs"), "fn logout() { clear(); }\n").unwrap();
  let now = "(auth) Add logout\n\nThe logout clears the session.\n\nLook closely: the cache is cleared first.\n\nWhy: the token outlives the tab.";
  let amended = repo.commit_with(&["--amend", "-a", "-m", now], &[("GIT_COMMITTER_DATE", "1700000400 +0000")]);
  assert_ne!(reviewed, logout);

  let whole = plan(&repo, false);
  let commit = &whole.interdiff.as_ref().unwrap().commits[1];
  assert_eq!((commit.id.as_deref(), commit.state), (Some(amended.as_str()), PairState::Rewritten));
  assert_eq!(
    hunk_words(commit.message_change.as_ref().unwrap()),
    [
      (1, vec!["Look closely".to_string(), "the cache is cleared first.".to_string()], vec![]),
      (5, vec![], vec!["Tests".to_string(), "`logout_test` passes.".to_string()]),
    ]
  );
}

/// A body rewritten alone keeps the lines of the commit: `reworded`, which Since Your Review lists, but its code is the
/// code the reviewer read, so the commits added on top stay the plain range and the plan reads no file of it
#[test]
fn a_body_rewritten_alone_is_reworded_its_code_unread_and_the_scope_on_top() {
  let (repo, _, [login, logout]) = repo();
  mark(&repo);
  repo.git(&["reset", "-q", "--hard", &login]);
  let reworded = repo.commit_with(
    &["--amend", "-m", "(auth) Add login\n\nThe login keeps the token."],
    &[("GIT_COMMITTER_DATE", "1700000500 +0000")],
  );
  repo.git(&["cherry-pick", &logout]);
  let copy = repo.head();
  let session = commit_session(&repo);

  let whole = plan(&repo, false);
  assert_eq!(
    commits(&whole),
    [
      (Some(reworded.as_str()), Some(login.as_str()), "(auth) Add login", PairState::Reworded),
      (Some(copy.as_str()), Some(logout.as_str()), "(auth) Add logout", PairState::Unchanged),
      added(&session, SESSION),
    ]
  );
  assert_eq!(
    hunk_words(whole.interdiff.as_ref().unwrap().commits[0].message_change.as_ref().unwrap()),
    [(0, vec!["The login keeps the token.".to_string()], vec![])]
  );
  assert_eq!(scope(&whole), Some((copy.as_str(), session.as_str(), false)));

  let since = plan(&repo, true);
  assert_eq!(
    commits(&since),
    [
      (Some(reworded.as_str()), Some(login.as_str()), "(auth) Add login", PairState::Reworded),
      added(&session, SESSION),
    ]
  );
  assert_eq!(paths(&since), ["session.rs"]);
}
