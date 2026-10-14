use crate::sync::{SyncOptions, sync_branches};
use crate::update_project::{UpdatePhase, UpdateProgress, UpdateProjectParams, UpdateProjectResult, check_baseline_update, previous_tip_of_forced_update, update_project_core};
use git_ops::update_branch::{CheckPurpose, UpdateCommit, UpdateKind, UpdateState, UpstreamRef, apply_update, check_update};
use pretty_assertions::assert_eq;
use std::cell::RefCell;
use std::fmt::Write;
use std::fs;
use sync_test_utils::TestReporter;
use sync_types::SyncEvent;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// An upstream repository with `shared.txt` and `other.txt`, and a clone of it whose `master` tracks `origin/master`
struct Setup {
  upstream: TestRepo,
  clone: TestRepo,
}

const SHARED: &str = "one\ntwo\nthree\n";

fn setup() -> Setup {
  let upstream = TestRepo::new();
  upstream.create_commit_with_files("Initial", &[("shared.txt", SHARED), ("other.txt", "other\n")]);
  let clone = TestRepo::new_empty();
  clone.clone_from(upstream.path()).unwrap();
  Setup { upstream, clone }
}

fn path(repo: &TestRepo) -> &str {
  repo.path().to_str().unwrap()
}

fn git(repo: &TestRepo, args: &[&str]) -> String {
  repo.git_executor().execute_command(args, path(repo)).unwrap()
}

fn update(repo: &TestRepo) -> anyhow::Result<UpdateProjectResult> {
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
  repo
    .git_executor()
    .execute_command_raw(&["status", "--porcelain", "--untracked-files=all"], path(repo))
    .unwrap()
    .trim_end()
    .to_string()
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
  assert_eq!(
    result.check.state,
    UpdateState::Blocked {
      paths: vec!["shared.txt".to_string()]
    }
  );
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
  assert_eq!(
    result.check.state,
    UpdateState::Blocked {
      paths: vec!["shared.txt".to_string()]
    }
  );
  assert_eq!(clone.head(), head);
  assert_eq!(git(&clone, &["show", ":shared.txt"]), "staged");
}

#[test]
fn untracked_file_that_upstream_adds_blocks() {
  let Setup { upstream, clone } = setup();
  upstream.create_commit("Add config", "dir/config.txt", "upstream\n");
  fs::create_dir(clone.path().join("dir")).unwrap();
  write(&clone, "dir/config.txt", "mine\n");
  let head = clone.head();

  let result = update(&clone).unwrap();
  assert_eq!(
    result.check.state,
    UpdateState::Blocked {
      paths: vec!["dir/config.txt".to_string()]
    }
  );
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
  assert_eq!(
    result.check.state,
    UpdateState::Blocked {
      paths: vec!["build.out".to_string()]
    }
  );
  assert_eq!(clone.head(), head);
  assert_eq!(read(&clone, "build.out"), "mine\n");
}

