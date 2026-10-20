use bd_cli::fixture_test::assert_covered;
use bd_cli::fixture_test::{assert_error, assert_refused_during, environment, git, run_in, run_json, start_cherry_pick, write};
use history_rewrite::repository_lock::lock_repository;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_utils::git_test_utils::TestRepo;

const SHARED: &str = "one\ntwo\nthree\n";

/// An upstream repository with `shared.txt` and `other.txt`, and a clone whose `master` tracks `origin/master`
fn clone_of_upstream() -> (TestRepo, TestRepo) {
  TestRepo::upstream_and_clone(&[("shared.txt", SHARED), ("other.txt", "other\n")])
}

/// `clone_of_upstream` with a local commit and a fetched upstream change of `shared.txt`
fn clone_with_a_fetched_change() -> (TestRepo, TestRepo) {
  let (upstream, clone) = clone_of_upstream();
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  git(&clone, &["fetch", "-q", "origin"]);
  (upstream, clone)
}

fn read(repo: &TestRepo, file: &str) -> String {
  std::fs::read_to_string(repo.path().join(file)).unwrap()
}

fn status(repo: &TestRepo) -> String {
  git(repo, &["status", "--porcelain", "--untracked-files=all"])
}

#[test_log::test]
fn update_fast_forwards_and_the_undo_moves_the_files_back() {
  let (upstream, clone) = clone_of_upstream();
  let previous = clone.head();
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  upstream.create_commit("Add new", "new.txt", "new\n");

  let (exit_code, output) = run_in(&clone, &["--full-ids", "update"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["state"], &output["previous_head"], &output["new_head"], &output["replayed"], &output["dry_run"]),
    (&json!("fast_forward"), &json!(previous), &json!(upstream.head()), &json!(0), &json!(false))
  );
  assert_eq!(output["undo_token"], json!(format!("update:{previous}..{}", upstream.head())));
  assert!(output["fetched_at"].is_u64(), "{output}");
  assert_eq!(read(&clone, "new.txt"), "new\n");
  assert_eq!(status(&clone), "");

  let (exit_code, undo) = run_in(&clone, &["--full-ids", "undo", output["undo_token"].as_str().unwrap()]);
  assert_eq!(exit_code, 0, "{undo}");
  assert_eq!((&undo["new_head"], &undo["previous_head"]), (&json!(previous), &json!(upstream.head())));
  assert_eq!(clone.head(), previous);
  assert_eq!(read(&clone, "shared.txt"), SHARED);
  assert!(!clone.path().join("new.txt").exists());
  assert_eq!(status(&clone), "");

  // a repeated undo is no error and changes nothing
  let (exit_code, again) = run_in(&clone, &["--full-ids", "undo", output["undo_token"].as_str().unwrap()]);
  assert_eq!((exit_code, &again["already_undone"], &again["new_head"]), (0, &json!(true), &json!(previous)), "{again}");
}

#[test_log::test]
fn update_rebases_the_local_commits_and_keeps_unrelated_local_changes() {
  let (upstream, clone) = clone_of_upstream();
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  write(&clone, "other.txt", "dirty other\n");

  let (exit_code, output) = run_in(&clone, &["update"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["state"], &output["replayed"], &output["dropped_already_upstream"]),
    (&json!("rebase"), &json!(1), &json!(0))
  );
  assert_eq!(git(&clone, &["rev-parse", "HEAD~1"]), upstream.head());
  assert_eq!(read(&clone, "shared.txt"), "one\nTWO\nthree\n");
  assert_eq!(read(&clone, "other.txt"), "dirty other\n");
  assert!(git(&clone, &["reflog", "-1", "--format=%gs", "refs/heads/master"]).starts_with("branch-deck: update project"));

  // nothing new: up to date, no token
  let (exit_code, output) = run_in(&clone, &["update", "--no-fetch"]);
  assert_eq!((exit_code, &output["state"], output.get("undo_token")), (0, &json!("up_to_date"), None), "{output}");
}

#[test_log::test]
fn uncommitted_changes_on_updated_files_block_with_their_paths() {
  let (upstream, clone) = clone_of_upstream();
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  write(&clone, "shared.txt", "local edit\n");
  let head = clone.head();

  let (exit_code, output) = run_in(&clone, &["update"]);
  assert_error(&output, exit_code, (4, "uncommitted_overlap"));
  assert_eq!(output["error"]["paths"], json!(["shared.txt"]));
  assert_eq!(output["error"]["retryable"], json!(false));
  assert_eq!(clone.head(), head);
  assert_eq!(read(&clone, "shared.txt"), "local edit\n");
  assert_eq!(git(&clone, &["stash", "list"]), "", "bd never stashes");
  // a dry run says the same
  let (exit_code, output) = run_in(&clone, &["update", "--dry-run"]);
  assert_error(&output, exit_code, (4, "uncommitted_overlap"));
}

