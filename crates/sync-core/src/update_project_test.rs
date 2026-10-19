use crate::repository_lock::{RepositoryLocked, lock_repository};
use crate::sync::{SyncOptions, sync_branches};
use crate::update_project::{
  FetchFailed, LOCKED_MESSAGE, UndoBlocked, UndoneUpdate, UpdateOptions, UpdatePhase, UpdateProgress, UpdateProjectParams, UpdateProjectResult, UpdateUndoToken, app_error,
  check_baseline_update, previous_tip_of_forced_update, undo_update_core, update_project_core, update_project_with,
};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::rewrite_guard::HeadMoved;
use git_ops::update_branch::{
  BranchSwitched, CheckPurpose, NotAnUpdate, UndoCheck, UpdateCommit, UpdateKind, UpdatePlan, UpdateState, UpstreamRef, apply_update, apply_update_undo, check_update,
  check_update_undo, no_landed_candidates,
};
use pretty_assertions::assert_eq;
use std::cell::RefCell;
use std::fmt::Write;
use std::fs;
use std::time::Duration;
use sync_test_utils::TestReporter;
use sync_types::SyncEvent;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// An upstream repository with `shared.txt` and `other.txt`, and a clone of it whose `master` tracks `origin/master`
pub(crate) struct Setup {
  pub(crate) upstream: TestRepo,
  pub(crate) clone: TestRepo,
}

const SHARED: &str = "one\ntwo\nthree\n";

pub(crate) fn setup() -> Setup {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("shared.txt", SHARED), ("other.txt", "other\n")]);
  Setup { upstream, clone }
}

pub(crate) fn path(repo: &TestRepo) -> &str {
  repo.path().to_str().unwrap()
}

pub(crate) fn git(repo: &TestRepo, args: &[&str]) -> String {
  repo.git_executor().execute_command(args, path(repo)).unwrap()
}

pub(crate) fn update(repo: &TestRepo) -> anyhow::Result<UpdateProjectResult> {
  update_project_core(
    repo.git_executor(),
    &UpdateProjectParams {
      repository_path: path(repo).to_string(),
    },
    &|_| {},
  )
}

/// `update`, with everything it reported while it ran
fn update_reporting_progress(repo: &TestRepo) -> (anyhow::Result<UpdateProjectResult>, Vec<UpdateProgress>) {
  let progress = RefCell::new(Vec::new());
  let result = update_project_core(
    repo.git_executor(),
    &UpdateProjectParams {
      repository_path: path(repo).to_string(),
    },
    &|it| progress.borrow_mut().push(it),
  );
  (result, progress.into_inner())
}

fn read(repo: &TestRepo, file: &str) -> String {
  fs::read_to_string(repo.path().join(file)).unwrap()
}

fn write(repo: &TestRepo, file: &str, content: &str) {
  fs::write(repo.path().join(file), content).unwrap();
}

/// `git status --porcelain` with untracked files
fn status(repo: &TestRepo) -> String {
  let output = repo.git_executor().cmd(&["status", "--porcelain", "--untracked-files=all"], path(repo)).bytes().unwrap();
  String::from_utf8_lossy(&output).trim_end().to_string()
}

const ORIGIN_MASTER: UpstreamRef<'static> = UpstreamRef {
  tracking_ref: "refs/remotes/origin/master",
  name: "origin/master",
  previous: None,
};

/// A local commit and a fetched upstream commit on another file: a clean rebase
fn fetched_rebase() -> Setup {
  let setup = setup();
  setup.clone.create_commit("(auth) Add login", "login.txt", "login\n");
  setup.upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  git(&setup.clone, &["fetch", "origin"]);
  setup
}

/// `fetched_rebase`, its HEAD, and the check of its clean rebase, ready to apply
fn checked_rebase() -> (Setup, String, UpdatePlan) {
  let setup = fetched_rebase();
  let head = setup.clone.head();
  let plan = check_update(setup.clone.git_executor(), path(&setup.clone), ORIGIN_MASTER, CheckPurpose::Apply, &no_landed_candidates).unwrap();
  assert_eq!(plan.check.state, UpdateState::Clean { kind: UpdateKind::Rebase });
  (setup, head, plan)
}

fn reflog(repo: &TestRepo) -> String {
  git(repo, &["reflog", "--format=%gs", "refs/heads/master"])
}

#[test]
fn fast_forward_updates_the_working_tree() {
  let Setup { upstream, clone } = setup();
  let upstream_head = upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  upstream.create_commit("Add new", "new.txt", "new\n");

  let (result, progress) = update_reporting_progress(&clone);
  let mut phases: Vec<UpdatePhase> = progress.iter().map(|it| it.phase).collect();
  phases.dedup();
  assert_eq!(phases, vec![UpdatePhase::Fetching, UpdatePhase::Checking, UpdatePhase::Updating]);
  let result = result.unwrap();
  assert_eq!(result.check.state, UpdateState::Clean { kind: UpdateKind::FastForward });
  let applied = result.applied.unwrap();
  assert_eq!(applied.kind, UpdateKind::FastForward);
  assert_eq!(clone.head(), upstream.head());
  assert_eq!(applied.new_head, upstream.head());
  assert_eq!(git(&clone, &["rev-parse", "HEAD~1"]), upstream_head);
  assert_eq!(read(&clone, "shared.txt"), "one\nTWO\nthree\n");
  assert_eq!(read(&clone, "new.txt"), "new\n");
  assert_eq!(status(&clone), "");
  assert!(result.check.fetched_at.is_some());
  assert!(reflog(&clone).starts_with("branch-deck: update project\n"), "{}", reflog(&clone));
}

#[test]
fn rebase_keeps_the_author_and_makes_the_current_user_the_committer() {
  let Setup { upstream, clone } = setup();
  clone.set_config("user.name", "Local Author").unwrap();
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  clone.create_commit("(auth) Add logout", "logout.txt", "logout\n");
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  clone.set_config("user.name", "Updater").unwrap();

  let result = update(&clone).unwrap();
  assert_eq!(result.check.state, UpdateState::Clean { kind: UpdateKind::Rebase });
  let applied = result.applied.unwrap();
  assert_eq!((applied.replayed, applied.dropped_already_upstream), (2, 0));
  assert_eq!(git(&clone, &["rev-parse", "HEAD~2"]), upstream.head());
  assert_eq!(
    git(&clone, &["log", "--format=%s|%an|%cn", "-2"]),
    "(auth) Add logout|Local Author|Updater\n(auth) Add login|Local Author|Updater"
  );
  assert_eq!(read(&clone, "shared.txt"), "one\nTWO\nthree\n");
  assert_eq!(read(&clone, "login.txt"), "login\n");
  assert_eq!(status(&clone), "");
}

