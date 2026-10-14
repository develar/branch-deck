use crate::commit_grouper::GroupingInputs;
use crate::landed::{dismiss_landed, read_report, record_sync};
use crate::sync::{SyncOptions, sync_branches};
use crate::update_project::check_baseline_update;
use crate::update_project::update_project_test::{Setup, git, path, setup, update};
use git_ops::update_branch::{UpdateKind, UpdateState};
use pretty_assertions::assert_eq;
use std::fs;
use sync_test_utils::TestReporter;
use sync_types::SyncEvent;
use sync_types::landed::LandedBranch;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// Commit `files` upstream as a robot would push the local commit `local` of `clone`: its author and author time, the
/// committer `Patronus`
fn land_as(upstream: &TestRepo, clone: &TestRepo, local: &str, message: &str, files: &[(&str, &str)]) -> String {
  let author = git(clone, &["log", "-1", "--format=%an%x00%ae%x00%ad", "--date=raw", local]);
  let mut fields = author.split('\0');
  let (name, email, date) = (fields.next().unwrap(), fields.next().unwrap(), fields.next().unwrap());
  for (file, content) in files {
    fs::write(upstream.path().join(file), content).unwrap();
    git(upstream, &["add", file]);
  }
  let env = [
    ("GIT_AUTHOR_NAME", name),
    ("GIT_AUTHOR_EMAIL", email),
    ("GIT_AUTHOR_DATE", date),
    ("GIT_COMMITTER_NAME", "Patronus"),
    ("GIT_COMMITTER_EMAIL", "patronus@example.com"),
  ];
  upstream.git_executor().execute_command_with_env(&["commit", "-m", message], path(upstream), &env).unwrap();
  upstream.head()
}

/// The commits of each landed branch, as `name: subject -> upstream id` lines, then the count that stays
fn describe(branches: &[LandedBranch]) -> Vec<String> {
  branches
    .iter()
    .map(|branch| {
      let commits: Vec<String> = branch
        .commits
        .iter()
        .map(|c| format!("{} -> {}", c.subject, c.upstream.as_ref().map_or("squashed", |u| u.id.as_str())))
        .collect();
      format!("{}: {} | {} stay", branch.name.as_deref().unwrap_or("unassigned"), commits.join(", "), branch.remaining)
    })
    .collect()
}

fn landing(repo: &TestRepo) -> Vec<String> {
  describe(&check_baseline_update(repo.git_executor(), path(repo), "origin/master").unwrap().unwrap().landing)
}

/// What the check inside a sync records: the undismissed report
fn record(repo: &TestRepo) -> Vec<String> {
  let check = check_baseline_update(repo.git_executor(), path(repo), "origin/master").unwrap().unwrap().check;
  describe(&record_sync(repo.git_executor(), path(repo), &check, &GroupingInputs::default()).unwrap())
}

#[test]
fn whole_branch_landed_by_a_rebase_upstream() {
  let Setup { upstream, clone } = setup();
  let login = clone.create_commit("(auth) Add login", "login.txt", "login\n");
  let logout = clone.create_commit("(auth) Add logout", "logout.txt", "logout\n");
  clone.create_commit("(ui) Add button", "button.txt", "button\n");
  upstream.create_commit("Someone else", "shared.txt", "one\nTWO\nthree\n");
  let landed_login = land_as(&upstream, &clone, &login, "Add login", &[("login.txt", "login\n")]);
  let landed_logout = land_as(&upstream, &clone, &logout, "Add logout", &[("logout.txt", "logout\n")]);
  git(&clone, &["fetch", "origin"]);

  let expected = vec![format!("auth: Add login -> {landed_login}, Add logout -> {landed_logout} | 0 stay")];
  assert_eq!(landing(&clone), expected);

  let result = update(&clone).unwrap();
  assert_eq!(describe(&result.landed), expected);
  let upstream_commit = result.landed[0].commits[0].upstream.as_ref().unwrap();
  assert_eq!(upstream_commit.committer, "Patronus");
  assert_eq!(git(&clone, &["log", "--format=%s", "-1"]), "(ui) Add button");
  assert_eq!(landing(&clone), Vec::<String>::new(), "nothing is incoming any more");
  assert_eq!(describe(&read_report(clone.git_executor(), path(&clone)).unwrap()), expected);
  assert_eq!(record(&clone), expected, "the sync after the update doesn't report it again");
}

