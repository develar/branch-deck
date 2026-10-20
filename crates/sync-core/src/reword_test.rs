use crate::regroup_test::{commits, groups, issue_group_plan, path, plan, regroup, repo, subjects};
use history_rewrite::issue_groups::ISSUE_GROUPS_REF;
use history_rewrite::regroup::{RegroupError, RegroupOptions, RegroupOutcome, restack_branch_core};
use history_rewrite::reword::{CommitReword, RewordText, reword_core};
use history_rewrite::stack_parents::STACK_PARENTS_REF;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn subject(commit: &str, subject: &str) -> CommitReword {
  CommitReword {
    commit: commit.to_string(),
    text: RewordText::Subject(subject.to_string()),
  }
}

fn message(commit: &str, message: &str) -> CommitReword {
  CommitReword {
    commit: commit.to_string(),
    text: RewordText::Message(message.to_string()),
  }
}

fn reword(repo: &TestRepo, rewords: &[CommitReword]) -> Result<RegroupOutcome, RegroupError> {
  reword_core(repo.git_executor(), path(repo), rewords, &RegroupOptions::default())
}

fn tree(repo: &TestRepo, commit: &str) -> String {
  repo.git_executor().execute_command(&["rev-parse", &format!("{commit}^{{tree}}")], path(repo)).unwrap()
}

fn assert_invalid(error: &RegroupError, needle: &str) {
  assert!(matches!(error, RegroupError::InvalidPlan { message, .. } if message.contains(needle)), "{error}");
}

#[test]
fn rewords_messages_in_one_rewrite() {
  let repo = repo(false);
  let ids = commits(&repo, &["(a) One", "(a) Two with a subject that is far too long", "Three"]);
  let trees: Vec<String> = ids.iter().map(|id| tree(&repo, id)).collect();

  let short = ids[1][..8].to_string();
  let outcome = reword(&repo, &[subject(&short, "(a) Two"), message(&ids[2], "Three\n\nWith a body")]).unwrap();

  assert_eq!(subjects(&repo, 3), ["(a) One", "(a) Two", "Three"]);
  assert_eq!(
    repo.git_executor().execute_command(&["log", "-1", "--format=%B"], path(&repo)).unwrap(),
    "Three\n\nWith a body"
  );
  assert_eq!(outcome.changed_count(), 2);
  assert_eq!(outcome.commits[0].new_id, ids[0], "an older commit stays");
  assert_eq!(outcome.new_head, repo.head());
  let new_trees: Vec<String> = outcome.commits.iter().map(|commit| tree(&repo, &commit.new_id)).collect();
  assert_eq!(new_trees, trees, "the trees stay");
  assert_eq!(groups(&outcome), [Some("a"), Some("a"), None]);
}

#[test]
fn a_subject_keeps_the_body_and_a_message_equal_after_cleanup_rewrites_nothing() {
  let repo = repo(false);
  commits(&repo, &["Two\n\nThe body"]);
  let head = repo.head();

  reword(&repo, &[subject(&head, "Two, shorter")]).unwrap();
  assert_eq!(
    repo.git_executor().execute_command(&["log", "-1", "--format=%B"], path(&repo)).unwrap(),
    "Two, shorter\n\nThe body"
  );

  let head = repo.head();
  let outcome = reword(&repo, &[message(&head, "\nTwo, shorter  \n\nThe body\n\n")]).unwrap();
  assert_eq!((outcome.changed_count(), outcome.new_head.as_str()), (0, head.as_str()));
}

#[test]
fn a_new_prefix_moves_the_commit_with_its_fixups() {
  let repo = repo(false);
  let ids = commits(&repo, &["(a) Fix login", "fixup! (a) Fix login", "(b) Other"]);
  let outcome = reword(&repo, &[subject(&ids[0], "(b) Fix login")]).unwrap();
  assert_eq!(subjects(&repo, 3), ["(b) Fix login", "fixup! (b) Fix login", "(b) Other"]);
  assert_eq!(groups(&outcome), [Some("b"), Some("b"), Some("b")]);
  assert_eq!(outcome.commits[0].previous_group.as_deref(), Some("a"));
}