#[test]
fn dirty_and_staged_files_on_untouched_paths_survive_byte_identical() {
  let Setup { upstream, clone } = setup();
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  write(&clone, "other.txt", "dirty other\r\nno newline");
  write(&clone, "login.txt", "staged login\n");
  git(&clone, &["add", "login.txt"]);
  write(&clone, "login.txt", "staged login\nand unstaged on top\n");
  write(&clone, "untracked.txt", "untracked\n");
  let staged_blob = git(&clone, &["rev-parse", ":login.txt"]);

  let result = update(&clone).unwrap();
  assert_eq!(result.check.state, UpdateState::Clean { kind: UpdateKind::Rebase });
  assert_eq!(read(&clone, "shared.txt"), "one\nTWO\nthree\n");
  assert_eq!(read(&clone, "other.txt"), "dirty other\r\nno newline");
  assert_eq!(read(&clone, "login.txt"), "staged login\nand unstaged on top\n");
  assert_eq!(read(&clone, "untracked.txt"), "untracked\n");
  assert_eq!(git(&clone, &["rev-parse", ":login.txt"]), staged_blob, "the staged version is kept");
  assert_eq!(status(&clone), "MM login.txt\n M other.txt\n?? untracked.txt");
}

#[test]
fn dirty_touched_path_blocks_and_changes_nothing() {
  let Setup { upstream, clone } = setup();
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  write(&clone, "shared.txt", "one\ntwo\nthree\nlocal\n");
  let head = clone.head();
  let reflog_before = reflog(&clone);

  let result = update(&clone).unwrap();
  assert_eq!(result.check.state, UpdateState::blocked(vec!["shared.txt".to_string()]));
  assert!(result.applied.is_none());
  assert_eq!(clone.head(), head);
  assert_eq!(read(&clone, "shared.txt"), "one\ntwo\nthree\nlocal\n");
  assert_eq!(reflog(&clone), reflog_before);
  // the fetch did happen: the tracking ref is at the upstream
  assert_eq!(git(&clone, &["rev-parse", "origin/master"]), upstream.head());
}

#[test]
fn staged_touched_path_blocks() {
  let Setup { upstream, clone } = setup();
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  write(&clone, "shared.txt", "staged\n");
  git(&clone, &["add", "shared.txt"]);
  write(&clone, "shared.txt", SHARED);
  let head = clone.head();

  let result = update(&clone).unwrap();
  assert_eq!(result.check.state, UpdateState::blocked(vec!["shared.txt".to_string()]));
  assert_eq!(clone.head(), head);
  assert_eq!(git(&clone, &["show", ":shared.txt"]), "staged");
}

#[test]
fn untracked_file_that_upstream_adds_blocks() {
  let Setup { upstream, clone } = setup();
  upstream.create_commit("Add config", "dir/config.txt", "upstream\n");
  fs::create_dir_all(clone.path().join("dir")).unwrap();
  write(&clone, "dir/config.txt", "mine\n");
  let head = clone.head();

  let result = update(&clone).unwrap();
  assert_eq!(result.check.state, UpdateState::blocked(vec!["dir/config.txt".to_string()]));
  assert_eq!(clone.head(), head);
  assert_eq!(read(&clone, "dir/config.txt"), "mine\n");
}

#[test]
fn ignored_file_that_upstream_adds_blocks() {
  let Setup { upstream, clone } = setup();
  upstream.create_commit("Add build output", "build.out", "upstream\n");
  write(&clone, ".git/info/exclude", "build.out\n");
  write(&clone, "build.out", "mine\n");
  let head = clone.head();

  let result = update(&clone).unwrap();
  // git would overwrite an ignored file silently
  assert_eq!(result.check.state, UpdateState::blocked(vec!["build.out".to_string()]));
  assert_eq!(clone.head(), head);
  assert_eq!(read(&clone, "build.out"), "mine\n");
}

#[test]
fn touched_only_file_passes_thanks_to_the_refresh() {
  let Setup { upstream, clone } = setup();
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  // same content, new stat data: the index entry is stale until refreshed
  let file = fs::File::options().write(true).open(clone.path().join("shared.txt")).unwrap();
  file.set_modified(std::time::SystemTime::now() + Duration::from_secs(5)).unwrap();
  drop(file);

  let result = update(&clone).unwrap();
  assert_eq!(result.check.state, UpdateState::Clean { kind: UpdateKind::FastForward });
  assert_eq!(read(&clone, "shared.txt"), "one\nTWO\nthree\n");
  assert_eq!(status(&clone), "");
}

#[test]
fn same_line_conflict_reports_files_and_commits_and_changes_nothing() {
  let Setup { upstream, clone } = setup();
  let first = clone.create_commit_with_timestamp("(auth) Add login", "login.txt", "login\n", Some(1_700_000_100));
  let second = clone.create_commit_with_timestamp("(auth) Change two", "shared.txt", "one\nlocal two\nthree\n", Some(1_700_000_200));
  upstream.create_commit("Upstream other", "other.txt", "upstream other\n");
  // written by Dana Lee, landed a day later by a robot: the committer is another person at another time
  write(&upstream, "shared.txt", "one\nupstream two\nthree\n");
  git(&upstream, &["add", "shared.txt"]);
  upstream
    .git_executor()
    .cmd(&["commit", "-q", "-m", "Upstream change"], path(&upstream))
    .env(&[
      ("GIT_AUTHOR_NAME", "Dana Lee"),
      ("GIT_AUTHOR_DATE", "1700000300 +0000"),
      ("GIT_COMMITTER_NAME", "Patronus"),
      ("GIT_COMMITTER_DATE", "1700090000 +0000"),
    ])
    .text()
    .unwrap();
  let upstream_change = upstream.head();
  let head = clone.head();
  let reflog_before = reflog(&clone);

  let result = update(&clone).unwrap();
  let check = result.check;
  let commit = |id: String, subject: &str, (author, time): (&str, u32), (committer, committer_time): (&str, u32), files: &[&str]| UpdateCommit {
    id,
    subject: subject.to_string(),
    author: author.to_string(),
    time,
    committer: committer.to_string(),
    committer_time,
    files: files.iter().map(ToString::to_string).collect(),
  };
  // only the commits that change the conflicting file carry it; the upstream lists only those
  assert_eq!(
    check.state,
    UpdateState::Conflict {
      files: vec!["shared.txt".to_string()],
      file_count: 1,
      commits: vec![
        commit(first, "(auth) Add login", ("Test User", 1_700_000_100), ("Test User", 1_700_000_100), &[]),
        commit(second, "(auth) Change two", ("Test User", 1_700_000_200), ("Test User", 1_700_000_200), &["shared.txt"]),
      ],
      upstream_commits: vec![commit(
        upstream_change,
        "Upstream change",
        ("Dana Lee", 1_700_000_300),
        ("Patronus", 1_700_090_000),
        &["shared.txt"]
      )],
    }
  );
  assert_eq!((check.branch.as_str(), check.upstream.as_str()), ("master", "origin/master"));
  assert_eq!(check.head, head);
  assert_eq!(check.upstream_head, upstream.head());
  assert_eq!(check.base, git(&clone, &["rev-parse", "HEAD~2"]));
  assert!(result.applied.is_none());
  assert_eq!(clone.head(), head);
  assert_eq!(read(&clone, "shared.txt"), "one\nlocal two\nthree\n");
  assert_eq!(reflog(&clone), reflog_before);
  assert_eq!(git(&clone, &["for-each-ref", "refs/branch-deck/"]), "", "the replay writes no ref");
}