#[test]
fn part_of_a_branch_landed() {
  let Setup { upstream, clone } = setup();
  let login = clone.create_commit("(auth) Add login", "login.txt", "login\n");
  clone.create_commit("(auth) Add logout", "logout.txt", "logout\n");
  let landed = land_as(&upstream, &clone, &login, "Add login", &[("login.txt", "login\n")]);
  git(&clone, &["fetch", "origin"]);

  assert_eq!(landing(&clone), vec![format!("auth: Add login -> {landed} | 1 stay")]);
}

/// Commit with an author and committer time; commits made within one second share a key otherwise
fn commit_at(repo: &TestRepo, message: &str, file: &str, content: &str, time: i64) -> String {
  repo.create_commit_with_timestamp(message, file, content, Some(time))
}

/// `IJPL-1 Add login` and its fixup locally; upstream the robot's commit with the fixup folded in, returned
fn push_with_a_fixup(upstream: &TestRepo, clone: &TestRepo) -> String {
  let login = commit_at(clone, "IJPL-1 Add login", "login.txt", "login\n", 1_700_000_100);
  commit_at(clone, "fixup! IJPL-1 Add login", "login.txt", "login fixed\n", 1_700_000_200);
  land_as(upstream, clone, &login, "IJPL-1 Add login", &[("login.txt", "login fixed\n")])
}

/// The landing of a check that must be a clean rebase
fn clean_landing(repo: &TestRepo) -> Vec<String> {
  let check = check_baseline_update(repo.git_executor(), path(repo), "origin/master").unwrap().unwrap();
  assert_eq!(check.check.state, UpdateState::Clean { kind: UpdateKind::Rebase });
  describe(&check.landing)
}

// An issue branch keeps the subjects as they are, so its fixups are pushed with the marker and a robot that
// autosquashes folds them into the target: the target alone adds other content than the upstream has, and a plain
// replay conflicts, as `git rebase` does
#[test]
fn fixups_folded_upstream_landed_with_their_target() {
  let Setup { upstream, clone } = setup();
  let landed = push_with_a_fixup(&upstream, &clone);
  commit_at(&clone, "(ui) Add button", "button.txt", "button\n", 1_700_000_300);
  git(&clone, &["fetch", "origin"]);

  let expected = vec![format!("IJPL-1: IJPL-1 Add login -> {landed}, fixup! IJPL-1 Add login -> {landed} | 0 stay")];
  assert_eq!(clean_landing(&clone), expected);

  let result = update(&clone).unwrap();
  assert_eq!(describe(&result.landed), expected);
  assert_eq!(git(&clone, &["log", "--format=%s", "-3"]), "(ui) Add button\nIJPL-1 Add login\nInitial");
  assert_eq!(fs::read_to_string(clone.path().join("login.txt")).unwrap(), "login fixed\n");
}

#[test]
fn fixup_made_after_the_push_stays_local() {
  let Setup { upstream, clone } = setup();
  let landed = push_with_a_fixup(&upstream, &clone);
  // after the Safe Push started
  commit_at(&clone, "fixup! IJPL-1 Add login", "login.txt", "login fixed\nmore\n", 1_700_000_300);
  git(&clone, &["fetch", "origin"]);

  assert_eq!(
    clean_landing(&clone),
    vec![format!("IJPL-1: IJPL-1 Add login -> {landed}, fixup! IJPL-1 Add login -> {landed} | 1 stay")]
  );

  update(&clone).unwrap();
  assert_eq!(git(&clone, &["log", "--format=%s", "-2"]), "fixup! IJPL-1 Add login\nIJPL-1 Add login");
  assert_eq!(fs::read_to_string(clone.path().join("login.txt")).unwrap(), "login fixed\nmore\n");
}

#[test]
fn commit_amended_after_the_push_is_never_left_out() {
  let Setup { upstream, clone } = setup();
  let login = clone.create_commit("(auth) Add login", "login.txt", "login\n");
  land_as(&upstream, &clone, &login, "Add login", &[("login.txt", "login\n")]);
  // an amend keeps the author time and the subject
  fs::write(clone.path().join("login.txt"), "login\nextra\n").unwrap();
  git(&clone, &["commit", "-q", "-a", "--amend", "--no-edit"]);
  let head = clone.head();
  git(&clone, &["fetch", "origin"]);

  let check = check_baseline_update(clone.git_executor(), path(&clone), "origin/master").unwrap().unwrap().check;
  assert!(matches!(check.state, UpdateState::Conflict { .. }), "{:?}", check.state);
  assert!(update(&clone).unwrap().applied.is_none());
  assert_eq!(clone.head(), head, "nothing changed, the amend is kept");
}