#[test_log::test]
fn update_and_its_dry_run_are_refused_during_a_cherry_pick() {
  let (_upstream, clone) = clone_with_a_fetched_change();
  start_cherry_pick(&clone, clone.path());
  assert_refused_during(&clone, "a cherry-pick", |extra| run_in(&clone, &[&["update"][..], extra].concat()));
  assert_eq!(read(&clone, "shared.txt"), SHARED);
}

/// `count` names under `dir`, each with `content`
fn files_of<'a>(names: &'a [String], content: &'a str) -> Vec<(&'a str, &'a str)> {
  names.iter().map(|name| (name.as_str(), content)).collect()
}

fn names(dir: &str, count: usize) -> Vec<String> {
  (0..count).map(|i| format!("{dir}/file{i:03}.txt")).collect()
}

#[test_log::test]
fn a_dry_run_of_a_wide_conflict_lists_the_first_files_and_counts_the_rest() {
  let (upstream, clone) = clone_of_upstream();
  let names = names("src", 60);
  clone.create_commit_with_files("(auth) Add the files", &files_of(&names, "mine\n"));
  upstream.create_commit_with_files("Add the files upstream", &files_of(&names, "theirs\n"));
  git(&clone, &["fetch", "-q", "origin"]);

  let (exit_code, output) = run_in(&clone, &["update", "--dry-run"]);
  assert_eq!(exit_code, 1, "{output}");
  // the output goes into an agent's context: the files with their commits, and how many more
  let conflict = &output["conflict"];
  assert_eq!(
    (conflict["files"].as_array().map(Vec::len), &conflict["files_omitted"]),
    (Some(50), &json!(10)),
    "{conflict}"
  );
  assert_eq!(conflict["commits"][0]["files"].as_array().map(Vec::len), Some(50));
  let (_, schema) = run_json(&environment(clone.path()), &["schema"]);
  assert_covered(&schema["outputs"]["update"], &schema["outputs"]["update"], &output, "wide conflict");
}

#[test_log::test]
fn many_blocking_paths_list_the_first_100_and_say_how_many() {
  let output = dry_run_blocked_by_untracked_files(120);
  assert_eq!(output["error"]["paths"].as_array().map(Vec::len), Some(100), "{output}");
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.ends_with("; `paths` lists the first 100 of 120: `git status` lists all"), "{message}");

  // one more than the list: all of them, never a count of one
  let output = dry_run_blocked_by_untracked_files(101);
  assert_eq!(output["error"]["paths"].as_array().map(Vec::len), Some(101), "{output}");
  let message = output["error"]["message"].as_str().unwrap();
  assert!(!message.contains("lists the first"), "{message}");
}

/// The error of `bd update --dry-run` in a clone whose untracked files sit on `count` paths the upstream adds
fn dry_run_blocked_by_untracked_files(count: usize) -> Value {
  let (upstream, clone) = clone_of_upstream();
  let names = names("gen", count);
  upstream.create_commit_with_files("Add generated files", &files_of(&names, "upstream\n"));
  std::fs::create_dir_all(clone.path().join("gen")).unwrap();
  for name in &names {
    std::fs::write(clone.path().join(name), "mine\n").unwrap();
  }
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  git(&clone, &["fetch", "-q", "origin"]);
  let (exit_code, output) = run_in(&clone, &["update", "--dry-run"]);
  assert_error(&output, exit_code, (4, "uncommitted_overlap"));
  output
}