#[test]
fn commit_already_upstream_is_dropped_and_reported() {
  let Setup { upstream, clone } = setup();
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  clone.create_commit("(fix) Fix two", "shared.txt", "one\nfixed two\nthree\n");
  // the fix was applied upstream, e.g. merged from a pull request
  upstream.create_commit("Fix two (#12)", "shared.txt", "one\nfixed two\nthree\n");

  let applied = update(&clone).unwrap().applied.unwrap();
  assert_eq!((applied.replayed, applied.dropped_already_upstream), (1, 1));
  assert_eq!(git(&clone, &["log", "--format=%s", "-2"]), "(auth) Add login\nFix two (#12)");
  assert_eq!(status(&clone), "");
}

/// The clone takes an upstream commit and adds a local one; then the upstream drops its commit and changes the same line
/// differently, without the clone fetching it
fn force_push_after_a_local_commit(upstream: &TestRepo, clone: &TestRepo) {
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  git(clone, &["fetch", "origin"]);
  git(clone, &["merge", "--ff-only", "origin/master"]);
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  upstream.reset_hard("HEAD~1").unwrap();
  upstream.create_commit("Rewritten change", "shared.txt", "one\nRewritten\nthree\n");
}

#[test]
fn upstream_force_push_does_not_replay_the_removed_commits() {
  let Setup { upstream, clone } = setup();
  force_push_after_a_local_commit(&upstream, &clone);

  let result = update(&clone).unwrap();
  assert_eq!(result.check.state, UpdateState::Clean { kind: UpdateKind::Rebase });
  assert_eq!(git(&clone, &["log", "--format=%s", "-3"]), "(auth) Add login\nRewritten change\nInitial");
  assert_eq!(read(&clone, "shared.txt"), "one\nRewritten\nthree\n");
}

#[test]
fn check_after_a_forced_fetch_elsewhere_uses_the_fork_point() {
  let Setup { upstream, clone } = setup();
  force_push_after_a_local_commit(&upstream, &clone);
  // a fetch in a terminal: the previous tip is only in the reflog of the tracking ref
  git(&clone, &["fetch", "origin"]);

  let check = check_baseline_update(clone.git_executor(), path(&clone), "origin/master").unwrap().unwrap().check;
  assert_eq!(check.state, UpdateState::Clean { kind: UpdateKind::Rebase });
  assert!(check.fetched_at.is_some(), "the terminal fetch wrote FETCH_HEAD");
}

#[test]
fn merges_in_the_local_commits_are_refused() {
  let Setup { upstream, clone } = setup();
  clone.checkout_new_branch("side").unwrap();
  clone.create_commit("Side", "side.txt", "side\n");
  clone.checkout("master").unwrap();
  clone.create_commit("Main", "main.txt", "main\n");
  clone.merge_no_ff("side", "Merge side").unwrap();
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  let head = clone.head();

  let error = update(&clone).unwrap_err();
  assert!(format!("{error:#}").contains("merge commit"), "{error:#}");
  assert_eq!(clone.head(), head);
}

#[test]
fn up_to_date_writes_no_reflog_entry() {
  let Setup { upstream: _upstream, clone } = setup();
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  let reflog_before = reflog(&clone);

  let result = update(&clone).unwrap();
  assert_eq!(result.check.state, UpdateState::UpToDate);
  assert!(result.applied.is_none());
  assert_eq!(reflog(&clone), reflog_before);
}

#[test]
fn rolls_back_when_a_file_changes_between_the_check_and_the_update() {
  let (Setup { upstream: _upstream, clone }, head, plan) = checked_rebase();
  write(&clone, "shared.txt", "edited meanwhile\n");

  let error = apply_update(clone.git_executor(), path(&clone), &plan).unwrap_err();
  assert!(format!("{error:#}").starts_with("Files changed during the update; nothing was changed"), "{error:#}");
  assert_eq!(clone.head(), head);
  assert_eq!(read(&clone, "shared.txt"), "edited meanwhile\n");
  assert_eq!(read(&clone, "login.txt"), "login\n");
  let reflog = reflog(&clone);
  assert!(reflog.starts_with("branch-deck: update project rolled back\nbranch-deck: update project\n"), "{reflog}");
}

#[test]
fn preview_check_cannot_be_applied() {
  let Setup { upstream: _upstream, clone } = fetched_rebase();
  let upstream_ref = ORIGIN_MASTER;

  let plan = check_update(clone.git_executor(), path(&clone), upstream_ref, CheckPurpose::Preview, &no_landed_candidates).unwrap();
  assert_eq!(plan.check.state, UpdateState::Clean { kind: UpdateKind::Rebase });
  apply_update(clone.git_executor(), path(&clone), &plan).unwrap_err();
  // the same state writes the same commits: no new objects on every sync
  let objects = git(&clone, &["count-objects"]);
  check_update(clone.git_executor(), path(&clone), upstream_ref, CheckPurpose::Preview, &no_landed_candidates).unwrap();
  assert_eq!(git(&clone, &["count-objects"]), objects);
}

