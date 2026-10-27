use crate::regroup_test::{commits, groups, path, regroup, subjects, sync};
use crate::stack_operations::{UndoRegroupParams, undo_regroup};
use domain_types::rewrite::{CommitReword, RewordResult, RewordText};
use history_rewrite::issue_groups::ISSUE_GROUPS_REF;
use history_rewrite::regroup::{FixupRefusal, RegroupError, RegroupOptions, RegroupOutcome, restack_branch_core};
use history_rewrite::repository_lock::LockWait;
use history_rewrite::reword::{RewordRequest, reword_core, reword_request_core};
use history_rewrite::stack_parents::STACK_PARENTS_REF;
use history_rewrite::test_commit::{issue_group_plan, plan};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, baseline_repo};

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

/// The reword of the app (the Tauri command `reword`): whole messages, the result with the undo token, the errors as the
/// app tells them
fn app_reword(repo: &TestRepo, rewords: &[(&str, &str)]) -> anyhow::Result<RewordResult> {
  let request = RewordRequest {
    repository_path: path(repo).to_string(),
    rewords: rewords.iter().map(|(commit, text)| message(commit, text)).collect(),
  };
  reword_request_core(repo.git_executor(), &request, &RegroupOptions::default())
}

fn full_message(repo: &TestRepo, commit: &str) -> String {
  repo.log(&["-1", "--pretty=format:%B", commit]).unwrap().trim_end().to_string()
}

fn tree(repo: &TestRepo, commit: &str) -> String {
  repo.git_executor().execute_command(&["rev-parse", &format!("{commit}^{{tree}}")], path(repo)).unwrap()
}

fn assert_invalid(error: &RegroupError, needle: &str) {
  assert!(matches!(error, RegroupError::InvalidPlan { message, .. } if message.contains(needle)), "{error}");
}

fn assert_stranded(error: &RegroupError, expected: FixupRefusal) {
  assert!(matches!(error, RegroupError::FixupsStranded { refusal, .. } if *refusal == expected), "{error}");
}

#[test]
fn rewords_messages_in_one_rewrite() {
  let repo = baseline_repo(false);
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
  let repo = baseline_repo(false);
  commits(&repo, &["Two\n\nThe body"]);
  let head = repo.head();

  reword(&repo, &[subject(&head, "Two, shorter")]).unwrap();
  assert_eq!(
    repo.git_executor().execute_command(&["log", "-1", "--format=%B"], path(&repo)).unwrap(),
    "Two, shorter\n\nThe body"
  );

  // cleaned up as `git commit` does: LF line endings, no trailing whitespace, no blank lines around
  let head = repo.head();
  let result = app_reword(&repo, &[(&head, "\r\nTwo, cleaned  \r\n\r\nThe body\r\n\r\n")]).unwrap();
  assert_eq!(full_message(&repo, &result.new_commit_ids[&head]), "Two, cleaned\n\nThe body");

  let head = repo.head();
  let outcome = reword(&repo, &[message(&head, "\nTwo, cleaned  \n\nThe body\n\n")]).unwrap();
  assert_eq!((outcome.changed_count(), outcome.new_head.as_str()), (0, head.as_str()));
  // the app keeps the commit and gets no undo
  let result = app_reword(&repo, &[(&head, "Two, cleaned\n\nThe body\n")]).unwrap();
  assert_eq!(
    (result.new_commit_ids[&head].as_str(), result.new_head.as_str(), result.undo_token),
    (head.as_str(), head.as_str(), None)
  );
}

#[test]
fn a_new_prefix_moves_the_commit_with_its_fixups() {
  let repo = baseline_repo(false);
  let ids = commits(&repo, &["(a) Fix login", "fixup! (a) Fix login", "(b) Other"]);
  let outcome = reword(&repo, &[subject(&ids[0], "(b) Fix login")]).unwrap();
  assert_eq!(subjects(&repo, 3), ["(b) Fix login", "fixup! (b) Fix login", "(b) Other"]);
  assert_eq!(groups(&outcome), [Some("b"), Some("b"), Some("b")]);
  assert_eq!(outcome.commits[0].previous_group.as_deref(), Some("a"));
}