#[test_log::test]
fn a_dry_run_reports_the_files_and_commits_of_a_conflict_and_changes_nothing() {
  let (upstream, clone) = clone_of_upstream();
  clone.create_commit("(auth) Change line 2", "shared.txt", "one\nmine\nthree\n");
  upstream.create_commit("Change line 2 upstream", "shared.txt", "one\ntheirs\nthree\n");
  git(&clone, &["fetch", "-q", "origin"]);
  let head = clone.head();

  let (exit_code, output) = run_in(&clone, &["update", "--dry-run"]);
  assert_eq!(exit_code, 1, "{output}");
  assert_eq!(output["state"], json!("conflict"));
  let conflict = &output["conflict"];
  assert_eq!(conflict["files"], json!(["shared.txt"]));
  assert_eq!(conflict.get("files_omitted"), None);
  assert_eq!(conflict["commits"][0]["subject"], json!("(auth) Change line 2"));
  assert_eq!(conflict["commits"][0]["files"], json!(["shared.txt"]));
  assert_eq!(conflict["upstream_commits"][0]["subject"], json!("Change line 2 upstream"));
  assert_eq!((output.get("undo_token"), output.get("session")), (None, None));
  assert_eq!(clone.head(), head);
  assert_eq!(read(&clone, "shared.txt"), "one\nmine\nthree\n");
  assert_eq!(status(&clone), "");
  assert_eq!(git(&clone, &["for-each-ref", "refs/branch-deck/"]), "", "a dry run starts no session");
}