#[test]
fn local_baseline_has_nothing_to_update() {
  let repo = TestRepo::new();
  repo.create_commit("Initial", "README.md", "readme");
  assert!(check_baseline_update(repo.git_executor(), path(&repo), "master").unwrap().is_none());
  let error = update(&repo).unwrap_err();
  assert!(format!("{error:#}").contains("local branch"), "{error:#}");
}

#[test]
fn check_refuses_while_a_rebase_is_in_progress() {
  let Setup { upstream: _upstream, clone } = setup();
  fs::create_dir_all(clone.path().join(".git/rebase-merge")).unwrap();
  let error = check_baseline_update(clone.git_executor(), path(&clone), "origin/master").unwrap_err();
  assert!(format!("{error:#}").contains("rebase"), "{error:#}");
}

#[test]
fn parses_the_forced_update_line_of_fetch() {
  let tracking = "refs/remotes/origin/master";
  let old = "20fc9136fedad65e0043914e762d3c5d42c22a4f";
  let new = "7a6f8932dbf91ca1470d2610fecd8de9794ac1de";
  assert_eq!(previous_tip_of_forced_update(&format!("+ {old} {new} {tracking}"), tracking), Some(old.to_string()));
  // a fast-forward needs no exclusion, a new ref has no previous tip, an up-to-date ref prints nothing
  assert_eq!(previous_tip_of_forced_update(&format!("  {old} {new} {tracking}"), tracking), None);
  assert_eq!(previous_tip_of_forced_update(&format!("* {} {new} {tracking}", "0".repeat(40)), tracking), None);
  assert_eq!(previous_tip_of_forced_update("", tracking), None);
  assert_eq!(previous_tip_of_forced_update(&format!("+ {old} {new} refs/remotes/origin/other"), tracking), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_reports_the_baseline_status() -> anyhow::Result<()> {
  let Setup { upstream, clone } = setup();
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  upstream.create_commit("Upstream change", "shared.txt", "one\nupstream two\nthree\n");
  git(&clone, &["fetch", "origin"]);
  clone.create_commit("(auth) Change two", "shared.txt", "one\nlocal two\nthree\n");

  let progress = TestReporter::new();
  sync_branches(clone.git_executor(), path(&clone), "test", progress.clone(), SyncOptions::default()).await?;
  let events = progress.get_events();
  // the check runs beside the sync: one event, at any point of the stream
  let statuses: Vec<&SyncEvent> = events.iter().filter(|e| matches!(e, SyncEvent::BaselineStatus { .. })).collect();
  assert_eq!(statuses.len(), 1);
  let SyncEvent::BaselineStatus {
    baseline_branch,
    update,
    check_error,
    ..
  } = statuses[0]
  else {
    unreachable!()
  };
  assert_eq!(baseline_branch, "origin/master");
  assert_eq!(check_error, &None);
  let state = &update.as_ref().unwrap().state;
  assert!(matches!(state, UpdateState::Conflict { files, .. } if files == &["shared.txt"]), "{state:?}");
  Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_on_a_detached_head_is_deferred_before_the_update_check() -> anyhow::Result<()> {
  let Setup { upstream: _upstream, clone } = setup();
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  git(&clone, &["checkout", "--detach", "HEAD"]);

  // the check of an update would fail on it ("detached"); the sync doesn't get that far, and reports nothing
  let progress = TestReporter::new();
  let error = sync_branches(clone.git_executor(), path(&clone), "test", progress.clone(), SyncOptions::default())
    .await
    .unwrap_err();
  assert_eq!(error.downcast_ref::<crate::sync::SyncDeferred>(), Some(&crate::sync::SyncDeferred::DetachedHead));
  assert!(progress.get_events().is_empty());
  Ok(())
}

#[test]
fn e2e_templates_have_the_expected_update_states() {
  use test_utils::repo_template::templates;
  let dir = tempfile::tempdir().unwrap();
  let git_executor = GitCommandExecutor::new();
  let check = |name: &str, template: templates::UpdateTemplate| {
    let repo = dir.path().join(name);
    template.build(&repo).unwrap();
    let repo = repo.to_str().unwrap().to_string();
    let check = check_baseline_update(&git_executor, &repo, "origin/master").unwrap().unwrap().check;
    (repo, check.state)
  };

  let (repo, state) = check("behind_upstream", templates::behind_upstream());
  assert_eq!(state, UpdateState::Clean { kind: UpdateKind::Rebase });
  let result = update_project_core(&git_executor, &UpdateProjectParams { repository_path: repo.clone() }, &|_| {}).unwrap();
  assert_eq!(result.applied.unwrap().replayed, 1);
  assert_eq!(fs::read_to_string(format!("{repo}/src/config.txt")).unwrap(), "debug=true\n", "the dirty file is kept");

  let (_, state) = check("update_conflict", templates::update_conflict());
  // the dialog shows when the upstream commit was written and, more than an hour later, when it landed
  assert!(
    matches!(&state, UpdateState::Conflict { files, file_count: 1, commits, upstream_commits }
      if files == &["src/app.txt"] && commits.len() == 2 && upstream_commits.len() == 1
        && upstream_commits[0].committer_time > upstream_commits[0].time + 3600),
    "{state:?}"
  );
  let (_, state) = check("update_conflict_wide", templates::update_conflict_wide());
  // the dialog lists the first 50 files and the newest 5 of the 7 upstream commits of each
  assert!(
    matches!(&state, UpdateState::Conflict { files, file_count: 60, upstream_commits, .. }
      if files.len() == 50 && files[0] == "src/wide/file00.txt" && upstream_commits.len() == 7),
    "{state:?}"
  );
  let (repo, state) = check("landed_upstream", templates::landed_upstream());
  assert_eq!(state, UpdateState::Clean { kind: UpdateKind::Rebase });
  let landing = check_baseline_update(&git_executor, &repo, "origin/master").unwrap().unwrap().landing;
  let landing: Vec<(Option<&str>, usize, u32)> = landing.iter().map(|b| (b.name.as_deref(), b.commits.len(), b.remaining)).collect();
  assert_eq!(landing, vec![(Some("feature-auth"), 1, 0), (Some("search"), 1, 1)]);

  let (_, state) = check("update_blocked", templates::update_blocked());
  assert_eq!(state, UpdateState::blocked(vec!["src/app.txt".to_string()]));
}

/// The state of the check inside a sync: the preview, without the network
fn preview(repo: &TestRepo) -> UpdateState {
  check_baseline_update(repo.git_executor(), path(repo), "origin/master").unwrap().unwrap().check.state
}

fn blocked(paths: &[&str]) -> UpdateState {
  UpdateState::blocked(paths.iter().map(|path| (*path).to_string()).collect())
}

#[test]
fn preview_blocks_dirty_and_staged_touched_paths_without_touching_the_index() {
  let Setup { upstream: _upstream, clone } = fetched_rebase();
  let index = fs::read(clone.path().join(".git/index")).unwrap();
  assert_eq!(preview(&clone), UpdateState::Clean { kind: UpdateKind::Rebase });

  write(&clone, "shared.txt", "one\ntwo\nthree\nlocal\n");
  assert_eq!(preview(&clone), blocked(&["shared.txt"]));
  write(&clone, "shared.txt", SHARED);
  assert_eq!(preview(&clone), UpdateState::Clean { kind: UpdateKind::Rebase }, "the same content again is clean");
  assert_eq!(fs::read(clone.path().join(".git/index")).unwrap(), index, "the preview never writes the index");

  // staged, with the file as in HEAD
  write(&clone, "shared.txt", "staged\n");
  git(&clone, &["add", "shared.txt"]);
  write(&clone, "shared.txt", SHARED);
  assert_eq!(preview(&clone), blocked(&["shared.txt"]));
  git(&clone, &["reset", "-q", "shared.txt"]);
  assert_eq!(preview(&clone), UpdateState::Clean { kind: UpdateKind::Rebase });
}

#[test]
fn preview_does_not_block_a_touched_only_file() {
  let Setup { upstream: _upstream, clone } = fetched_rebase();
  let file = fs::File::options().write(true).open(clone.path().join("shared.txt")).unwrap();
  file.set_modified(std::time::SystemTime::now() + Duration::from_secs(5)).unwrap();
  drop(file);
  let index = fs::read(clone.path().join(".git/index")).unwrap();
  assert_eq!(git(&clone, &["diff-files", "--name-only"]), "shared.txt", "the index entry is stale");

  assert_eq!(preview(&clone), UpdateState::Clean { kind: UpdateKind::Rebase });
  assert_eq!(fs::read(clone.path().join(".git/index")).unwrap(), index);
}

/// Point `link` of `repo` at `target`, replacing what is there
#[cfg(unix)]
fn symlink(repo: &TestRepo, target: &str) {
  let link = repo.path().join("link");
  let _ = fs::remove_file(&link);
  std::os::unix::fs::symlink(target, link).unwrap();
}

/// A symlink that the update changes blocks only when it points elsewhere locally: its target is compared with the blob
/// of HEAD without a process
#[cfg(unix)]
#[test]
fn preview_compares_a_symlink_by_its_target() {
  let Setup { upstream, clone } = setup();
  symlink(&upstream, "shared.txt");
  upstream.git(&["add", "link"]);
  upstream.git(&["commit", "-q", "-m", "Add link"]);
  git(&clone, &["pull", "-q", "--ff-only"]);
  symlink(&upstream, "other.txt");
  upstream.git(&["commit", "-q", "-am", "Retarget link"]);
  git(&clone, &["fetch", "origin"]);

  let counting = GitCommandExecutor::new().with_process_stats("test");
  let check = check_baseline_update(&counting, path(&clone), "origin/master").unwrap().unwrap();
  assert_eq!(check.check.state, UpdateState::Clean { kind: UpdateKind::FastForward });
  let processes = counting.process_stats().unwrap().by_command();
  assert!(!processes.contains_key("hash-object"), "{processes:?}");

  symlink(&clone, "elsewhere");
  assert_eq!(preview(&clone), blocked(&["link"]));
  symlink(&clone, "shared.txt");
  assert_eq!(preview(&clone), UpdateState::Clean { kind: UpdateKind::FastForward });
}

#[test]
fn preview_blocks_untracked_and_ignored_files_that_upstream_adds() {
  let Setup { upstream, clone } = setup();
  upstream.create_commit_with_files("Add files", &[("dir/config.txt", "upstream\n"), ("build.out", "upstream\n")]);
  git(&clone, &["fetch", "origin"]);
  fs::create_dir_all(clone.path().join("dir")).unwrap();
  write(&clone, "dir/config.txt", "mine\n");
  write(&clone, ".git/info/exclude", "build.out\n");
  write(&clone, "build.out", "mine\n");
  assert_eq!(preview(&clone), blocked(&["build.out", "dir/config.txt"]));
}

#[test]
fn file_replaced_by_a_directory_upstream_is_not_in_the_way() {
  let Setup { upstream, clone } = setup();
  git(&upstream, &["rm", "-q", "other.txt"]);
  upstream.create_commit("Turn other.txt into a directory", "other.txt/inner.txt", "inner\n");
  git(&clone, &["fetch", "origin"]);
  assert_eq!(preview(&clone), UpdateState::Clean { kind: UpdateKind::FastForward });

  update(&clone).unwrap().applied.unwrap();
  assert_eq!(read(&clone, "other.txt/inner.txt"), "inner\n");
}

#[test]
fn preview_reuses_the_replay_of_an_unchanged_state() {
  let Setup { upstream, clone } = fetched_rebase();
  let trace_dir = tempfile::tempdir().unwrap();
  let trace = trace_dir.path().join("trace.json");
  let replays = || {
    let replays = fs::read_to_string(&trace)
      .unwrap_or_default()
      .lines()
      .filter(|line| line.contains("\"event\":\"start\"") && line.contains("\"replay\""))
      .count();
    let _ = fs::remove_file(&trace);
    replays
  };
  // the trace of this test's git only: the process environment is shared with the tests that run beside it
  let traced = clone.git_executor().with_env("GIT_TRACE2_EVENT", trace.to_str().unwrap());
  let preview = |repo: &TestRepo| check_baseline_update(&traced, path(repo), "origin/master").unwrap().unwrap().check.state;
  assert_eq!(preview(&clone), UpdateState::Clean { kind: UpdateKind::Rebase });
  assert_eq!(replays(), 1);
  assert_eq!(preview(&clone), UpdateState::Clean { kind: UpdateKind::Rebase });
  assert_eq!(replays(), 0, "the same HEAD and upstream reuse the replay");

  // the local changes are checked every time
  write(&clone, "shared.txt", "local\n");
  assert_eq!(preview(&clone), blocked(&["shared.txt"]));
  assert_eq!(replays(), 0);

  // a fetch replays again
  upstream.create_commit("More upstream", "more.txt", "more\n");
  git(&clone, &["fetch", "origin"]);
  assert_eq!(preview(&clone), blocked(&["shared.txt"]));
  assert_eq!(replays(), 1);
}

#[test]
fn fetch_reports_progress_until_the_last_stage_ends() {
  let Setup { upstream, clone } = setup();
  // more objects than `fetch.unpackLimit`, so the fetch indexes a pack (`Receiving objects`), of noise that doesn't compress
  let mut state = 0x9e37_79b9_7f4a_7c15_u64;
  let mut noise = || {
    let mut content = String::new();
    for _ in 0..4096 {
      state ^= state << 13;
      state ^= state >> 7;
      state ^= state << 17;
      write!(content, "{state:016x}").unwrap();
    }
    content
  };
  let files: Vec<(String, String)> = (0..150).map(|i| (format!("data/{i}.txt"), noise())).collect();
  let files: Vec<(&str, &str)> = files.iter().map(|(name, content)| (name.as_str(), content.as_str())).collect();
  upstream.create_commit_with_files("Add data", &files);

  let (result, progress) = update_reporting_progress(&clone);
  assert_eq!(result.unwrap().check.state, UpdateState::Clean { kind: UpdateKind::FastForward });
  let fetch: Vec<_> = progress.iter().filter_map(|it| it.fetch.as_ref()).collect();
  assert!(progress.iter().all(|it| it.fetch.is_none() || it.phase == UpdatePhase::Fetching));
  assert!(fetch.windows(2).all(|pair| pair[0].percent <= pair[1].percent), "{fetch:?}");
  // without deltas there is no `Resolving deltas` stage: receiving ends the fetch at 95 %
  let last = fetch.last().expect("no fetch progress");
  let last_stage_ended = last.line.starts_with("Receiving objects: 100% ") || last.line.starts_with("Resolving deltas: 100% ");
  assert!(last_stage_ended, "{fetch:?}");
  assert!(last.percent >= 95, "{fetch:?}");
  assert!(last.transferred.as_deref().is_some_and(|it| it.ends_with("MiB")), "{fetch:?}");
}

/// An update of `clone` from a local commit onto an upstream that changes `shared.txt`, adds `new.txt` and deletes
/// `other.txt`; returns its undo token
fn applied_update(setup: &Setup) -> UpdateUndoToken {
  let Setup { upstream, clone } = setup;
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  upstream.create_commit("Add new", "new.txt", "new\n");
  git(upstream, &["rm", "-q", "other.txt"]);
  git(upstream, &["commit", "-q", "-m", "Remove other"]);
  let result = update(clone).unwrap();
  UpdateUndoToken {
    previous_head: result.check.head,
    new_head: result.applied.unwrap().new_head,
  }
}

fn undo(repo: &TestRepo, token: &UpdateUndoToken) -> anyhow::Result<UndoneUpdate> {
  undo_update_core(repo.git_executor(), path(repo), token, Duration::from_millis(200))
}

#[test]
fn undo_moves_the_branch_and_the_files_back() {
  let setup = setup();
  let token = applied_update(&setup);
  let clone = &setup.clone;
  write(clone, "login.txt", "dirty login\n");
  write(clone, "untracked.txt", "untracked\n");

  let undone = undo(clone, &token).unwrap();
  assert_eq!(undone.branch, "master");
  assert_eq!(clone.head(), token.previous_head);
  assert_eq!(read(clone, "shared.txt"), SHARED);
  assert_eq!(read(clone, "other.txt"), "other\n");
  assert!(!clone.path().join("new.txt").exists());
  // local changes on paths the undo doesn't write stay
  assert_eq!(read(clone, "login.txt"), "dirty login\n");
  assert_eq!(status(clone), " M login.txt\n?? untracked.txt");
  assert!(reflog(clone).starts_with("branch-deck: undo update\nbranch-deck: update project\n"), "{}", reflog(clone));

  // a second undo of the same update changes nothing and is no error
  let again = undo(clone, &token).unwrap();
  assert!(again.already_undone && !again.landed_restored, "{again:?}");
  assert_eq!(clone.head(), token.previous_head);
}

#[test]
fn the_undo_works_without_reflogs_because_the_update_writes_one() {
  let setup = setup();
  let clone = &setup.clone;
  clone.set_config("core.logAllRefUpdates", "false").unwrap();
  fs::remove_dir_all(clone.path().join(".git/logs")).unwrap();
  let token = applied_update(&setup);

  undo(clone, &token).unwrap();
  assert_eq!(clone.head(), token.previous_head);
  assert!(undo(clone, &token).unwrap().already_undone);
}

#[test]
fn undo_is_refused_when_local_changes_overlap() {
  let setup = setup();
  let token = applied_update(&setup);
  let clone = &setup.clone;
  write(clone, "shared.txt", "edited after the update\n");
  // an ignored file where the undo brings back a file the update deleted: `read-tree -u` would overwrite it silently
  fs::write(clone.path().join(".git/info/exclude"), "other.txt\n").unwrap();
  write(clone, "other.txt", "ignored\n");
  let reflog_before = reflog(clone);

  let error = undo(clone, &token).unwrap_err();
  let blocked = error.downcast_ref::<UndoBlocked>().unwrap_or_else(|| panic!("not UndoBlocked: {error:#}"));
  assert_eq!(blocked.paths, vec!["shared.txt".to_string()]);
  assert_eq!(clone.head(), token.new_head);
  assert_eq!(reflog(clone), reflog_before);

  // the dry run passes once the edit is gone; the ignored file still blocks
  write(clone, "shared.txt", "one\nTWO\nthree\n");
  let error = undo(clone, &token).unwrap_err();
  assert_eq!(
    error.downcast_ref::<UndoBlocked>().map(|it| it.paths.clone()),
    Some(vec!["other.txt".to_string()]),
    "{error:#}"
  );
  assert_eq!(read(clone, "other.txt"), "ignored\n");
  assert_eq!(clone.head(), token.new_head);
}

#[test]
fn undo_is_refused_after_the_branch_moved() {
  let setup = setup();
  let token = applied_update(&setup);
  let clone = &setup.clone;
  let later = clone.create_commit("(auth) Add logout", "logout.txt", "logout\n");

  let error = undo(clone, &token).unwrap_err();
  let moved = error.downcast_ref::<HeadMoved>().unwrap_or_else(|| panic!("not HeadMoved: {error:#}"));
  assert_eq!((moved.expected.as_str(), moved.actual.as_str()), (token.new_head.as_str(), later.as_str()));
  assert_eq!(clone.head(), later);
}

#[test]
fn undo_needs_the_token_of_the_last_update() {
  let setup = setup();
  let token = applied_update(&setup);
  let clone = &setup.clone;
  // any other commit: the reflog says the branch came from `previous_head`
  let other = UpdateUndoToken {
    previous_head: setup.upstream.head(),
    new_head: token.new_head.clone(),
  };
  let error = undo(clone, &other).unwrap_err();
  assert!(error.downcast_ref::<NotAnUpdate>().is_some(), "{error:#}");
  assert_eq!(clone.head(), token.new_head);
}

#[test]
fn undo_tokens_are_strict() {
  let a = "a".repeat(40);
  let b = "b".repeat(40);
  let token = UpdateUndoToken::parse(&format!("update:{a}..{b}")).unwrap();
  assert_eq!((token.previous_head.as_str(), token.new_head.as_str()), (a.as_str(), b.as_str()));
  assert_eq!(token.to_string(), format!("update:{a}..{b}"));
  for text in [
    format!("{a}..{b}"),
    format!("update:{}..{b}", &a[..12]),
    format!("update:{a}..{a}"),
    format!("update:{a}..{}", "b".repeat(64)),
    format!("update:{}..{b}", "A".repeat(40)),
    format!("update:{a}...{b}"),
  ] {
    assert_eq!(UpdateUndoToken::parse(&text), None, "{text}");
  }
  assert!(UpdateUndoToken::is_update_token(&format!("update:{a}")));
}

#[test]
fn update_takes_the_repository_lock_after_the_fetch_and_says_it_waits() {
  let Setup { upstream, clone } = setup();
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  let head = clone.head();
  // another Branch Deck process (bd) holds the lock for the whole wait
  let held = lock_repository(clone.git_executor(), path(&clone), Duration::from_millis(200)).unwrap();
  let options = UpdateOptions {
    fetch: true,
    lock_timeout: Duration::from_millis(200),
  };

  let progress = RefCell::new(Vec::new());
  let error = update_project_with(clone.git_executor(), path(&clone), options, &|it| progress.borrow_mut().push(it.phase)).unwrap_err();
  assert!(error.downcast_ref::<RepositoryLocked>().is_some(), "{error:#}");
  // the wait is a step of its own, told once; the check never started
  let mut phases = progress.into_inner();
  phases.dedup();
  assert_eq!(phases, vec![UpdatePhase::Fetching, UpdatePhase::Waiting]);
  // the app says who holds it, without the lock file
  assert_eq!(format!("{:#}", app_error(error)), LOCKED_MESSAGE);
  // the fetch ran before the lock; nothing else changed
  assert_eq!(git(&clone, &["rev-parse", "origin/master"]), upstream.head());
  assert_eq!(clone.head(), head);

  drop(held);
  let progress = RefCell::new(Vec::new());
  let result = update_project_with(clone.git_executor(), path(&clone), options, &|it| progress.borrow_mut().push(it.phase)).unwrap();
  assert_eq!(result.check.state, UpdateState::Clean { kind: UpdateKind::FastForward });
  // a free lock is no step
  assert!(!progress.into_inner().contains(&UpdatePhase::Waiting));
  assert_eq!(clone.head(), upstream.head());
}

#[test]
fn update_without_a_fetch_goes_onto_the_last_fetched_state() {
  let Setup { upstream, clone } = fetched_rebase();
  let fetched = git(&clone, &["rev-parse", "origin/master"]);
  upstream.create_commit("Not fetched", "later.txt", "later\n");
  let options = UpdateOptions {
    fetch: false,
    lock_timeout: Duration::from_millis(200),
  };

  let result = update_project_with(clone.git_executor(), path(&clone), options, &|_| {}).unwrap();
  assert_eq!(result.check.state, UpdateState::Clean { kind: UpdateKind::Rebase });
  assert_eq!(result.check.upstream_head, fetched);
  assert_eq!(git(&clone, &["rev-parse", "HEAD~1"]), fetched);
  assert_eq!(git(&clone, &["rev-parse", "origin/master"]), fetched);
}

#[test]
fn a_failed_fetch_is_fetch_failed() {
  let Setup { upstream, clone } = setup();
  git(&clone, &["remote", "set-url", "origin", &format!("{}-gone", path(&upstream))]);
  let error = update(&clone).unwrap_err();
  let failed = error.downcast_ref::<FetchFailed>().unwrap_or_else(|| panic!("not FetchFailed: {error:#}"));
  assert_eq!(failed.upstream, "origin/master");
}

/// Two branches at the same commit: a `git switch` to the other one after the check must not move it
#[test]
fn a_switch_to_another_branch_at_the_same_commit_since_the_check_is_refused() {
  let (Setup { upstream: _upstream, clone }, head, plan) = checked_rebase();
  git(&clone, &["switch", "-q", "-c", "other"]);
  let reflog_before = reflog(&clone);

  let error = apply_update(clone.git_executor(), path(&clone), &plan).unwrap_err();
  let switched = error.downcast_ref::<BranchSwitched>().unwrap_or_else(|| panic!("not BranchSwitched: {error:#}"));
  assert_eq!((switched.expected.as_str(), switched.actual.as_deref()), ("master", Some("other")));
  assert_eq!(git(&clone, &["rev-parse", "master", "other"]), format!("{head}\n{head}"));
  assert_eq!(reflog(&clone), reflog_before);
  assert_eq!(read(&clone, "shared.txt"), SHARED);
  assert_eq!(status(&clone), "");

  // on the checked branch the update applies; then the same for its undo
  git(&clone, &["switch", "-q", "master"]);
  let applied = apply_update(clone.git_executor(), path(&clone), &plan).unwrap();
  assert_eq!(git(&clone, &["rev-parse", "other"]), head);
  let UndoCheck::Ready(undo) = check_update_undo(clone.git_executor(), path(&clone), &head, &applied.new_head).unwrap() else {
    panic!("the update is the last move")
  };
  git(&clone, &["branch", "-q", "other2", &applied.new_head]);
  git(&clone, &["switch", "-q", "other2"]);
  let error = apply_update_undo(clone.git_executor(), path(&clone), &undo).unwrap_err();
  assert!(error.downcast_ref::<BranchSwitched>().is_some(), "{error:#}");
  assert_eq!(git(&clone, &["rev-parse", "master", "other2"]), format!("{0}\n{0}", applied.new_head));
}

/// A process killed between the move and the files leaves HEAD at the new head and the index and the files at the
/// previous one; the undo is the recovery
#[test]
fn the_undo_recovers_an_update_killed_before_the_files_followed() {
  let (Setup { upstream: _upstream, clone }, head, plan) = checked_rebase();
  let new_head = plan.replayed_head().unwrap().to_string();
  // the move of `apply_update`, without the files
  git(&clone, &["update-ref", "-m", "branch-deck: update project", "HEAD", &new_head, &head]);
  assert_eq!(read(&clone, "shared.txt"), SHARED);

  let token = UpdateUndoToken {
    previous_head: head.clone(),
    new_head,
  };
  undo(&clone, &token).unwrap();
  assert_eq!(clone.head(), head);
  assert_eq!(read(&clone, "shared.txt"), SHARED);
  assert_eq!(status(&clone), "");
}

/// A clone and an upstream that both change each of `count` files on the same line, fetched
fn wide_conflict(count: usize) -> Setup {
  let names: Vec<String> = (0..count).map(|i| format!("src/file{i:03}.txt")).collect();
  let with = |content: &'static str| names.iter().map(|name| (name.as_str(), content)).collect::<Vec<_>>();
  let (upstream, clone) = TestRepo::upstream_and_clone(&with("one\n"));
  clone.create_commit_with_files("(auth) Change all", &with("mine\n"));
  upstream.create_commit_with_files("Upstream change all", &with("theirs\n"));
  git(&clone, &["fetch", "origin"]);
  Setup { upstream, clone }
}

#[test]
fn wide_conflict_reports_the_first_files_with_their_commits_and_counts_all() {
  let Setup { upstream: _upstream, clone } = wide_conflict(60);
  let UpdateState::Conflict {
    files,
    file_count,
    commits,
    upstream_commits,
  } = preview(&clone)
  else {
    panic!("not a conflict");
  };
  // a monorepo update can conflict in thousands of files: the check reports the ones it attributes commits to
  assert_eq!((files.len(), file_count), (50, 60));
  assert_eq!((files[0].as_str(), files[49].as_str()), ("src/file000.txt", "src/file049.txt"));
  assert_eq!(commits[0].files, files);
  assert_eq!(upstream_commits[0].files, files);
}

#[test]
fn a_conflict_one_file_wider_than_the_list_reports_every_file_with_its_commits() {
  let Setup { upstream: _upstream, clone } = wide_conflict(51);
  let UpdateState::Conflict { files, file_count, commits, .. } = preview(&clone) else {
    panic!("not a conflict");
  };
  // the count of the rest is never 1: the one file costs no more
  assert_eq!((files.len(), file_count), (51, 51));
  assert_eq!(commits[0].files, files);
}

/// The blocked update of a clone whose untracked files sit on `count` paths the upstream adds
fn blocked_by_untracked_files(count: usize) -> (Setup, UpdateState) {
  let Setup { upstream, clone } = setup();
  let names: Vec<String> = (0..count).map(|i| format!("gen/file{i:03}.txt")).collect();
  let files: Vec<(&str, &str)> = names.iter().map(|name| (name.as_str(), "upstream\n")).collect();
  upstream.create_commit_with_files("Add generated files", &files);
  fs::create_dir_all(clone.path().join("gen")).unwrap();
  for name in &names {
    write(&clone, name, "mine\n");
  }
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  git(&clone, &["fetch", "origin"]);
  let state = preview(&clone);
  (Setup { upstream, clone }, state)
}

#[test]
fn many_blocking_paths_are_counted() {
  let (_setup, state) = blocked_by_untracked_files(120);
  let UpdateState::Blocked { paths, path_count } = state else {
    panic!("not blocked");
  };
  assert_eq!((paths.len(), path_count), (100, 120));
  assert_eq!(paths[0], "gen/file000.txt");
  // one more than the list: all of them, never a count of one
  let (_setup, state) = blocked_by_untracked_files(101);
  let UpdateState::Blocked { paths, path_count } = state else {
    panic!("not blocked");
  };
  assert_eq!((paths.len(), path_count), (101, 101));
}

/// Loose objects: a replay writes commits, so an unchanged count means nothing was replayed
fn loose_objects(repo: &TestRepo) -> String {
  git(repo, &["count-objects"])
}

/// A clean first commit and a second one that conflicts with the upstream, fetched: the replay writes the first
fn conflict_after_a_replayed_commit() -> Setup {
  let Setup { upstream, clone } = setup();
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  clone.create_commit("(auth) Change two", "shared.txt", "one\nlocal two\nthree\n");
  upstream.create_commit("Upstream change", "shared.txt", "one\nupstream two\nthree\n");
  git(&clone, &["fetch", "origin"]);
  Setup { upstream, clone }
}

#[test]
fn update_after_the_sync_found_a_conflict_does_not_replay_again() {
  let Setup { upstream: _upstream, clone } = conflict_after_a_replayed_commit();
  let state = preview(&clone);
  assert!(matches!(state, UpdateState::Conflict { .. }), "{state:?}");
  let objects = loose_objects(&clone);

  // the replay of the update writes the commits with the current committer date, other objects than the preview's
  let plan = check_update(clone.git_executor(), path(&clone), ORIGIN_MASTER, CheckPurpose::Apply, &no_landed_candidates).unwrap();
  assert_eq!(plan.check.state, state);
  assert_eq!(loose_objects(&clone), objects);
}

#[test]
fn sync_after_a_conflicting_update_does_not_replay_again() {
  let Setup { upstream: _upstream, clone } = conflict_after_a_replayed_commit();
  let plan = check_update(clone.git_executor(), path(&clone), ORIGIN_MASTER, CheckPurpose::Apply, &no_landed_candidates).unwrap();
  assert!(matches!(plan.check.state, UpdateState::Conflict { .. }), "{:?}", plan.check.state);
  let objects = loose_objects(&clone);

  // the preview replays with a fixed committer date: a replay would write new commits
  assert_eq!(preview(&clone), plan.check.state);
  assert_eq!(loose_objects(&clone), objects);
}