#[test]
fn touched_only_file_passes_thanks_to_the_refresh() {
  let Setup { upstream, clone } = setup();
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  // same content, new stat data: the index entry is stale until refreshed
  let file = fs::File::options().write(true).open(clone.path().join("shared.txt")).unwrap();
  file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5)).unwrap();
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
  git(&upstream, &["config", "user.name", "Dana Lee"]);
  let upstream_change = upstream.create_commit_with_timestamp("Upstream change", "shared.txt", "one\nupstream two\nthree\n", Some(1_700_000_300));
  let head = clone.head();
  let reflog_before = reflog(&clone);

  let result = update(&clone).unwrap();
  let check = result.check;
  let commit = |id: String, subject: &str, author: &str, time: u32, files: &[&str]| UpdateCommit {
    id,
    subject: subject.to_string(),
    author: author.to_string(),
    time,
    files: files.iter().map(ToString::to_string).collect(),
  };
  // only the commits that change the conflicting file carry it; the upstream lists only those
  assert_eq!(
    check.state,
    UpdateState::Conflict {
      files: vec!["shared.txt".to_string()],
      commits: vec![
        commit(first, "(auth) Add login", "Test User", 1_700_000_100, &[]),
        commit(second, "(auth) Change two", "Test User", 1_700_000_200, &["shared.txt"]),
      ],
      upstream_commits: vec![commit(upstream_change, "Upstream change", "Dana Lee", 1_700_000_300, &["shared.txt"])],
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

  let check = check_baseline_update(clone.git_executor(), path(&clone), "origin/master").unwrap().unwrap();
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
  let Setup { upstream: _upstream, clone } = fetched_rebase();
  let head = clone.head();

  let plan = check_update(clone.git_executor(), path(&clone), ORIGIN_MASTER, CheckPurpose::Apply).unwrap();
  assert_eq!(plan.check.state, UpdateState::Clean { kind: UpdateKind::Rebase });
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

  let plan = check_update(clone.git_executor(), path(&clone), upstream_ref, CheckPurpose::Preview).unwrap();
  assert_eq!(plan.check.state, UpdateState::Clean { kind: UpdateKind::Rebase });
  assert!(apply_update(clone.git_executor(), path(&clone), &plan).is_err());
  // the same state writes the same commits: no new objects on every sync
  let objects = git(&clone, &["count-objects"]);
  check_update(clone.git_executor(), path(&clone), upstream_ref, CheckPurpose::Preview).unwrap();
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
  fs::create_dir(clone.path().join(".git/rebase-merge")).unwrap();
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
async fn sync_reports_a_failed_check_without_failing() -> anyhow::Result<()> {
  let Setup { upstream: _upstream, clone } = setup();
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  git(&clone, &["checkout", "--detach", "HEAD"]);

  let progress = TestReporter::new();
  sync_branches(clone.git_executor(), path(&clone), "test", progress.clone(), SyncOptions::default()).await?;
  let status = progress.get_events().into_iter().find(|e| matches!(e, SyncEvent::BaselineStatus { .. })).unwrap();
  let SyncEvent::BaselineStatus { update, check_error, .. } = status else {
    unreachable!()
  };
  assert!(update.is_none());
  assert!(check_error.unwrap().contains("detached"));
  Ok(())
}

#[test]
fn e2e_templates_have_the_expected_update_states() {
  use test_utils::repo_template::templates;
  let dir = tempfile::tempdir().unwrap();
  let git_executor = git_executor::git_command_executor::GitCommandExecutor::new();
  let check = |name: &str, template: templates::UpdateTemplate| {
    let repo = dir.path().join(name);
    template.build(&repo).unwrap();
    let repo = repo.to_str().unwrap().to_string();
    let check = check_baseline_update(&git_executor, &repo, "origin/master").unwrap().unwrap();
    (repo, check.state)
  };

  let (repo, state) = check("behind_upstream", templates::behind_upstream());
  assert_eq!(state, UpdateState::Clean { kind: UpdateKind::Rebase });
  let result = update_project_core(&git_executor, &UpdateProjectParams { repository_path: repo.clone() }, &|_| {}).unwrap();
  assert_eq!(result.applied.unwrap().replayed, 1);
  assert_eq!(fs::read_to_string(format!("{repo}/src/config.txt")).unwrap(), "debug=true\n", "the dirty file is kept");

  let (_, state) = check("update_conflict", templates::update_conflict());
  assert!(
    matches!(&state, UpdateState::Conflict { files, commits, upstream_commits }
      if files == &["src/app.txt"] && commits.len() == 2 && upstream_commits.len() == 1),
    "{state:?}"
  );
  let (_, state) = check("update_blocked", templates::update_blocked());
  assert_eq!(
    state,
    UpdateState::Blocked {
      paths: vec!["src/app.txt".to_string()]
    }
  );
}

/// The state of the check inside a sync: the preview, without the network
fn preview(repo: &TestRepo) -> UpdateState {
  check_baseline_update(repo.git_executor(), path(repo), "origin/master").unwrap().unwrap().state
}

fn blocked(paths: &[&str]) -> UpdateState {
  UpdateState::Blocked {
    paths: paths.iter().map(|path| (*path).to_string()).collect(),
  }
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
  file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5)).unwrap();
  drop(file);
  let index = fs::read(clone.path().join(".git/index")).unwrap();
  assert_eq!(git(&clone, &["diff-files", "--name-only"]), "shared.txt", "the index entry is stale");

  assert_eq!(preview(&clone), UpdateState::Clean { kind: UpdateKind::Rebase });
  assert_eq!(fs::read(clone.path().join(".git/index")).unwrap(), index);
}

#[test]
fn preview_blocks_untracked_and_ignored_files_that_upstream_adds() {
  let Setup { upstream, clone } = setup();
  upstream.create_commit_with_files("Add files", &[("dir/config.txt", "upstream\n"), ("build.out", "upstream\n")]);
  git(&clone, &["fetch", "origin"]);
  fs::create_dir(clone.path().join("dir")).unwrap();
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
  // nextest runs every test in its own process
  unsafe { std::env::set_var("GIT_TRACE2_EVENT", &trace) };
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
  unsafe { std::env::remove_var("GIT_TRACE2_EVENT") };
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
  assert!(last.received.as_deref().is_some_and(|it| it.ends_with("MiB")), "{fetch:?}");
}