#[test]
fn a_reword_that_strands_a_fixup_is_refused() {
  // by subject: the fixup would fold into nothing
  let repo = baseline_repo(false);
  let ids = commits(&repo, &["Fix login", "fixup! Fix login"]);
  let head = repo.head();
  // the app says how it folds them
  let error = app_reword(&repo, &[(&ids[0], "Fix the login")]).unwrap_err().to_string();
  assert_eq!(
    error,
    format!(
      "Fixups would fold into another target or none: {} (squash the fixups into their target first)",
      &ids[1][..8]
    )
  );
  assert_stranded(&reword(&repo, &[subject(&ids[0], "Fix the login")]).unwrap_err(), FixupRefusal::Retargeted);
  // a marker removed
  assert_stranded(&reword(&repo, &[subject(&ids[1], "Fix login again")]).unwrap_err(), FixupRefusal::MarkerChanged);
  assert_eq!(repo.head(), head, "nothing is written");

  // by id: the target gets a new id
  let repo = baseline_repo(false);
  let ids = commits(&repo, &["One", "Two"]);
  let by_id = repo.create_commit(&format!("fixup! {}", &ids[1][..8]), "two.txt", "fixed");
  assert_stranded(&reword(&repo, &[subject(&ids[0], "One, reworded")]).unwrap_err(), FixupRefusal::NamedById);
  // the same for a regroup, which recreates the target too
  assert_stranded(&regroup(&repo, &plan(&[("a", &[&ids[0]])], &[])).unwrap_err(), FixupRefusal::NamedById);
  // rewording the fixup itself is no change of its target
  reword(&repo, &[subject(&by_id, &format!("fixup! {}", &ids[1][..10]))]).unwrap();

  // a waiting amend! would replace the new message at the next fold
  let repo = baseline_repo(false);
  let ids = commits(&repo, &["One", "amend! One\n\nThe message the amend! gives"]);
  assert_stranded(&reword(&repo, &[message(&ids[0], "One\n\nAnother body")]).unwrap_err(), FixupRefusal::AmendWaits);
}

#[test]
fn a_plan_that_rewords_a_fixup_with_its_target_passes() {
  let repo = baseline_repo(false);
  let ids = commits(&repo, &["Fix login", "fixup! Fix login"]);
  reword(&repo, &[subject(&ids[0], "Fix the login"), subject(&ids[1], "fixup! Fix the login")]).unwrap();
  assert_eq!(subjects(&repo, 2), ["Fix the login", "fixup! Fix the login"]);
}

#[test]
fn a_reword_keeps_the_issue_groups_and_the_stack_parents() {
  let repo = baseline_repo(true);
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
  let repo = baseline_repo(true);
  let ids = commits(&repo, &["(auth) Add auth", "(auth/login) Add form"]);
  let error = reword(&repo, &[subject(&ids[0], "(login/auth) Add auth")]).unwrap_err();
  assert_invalid(&error, "break the stacks");
}

#[test]
fn refuses_ids_and_texts_it_cannot_reword() {
  let repo = baseline_repo(false);
  let initial = repo.head();
  let ids = commits(&repo, &["One"]);
  let error = reword(&repo, &[subject(&initial, "Initial")]).unwrap_err();
  assert!(matches!(&error, RegroupError::NotAheadOfBaseline { commits } if *commits == [initial]), "{error}");
  let error = reword(&repo, &[subject("abcdef12", "Nothing")]).unwrap_err();
  assert!(matches!(error, RegroupError::NotAheadOfBaseline { .. }), "{error}");
  assert_invalid(&reword(&repo, &[message(&ids[0], "  \n\n  ")]).unwrap_err(), "subject line");
  // also when another reword of the same rewrite is fine
  let second = repo.create_commit("Two", "two.txt", "two");
  assert_invalid(&reword(&repo, &[message(&ids[0], "One reworded"), message(&second, "  \n\n")]).unwrap_err(), "subject line");
  assert_eq!(repo.head(), second, "nothing is written");
  assert_invalid(&reword(&repo, &[subject(&ids[0], "Two\nlines")]).unwrap_err(), "more than one line");
  assert_invalid(&reword(&repo, &[subject(&ids[0], "A"), subject(&ids[0], "B")]).unwrap_err(), "more than once");
  assert_invalid(&reword(&repo, &[]).unwrap_err(), "No commit");
}