#[test]
fn branch_squashed_upstream_landed_without_an_upstream_commit() {
  let Setup { upstream, clone } = setup();
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  clone.create_commit("(auth) Add logout", "logout.txt", "logout\n");
  upstream.create_commit_with_files("Auth (#12)", &[("login.txt", "login\n"), ("logout.txt", "logout\n")]);
  git(&clone, &["fetch", "origin"]);

  assert_eq!(landing(&clone), vec!["auth: Add login -> squashed, Add logout -> squashed | 0 stay".to_string()]);
}

#[test]
fn landed_commits_are_reported_while_the_update_conflicts() {
  let Setup { upstream, clone } = setup();
  let login = clone.create_commit("(auth) Add login", "login.txt", "login\n");
  clone.create_commit("(ui) Change two", "shared.txt", "one\nlocal two\nthree\n");
  let landed = land_as(&upstream, &clone, &login, "Add login", &[("login.txt", "login\n")]);
  upstream.create_commit("Upstream two", "shared.txt", "one\nupstream two\nthree\n");
  git(&clone, &["fetch", "origin"]);

  let check = check_baseline_update(clone.git_executor(), path(&clone), "origin/master").unwrap().unwrap();
  assert!(matches!(check.check.state, UpdateState::Conflict { .. }), "{:?}", check.check.state);
  assert_eq!(describe(&check.landing), vec![format!("auth: Add login -> {landed} | 0 stay")]);
}

#[test]
fn same_author_and_time_with_another_subject_is_not_landed() {
  let Setup { upstream, clone } = setup();
  let login = clone.create_commit("(auth) Add login", "login.txt", "login\n");
  land_as(&upstream, &clone, &login, "Add sign-in", &[("signin.txt", "sign-in\n")]);
  git(&clone, &["fetch", "origin"]);

  assert_eq!(landing(&clone), Vec::<String>::new());
}

#[test]
fn update_made_elsewhere_is_reported_by_the_next_sync_once() {
  let Setup { upstream, clone } = setup();
  let login = clone.create_commit("(auth) Add login", "login.txt", "login\n");
  clone.create_commit("(ui) Add button", "button.txt", "button\n");
  assert_eq!(record(&clone), Vec::<String>::new(), "the first sync only remembers where the branch is");

  let landed = land_as(&upstream, &clone, &login, "Add login", &[("login.txt", "login\n")]);
  // the IDE's Update Project
  git(&clone, &["pull", "--rebase"]);
  assert_eq!(git(&clone, &["log", "--format=%s", "-2"]), "(ui) Add button\nAdd login");

  let expected = vec![format!("auth: Add login -> {landed} | 0 stay")];
  assert_eq!(record(&clone), expected);
  assert_eq!(record(&clone), expected, "reported once, kept until dismissed");

  dismiss_landed(clone.git_executor(), path(&clone)).unwrap();
  assert_eq!(record(&clone), Vec::<String>::new());
}

#[test]
fn reword_squash_or_amend_between_syncs_is_not_landed() {
  let Setup { upstream: _upstream, clone } = setup();
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  clone.create_commit("(auth) Add logout", "logout.txt", "logout\n");
  assert_eq!(record(&clone), Vec::<String>::new());

  git(&clone, &["reset", "--soft", "HEAD~2"]);
  git(&clone, &["commit", "-m", "(auth) Add login and logout"]);
  assert_eq!(record(&clone), Vec::<String>::new());
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_sends_the_landing_and_the_report() -> anyhow::Result<()> {
  let Setup { upstream, clone } = setup();
  let login = clone.create_commit("(auth) Add login", "login.txt", "login\n");
  clone.create_commit("(ui) Add button", "button.txt", "button\n");
  let landed = land_as(&upstream, &clone, &login, "Add login", &[("login.txt", "login\n")]);
  git(&clone, &["fetch", "origin"]);

  let baseline_status = || async {
    let progress = TestReporter::new();
    sync_branches(clone.git_executor(), path(&clone), "test", progress.clone(), SyncOptions::default()).await?;
    let event = progress.get_events().into_iter().find(|e| matches!(e, SyncEvent::BaselineStatus { .. })).unwrap();
    let SyncEvent::BaselineStatus { landing, landed, .. } = event else { unreachable!() };
    anyhow::Ok((describe(&landing), describe(&landed)))
  };
  let expected = vec![format!("auth: Add login -> {landed} | 0 stay")];
  assert_eq!(baseline_status().await?, (expected.clone(), Vec::new()));

  update(&clone)?;
  assert_eq!(baseline_status().await?, (Vec::new(), expected));
  Ok(())
}