#[test_log::test]
fn dry_run_neither_fetches_nor_writes() {
  let (upstream, clone) = clone_with_a_fetched_change();
  let fetched = git(&clone, &["rev-parse", "origin/master"]);
  upstream.create_commit("Not fetched", "later.txt", "later\n");
  write(&clone, "untracked.txt", "untracked\n");
  let state = |repo: &TestRepo| {
    (
      git(repo, &["for-each-ref", "--format=%(refname) %(objectname)"]),
      std::fs::read(repo.path().join(".git/index")).unwrap(),
      status(repo),
      git(repo, &["reflog", "--format=%gs", "refs/heads/master"]),
    )
  };
  let before = state(&clone);

  let (exit_code, output) = run_in(&clone, &["update", "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["state"], &output["dry_run"]), (&json!("rebase"), &json!(true)));
  assert_eq!(output["upstream_head"], json!(git_ops::revs::short_id(&fetched)));
  assert_eq!((output.get("undo_token"), output.get("replayed")), (None, None));
  assert_eq!(state(&clone), before);
  assert!(!clone.path().join(".git/branch-deck/landed.json").exists());

  // --no-fetch updates onto the fetched state, not the newer upstream
  let (exit_code, output) = run_in(&clone, &["update", "--no-fetch"]);
  assert_eq!((exit_code, &output["state"]), (0, &json!("rebase")), "{output}");
  assert_eq!(git(&clone, &["rev-parse", "HEAD~1"]), fetched);
}

#[test_log::test]
fn the_undo_is_refused_when_local_changes_overlap_or_the_branch_moved() {
  let (upstream, clone) = clone_of_upstream();
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  let (_, output) = run_in(&clone, &["update"]);
  let token = output["undo_token"].as_str().unwrap().to_string();
  let updated = clone.head();

  write(&clone, "shared.txt", "edited after the update\n");
  let (exit_code, undo) = run_in(&clone, &["undo", &token]);
  assert_error(&undo, exit_code, (4, "uncommitted_overlap"));
  assert_eq!(undo["error"]["paths"], json!(["shared.txt"]));
  assert_eq!(clone.head(), updated);
  assert_eq!(read(&clone, "shared.txt"), "edited after the update\n");

  write(&clone, "shared.txt", "one\nTWO\nthree\n");
  clone.create_commit("(auth) Add logout", "logout.txt", "logout\n");
  let (exit_code, undo) = run_in(&clone, &["undo", &token]);
  assert_error(&undo, exit_code, (4, "undo_stale"));
  assert_eq!(undo["error"]["retryable"], json!(false));

  // only full ids
  let (previous, new) = token.trim_start_matches("update:").split_once("..").unwrap();
  let (exit_code, undo) = run_in(&clone, &["undo", &format!("update:{}..{}", &previous[..12], &new[..12])]);
  assert_error(&undo, exit_code, (2, "invalid_arguments"));
}

#[test_log::test]
fn the_update_waits_for_the_repository_lock_after_its_fetch() {
  let (upstream, clone) = clone_of_upstream();
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  let head = clone.head();
  // the app holds the lock
  let held = lock_repository(clone.git_executor(), clone.path().to_str().unwrap(), std::time::Duration::from_millis(100)).unwrap();

  let (exit_code, output) = run_in(&clone, &["update"]);
  assert_error(&output, exit_code, (3, "locked"));
  assert_eq!(git(&clone, &["rev-parse", "origin/master"]), upstream.head(), "the fetch ran before the lock");
  assert_eq!(clone.head(), head);

  drop(held);
  let (exit_code, output) = run_in(&clone, &["update", "--no-fetch"]);
  assert_eq!((exit_code, &output["state"]), (0, &json!("fast_forward")), "{output}");
}

#[test_log::test]
fn a_failed_fetch_is_a_network_error() {
  let (upstream, clone) = clone_of_upstream();
  git(&clone, &["remote", "set-url", "origin", &format!("{}-gone", upstream.path().display())]);
  let (exit_code, output) = run_in(&clone, &["update"]);
  assert_error(&output, exit_code, (5, "network_error"));
  assert_eq!(output["error"]["retryable"], json!(true));
}

#[test_log::test]
fn update_outputs_match_the_schema() {
  let (upstream, clone) = clone_of_upstream();
  let (_, schema) = run_json(&environment(clone.path()), &["schema"]);
  let update_schema = &schema["outputs"]["update"];

  clone.create_commit("(auth) Change line 2", "shared.txt", "one\nmine\nthree\n");
  upstream.create_commit("Change line 2 upstream", "shared.txt", "one\ntheirs\nthree\n");
  git(&clone, &["fetch", "-q", "origin"]);
  let (_, conflict) = run_in(&clone, &["update", "--dry-run"]);
  assert_covered(update_schema, update_schema, &conflict, "update conflict");

  git(&clone, &["reset", "-q", "--hard", "origin/master"]);
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  upstream.create_commit("More upstream", "more.txt", "more\n");
  let (_, update) = run_in(&clone, &["update"]);
  assert_eq!(update["state"], json!("rebase"), "{update}");
  assert_covered(update_schema, update_schema, &update, "update");

  write(&clone, "more.txt", "edited\n");
  let (_, error) = run_in(&clone, &["undo", update["undo_token"].as_str().unwrap()]);
  assert_covered(&schema["outputs"]["error"], &schema["outputs"]["error"], &error, "error with paths");
  write(&clone, "more.txt", "more\n");
  let (_, undo) = run_in(&clone, &["undo", update["undo_token"].as_str().unwrap()]);
  assert_eq!(undo["landed_restored"], json!(true), "{undo}");
  assert_covered(&schema["outputs"]["undo"], &schema["outputs"]["undo"], &undo, "undo of an update");
}

/// A fetched upstream change and a branch `peer` at HEAD, in `clone`; bd runs a git wrapper whose `case "$*"` arm, built by
/// `arm` from the directory of the wrapper and the repository, runs before the real git: a peer's `git switch` at a
/// chosen moment
#[cfg(unix)]
struct SwitchSetup {
  clone: TestRepo,
  head: String,
  new_head: String,
  directory: tempfile::TempDir,
  environment: bd_core::context::Environment,
  _upstream: TestRepo,
}

#[cfg(unix)]
impl SwitchSetup {
  fn new(arm: impl FnOnce(&std::path::Path, &std::path::Path) -> String) -> Self {
    let (upstream, clone) = clone_of_upstream();
    let new_head = upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
    git(&clone, &["fetch", "-q", "origin"]);
    git(&clone, &["branch", "peer"]);
    let directory = tempfile::tempdir().unwrap();
    let script = directory.path().join("git");
    let text = format!("#!/bin/sh\ncase \" $* \" in {}\nesac\nexec git \"$@\"\n", arm(directory.path(), clone.path()));
    std::fs::write(&script, text).unwrap();
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let mut environment = environment(clone.path());
    environment.git = Some(script.to_str().unwrap().to_string());
    Self {
      head: clone.head(),
      clone,
      new_head,
      directory,
      environment,
      _upstream: upstream,
    }
  }

  fn run(&self, args: &[&str]) -> (i32, Value) {
    let mut all = vec!["--prefix", "test-user"];
    all.extend_from_slice(args);
    run_json(&self.environment, &all)
  }
}

/// The switch right before the first `update-ref --stdin` once the file `switch-once` exists: between bd's check and
/// its move
#[cfg(unix)]
#[test_log::test]
fn a_switch_between_the_check_and_the_move_moves_no_branch() {
  let setup = SwitchSetup::new(|directory, _| {
    let mark = directory.join("switch-once");
    format!(
      "*\" update-ref \"*\"--stdin\"*) if [ -f '{0}' ]; then rm -f '{0}'; git symbolic-ref HEAD refs/heads/peer; fi;;",
      mark.display()
    )
  });
  let SwitchSetup { clone, head, .. } = &setup;
  let mark = setup.directory.path().join("switch-once");

  std::fs::write(&mark, "").unwrap();
  let (exit_code, output) = setup.run(&["update", "--no-fetch"]);
  assert_error(&output, exit_code, (3, "head_moved"));
  assert_eq!(git(clone, &["symbolic-ref", "--short", "HEAD"]), "peer");
  assert_eq!(git(clone, &["rev-parse", "master", "peer"]), format!("{head}\n{head}"));
  assert_eq!(read(clone, "shared.txt"), SHARED);

  // the same for the undo: back on master, update, then the switch before the move back
  git(clone, &["symbolic-ref", "HEAD", "refs/heads/master"]);
  let (exit_code, output) = setup.run(&["update", "--no-fetch"]);
  assert_eq!(exit_code, 0, "{output}");
  git(clone, &["branch", "-f", "peer", &setup.new_head]);
  std::fs::write(&mark, "").unwrap();
  let (exit_code, undo) = setup.run(&["undo", output["undo_token"].as_str().unwrap()]);
  assert_error(&undo, exit_code, (4, "undo_stale"));
  assert_eq!(git(clone, &["rev-parse", "master", "peer"]), format!("{0}\n{0}", setup.new_head));
}

/// The switch at the first `symbolic-ref` while `HEAD.lock` exists, which is bd's check under the locks of the prepared
/// transaction: it fails, and the move commits on the checked branch
#[cfg(unix)]
#[test_log::test]
fn a_switch_during_the_move_fails_on_the_head_lock() {
  let setup = SwitchSetup::new(|directory, repository| {
    format!(
      "*\" symbolic-ref \"*) if [ -f '{0}' ] && [ ! -f '{1}' ]; then git symbolic-ref HEAD refs/heads/peer 2>/dev/null; echo $? > '{1}'; fi;;",
      repository.join(".git/HEAD.lock").display(),
      directory.join("switch-status").display()
    )
  });
  let SwitchSetup { clone, head, new_head, .. } = &setup;

  let (exit_code, output) = setup.run(&["update", "--no-fetch"]);
  assert_eq!((exit_code, &output["state"]), (0, &json!("fast_forward")), "{output}");
  let switch_status = std::fs::read_to_string(setup.directory.path().join("switch-status")).expect("the switch ran while HEAD.lock was held");
  assert_ne!(switch_status.trim(), "0", "the switch must fail on HEAD.lock");
  assert_eq!(git(clone, &["symbolic-ref", "--short", "HEAD"]), "master");
  assert_eq!(git(clone, &["rev-parse", "master", "peer"]), format!("{new_head}\n{head}"));
  assert_eq!(read(clone, "shared.txt"), "one\nTWO\nthree\n");
}

/// The arms of a wrapper that fails the `read-tree -u` of the files once the file `crash` exists, and then the rollback
/// of the branch: the state of a process killed between the move of the branch and its files
#[cfg(unix)]
fn crash_before_the_files(directory: &std::path::Path) -> String {
  let crash = directory.join("crash");
  let rollback = directory.join("rollback");
  format!(
    "*\" read-tree -m -u --no-recurse-submodules \"*) if [ -f '{0}' ]; then rm -f '{0}'; touch '{1}'; exit 1; fi;;\n\
     *\" update-ref \"*\"--stdin\"*) if [ -f '{1}' ]; then rm -f '{1}'; exit 1; fi;;",
    crash.display(),
    rollback.display()
  )
}

#[cfg(unix)]
fn pending_marks(repo: &TestRepo) -> String {
  git(repo, &["for-each-ref", "--format=%(refname) %(objectname)", "refs/branch-deck/update-pending/"])
}

#[cfg(unix)]
#[test_log::test]
fn an_update_killed_before_its_files_followed_is_finished_by_the_next_update() {
  let setup = SwitchSetup::new(|directory, _| crash_before_the_files(directory));
  let SwitchSetup { clone, head, new_head, .. } = &setup;
  std::fs::write(setup.directory.path().join("crash"), "").unwrap();
  let (exit_code, _) = setup.run(&["update", "--no-fetch"]);
  assert_ne!(exit_code, 0);
  // the branch moved with its mark, the files did not
  assert_eq!(clone.head(), *new_head);
  assert_eq!(read(clone, "shared.txt"), SHARED);
  assert_eq!(pending_marks(clone), format!("refs/branch-deck/update-pending/main {head}"));

  let (exit_code, session) = setup.run(&["--full-ids", "update", "--status"]);
  assert_eq!(exit_code, 0, "{session}");
  assert_eq!(session["pending_update"]["files_at"], json!(head), "{session}");
  assert!(session["pending_update"]["next"].as_str().unwrap().ends_with(" update --no-fetch"), "{session}");

  let (exit_code, output) = setup.run(&["update", "--no-fetch"]);
  assert_eq!((exit_code, &output["state"]), (0, &json!("up_to_date")), "{output}");
  assert_eq!(read(clone, "shared.txt"), "one\nTWO\nthree\n");
  assert_eq!((status(clone), pending_marks(clone)), (String::new(), String::new()));
}

#[cfg(unix)]
#[test_log::test]
fn an_undo_killed_before_its_files_followed_is_finished_by_the_next_undo() {
  let setup = SwitchSetup::new(|directory, _| crash_before_the_files(directory));
  let SwitchSetup { clone, head, new_head, .. } = &setup;
  let (exit_code, output) = setup.run(&["update", "--no-fetch"]);
  assert_eq!(exit_code, 0, "{output}");
  let token = output["undo_token"].as_str().unwrap();

  std::fs::write(setup.directory.path().join("crash"), "").unwrap();
  let (exit_code, _) = setup.run(&["undo", token]);
  assert_ne!(exit_code, 0);
  assert_eq!(clone.head(), *head);
  assert_eq!(pending_marks(clone), format!("refs/branch-deck/update-pending/main {new_head}"));

  let (exit_code, undo) = setup.run(&["undo", token]);
  assert_eq!((exit_code, &undo["already_undone"]), (0, &json!(true)), "{undo}");
  assert_eq!(read(clone, "shared.txt"), SHARED);
  assert_eq!((status(clone), pending_marks(clone)), (String::new(), String::new()));
}

/// The branch moved after the interrupted update: bd changes nothing and names the mark
#[cfg(unix)]
#[test_log::test]
fn an_interrupted_update_of_a_branch_that_moved_since_is_reported() {
  let setup = SwitchSetup::new(|directory, _| crash_before_the_files(directory));
  let SwitchSetup { clone, head, .. } = &setup;
  std::fs::write(setup.directory.path().join("crash"), "").unwrap();
  let (exit_code, _) = setup.run(&["update", "--no-fetch"]);
  assert_ne!(exit_code, 0);
  git(clone, &["update-ref", "refs/heads/master", head]);

  let (exit_code, output) = setup.run(&["update", "--no-fetch"]);
  assert_error(&output, exit_code, (4, "finish_pending"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains(&format!("git update-ref -d refs/branch-deck/update-pending/main {head}")), "{message}");
  assert_eq!((clone.head(), pending_marks(clone)), (head.clone(), format!("refs/branch-deck/update-pending/main {head}")));
}

/// A lock that a killed git left: the error is `locked`, names the file, and nothing moves
#[test_log::test]
fn a_stale_head_lock_is_locked_and_names_the_file() {
  let (upstream, clone) = clone_of_upstream();
  let head = clone.head();
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  git(&clone, &["fetch", "-q", "origin"]);
  std::fs::write(clone.path().join(".git/HEAD.lock"), "").unwrap();

  let (exit_code, output) = run_in(&clone, &["update", "--no-fetch"]);
  assert_error(&output, exit_code, (3, "locked"));
  let message = output["error"]["message"].as_str().unwrap();
  // the path as git names it: `C:/...` on Windows
  let named = git(&clone, &["rev-parse", "--path-format=absolute", "--git-path", "HEAD.lock"]);
  assert!(message.starts_with(&format!("{named} exists")), "{message}");
  assert_eq!((clone.head(), read(&clone, "shared.txt")), (head, SHARED.to_string()));
}

/// With `core.autocrlf=true`, the files of an update and of its undo are checked out with CRLF and the status is clean
#[test_log::test]
fn an_update_with_autocrlf_checks_out_crlf_and_leaves_a_clean_status() {
  let (upstream, clone) = clone_of_upstream();
  git(&clone, &["config", "core.autocrlf", "true"]);
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");

  let (exit_code, output) = run_in(&clone, &["update"]);
  assert_eq!((exit_code, &output["state"]), (0, &json!("fast_forward")), "{output}");
  assert_eq!((read(&clone, "shared.txt"), status(&clone)), ("one\r\nTWO\r\nthree\r\n".to_string(), String::new()));

  let (exit_code, undo) = run_in(&clone, &["undo", output["undo_token"].as_str().unwrap()]);
  assert_eq!(exit_code, 0, "{undo}");
  assert_eq!((read(&clone, "shared.txt"), status(&clone)), ("one\r\ntwo\r\nthree\r\n".to_string(), String::new()));
}