#[test]
fn rewords_a_middle_commit_and_keeps_the_commits_after_it() {
  let repo = baseline_repo(false);
  let ids = commits(&repo, &["(auth) Add login", "(auth) Add logout", "(api) Add endpoint"]);
  let head_tree = tree(&repo, "HEAD");
  // uncommitted work is not touched
  std::fs::write(repo.path().join("file2.txt"), "work in progress").unwrap();

  let result = app_reword(&repo, &[(&ids[1], "(auth) Add logout endpoint\n\nClears the session cookie.")]).unwrap();

  let new_id = &result.new_commit_ids[&ids[1]];
  assert_ne!(new_id, &ids[1]);
  assert_eq!(full_message(&repo, new_id), "(auth) Add logout endpoint\n\nClears the session cookie.");
  assert_eq!(subjects(&repo, 3), ["(auth) Add login", "(auth) Add logout endpoint", "(api) Add endpoint"]);
  assert_eq!(&repo.rev_parse("HEAD~1").unwrap(), new_id);
  assert_eq!((result.previous_head.as_str(), result.new_head.as_str()), (ids[2].as_str(), repo.head().as_str()));
  assert_eq!(result.undo_token, Some(format!("{}..{}", ids[2], repo.head())));
  assert_eq!(tree(&repo, "HEAD"), head_tree);
  assert_eq!(std::fs::read_to_string(repo.path().join("file2.txt")).unwrap(), "work in progress");
}

#[test]
fn rewords_several_commits_in_one_pass() {
  let repo = baseline_repo(false);
  let ids = commits(&repo, &["(feature-x) ABC-1 Add login", "ABC-1 Add logout", "(feature-x) ABC-1 Add session"]);
  let head_tree = tree(&repo, "HEAD");

  let result = app_reword(&repo, &[(&ids[0], "ABC-1 Add login"), (&ids[1], "ABC-1 Add logout"), (&ids[2], "ABC-1 Add session")]).unwrap();

  assert_eq!(subjects(&repo, 3), ["ABC-1 Add login", "ABC-1 Add logout", "ABC-1 Add session"]);
  assert_eq!(result.new_commit_ids[&ids[2]], repo.head());
  assert_eq!(result.new_commit_ids[&ids[0]], repo.rev_parse("HEAD~2").unwrap());
  // its message did not change, but it is recreated on top of the reworded first commit
  assert_eq!(result.new_commit_ids[&ids[1]], repo.rev_parse("HEAD~1").unwrap());
  assert_ne!(result.new_commit_ids[&ids[1]], ids[1]);
  assert_eq!(tree(&repo, "HEAD"), head_tree);
}

#[test]
fn refuses_to_reword_while_a_merge_is_in_progress() {
  let repo = baseline_repo(false);
  let ids = commits(&repo, &["Add login"]);
  std::fs::write(repo.path().join("file0.txt"), "changed").unwrap();
  std::fs::write(repo.path().join(".git/MERGE_HEAD"), format!("{}\n", ids[0])).unwrap();

  let error = reword(&repo, &[message(&ids[0], "Add login form")]).unwrap_err();

  assert!(
    matches!(&error, RegroupError::OperationInProgress(message) if message.contains("a merge is in progress")),
    "{error}"
  );
  assert_eq!(repo.head(), ids[0]);
  assert_eq!(std::fs::read_to_string(repo.path().join("file0.txt")).unwrap(), "changed");
}

#[test]
fn rewrite_refused_on_detached_head() {
  let repo = baseline_repo(false);
  let ids = commits(&repo, &["Add login"]);
  repo.checkout(&ids[0]).unwrap();

  let error = reword(&repo, &[message(&ids[0], "Add login form")]).unwrap_err();

  assert!(matches!(&error, RegroupError::DetachedHead(message) if message.contains("detached HEAD")), "{error}");
  assert_eq!(repo.head(), ids[0]);
}

/// The app's list was read before the commit was reworded or dropped in a terminal: the app says to sync
#[test]
fn refuses_a_commit_that_left_the_current_branch() {
  let repo = baseline_repo(false);
  let ids = commits(&repo, &["Add login"]);
  let replacement = app_reword(&repo, &[(&ids[0], "Add the login form")]).unwrap().new_head;

  let error = app_reword(&repo, &[(&ids[0], "Add login page")]).unwrap_err();

  assert_eq!(error.to_string(), format!("Commit {} is no longer on the current branch, sync and try again", &ids[0][..8]));
  assert_eq!(repo.head(), replacement);
}