#[test]
fn a_reword_that_strands_a_fixup_is_refused() {
  // by subject: the fixup would fold into nothing
  let repo = repo(false);
  let ids = commits(&repo, &["Fix login", "fixup! Fix login"]);
  let head = repo.head();
  assert_invalid(&reword(&repo, &[subject(&ids[0], "Fix the login")]).unwrap_err(), "another target or none");
  // a marker removed
  assert_invalid(&reword(&repo, &[subject(&ids[1], "Fix login again")]).unwrap_err(), "marker");
  assert_eq!(repo.head(), head, "nothing is written");

  // by id: the target gets a new id
  let repo = self::repo(false);
  let ids = commits(&repo, &["One", "Two"]);
  let by_id = repo.create_commit(&format!("fixup! {}", &ids[1][..8]), "two.txt", "fixed");
  assert_invalid(&reword(&repo, &[subject(&ids[0], "One, reworded")]).unwrap_err(), "by its id");
  // the same for a regroup, which recreates the target too
  assert_invalid(&regroup(&repo, &plan(&[("a", &[&ids[0]])], &[])).unwrap_err(), "by its id");
  // rewording the fixup itself is no change of its target
  reword(&repo, &[subject(&by_id, &format!("fixup! {}", &ids[1][..10]))]).unwrap();

  // a waiting amend! would replace the new message at the next fold
  let repo = self::repo(false);
  let ids = commits(&repo, &["One", "amend! One\n\nThe message the amend! gives"]);
  assert_invalid(&reword(&repo, &[message(&ids[0], "One\n\nAnother body")]).unwrap_err(), "amend!");
}

#[test]
fn a_plan_that_rewords_a_fixup_with_its_target_passes() {
  let repo = repo(false);
  let ids = commits(&repo, &["Fix login", "fixup! Fix login"]);
  reword(&repo, &[subject(&ids[0], "Fix the login"), subject(&ids[1], "fixup! Fix the login")]).unwrap();
  assert_eq!(subjects(&repo, 2), ["Fix the login", "fixup! Fix the login"]);
}

#[test]
fn a_reword_keeps_the_issue_groups_and_the_stack_parents() {
  let repo = repo(true);
  let ids = commits(&repo, &["(auth) Add auth", "(login) Add form", "ABC-1 Add parser"]);
  restack_branch_core(repo.git_executor(), path(&repo), "login", Some("auth"), &RegroupOptions::default()).unwrap();
  regroup(&repo, &issue_group_plan("login", &["ABC-1"])).unwrap();
  let stores = |repo: &TestRepo| [ISSUE_GROUPS_REF, STACK_PARENTS_REF].map(|store| repo.git_executor().execute_command(&["rev-parse", store], path(repo)).unwrap());
  let before = stores(&repo);

  let outcome = reword(&repo, &[subject(&ids[0], "(auth) Add the auth")]).unwrap();
  assert_eq!((outcome.issue_groups.is_none(), outcome.stack_parents.is_none()), (true, true));
  assert_eq!(stores(&repo), before);
  assert_eq!(groups(&outcome), [Some("auth"), Some("login"), Some("login")]);
  assert_eq!(outcome.forest.unwrap().declared_parent("login"), Some("auth"));
}

#[test]
fn a_reword_that_breaks_a_stack_is_refused() {
  let repo = repo(true);
  let ids = commits(&repo, &["(auth) Add auth", "(auth/login) Add form"]);
  let error = reword(&repo, &[subject(&ids[0], "(login/auth) Add auth")]).unwrap_err();
  assert_invalid(&error, "break the stacks");
}

#[test]
fn refuses_ids_and_texts_it_cannot_reword() {
  let repo = repo(false);
  let initial = repo.head();
  let ids = commits(&repo, &["One"]);
  let error = reword(&repo, &[subject(&initial, "Initial")]).unwrap_err();
  assert!(matches!(&error, RegroupError::NotAheadOfBaseline { commits } if *commits == [initial]), "{error}");
  let error = reword(&repo, &[subject("abcdef12", "Nothing")]).unwrap_err();
  assert!(matches!(error, RegroupError::NotAheadOfBaseline { .. }), "{error}");
  assert_invalid(&reword(&repo, &[message(&ids[0], "  \n\n  ")]).unwrap_err(), "subject line");
  assert_invalid(&reword(&repo, &[subject(&ids[0], "Two\nlines")]).unwrap_err(), "more than one line");
  assert_invalid(&reword(&repo, &[subject(&ids[0], "A"), subject(&ids[0], "B")]).unwrap_err(), "more than once");
  assert_invalid(&reword(&repo, &[]).unwrap_err(), "No commit");
}