/// `git status` and the index, to compare before and after a rewrite
fn index_and_status(repo: &TestRepo) -> (String, String) {
  let git = repo.git_executor();
  (
    String::from_utf8_lossy(&git.cmd(&["status", "--porcelain=v2", "-z"], path(repo)).bytes().unwrap()).into_owned(),
    git.execute_command(&["ls-files", "-s"], path(repo)).unwrap(),
  )
}

fn identities(repo: &TestRepo, range: &str) -> Vec<String> {
  repo
    .log(&["--date=raw", "--format=%an <%ae> %ad | %cn <%ce> %cd", range])
    .unwrap()
    .lines()
    .map(str::to_string)
    .collect()
}

#[test]
fn reword_preserves_committer() {
  let repo = TestRepo::new();
  let git = repo.git_executor();
  let commit = |message: &str, file: &str, date: &str| {
    std::fs::write(repo.path().join(file), message).unwrap();
    git.execute_command(&["add", file], path(&repo)).unwrap();
    let env = [
      ("GIT_AUTHOR_NAME", "Ann Author"),
      ("GIT_AUTHOR_EMAIL", "ann@example.com"),
      ("GIT_AUTHOR_DATE", date),
      ("GIT_COMMITTER_NAME", "Carl Committer"),
      ("GIT_COMMITTER_EMAIL", "carl@example.com"),
      ("GIT_COMMITTER_DATE", "1700000500 +0530"),
    ];
    git.cmd(&["commit", "-q", "-m", message], path(&repo)).env(&env).text().unwrap();
    repo.head()
  };
  let base = commit("Base", "base.rs", "1700000000 +0100");
  let first = commit("(auth) Add login", "login.rs", "1700000100 -0700");
  commit("(auth) Add logout\n\nKeeps  trailing   spaces  ", "logout.rs", "1700000200 +0200");
  let before = identities(&repo, &format!("{base}..HEAD"));
  let message_before = full_message(&repo, "HEAD");
  // staged and unstaged work
  std::fs::write(repo.path().join("login.rs"), "staged").unwrap();
  git.execute_command(&["add", "login.rs"], path(&repo)).unwrap();
  std::fs::write(repo.path().join("logout.rs"), "unstaged").unwrap();
  let state_before = index_and_status(&repo);

  app_reword(&repo, &[(&first, "(auth) Add the login form")]).unwrap();

  assert_eq!(identities(&repo, &format!("{base}..HEAD")), before);
  assert_eq!(full_message(&repo, "HEAD"), message_before);
  assert_eq!(index_and_status(&repo), state_before);
  let reflog = repo.log(&["-g", "-1", "--format=%gs"]).unwrap();
  assert!(reflog.starts_with("branch-deck: reword"), "{reflog}");
  assert_eq!(git.execute_command(&["for-each-ref", "refs/branch-deck/"], path(&repo)).unwrap(), "");
}

/// The Undo of the app's reword moves HEAD back with its token: the next sync finds the old messages and builds the
/// virtual branch from the old commits again, its copy reused by the mapping notes
#[test]
fn undo_of_a_reword_brings_back_the_old_virtual_commits() {
  let repo = baseline_repo(false);
  let ids = commits(&repo, &["(auth) Add login", "(cache) Add cache"]);
  sync(&repo);
  let virtual_tips = |repo: &TestRepo| {
    repo
      .git_executor()
      .execute_command(&["rev-parse", "test-prefix/virtual/auth", "test-prefix/virtual/cache"], path(repo))
      .unwrap()
  };
  let before = virtual_tips(&repo);

  let result = app_reword(&repo, &[(&ids[0], "(auth) Add login\n\nWith a remember-me box.")]).unwrap();
  sync(&repo);
  assert_ne!(virtual_tips(&repo), before);

  let token = result.undo_token.unwrap();
  undo_regroup(
    repo.git_executor(),
    &UndoRegroupParams {
      repository_path: path(&repo).to_string(),
      token,
    },
    LockWait::default(),
  )
  .unwrap();
  assert_eq!(repo.head(), ids[1]);
  sync(&repo);
  assert_eq!(virtual_tips(&repo), before);
}
