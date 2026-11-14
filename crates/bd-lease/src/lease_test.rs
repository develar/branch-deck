#[cfg(unix)]
use crate::clones_lock_test::wait_for_the_slot;
use crate::clones_lock_test::{get_with_steps, kinds, new_slot_steps, printed};
use crate::lease_command::{LeaseContext, LeaseStatusArgs, lease_get_output, lease_status};
use bd_cli::fixture_test::assert_covered;
#[cfg(unix)]
use bd_cli::fixture_test::beside_a_held_sync;
use bd_cli::fixture_test::{PREFIX, assert_error, canonical, environment, git, gone_process, repo_on_baseline, repo_with_baseline, run_json};
use bd_core::context::Environment;
use git_executor::git_command_executor::GitCommandExecutor;
use lease_pool::landed_work::{LandedCommit, Proof};
use lease_pool::lease::GetStep;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use test_utils::git_test_utils::{TestRepo, TestWorktree};
use test_utils::history::HistoryCommit;

/// A repository with a commit on master, the home directory its pool lives in and the global git config of bd
pub(crate) struct Fixture {
  pub(crate) repo: TestRepo,
  pub(crate) home: tempfile::TempDir,
  /// Empty unless a test writes it
  pub(crate) global_config: tempfile::NamedTempFile,
}

/// brd started in `cwd`, the pool in `home`, `global_config` its global git config: the environment of a `Fixture`, also
/// in a child process that has only its paths
pub(crate) fn pool_environment(cwd: &Path, home: &Path, global_config: &Path) -> Environment {
  let mut environment = environment(cwd);
  environment.home = Some(home.to_path_buf());
  environment.git_config_global = Some(global_config.to_path_buf());
  environment.lock_timeout = Duration::from_secs(10);
  environment
}

/// A child process killed when dropped, also when an assertion fails
pub(crate) struct Killed(pub(crate) std::process::Child);

impl Drop for Killed {
  fn drop(&mut self) {
    let _ = self.0.kill();
    let _ = self.0.wait();
  }
}

impl Fixture {
  pub(crate) fn new() -> Self {
    let (repo, _) = repo_on_baseline(&[("a.txt", "a\n")], &[HistoryCommit::new("Add b", &[("b.txt", "b\n")])], ".");
    Self {
      repo,
      home: tempfile::tempdir().unwrap(),
      global_config: tempfile::NamedTempFile::new().unwrap(),
    }
  }

  /// brd started in `cwd`, the pool in the fixture's home
  pub(crate) fn environment(&self, cwd: &Path) -> Environment {
    pool_environment(cwd, self.home.path(), self.global_config.path())
  }

  /// `bd <args>` in the repository
  pub(crate) fn run(&self, args: &[&str]) -> (i32, Value) {
    self.run_in(self.repo.path(), args)
  }

  pub(crate) fn run_in(&self, cwd: &Path, args: &[&str]) -> (i32, Value) {
    run_json(&self.environment(cwd), args)
  }

  /// `brd lease get <args>`, which must succeed
  pub(crate) fn get(&self, args: &[&str]) -> Value {
    let (exit_code, output) = self.run(&[&["lease", "get"][..], args].concat());
    assert_eq!(exit_code, 0, "{output}");
    output
  }

  /// `brd lease return` run inside the slot, which must succeed
  pub(crate) fn give_back(&self, path: &str) -> Value {
    let (exit_code, output) = self.run_in(Path::new(path), &["lease", "return"]);
    assert_eq!(exit_code, 0, "{output}");
    output
  }

  pub(crate) fn status(&self) -> Value {
    let (exit_code, output) = self.run(&["lease", "status"]);
    assert_eq!(exit_code, 0, "{output}");
    output
  }

  /// `$GIT_COMMON_DIR/branch-deck/leases.json`
  pub(crate) fn state_file(&self) -> PathBuf {
    self.repo.path().join(".git/branch-deck/leases.json")
  }
}

/// `path` as the pool names a slot: canonical, with backslashes on Windows
pub(crate) fn slot_path(path: &str) -> String {
  canonical(Path::new(path)).to_string_lossy().into_owned()
}

/// git in a slot; panics on a failure
pub(crate) fn slot_git(path: &str, args: &[&str]) -> String {
  GitCommandExecutor::new()
    .execute_command(args, path)
    .unwrap_or_else(|e| panic!("git {args:?} in {path}: {e}"))
}

pub(crate) fn path_of(output: &Value) -> String {
  output["path"].as_str().unwrap_or_else(|| panic!("no path: {output}")).to_string()
}

/// The `state` of each slot of `brd lease status`, by path
pub(crate) fn states(status: &Value) -> Vec<(String, String)> {
  status["slots"]
    .as_array()
    .unwrap()
    .iter()
    .map(|slot| (slot["path"].as_str().unwrap().to_string(), slot["state"].as_str().unwrap().to_string()))
    .collect()
}

/// A `sleep` with its working directory in `dir`, in a process group of its own: a process of another session, which
/// keeps a slot busy (a child in the test's group is of the session that runs bd, as the `jq` of its pipeline)
#[cfg(unix)]
pub(crate) fn busy_in(dir: &str) -> std::process::Child {
  use std::os::unix::process::CommandExt;
  std::process::Command::new("sleep").arg("30").current_dir(dir).process_group(0).spawn().unwrap()
}

/// The bug of the Treehouse pool: a free slot parked at a commit that a rewrite of master made unreachable was never
/// reused. The pool reuses it, and only the diff to the new HEAD is written
#[test_log::test]
fn a_free_slot_is_reused_after_master_was_rewritten_past_its_commit() {
  let fixture = Fixture::new();
  let repo = &fixture.repo;
  let first = fixture.get(&["--holder", "lane-1"]);
  let path = path_of(&first);
  assert_eq!(
    (&first["reused"], &first["source_head"], &first["holder"], &first["seeded"]),
    (&json!(false), &json!(repo.head()), &json!("lane-1"), &json!([]))
  );
  // the leaf directory keeps the name of the repository
  assert_eq!(Path::new(&path).file_name(), repo.path().file_name());
  assert!(Path::new(&path).starts_with(canonical(fixture.home.path()).join(".branch-deck/pool")), "{path}");
  assert_eq!(slot_git(&path, &["rev-parse", "HEAD"]), repo.head());
  fixture.give_back(&path);
  assert_eq!(states(&fixture.status()), [(path.clone(), "free".to_string())]);

  let parked = repo.head();
  std::fs::write(repo.path().join("b.txt"), "rewritten\n").unwrap();
  git(repo, &["commit", "-q", "-a", "--amend", "-m", "Add b, rewritten"]);
  let (_, ancestor) = repo
    .git_executor()
    .execute_command_with_status(&["merge-base", "--is-ancestor", &parked, "master"], repo.path_str())
    .unwrap();
  assert_ne!(ancestor, 0, "the parked commit is no longer in master");

  let second = fixture.get(&[]);
  assert_eq!(
    (path_of(&second), &second["reused"], &second["source_head"], &second["slot"]),
    (path.clone(), &json!(true), &json!(repo.head()), &first["slot"])
  );
  assert!(second["holder"].as_str().unwrap().starts_with("agent-"), "{second}");
  assert_ne!(second["lease_id"], first["lease_id"]);
  assert_eq!(slot_git(&path, &["rev-parse", "HEAD"]), repo.head());
  assert_eq!(std::fs::read_to_string(Path::new(&path).join("b.txt")).unwrap(), "rewritten\n");
  // detached: no branch of the slot moves with a later commit
  let (_, attached) = GitCommandExecutor::new().execute_command_with_status(&["symbolic-ref", "-q", "HEAD"], &path).unwrap();
  assert_ne!(attached, 0);
  assert_eq!(states(&fixture.status()), [(path.clone(), "ready".to_string())]);

  // master is rewritten past the leased commit again: the commit the slot started with is no unlanded work
  git(repo, &["commit", "-q", "--amend", "-m", "Add b, rewritten again"]);
  fixture.give_back(&path);
}

/// A free slot of the pool with the untracked `left.txt` in it
fn free_slot_with_a_stray_file(fixture: &Fixture) -> String {
  let slot = path_of(&fixture.get(&[]));
  fixture.give_back(&slot);
  std::fs::write(Path::new(&slot).join("left.txt"), "left behind\n").unwrap();
  slot
}

/// A free slot with local changes is skipped, marked dirty and tried last; with no other free slot the pool grows
#[test_log::test]
fn a_dirty_free_slot_is_skipped_and_a_new_slot_is_added() {
  let fixture = Fixture::new();
  let first = free_slot_with_a_stray_file(&fixture);

  let second = fixture.get(&[]);
  assert_eq!(second["reused"], json!(false));
  assert_ne!(path_of(&second), first);
  assert_eq!(
    second["skipped"],
    json!([{ "slot": fixture.status()["slots"][0]["slot"], "path": first, "reason": "dirty", "paths": ["left.txt"] }])
  );
  assert_eq!(fixture.status()["slots"][0]["dirty"], json!(true));

  // once clean, the dirty slot is reused, the mark cleared
  std::fs::remove_file(Path::new(&first).join("left.txt")).unwrap();
  let third = fixture.get(&[]);
  assert_eq!((path_of(&third), &third["reused"]), (first, &json!(true)));
  assert_eq!(fixture.status()["slots"][0].get("dirty"), None);
}

/// A get skips a dirty slot without a `status` while one of its dirty paths has its stat of the `status` that found it
/// and the slot did not move: an edit elsewhere in the tree keeps it skipped; a change of `info/exclude`, which may hide
/// the stray file, brings the `status` back, and so does one of `.gitignore`
#[test_log::test]
fn a_dirty_slot_that_its_files_prove_dirty_is_skipped_without_a_status() {
  let fixture = Fixture::new();
  let first = free_slot_with_a_stray_file(&fixture);
  let marked = fixture.get(&[]);
  let steps = std::sync::Mutex::new(Vec::new());
  let get = |holder: &str| {
    steps.lock().unwrap().clear();
    let got = serde_json::to_value(get_with_steps(&fixture, holder, &steps).unwrap()).unwrap();
    assert_eq!(got["reused"], json!(false), "{got}");
    (got["skipped"][0]["paths"].clone(), kinds(&steps))
  };
  let without_status = [&["reserve", "reserve adding"][..], new_slot_steps()].concat();
  assert_eq!(get("unchanged"), (marked["skipped"][0]["paths"].clone(), without_status.clone()));
  std::fs::write(Path::new(&first).join("another.txt"), "elsewhere\n").unwrap();
  assert_eq!(get("an edit elsewhere"), (json!(["left.txt"]), without_status));
  let exclude = fixture.repo.path().join(".git/info/exclude");
  std::fs::create_dir_all(exclude.parent().unwrap()).unwrap();
  std::fs::write(&exclude, "/left.txt\n").unwrap();
  assert_eq!(
    get("a change of info/exclude"),
    (json!(["another.txt"]), [&["reserve", "status", "reserve", "reserve adding"][..], new_slot_steps()].concat())
  );
  // the clone of this template, whose untracked `.gitignore` hides a file, is clean too
  // (`a_clone_of_a_template_with_an_untracked_gitignore_is_clean`)
  std::fs::write(Path::new(&first).join(".gitignore"), "/another.txt\n").unwrap();
  assert_eq!(
    get("a change of .gitignore"),
    (json!([".gitignore"]), [&["reserve", "status", "reserve", "reserve adding"][..], new_slot_steps()].concat())
  );
}

/// A free slot with a process in it is skipped as busy, and the get adds a new slot
#[cfg(unix)]
#[test_log::test]
fn a_busy_free_slot_is_skipped() {
  let fixture = Fixture::new();
  let first = path_of(&fixture.get(&[]));
  fixture.give_back(&first);
  let mut sleep = busy_in(&first);
  let (exit_code, second) = fixture.run(&["lease", "get"]);
  sleep.kill().unwrap();
  sleep.wait().unwrap();
  assert_eq!(exit_code, 0, "{second}");
  assert_ne!(path_of(&second), first);
  let skipped = &second["skipped"][0];
  assert_eq!(
    (&skipped["path"], &skipped["reason"], &skipped["processes"][0]),
    (
      &json!(first),
      &json!("busy"),
      &json!({ "pid": sleep.id(), "command": "sleep", "argv": "sleep 30", "kind": "other" })
    )
  );
}

/// The `argv` of every git that started since the trace file was emptied, from its trace2 `start` events
#[cfg(unix)]
fn started_gits(trace: &Path) -> Vec<Vec<String>> {
  let text = std::fs::read_to_string(trace).unwrap_or_default();
  std::fs::write(trace, "").unwrap();
  text
    .lines()
    .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    .filter(|event| event["event"] == json!("start"))
    .map(|event| event["argv"].as_array().unwrap().iter().map(|arg| arg.as_str().unwrap().to_string()).collect())
    .collect()
}

/// The `argv` of the gits whose subcommand (after the `-c` options) is `subcommand`
#[cfg(unix)]
fn gits_of<'a>(gits: &'a [Vec<String>], subcommand: &str) -> Vec<&'a Vec<String>> {
  gits
    .iter()
    .filter(|argv| {
      argv
        .iter()
        .skip(1)
        .filter(|arg| !arg.starts_with('-') && !arg.contains('='))
        .take(1)
        .any(|arg| arg == subcommand)
    })
    .collect()
}

/// An ignored file in the source checkout that nobody can read: `clonefile(2)` of the source fails with `EACCES`, and
/// the get checks out instead. Readable again when the guard drops, so the temporary directory goes
#[cfg(unix)]
struct UnreadableFile(PathBuf);

#[cfg(unix)]
impl UnreadableFile {
  fn new(repo: &TestRepo) -> Self {
    use std::os::unix::fs::PermissionsExt;
    let info = repo.path().join(".git/info");
    std::fs::create_dir_all(&info).unwrap();
    std::fs::write(info.join("exclude"), "/unreadable.bin\n").unwrap();
    let path = repo.path().join("unreadable.bin");
    std::fs::write(&path, "secret").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    Self(path)
  }
}

#[cfg(unix)]
impl Drop for UnreadableFile {
  fn drop(&mut self) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o644));
  }
}

/// The checkouts and the moves of the pool (the `reset --merge` of a reused slot, the checkout of a cloned one and of a
/// new one after its `worktree add --no-checkout`) run with `checkout.workers=0`, one worker per core, unless the
/// repository sets `checkout.workers` itself
#[cfg(unix)]
#[test_log::test]
fn every_checkout_of_the_pool_runs_with_a_worker_per_core_unless_the_repository_sets_it() {
  let fixture = Fixture::new();
  let trace = fixture.home.path().join("trace2.json");
  let _env = test_utils::env_vars::EnvVars::set(&[("GIT_TRACE2_EVENT", &trace)]);
  let parallel = |argv: &&Vec<String>| argv.windows(2).any(|pair| pair[0] == "-c" && pair[1] == "checkout.workers=0");
  let moves = |gits: &[Vec<String>]| gits_of(gits, "reset").into_iter().filter(|argv| argv.contains(&"--merge".to_string())).count();

  // a new slot by a registration and a full checkout: the clone of the source fails on a file it can't read
  let unreadable = UnreadableFile::new(&fixture.repo);
  let added = fixture.get(&[]);
  drop(unreadable);
  assert_eq!(added["created_by"], json!("checkout"), "{added}");
  let registered = |gits: &[Vec<String>]| {
    gits_of(gits, "worktree")
      .iter()
      .any(|argv| argv.contains(&"add".to_string()) && argv.contains(&"--no-checkout".to_string()))
  };
  let gits = started_gits(&trace);
  let checkouts = gits_of(&gits, "checkout");
  assert!(registered(&gits) && !checkouts.is_empty() && checkouts.iter().all(parallel), "{gits:?}");
  // a reused slot, then a new one by a clone of the source on macOS, by a registration and a checkout elsewhere
  let path = path_of(&added);
  fixture.give_back(&path);
  fixture.repo.create_commit("Add c", "c.txt", "c\n");
  started_gits(&trace);
  fixture.get(&[]);
  let cloned = fixture.get(&[]);
  let gits = started_gits(&trace);
  let checkouts = gits_of(&gits, "checkout");
  let resets = gits_of(&gits, "reset");
  assert!(moves(&gits) == 1 && resets.iter().all(parallel), "the reused slot moves by the diff: {gits:?}");
  if cfg!(target_os = "macos") {
    assert_eq!(cloned["created_by"], json!("clone"), "{cloned}");
  } else {
    assert!(registered(&gits), "{gits:?}");
  }
  assert!(!checkouts.is_empty() && checkouts.iter().all(parallel), "{gits:?}");

  fixture.give_back(&path);
  fixture.repo.set_config("checkout.workers", "2").unwrap();
  fixture.repo.create_commit("Add d", "d.txt", "d\n");
  started_gits(&trace);
  fixture.get(&[]);
  let gits = started_gits(&trace);
  let resets = gits_of(&gits, "reset");
  assert!(moves(&gits) == 1 && !resets.iter().any(parallel), "{gits:?}");
}

/// Every `status` of the pool lists untracked files in the mode `status.showUntrackedFiles` configures, `all` or
/// `normal`: git's untracked cache answers only in the mode it was built for. `no` lists them as `normal` does, so an
/// untracked file still makes a slot dirty
#[cfg(unix)]
#[test_log::test]
fn every_status_of_the_pool_lists_untracked_files_in_the_mode_the_repository_configures() {
  let fixture = Fixture::new();
  let trace = fixture.home.path().join("trace2.json");
  let _env = test_utils::env_vars::EnvVars::set(&[("GIT_TRACE2_EVENT", &trace)]);
  // the `--untracked-files` of each `status`, of a get and the return of its slot
  let modes = |gits: &[Vec<String>]| -> Vec<String> {
    gits_of(gits, "status")
      .iter()
      .map(|argv| argv.iter().find(|arg| arg.starts_with("--untracked-files=")).cloned().unwrap_or_default())
      .collect()
  };
  let get_and_return = || {
    let path = path_of(&fixture.get(&[]));
    fixture.give_back(&path);
    (path, modes(&started_gits(&trace)))
  };

  let (path, unset) = get_and_return();
  assert!(!unset.is_empty() && unset.iter().all(|mode| mode == "--untracked-files=normal"), "{unset:?}");
  fixture.repo.set_config("status.showUntrackedFiles", "all").unwrap();
  let (_, all) = get_and_return();
  assert!(!all.is_empty() && all.iter().all(|mode| mode == "--untracked-files=all"), "{all:?}");

  fixture.repo.set_config("status.showUntrackedFiles", "no").unwrap();
  std::fs::write(Path::new(&path).join("left.txt"), "left behind\n").unwrap();
  let got = fixture.get(&[]);
  assert_eq!((&got["skipped"][0]["path"], &got["skipped"][0]["paths"]), (&json!(path), &json!(["left.txt"])), "{got}");
  let no = modes(&started_gits(&trace));
  assert!(!no.is_empty() && no.iter().all(|mode| mode == "--untracked-files=normal"), "{no:?}");
}

/// A get takes the free slot returned last first: its index, fsmonitor daemon and build caches are the warmest and its
/// commit the closest; the order they joined the pool decides only among the slots never returned. Each return records
/// its number in the pool and its time, by which the shrink orders the slots of every pool
#[test_log::test]
fn the_free_slot_returned_last_is_leased_first() {
  let fixture = Fixture::new();
  let first = path_of(&fixture.get(&[]));
  let second = path_of(&fixture.get(&[]));
  let before = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
  fixture.give_back(&first);
  fixture.give_back(&second);
  let state: Value = serde_json::from_str(&std::fs::read_to_string(fixture.state_file()).unwrap()).unwrap();
  let returns: Vec<(&Value, bool)> = state["slots"]
    .as_array()
    .unwrap()
    .iter()
    .map(|slot| (&slot["returned"], slot["returned_at"].as_u64().is_some_and(|at| at >= before)))
    .collect();
  assert_eq!(returns, [(&json!(1), true), (&json!(2), true)], "{state}");
  assert_eq!(path_of(&fixture.get(&[])), second);
  assert_eq!(path_of(&fixture.get(&[])), first);
}

/// A sync in a lease marks its worktree synced (`synced_mark`), in the git directory of the slot, which the slot keeps
/// across its leases: the return takes the mark out, and so does the get of a free slot that still has one, so the next
/// lease of the slot is a worktree no sync built anything for, and a window that opens it shows the Ready to Sync card
/// rather than syncing it of itself
#[test_log::test]
fn a_return_takes_out_the_mark_of_a_sync_in_the_lease() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  let mark = slot_git(&path, &["rev-parse", "--path-format=absolute", "--git-path", sync_core::synced_mark::SYNCED_FILE]);
  sync_core::synced_mark::mark_synced(Path::new(mark.trim()), PREFIX);
  assert!(sync_core::synced_mark::synced_before(&path, PREFIX));

  fixture.give_back(&path);
  assert!(!sync_core::synced_mark::synced_before(&path, PREFIX));
  assert_eq!(path_of(&fixture.get(&[])), path, "the same slot again");
  assert!(!sync_core::synced_mark::synced_before(&path, PREFIX));

  // a free slot a return of an older build left marked: the get takes the mark out too
  fixture.give_back(&path);
  sync_core::synced_mark::mark_synced(Path::new(mark.trim()), PREFIX);
  assert_eq!(path_of(&fixture.get(&[])), path);
  assert!(!sync_core::synced_mark::synced_before(&path, PREFIX));
}

/// The epoch of a worktree is the lease its slot holds now (`worktree_epoch`): what the app recorded of the slot by its
/// path under one lease is of another epoch once the slot is returned and leased again, so it counts as absent; the
/// main checkout, which is no slot, has none
#[test_log::test]
fn the_epoch_of_a_slot_is_its_lease_and_changes_with_each_return_and_get() {
  let fixture = Fixture::new();
  let got = fixture.get(&[]);
  let path = path_of(&got);
  let first = lease_pool::epoch::worktree_epoch(Path::new(&path));
  assert_eq!(first.as_deref(), got["lease_id"].as_str());

  fixture.give_back(&path);
  let free = lease_pool::epoch::worktree_epoch(Path::new(&path));
  assert!(free.is_some() && free != first, "{free:?}");

  let again = fixture.get(&[]);
  assert_eq!(path_of(&again), path, "the same slot again");
  let second = lease_pool::epoch::worktree_epoch(Path::new(&path));
  assert_eq!(second.as_deref(), again["lease_id"].as_str());
  assert!(second != first && second != free);

  assert_eq!(lease_pool::epoch::worktree_epoch(fixture.repo.path()), None, "the main checkout is no slot");
}

/// The `status` of the pool writes the slot's index: after a get and its return, a `status` that may not write (the
/// executor's default) finds the untracked cache filled and opens almost no directory, where a cold index opens all
#[cfg(unix)]
#[test_log::test]
fn the_status_of_a_slot_leaves_its_untracked_cache_warm() {
  let fixture = Fixture::new();
  fixture.repo.set_config("core.untrackedCache", "true").unwrap();
  let files: Vec<(String, String)> = (0..20).map(|n| (format!("dir{n}/sub/file.txt"), format!("{n}\n"))).collect();
  let files: Vec<(&str, &str)> = files.iter().map(|(path, content)| (path.as_str(), content.as_str())).collect();
  fixture.repo.create_commit_with_files("Add the directories", &files);
  let trace = fixture.home.path().join("trace2.json");
  // the counters of `read_directory` are nested deeper than the 2 levels trace2 keeps by default
  let _env = test_utils::env_vars::EnvVars::set(&[("GIT_TRACE2_EVENT", trace.as_os_str()), ("GIT_TRACE2_EVENT_NESTING", std::ffi::OsStr::new("10"))]);
  // `opendir` of the `read_directory` of one `status` in the slot that may not write its index
  let opened = |path: &str| -> u64 {
    std::fs::write(&trace, "").unwrap();
    slot_git(path, &["status", "--porcelain=v1", "-z", "--untracked-files=normal"]);
    std::fs::read_to_string(&trace)
      .unwrap()
      .lines()
      .filter_map(|line| serde_json::from_str::<Value>(line).ok())
      .filter(|event| event["event"] == json!("data") && event["category"] == json!("read_directory") && event["key"] == json!("opendir"))
      .filter_map(|event| event["value"].as_str().and_then(|value| value.parse().ok()))
      .next_back()
      .unwrap_or_else(|| panic!("no read_directory in the trace"))
  };

  let path = path_of(&fixture.get(&[]));
  // git leaves a directory changed in the second of the index write out of the cache; a real slot's are older
  let hour_ago = std::time::SystemTime::now() - Duration::from_secs(3600);
  for (dir, _) in &files {
    for dir in Path::new(dir).ancestors().skip(1).filter(|dir| !dir.as_os_str().is_empty()) {
      std::fs::File::open(Path::new(&path).join(dir)).unwrap().set_modified(hour_ago).unwrap();
    }
  }
  fixture.give_back(&path);
  let warm = opened(&path);
  assert!(warm <= 2, "a warm status opened {warm} directories");
  // the same `status` on an index whose cache is empty opens every directory, again and again
  slot_git(&path, &["update-index", "--no-untracked-cache"]);
  slot_git(&path, &["update-index", "--untracked-cache"]);
  let cold = (opened(&path), opened(&path));
  assert!(cold.0 > 40 && cold.1 > 40, "a cold status opened {cold:?} directories");
}

/// The entries of the pool directory of `got`, without the slot names: what a clone may leave behind
#[cfg(unix)]
fn pool_entries(got: &Value) -> Vec<String> {
  let mut entries: Vec<String> = std::fs::read_dir(got["pool"].as_str().unwrap())
    .unwrap()
    .flatten()
    .flat_map(|number| {
      std::fs::read_dir(number.path())
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
    })
    .collect();
  entries.sort();
  entries
}

/// A source with an ignored build output, a change of a tracked file, a staged new file and an untracked file
#[cfg(target_os = "macos")]
fn source_with_local_work(fixture: &Fixture) {
  let repo = &fixture.repo;
  repo.create_commit("Ignore the build", ".gitignore", "/build/\n");
  std::fs::create_dir_all(repo.path().join("build")).unwrap();
  std::fs::write(repo.path().join("build/out.txt"), "warm\n").unwrap();
  std::fs::write(repo.path().join("a.txt"), "peer edit\n").unwrap();
  std::fs::write(repo.path().join("staged.txt"), "staged\n").unwrap();
  git(repo, &["add", "staged.txt"]);
  std::fs::write(repo.path().join("peer.txt"), "untracked\n").unwrap();
}

/// With no free slot, a new slot is an APFS clone of the source checkout: its HEAD's files, its ignored build outputs
/// (the warm cache), never its local changes, staged files or untracked files; the source keeps all of them
#[cfg(target_os = "macos")]
#[test_log::test]
fn a_new_slot_is_a_clone_of_the_source_without_its_local_work() {
  let fixture = Fixture::new();
  source_with_local_work(&fixture);
  let got = fixture.get(&[]);
  let path = path_of(&got);
  assert_eq!(
    (&got["created_by"], &got["cloned_from"], &got["reused"]),
    (&json!("clone"), &json!(git(&fixture.repo, &["rev-parse", "--show-toplevel"])), &json!(false))
  );
  let slot = Path::new(&path);
  assert_eq!(std::fs::read_to_string(slot.join("a.txt")).unwrap(), "a\n");
  assert_eq!(std::fs::read_to_string(slot.join("build/out.txt")).unwrap(), "warm\n");
  assert!(!slot.join("staged.txt").exists() && !slot.join("peer.txt").exists());
  assert_eq!(slot_git(&path, &["status", "--porcelain", "--untracked-files=all"]), "");
  assert_eq!(slot_git(&path, &["rev-parse", "HEAD"]), fixture.repo.head());
  // a linked worktree of the repository, its `.git` a file
  assert!(slot.join(".git").is_file());
  assert_eq!(
    slot_git(&path, &["rev-parse", "--path-format=absolute", "--git-common-dir"]),
    git(&fixture.repo, &["rev-parse", "--path-format=absolute", "--git-common-dir"])
  );
  assert_eq!(pool_entries(&got), [fixture.repo.path().file_name().unwrap().to_string_lossy()]);
  // the source as it was
  assert_eq!(std::fs::read_to_string(fixture.repo.path().join("a.txt")).unwrap(), "peer edit\n");
  // the output is trimmed: the first line loses its leading space
  assert_eq!(git(&fixture.repo, &["status", "--porcelain"]), "M a.txt\nA  staged.txt\n?? peer.txt");
  fixture.give_back(&path);
}

/// A free slot the get passed over (here a dirty one) is the template of the new slot: marked `templating` for the
/// clone, free again afterwards; the new slot is at the source HEAD, without the template's untracked file and with
/// its ignored files
#[cfg(target_os = "macos")]
#[test_log::test]
fn a_new_slot_is_a_clone_of_a_free_slot() {
  let fixture = Fixture::new();
  fixture.repo.create_commit("Ignore the build", ".gitignore", "/build/\n");
  let template = path_of(&fixture.get(&[]));
  fixture.give_back(&template);
  std::fs::create_dir_all(Path::new(&template).join("build")).unwrap();
  std::fs::write(Path::new(&template).join("build/cache.txt"), "warm\n").unwrap();
  std::fs::write(Path::new(&template).join("left.txt"), "left\n").unwrap();
  fixture.repo.create_commit("Add c", "c.txt", "c\n");

  let got = fixture.get(&[]);
  let path = path_of(&got);
  assert_ne!(path, template);
  assert_eq!((&got["created_by"], &got["cloned_from"]), (&json!("clone"), &json!(template)), "{got}");
  let slot = Path::new(&path);
  assert_eq!(std::fs::read_to_string(slot.join("c.txt")).unwrap(), "c\n");
  assert_eq!(std::fs::read_to_string(slot.join("build/cache.txt")).unwrap(), "warm\n");
  assert!(!slot.join("left.txt").exists());
  assert_eq!(slot_git(&path, &["status", "--porcelain"]), "");
  // the template is free again, still dirty, its files as they were
  let status = fixture.status();
  assert_eq!(states(&status), [(template.clone(), "free".to_string()), (path, "ready".to_string())]);
  assert_eq!(status["slots"][0]["dirty"], json!(true));
  assert!(Path::new(&template).join("left.txt").exists());
}

/// A clone that `clonefile(2)` refuses (here a file it can't read) leaves nothing behind, and the get checks out instead
#[cfg(unix)]
#[test_log::test]
fn a_clone_that_fails_falls_back_to_a_checkout() {
  let fixture = Fixture::new();
  let unreadable = UnreadableFile::new(&fixture.repo);
  let got = fixture.get(&[]);
  drop(unreadable);
  assert_eq!((&got["created_by"], got.get("cloned_from")), (&json!("checkout"), None), "{got}");
  assert_eq!(pool_entries(&got), [fixture.repo.path().file_name().unwrap().to_string_lossy()]);
  let path = path_of(&got);
  assert_eq!(slot_git(&path, &["status", "--porcelain"]), "");
  assert_eq!(std::fs::read_to_string(Path::new(&path).join("b.txt")).unwrap(), "b\n");
}

/// The trash of the pool of `fixture` is empty, and the log `text` has `freed.1` lines `freed.0`, one per slot a command
/// freed, each before the line `deleted` of the delete of their trash: the command deleted it once every slot was free
pub(crate) fn assert_trash_deleted_once_free(fixture: &Fixture, text: &str, freed: (&str, usize), deleted: &str) {
  let trash = fixture.repo.path().join(".git/branch-deck/trash");
  assert_eq!(std::fs::read_dir(&trash).map_or(0, Iterator::count), 0, "the trash is empty");
  let lines: Vec<usize> = text.match_indices(freed.0).map(|(at, _)| at).collect();
  let deleted = text.find(deleted).unwrap_or_else(|| panic!("no {deleted:?} in {text}"));
  assert!(lines.len() == freed.1 && lines.iter().all(|&at| at < deleted), "{text}");
}

/// Commit `file` in the slot at `path`
pub(crate) fn commit_in(path: &str, subject: &str, file: &str) -> String {
  commit_content(path, subject, file, &format!("{subject}\n"))
}

/// A lease whose commits landed by `brd pick --follow`, on master with a peer commit, after which a peer rewrote master:
/// the slot's HEAD is reachable from no branch. Returns the fixture and the slot
fn followed_then_rewritten() -> (Fixture, String) {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  commit_in(&path, "(lane) Add a", "lane-a.txt");
  commit_in(&path, "(lane) Add b", "lane-b.txt");
  fixture.repo.create_commit("(peer) Add p", "p.txt", "p\n");
  let (exit_code, picked) = bd_cli::fixture_test::run_in(&fixture.repo, &["pick", "--from", &path, "--follow"]);
  assert_eq!((exit_code, &picked["followed"]["state"]), (0, &json!("moved")), "{picked}");
  let followed = slot_git(&path, &["rev-parse", "HEAD"]);
  git(&fixture.repo, &["commit", "-q", "--amend", "-m", "(lane) Add b, reworded"]);
  let (_, reachable) = fixture
    .repo
    .git_executor()
    .execute_command_with_status(&["merge-base", "--is-ancestor", &followed, "master"], fixture.repo.path_str())
    .unwrap();
  assert_ne!(reachable, 0, "the followed tip is no longer in master");
  (fixture, path)
}

/// After a follow and a rewrite of master, the slot's commits are landed by content: the return decides by a dry run
/// of the pick onto the source branch, not by reachability
#[test_log::test]
fn a_return_after_a_follow_and_a_rewrite_of_master_finds_the_work_landed_by_content() {
  let (fixture, path) = followed_then_rewritten();
  let returned = fixture.give_back(&path);
  assert_eq!((&returned["landed_check"], &returned["landed_branch"]), (&json!("content"), &json!("master")), "{returned}");
  assert_eq!(returned.get("given_up"), None);
}

/// Write `content` into `file` of the slot at `path` and commit it as `subject`
pub(crate) fn commit_content(path: &str, subject: &str, file: &str, content: &str) -> String {
  std::fs::write(Path::new(path).join(file), content).unwrap();
  slot_git(path, &["add", file]);
  slot_git(path, &["commit", "-q", "-m", subject]);
  slot_git(path, &["rev-parse", "HEAD"])
}

fn commit_as_other_author(repo: &TestRepo, subject: &str, files: &[(&str, &str)]) -> String {
  for (file, content) in files {
    std::fs::write(repo.path().join(file), content).unwrap();
    git(repo, &["add", file]);
  }
  slot_git(
    repo.path_str(),
    &["-c", "user.name=Target Author", "-c", "user.email=target@example.com", "commit", "-q", "-m", subject],
  );
  git(repo, &["rev-parse", "HEAD"])
}

/// The `(commit, proof)` of each commit a return or a takeover found landed
pub(crate) fn proofs(landed: &Value) -> Vec<(String, String)> {
  landed
    .as_array()
    .unwrap_or_else(|| panic!("no landed commits: {landed}"))
    .iter()
    .map(|commit| (commit["commit"].as_str().unwrap().to_string(), commit["proof"].as_str().unwrap().to_string()))
    .collect()
}

/// A commit of the slot landed on master with a conflict resolution, as the twin of `brd update`: the same identity, and
/// a change that holds every line of the commit and its fixup, with a line more. Its pick conflicts, its patch id
/// differs, and the ladder proves nothing; the twin proves both
#[test_log::test]
fn a_return_finds_a_commit_that_landed_with_a_conflict_resolution() {
  let fixture = Fixture::new();
  fixture.repo.create_commit("Add f", "f.txt", "a\nz\n");
  let path = path_of(&fixture.get(&[]));
  let commit = commit_content(&path, "(lane) Add b to f", "f.txt", "a\nb\nz\n");
  let fixup = commit_content(&path, "fixup! (lane) Add b to f", "f.txt", "a\nb\nb2\nz\n");
  fixture.repo.create_commit("(peer) Add m to f", "f.txt", "a\nm\nz\n");
  // the robot's resolution: the commit with its fixup folded in, onto the peer's line, and one line of its own
  std::fs::write(fixture.repo.path().join("f.txt"), "a\nb\nb2\nm\nz2\n").unwrap();
  git(&fixture.repo, &["commit", "-q", "-a", "-C", &commit]);

  let returned = fixture.give_back(&path);
  assert_eq!(
    (proofs(&returned["landed"]), &returned["landed_check"]),
    (vec![(fixup, "twin".to_string()), (commit, "twin".to_string())], &json!("content")),
    "{returned}"
  );
}

#[test_log::test]
fn a_return_counts_two_folded_fixups_as_landed_without_confirmation() {
  let fixture = Fixture::new();
  fixture.repo.create_commit("Add f", "f.txt", "a\nz\n");
  let source_head = git(&fixture.repo, &["rev-parse", "HEAD"]);
  let path = path_of(&fixture.get(&[]));
  let target = commit_as_other_author(&fixture.repo, "(lane) X", &[("target.txt", "target\n")]);
  let first = commit_content(&path, "fixup! (lane) X", "folded-one.txt", "first\n");
  let second = commit_content(&path, "fixup! (lane) X", "folded-two.txt", "second\n");

  let (pick_code, picked) = fixture.run(&["--prefix", PREFIX, "pick", "--from", &path]);
  assert_eq!(pick_code, 0, "{picked}");
  let (fold_code, folded) = fixture.run(&["--prefix", PREFIX, "fold"]);
  assert_eq!(fold_code, 0, "{folded}");
  assert!(target.starts_with(folded["folded"][0]["commit"].as_str().unwrap()), "{folded}");
  let folded_fixups = folded["folded"][0]["fixups"].as_array().unwrap();
  assert_eq!(folded_fixups.len(), 2, "{folded}");
  assert_eq!(std::fs::read_to_string(fixture.repo.path().join("folded-one.txt")).unwrap(), "first\n");
  assert_eq!(std::fs::read_to_string(fixture.repo.path().join("folded-two.txt")).unwrap(), "second\n");
  assert_eq!(git(&fixture.repo, &["show", "HEAD:folded-one.txt"]), "first");
  assert_eq!(git(&fixture.repo, &["show", "HEAD:folded-two.txt"]), "second");
  assert_eq!(git(&fixture.repo, &["rev-parse", "HEAD~1"]), source_head);
  let folded_head = git(&fixture.repo, &["rev-parse", "HEAD"]);
  let pairs = [(vec![first.as_str()], folded_head.as_str()), (vec![second.as_str()], folded_head.as_str())];
  let interdiffs = git_ops::landed_pairing::interdiffs(fixture.repo.git_executor(), &path, &pairs).unwrap();
  assert!(interdiffs.iter().all(|interdiff| interdiff.local_only.is_empty()), "{interdiffs:?}");

  let twin = commit_content(&path, "(lane) Add b to f", "f.txt", "a\nb\nz\n");
  fixture.repo.create_commit("(peer) Add m to f", "f.txt", "a\nm\nz\n");
  std::fs::write(fixture.repo.path().join("f.txt"), "a\nb\nm\nz\n").unwrap();
  git(&fixture.repo, &["add", "f.txt"]);
  git(&fixture.repo, &["commit", "-q", "-C", &twin, "--date=2001-02-03T04:05:06Z"]);
  let newest_target = git(&fixture.repo, &["log", "--format=%H", "-1", "--grep=X", "master"]);
  assert_eq!(newest_target, folded_head);

  let returned = fixture.give_back(&path);
  let landed = proofs(&returned["landed"]);
  assert_eq!(landed.len(), 3, "{returned}");
  for fixup in [&first, &second] {
    assert_eq!(
      landed.iter().find(|(commit, _)| commit == fixup).map(|(_, proof)| proof.as_str()),
      Some("folded_fixup"),
      "{returned}"
    );
  }
  assert_eq!(
    landed.iter().find(|(commit, _)| commit == &twin).map(|(_, proof)| proof.as_str()),
    Some("subject_twin"),
    "{returned}"
  );
  assert_eq!(returned.get("given_up"), None, "{returned}");
}

#[test_log::test]
fn a_folded_fixup_stays_unlanded_when_its_target_lacks_a_changed_line() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  commit_as_other_author(&fixture.repo, "(lane) X", &[("folded-one.txt", "first\n")]);
  std::fs::write(Path::new(&path).join("folded-one.txt"), "first\n").unwrap();
  std::fs::write(Path::new(&path).join("folded-two.txt"), "second\n").unwrap();
  slot_git(&path, &["add", "folded-one.txt", "folded-two.txt"]);
  slot_git(&path, &["commit", "-q", "-m", "fixup! (lane) X"]);
  let fixup = slot_git(&path, &["rev-parse", "HEAD"]);

  let (exit_code, output) = fixture.run_in(Path::new(&path), &["lease", "return"]);
  assert_error(&output, exit_code, (4, "unlanded_commits"));
  assert_eq!(output["error"]["commits"], json!([fixup]), "{output}");
}

#[test]
fn a_folded_fixup_proof_is_named_in_the_return_output() {
  let output = crate::lease_command::landed_output(&[LandedCommit {
    id: "fixup-id".to_string(),
    subject: "fixup! (lane) X".to_string(),
    proof: Proof::FoldedFixup { target: "target-id".to_string() },
  }]);
  assert_eq!(serde_json::to_value(output).unwrap()[0]["proof"], json!("folded_fixup"));
}

/// A commit of the slot removes a line that master rewrote meanwhile, and the robot's resolution removed the rewritten
/// one: its twin lacks the removal of the old line, which the twin's parent no longer had, so nothing is lost. A commit
/// whose twin lacks the removal of a line the parent of the twin still holds is unlanded
#[test_log::test]
fn a_return_finds_a_commit_whose_twin_lacks_only_a_removal_master_made_before() {
  let fixture = Fixture::new();
  fixture.repo.create_commit_with_files("Add f and h", &[("f.txt", "a\nx\nz\n"), ("h.txt", "dup\nk\ndup\n")]);
  let path = path_of(&fixture.get(&[]));
  std::fs::write(Path::new(&path).join("g.txt"), "g\n").unwrap();
  slot_git(&path, &["add", "g.txt"]);
  let commit = commit_content(&path, "(lane) Drop x, add g", "f.txt", "a\nz\n");
  let kept = commit_content(&path, "(lane) Drop a dup", "h.txt", "k\ndup\n");
  fixture.repo.create_commit("(peer) Rewrite x", "f.txt", "a\ny\nz\n");
  // the resolution: the rewritten line goes, g comes
  std::fs::write(fixture.repo.path().join("f.txt"), "a\nz\n").unwrap();
  std::fs::write(fixture.repo.path().join("g.txt"), "g\n").unwrap();
  git(&fixture.repo, &["add", "f.txt", "g.txt"]);
  git(&fixture.repo, &["commit", "-q", "-C", &commit]);
  // a later change where the line was, so the pick of the commit onto master conflicts and proves nothing
  fixture.repo.create_commit("(peer) Add w", "f.txt", "a\nw\nz\n");
  // a twin of the second without its removal, while its parent still holds the line
  std::fs::write(fixture.repo.path().join("other.txt"), "other\n").unwrap();
  git(&fixture.repo, &["add", "other.txt"]);
  git(&fixture.repo, &["commit", "-q", "-C", &kept]);

  let (exit_code, output) = fixture.run_in(Path::new(&path), &["lease", "return"]);
  assert_error(&output, exit_code, (4, "unlanded_commits"));
  assert_eq!(output["error"]["commits"], json!([kept]), "{output}");
  slot_git(&path, &["reset", "-q", "--hard", "HEAD^"]);
  let returned = fixture.give_back(&path);
  assert_eq!(proofs(&returned["landed"]), vec![(commit, "twin_removed_before".to_string())], "{returned}");
}

/// A commit of the slot is on master made anew, at another author time: the same author and subject pair it, and its
/// lines decide, as for a twin. A commit of the same author and subject that lacks a line of the slot's is unlanded
#[test_log::test]
fn a_return_finds_a_commit_made_anew_by_its_author_and_subject() {
  let fixture = Fixture::new();
  fixture.repo.create_commit_with_files("Add f and h", &[("f.txt", "a\nz\n"), ("h.txt", "q\n")]);
  let path = path_of(&fixture.get(&[]));
  let commit = commit_content(&path, "(lane) Add b to f", "f.txt", "a\nb\nz\n");
  let other = commit_content(&path, "(lane) Add r to h", "h.txt", "q\nr\n");
  fixture.repo.create_commit("(peer) Add m to f", "f.txt", "a\nm\nz\n");
  let anew = |file: &str, content: &str, message_of: &str| {
    std::fs::write(fixture.repo.path().join(file), content).unwrap();
    git(&fixture.repo, &["add", file]);
    git(&fixture.repo, &["commit", "-q", "-C", message_of, "--date=2001-02-03T04:05:06Z"]);
  };
  anew("f.txt", "a\nb\nm\nz\n", &commit);
  // the same subject without the line of the slot's commit
  anew("h.txt", "q\ns\n", &other);

  let (exit_code, output) = fixture.run_in(Path::new(&path), &["lease", "return"]);
  assert_error(&output, exit_code, (4, "unlanded_commits"));
  assert_eq!(output["error"]["commits"], json!([other]), "{output}");
  slot_git(&path, &["reset", "-q", "--hard", "HEAD^"]);
  let returned = fixture.give_back(&path);
  assert_eq!(proofs(&returned["landed"]), vec![(commit, "subject_twin".to_string())], "{returned}");
}

/// A commit of the slot landed by its virtual copy: the sync resolved it by words into another change (its mapping
/// note, `v-commit-v1`), and the robot cherry-picked that copy onto master. Nothing of the commit itself is on master,
/// so only the note proves it
#[test_log::test]
fn a_return_finds_a_commit_that_landed_by_its_virtual_copy() {
  let fixture = Fixture::new();
  let got = fixture.get(&[]);
  let path = path_of(&got);
  let commit = commit_content(&path, "(lane) Add n", "n.txt", "n\n");
  let repo = &fixture.repo;
  git(repo, &["switch", "-q", "-c", "lane/virtual/lane", got["source_head"].as_str().unwrap()]);
  std::fs::write(repo.path().join("n.txt"), "n by words\n").unwrap();
  git(repo, &["add", "n.txt"]);
  git(repo, &["commit", "-q", "-m", "Add n", "--date=2001-02-03T04:05:06Z"]);
  let copy = repo.head();
  git(repo, &["switch", "-q", "master"]);
  git(repo, &["branch", "-q", "-D", "lane/virtual/lane"]);
  git(repo, &["cherry-pick", &copy]);
  git(repo, &["notes", "add", "-m", &format!("v-commit-v1:{copy}"), &commit]);

  let returned = fixture.give_back(&path);
  assert_eq!(proofs(&returned["landed"]), vec![(commit, "note".to_string())], "{returned}");
  assert_eq!(returned["landed"][0]["virtual_commit"], json!(copy), "{returned}");
}

/// A pick that landed the commit by a fast-forward and a reset of master that took it back by hand: the reflog of
/// master names the commit as a fork point, so a dry run of the pick finds nothing to pick, yet master lacks it. No
/// proof finds it there, and the return refuses it
#[test_log::test]
fn a_lease_whose_home_was_reset_past_its_commits_is_not_landed() {
  assert_unlanded_after_a_move_taken_back("merge: Fast-forward", "reset: moving to HEAD~1");
}

/// A fast-forward of master to the slot's commit (reflog message `moved`), then a move back (`back`): the return refuses
/// the commit, which master lacks
fn assert_unlanded_after_a_move_taken_back(moved: &str, back: &str) {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  let base = fixture.repo.head();
  let commit = commit_in(&path, "(lane) Add c", "lane-c.txt");
  fixture.repo.git(&["update-ref", "-m", moved, "refs/heads/master", &commit, &base]);
  fixture.repo.git(&["update-ref", "-m", back, "refs/heads/master", &base, &commit]);

  let (exit_code, output) = fixture.run_in(Path::new(&path), &["lease", "return"]);
  assert_error(&output, exit_code, (4, "unlanded_commits"));
  assert_eq!(output["error"]["commits"], json!([commit]));
}

/// A commit that master lacks is refused, and the refusal names only it: not the followed, landed commits that the
/// rewrite left unreachable
#[test_log::test]
fn a_return_refuses_only_the_commit_that_the_source_branch_lacks() {
  let (fixture, path) = followed_then_rewritten();
  let unlanded = commit_in(&path, "(lane) Add c", "lane-c.txt");
  let (exit_code, output) = fixture.run_in(Path::new(&path), &["lease", "return"]);
  assert_error(&output, exit_code, (4, "unlanded_commits"));
  assert_eq!(output["error"]["commits"], json!([unlanded]));
  assert!(output["error"]["message"].as_str().unwrap().contains("master lacks by content"), "{output}");
  // a source without a branch: reachability decides, and says why
  let mut state: Value = serde_json::from_slice(&std::fs::read(fixture.state_file()).unwrap()).unwrap();
  state["slots"][0]["lease"].as_object_mut().unwrap().remove("source_branch");
  std::fs::write(fixture.state_file(), state.to_string()).unwrap();
  let (exit_code, output) = fixture.run_in(Path::new(&path), &["lease", "return"]);
  assert_error(&output, exit_code, (4, "unlanded_commits"));
  assert!(output["error"]["commits"].as_array().unwrap().len() > 1, "{output}");
  assert!(output["error"]["message"].as_str().unwrap().contains("detached HEAD"), "{output}");
}

/// A coordinator returns the lease of another session by its id from the shared checkout, without `--workspace`
#[test_log::test]
fn a_return_by_lease_id_from_another_checkout_returns_the_slot_of_the_lease() {
  let fixture = Fixture::new();
  let got = fixture.get(&[]);
  let lease_id = got["lease_id"].as_str().unwrap();
  let (exit_code, output) = fixture.run(&["lease", "return", "--lease-id", lease_id]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["lease_id"], &output["repository"]), (&got["lease_id"], &got["path"]), "{output}");
  assert_eq!(states(&fixture.status())[0].1, "free");
}

/// A fast-forward pick of the slot's commit that rolled back left the commit in the reflog of master, and its rollback
/// took it back: the commit is unlanded, not a fork point that makes it landed
#[test_log::test]
fn a_return_after_a_pick_that_rolled_back_refuses_the_commit() {
  assert_unlanded_after_a_move_taken_back("branch-deck: pick", "branch-deck: pick rolled back");
}

/// A session lands a commit with `brd pick --follow`, then fixes its message on the shared branch from its own worktree
/// (`brd -C <shared checkout> reword`): the worktree needs nothing, its old tip is landed by content, a later pick
/// lands only a new commit and moves the worktree, and the return passes
#[test_log::test]
fn a_session_rewords_its_landed_commit_from_its_lease() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  commit_in(&path, "(lane) Add a with a subject that is far longer than the rule of the repository allows", "lane-a.txt");
  // a fast-forward: the shared branch is at the worktree's tip
  let (exit_code, picked) = bd_cli::fixture_test::run_in(&fixture.repo, &["pick", "--from", &path, "--follow"]);
  assert_eq!((exit_code, &picked["followed"]["state"]), (0, &json!("at_tip")), "{picked}");
  let shared = fixture.repo.path_str();
  let head = fixture.repo.head();

  let (exit_code, reworded) = fixture.run_in(
    Path::new(&path),
    &["-C", shared, "--prefix", PREFIX, "reword", &head, "--subject", "(lane) Add a", "--expect-head", &head],
  );
  assert_eq!((exit_code, &reworded["rewritten"]), (0, &json!(1)), "{reworded}");
  assert_eq!(git(&fixture.repo, &["log", "-1", "--format=%s"]), "(lane) Add a");
  assert_eq!(slot_git(&path, &["rev-parse", "HEAD"]), head, "the worktree stays where it was");

  // nothing left to pick: the old tip is the fork point
  let (exit_code, output) = bd_cli::fixture_test::run_in(&fixture.repo, &["pick", "--from", &path, "--dry-run"]);
  assert_eq!((exit_code, &output["state"]), (0, &json!("up_to_date")), "{output}");

  // a new commit lands alone, and the worktree follows onto the reworded branch
  let new = commit_in(&path, "(lane) Add b", "lane-b.txt");
  let (exit_code, output) = bd_cli::fixture_test::run_in(&fixture.repo, &["pick", "--from", &path, "--follow"]);
  assert_eq!((exit_code, &output["followed"]["state"]), (0, &json!("moved")), "{output}");
  let picked: Vec<&str> = output["picked"].as_array().unwrap().iter().map(|commit| commit["subject"].as_str().unwrap()).collect();
  assert_eq!(picked, ["(lane) Add b"], "{output}");
  assert_eq!(output.get("already_present"), None, "{output}");
  assert_eq!(slot_git(&path, &["rev-parse", "HEAD"]), fixture.repo.head());
  assert_ne!(fixture.repo.head(), new, "a copy on the shared branch");

  let returned = fixture.give_back(&path);
  assert_eq!(returned.get("given_up"), None, "{returned}");
}

/// A session drops its own commit after the pick landed it: `brd drop` in the lease refuses it as `drop_in_home` and names
/// the two steps; the drop in the shared checkout takes it and its file out there, and the next pick lands nothing,
/// moves the lease past it and lets the lease go back
#[test_log::test]
fn a_session_drops_its_landed_commit_in_the_shared_checkout_and_its_lease_follows() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  let base = fixture.repo.head();
  commit_in(&path, "(lane) Add a", "lane-a.txt");
  let (exit_code, picked) = bd_cli::fixture_test::run_in(&fixture.repo, &["pick", "--from", &path, "--follow"]);
  assert_eq!(exit_code, 0, "{picked}");
  let shared = fixture.repo.path_str();
  let head = fixture.repo.head();

  let (exit_code, refused) = fixture.run_in(Path::new(&path), &["--prefix", PREFIX, "drop", &head]);
  assert_error(&refused, exit_code, (4, "drop_in_home"));
  let steps: Vec<&str> = refused["error"]["next_steps"]
    .as_array()
    .unwrap()
    .iter()
    .map(|step| step["command"].as_str().unwrap())
    .collect();
  assert_eq!(steps.len(), 2, "{refused}");
  assert!(steps[0].ends_with(&format!("drop {head}")) && steps[1].ends_with("lease pick"), "{refused}");
  assert_eq!(fixture.repo.head(), head, "nothing changed");

  let (exit_code, dropped) = fixture.run_in(Path::new(&path), &["-C", shared, "--prefix", PREFIX, "drop", &head, "--expect-head", &head]);
  assert_eq!(exit_code, 0, "{dropped}");
  assert_eq!(fixture.repo.head(), base);
  assert!(!fixture.repo.path().join("lane-a.txt").exists());

  // nothing to land: the lease moves past the dropped commit, its file with it
  let (exit_code, output) = bd_cli::fixture_test::run_in(&fixture.repo, &["pick", "--from", &path, "--follow"]);
  assert_eq!((exit_code, &output["state"]), (0, &json!("up_to_date")), "{output}");
  assert_eq!(slot_git(&path, &["rev-parse", "HEAD"]), base);
  assert!(!Path::new(&path).join("lane-a.txt").exists());

  let returned = fixture.give_back(&path);
  assert_eq!(returned.get("given_up"), None, "{returned}");
}

/// A session rewords its own commit in the lease before the pick: `brd reword` moves the detached HEAD of the lease, which
/// stays detached, and the pick lands the new message; a commit the home branch has is `not_ahead_of_baseline`
#[test_log::test]
fn a_session_rewords_its_unlanded_commit_in_the_detached_lease() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  let landed = fixture.repo.head();
  let own = commit_in(&path, "(lane) write the date", "lane-a.txt");

  let reword = |commit: &str, subject: &str| fixture.run_in(Path::new(&path), &["--prefix", PREFIX, "reword", commit, "--subject", subject]);
  let (exit_code, reworded) = reword(&own, "(lane) setup build: write the date");
  assert_eq!((exit_code, &reworded["rewritten"]), (0, &json!(1)), "{reworded}");
  assert_eq!(slot_git(&path, &["log", "-1", "--format=%s"]), "(lane) setup build: write the date");
  assert_eq!(slot_git(&path, &["rev-parse", "--abbrev-ref", "HEAD"]), "HEAD", "the lease stays detached");
  assert_eq!(fixture.repo.head(), landed, "the home branch stays");

  let (exit_code, refused) = reword(&landed, "Something else");
  assert_error(&refused, exit_code, (2, "not_ahead_of_baseline"));

  let (exit_code, picked) = fixture.run_in(Path::new(&path), &["--prefix", PREFIX, "lease", "pick"]);
  assert_eq!(exit_code, 0, "{picked}");
  assert_eq!(git(&fixture.repo, &["log", "-1", "--format=%s"]), "(lane) setup build: write the date");
  let returned = fixture.give_back(&path);
  assert_eq!(returned.get("given_up"), None, "{returned}");
}

/// A worktree that did not follow its pick still has its own commit: after a reword of the copy on the shared branch,
/// a pick finds it present by its patch, never an empty commit, and the return passes by content
#[test_log::test]
fn a_pick_without_follow_finds_the_reworded_commit_present_by_patch_id() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  let own = commit_in(&path, "(lane) Add a with a subject that is far longer than the rule allows", "lane-a.txt");
  // a peer's commit first, so that the pick lands a copy
  fixture.repo.create_commit("(peer) Add p", "p.txt", "p\n");
  let (exit_code, picked) = bd_cli::fixture_test::run_in(&fixture.repo, &["pick", "--from", &path]);
  assert_eq!(exit_code, 0, "{picked}");
  let head = fixture.repo.head();
  let (exit_code, reworded) = bd_cli::fixture_test::run_in(&fixture.repo, &["reword", &head, "--subject", "(lane) Add a"]);
  assert_eq!(exit_code, 0, "{reworded}");

  let (exit_code, output) = bd_cli::fixture_test::run_in(&fixture.repo, &["pick", "--from", &path, "--dry-run"]);
  assert_eq!((exit_code, &output["state"]), (0, &json!("up_to_date")), "{output}");
  let present = &output["already_present"][0];
  assert_eq!(
    (&present["commit"], &present["by"], &present["on"]),
    (
      &json!(git_ops::revs::short_id(&own)),
      &json!("patch_id"),
      &json!(git_ops::revs::short_id(&fixture.repo.head()))
    ),
    "{output}"
  );
  let returned = fixture.give_back(&path);
  assert_eq!(returned.get("given_up"), None, "{returned}");
}

/// A fake Bazel server: `sh` blocked in `read` on a pipe (no child process), its argv naming `workspace` as Bazel's
/// does, its working directory `cwd`; `trap` keeps it alive on SIGTERM. Killed when dropped
#[cfg(unix)]
struct FakeServer {
  child: std::process::Child,
  _ready: tempfile::TempDir,
}

/// How long a test waits for a state of a fake server that a healthy run reaches at once: a limit no run reaches, which
/// turns a hang into a failure
#[cfg(unix)]
const SERVER_STATE: Duration = Duration::from_mins(2);

#[cfg(unix)]
impl FakeServer {
  fn start(workspace: &str, cwd: &Path, ignore_term: bool) -> Self {
    use std::os::unix::process::CommandExt;
    // the file says that `sh` runs the script: its argv shows from the exec on, its `trap` only once `sh` ran it, and a
    // SIGTERM before that ends a server meant to ignore it. Outside the slot, which it would make dirty
    let ready = tempfile::tempdir().unwrap();
    let ready_file = ready.path().join("ready");
    let script = if ignore_term {
      "trap '' TERM; : > \"$FAKE_SERVER_READY\"; read line"
    } else {
      ": > \"$FAKE_SERVER_READY\"; read line"
    };
    // a group of its own, as a Bazel server leads its own session
    let child = std::process::Command::new("sh")
      .process_group(0)
      .args([
        "-c",
        script,
        "bazel(fake)",
        "--output_base=/tmp/fake-output-base",
        &format!("--workspace_directory={workspace}"),
      ])
      .env("FAKE_SERVER_READY", &ready_file)
      .current_dir(cwd)
      .stdin(std::process::Stdio::piped())
      .spawn()
      .unwrap();
    let mut server = Self { child, _ready: ready };
    let deadline = std::time::Instant::now() + SERVER_STATE;
    let pid = server.pid();
    while !ready_file.exists()
      || !sync_utils::process_scan::ProcessScan::scan()
        .inside(Path::new(workspace))
        .iter()
        .any(|process| process.pid == pid)
    {
      assert!(server.running(), "the fake server exited before it was ready");
      assert!(std::time::Instant::now() < deadline, "the fake server never showed up");
      std::thread::sleep(Duration::from_millis(20));
    }
    server
  }

  fn pid(&self) -> u32 {
    self.child.id()
  }

  /// Whether it still runs; reaps it once it exited
  fn running(&mut self) -> bool {
    self.child.try_wait().unwrap().is_none()
  }

  /// Whether it exits: a stop names a server gone once the system tells its pid gone, a moment before its parent (the
  /// test) can reap it on macOS, so `running` right after a stop may still see it
  fn exits(&mut self) -> bool {
    let deadline = std::time::Instant::now() + SERVER_STATE;
    while self.running() {
      if std::time::Instant::now() >= deadline {
        return false;
      }
      std::thread::sleep(Duration::from_millis(10));
    }
    true
  }
}

#[cfg(unix)]
impl Drop for FakeServer {
  fn drop(&mut self) {
    let _ = self.child.kill();
    let _ = self.child.wait();
  }
}

/// The return asks the Bazel servers of the slot (its root and `community/`) to exit with SIGTERM, waits for them and
/// names them; their output bases stay
#[cfg(unix)]
#[test_log::test]
fn a_return_stops_the_bazel_servers_of_the_slot() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  let mut root = FakeServer::start(&path, Path::new(&path), false);
  let mut community = FakeServer::start(&format!("{path}/community"), Path::new(&path), false);
  let returned = fixture.give_back(&path);
  let mut stopped: Vec<(u64, String, String)> = returned["stopped_servers"]
    .as_array()
    .unwrap_or_else(|| panic!("{returned}"))
    .iter()
    .map(|server| {
      (
        server["pid"].as_u64().unwrap(),
        server["workspace"].as_str().unwrap().to_string(),
        server["output_base"].as_str().unwrap().to_string(),
      )
    })
    .collect();
  stopped.sort();
  let mut expected = vec![
    (u64::from(root.pid()), path.clone(), "/tmp/fake-output-base".to_string()),
    (u64::from(community.pid()), format!("{path}/community"), "/tmp/fake-output-base".to_string()),
  ];
  expected.sort();
  assert_eq!(stopped, expected);
  assert!(root.exits() && community.exits());
  assert_eq!(states(&fixture.status())[0].1, "free");
}

/// Another process in the slot refuses before any server is signalled, and the refusal names each process with its
/// argv and kind; a server that ignores SIGTERM keeps the slot busy
#[cfg(unix)]
#[test_log::test]
fn a_return_signals_no_other_process_and_a_server_that_does_not_exit_keeps_the_slot_busy() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  let mut server = FakeServer::start(&path, Path::new(&path), false);
  let mut other = busy_in(&path);
  let (exit_code, output) = fixture.run_in(Path::new(&path), &["lease", "return"]);
  let _ = other.kill();
  let _ = other.wait();
  assert_error(&output, exit_code, (4, "slot_busy"));
  let mut processes: Vec<(u64, String)> = output["error"]["processes"]
    .as_array()
    .unwrap_or_else(|| panic!("{output}"))
    .iter()
    .map(|process| (process["pid"].as_u64().unwrap(), process["kind"].as_str().unwrap().to_string()))
    .collect();
  processes.sort();
  let mut expected = vec![(u64::from(other.id()), "other".to_string()), (u64::from(server.pid()), "bazel_server".to_string())];
  expected.sort();
  assert_eq!(processes, expected);
  let message = output["error"]["message"].as_str().unwrap();
  assert!(
    message.contains(&format!("sleep ({}: sleep 30)", other.id())) && message.contains("stops the Bazel servers"),
    "{output}"
  );
  assert!(server.running(), "a refusal signals no server");
  drop(server);

  let mut stubborn = FakeServer::start(&path, Path::new(&path), true);
  let mut environment = fixture.environment(Path::new(&path));
  environment.server_stop_timeout = Duration::from_millis(500);
  let (exit_code, output) = run_json(&environment, &["lease", "return"]);
  assert_error(&output, exit_code, (4, "slot_busy"));
  assert_eq!(output["error"]["processes"][0]["pid"], json!(stubborn.pid()), "{output}");
  assert!(output["error"]["message"].as_str().unwrap().contains("did not exit on SIGTERM"), "{output}");
  assert!(stubborn.running());
  assert_eq!(states(&fixture.status())[0].1, "ready");
}

/// A get stops the Bazel server of a free slot, also one whose working directory is elsewhere (its argv names the
/// slot), and leases the slot; a server that does not exit skips the slot as busy
#[cfg(unix)]
#[test_log::test]
fn a_get_stops_the_bazel_server_of_a_free_slot_and_leases_it() {
  let fixture = Fixture::new();
  let first = path_of(&fixture.get(&[]));
  fixture.give_back(&first);
  let mut server = FakeServer::start(&first, fixture.home.path(), false);
  let second = fixture.get(&[]);
  assert_eq!((path_of(&second), &second["reused"]), (first.clone(), &json!(true)), "{second}");
  assert_eq!(
    (&second["stopped_servers"][0]["pid"], &second["stopped_servers"][0]["workspace"]),
    (&json!(server.pid()), &json!(first)),
    "{second}"
  );
  assert!(server.exits());
  fixture.give_back(&first);

  let stubborn = FakeServer::start(&first, fixture.home.path(), true);
  let mut environment = fixture.environment(fixture.repo.path());
  environment.server_stop_timeout = Duration::from_millis(500);
  let (exit_code, third) = run_json(&environment, &["lease", "get"]);
  assert_eq!(exit_code, 0, "{third}");
  assert_ne!(path_of(&third), first);
  assert_eq!(
    (
      &third["skipped"][0]["reason"],
      &third["skipped"][0]["processes"][0]["pid"],
      &third["skipped"][0]["processes"][0]["kind"]
    ),
    (&json!("busy"), &json!(stubborn.pid()), &json!("bazel_server")),
    "{third}"
  );
  assert_eq!(states(&fixture.status())[0], (first, "free".to_string()));
}

/// A process in the slot that runs in the process group of bd (the `jq` of `brd lease return | jq`, the `caffeinate` of
/// the agent) is of the session that returns the slot: it keeps no slot busy
#[cfg(unix)]
#[test_log::test]
fn a_process_of_the_group_of_bd_does_not_keep_the_slot_busy() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  let mut pipeline = std::process::Command::new("sleep").arg("30").current_dir(&path).spawn().unwrap();
  let (exit_code, output) = fixture.run_in(Path::new(&path), &["lease", "return"]);
  let _ = pipeline.kill();
  let _ = pipeline.wait();
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(states(&fixture.status())[0].1, "free");
}

/// A fake `git maintenance run --auto --detach` in `cwd`: `sh` named `git` in its argv, blocked in `read` on a pipe (no
/// child process); it ends when its stdin closes
#[cfg(unix)]
fn fake_maintenance(cwd: &str) -> std::process::Child {
  use std::os::unix::process::CommandExt;
  let child = std::process::Command::new("sh")
    .arg0("git")
    .process_group(0)
    .args(["-c", "read line", "maintenance", "run", "--auto", "--detach"])
    .current_dir(cwd)
    .stdin(std::process::Stdio::piped())
    .spawn()
    .unwrap();
  let deadline = std::time::Instant::now() + Duration::from_mins(1);
  while !sync_utils::process_scan::ProcessScan::scan().git_maintenance().contains(&child.id()) {
    assert!(std::time::Instant::now() < deadline, "the fake maintenance never showed up");
    std::thread::sleep(Duration::from_millis(20));
  }
  child
}

/// git's background maintenance in the slot (a repack after a commit, minutes in a large repository) keeps it busy for
/// neither a return, which names it, nor a get
#[cfg(unix)]
#[test_log::test]
fn git_maintenance_keeps_no_slot_busy_and_is_named() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  let mut stuck = fake_maintenance(&path);
  let returned = fixture.give_back(&path);
  let _ = stuck.kill();
  let _ = stuck.wait();
  assert_eq!(returned["maintenance"], json!([stuck.id()]), "{returned}");

  // a free slot with maintenance in it is reused, not skipped
  let mut running = fake_maintenance(&path);
  let got = fixture.get(&[]);
  let _ = running.kill();
  let _ = running.wait();
  assert_eq!((path_of(&got), &got["reused"], got.get("skipped")), (path, &json!(true), None), "{got}");
}

/// With `maintenance.auto` false (as every fixture sets it) a commit in a slot starts no maintenance; with
/// `branchdeck.leaseMaintenance` the return starts it once the slot is free, in the common dir, and without it none
#[cfg(unix)]
#[test_log::test]
fn a_return_starts_the_maintenance_that_branchdeck_lease_maintenance_asks_for() {
  let fixture = Fixture::new();
  let trace = fixture.home.path().join("trace2.json");
  let _env = test_utils::env_vars::EnvVars::set(&[("GIT_TRACE2_EVENT", &trace)]);
  let started = |gits: &[Vec<String>]| gits_of(gits, "maintenance").len();
  let path = path_of(&fixture.get(&[]));
  started_gits(&trace);
  let returned = fixture.give_back(&path);
  assert_eq!((started(&started_gits(&trace)), returned.get("maintenance_started")), (0, None), "{returned}");

  git(&fixture.repo, &["config", "branchdeck.leaseMaintenance", "true"]);
  let path = path_of(&fixture.get(&[]));
  commit_in(&path, "Add c", "c.txt");
  slot_git(&path, &["branch", "keep-c"]);
  assert_eq!(started(&started_gits(&trace)), 0, "a commit starts no maintenance");
  let returned = fixture.give_back(&path);
  let gits = started_gits(&trace);
  assert_eq!(returned["maintenance_started"], json!(true), "{returned}");
  let maintenance = gits_of(&gits, "maintenance");
  assert!(
    maintenance
      .iter()
      .any(|argv| argv.ends_with(&["run".to_string(), "--auto".to_string(), "--detach".to_string()])),
    "{gits:?}"
  );
}

/// A get refuses below the free space that `--min-free` or `branchdeck.leaseMinFree` asks, naming both and the free
/// slots, with nothing leased; a minimum past the size of the volume deletes no build output and no free slot for it,
/// as nothing could reach it, and the message says so; `--min-free 0` checks nothing; a size that does not parse is an
/// invalid argument
#[cfg(unix)]
#[test_log::test]
fn a_get_below_the_minimum_free_space_refuses_and_names_it() {
  let fixture = Fixture::new();
  let first = path_of(&fixture.get(&[]));
  fixture.give_back(&first);
  let (exit_code, output) = fixture.run(&["lease", "get", "--min-free", "1P"]);
  assert_error(&output, exit_code, (4, "low_disk_space"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(
    message.contains("1048576.0 GiB")
      && message.contains("1 free slot(s)")
      && message.contains("in all, less than the minimum, so the get deleted no build output and no free slot below `branchdeck.leaseMaxSlots` for it")
      && message.contains("never pass `--min-free 0` on your own"),
    "{output}"
  );
  assert_eq!(states(&fixture.status()), [(first, "free".to_string())]);

  git(&fixture.repo, &["config", "branchdeck.leaseMinFree", "1P"]);
  let (exit_code, output) = fixture.run(&["lease", "get"]);
  assert_error(&output, exit_code, (4, "low_disk_space"));
  let got = fixture.get(&["--min-free", "0"]);
  assert!(got["free_bytes"].as_u64().unwrap() > 0, "{got}");
  fixture.give_back(&path_of(&got));

  git(&fixture.repo, &["config", "branchdeck.leaseMinFree", "lots"]);
  let (exit_code, output) = fixture.run(&["lease", "get"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  assert!(output["error"]["message"].as_str().unwrap().contains("branchdeck.leaseMinFree"), "{output}");
  let (exit_code, output) = fixture.run(&["lease", "get", "--min-free", "1 week"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
}

const GIB: u64 = 1 << 30;
/// The size of the volume of a `ShrinkFixture`, which holds its minimum of 100 GiB
const VOLUME_BYTES: u64 = 1024 * GIB;
/// The bytes of the cargo unit each free slot of a `ShrinkFixture` keeps
const UNIT_BYTES: usize = 64 * 1024;

/// Three free slots of the pool, returned in order (the first longest ago), each with a cargo unit of its last build in
/// `target/`, which the committed rule `cargo target` keeps (one of the 8 newest), and a volume of the test's own
/// (`Volume`), against a minimum of 100 GiB
struct ShrinkFixture {
  fixture: Fixture,
  /// The slots, the first returned first
  slots: [String; 3],
}

impl ShrinkFixture {
  fn new() -> Self {
    let fixture = Fixture::new();
    fixture.repo.create_commit_with_files(
      "Add the clean rules",
      &[
        (".gitignore", "/target/\n"),
        (".branchdeck.toml", "[lease]\nclean = [{ cargo = \"target\", keep = 8, age = \"1h\" }]\n"),
      ],
    );
    let slots: Vec<String> = (1..=3).map(|n| path_of(&fixture.get(&["--holder", &format!("lane-{n}")]))).collect();
    for slot in &slots {
      fixture.give_back(slot);
      let unit = Self::unit(slot);
      std::fs::create_dir_all(unit.parent().unwrap()).unwrap();
      std::fs::write(Path::new(slot).join("target/debug/.cargo-lock"), "").unwrap();
      std::fs::write(unit, vec![b'x'; UNIT_BYTES]).unwrap();
    }
    Self {
      fixture,
      slots: slots.try_into().unwrap(),
    }
  }

  /// The file of the cargo unit of `slot`
  fn unit(slot: &str) -> PathBuf {
    Path::new(slot).join("target/debug/build/git-ops/0123456789abcdef/fingerprint/dep")
  }

  /// `brd lease get` on `volume`, each step recorded in `steps`
  fn get(&self, volume: Volume, steps: &std::sync::Mutex<Vec<GetStep>>) -> anyhow::Result<lease_pool::lease::LeaseGot> {
    self.get_reporting(volume, &|step| steps.lock().unwrap().push(step))
  }

  /// `brd lease get` on `volume`, each step told to `progress`
  fn get_reporting(&self, volume: Volume, progress: &(dyn Fn(GetStep) + Sync)) -> anyhow::Result<lease_pool::lease::LeaseGot> {
    // what the shrink or the eviction moved into the trash still takes its space, as on a volume, until it is deleted
    let trash = self.fixture.repo.path().join(".git/branch-deck/trash");
    let space = |_: &Path| {
      let gone = |path: fn(&str) -> PathBuf, in_trash: &str| (self.slots.iter().filter(|slot| !path(slot).exists()).count() as u64).saturating_sub(files_named(&trash, in_trash));
      Some(lease_pool::free_space::VolumeSpace {
        available: (volume.base + volume.per_unit * gone(Self::unit, "dep") + volume.per_slot * gone(|slot| PathBuf::from(slot), ".branchdeck.toml")) * GIB,
        total: VOLUME_BYTES,
      })
    };
    self.get_on(&space, progress)
  }

  /// `brd lease get` on the volume `space` tells, each step told to `progress`
  fn get_on(&self, space: &lease_pool::free_space::SpaceOf<'_>, progress: &(dyn Fn(GetStep) + Sync)) -> anyhow::Result<lease_pool::lease::LeaseGot> {
    let environment = self.fixture.environment(self.fixture.repo.path());
    let context = LeaseContext::resolve(&environment, None).map_err(|error| anyhow::anyhow!("{}", error.message))?;
    lease_pool::lease::lease_get_core(
      &context.git,
      &lease_pool::lease::LeaseGetParams {
        source: &context.dirs,
        home: environment.home.as_deref(),
        holder: Some("low".to_string()),
        branch: None,
        min_free: Some(100 * GIB),
        server_timeout: context.server_stop_timeout,
        bazel: None,
        lock_timeout: context.lock_timeout,
        progress: Some(progress),
        space: Some(space),
      },
    )
  }

  /// A get on `volume` that refuses as `low_disk_space`, each step recorded in `steps`
  fn refused(&self, volume: Volume, steps: &std::sync::Mutex<Vec<GetStep>>) -> Refused {
    let error = self.get(volume, steps).unwrap_err();
    let refusal = error.downcast_ref::<lease_pool::lease::LeaseRefusal>().unwrap_or_else(|| panic!("{error:#}"));
    let lease_pool::lease::LeaseRefusal::LowDiskSpace { available, freed, .. } = refusal else {
      panic!("{refusal}");
    };
    Refused {
      available: *available,
      freed: **freed,
      message: refusal.to_string(),
    }
  }

  /// The output of `got` as brd prints it, checked against `brd schema`
  fn printed(&self, got: &lease_pool::lease::LeaseGot) -> Value {
    let environment = self.fixture.environment(self.fixture.repo.path());
    let context = LeaseContext::resolve(&environment, None).unwrap();
    printed(&self.fixture, &lease_get_output(&context, got).unwrap())
  }

  /// Which of the slots still have their unit
  fn units(&self) -> Vec<bool> {
    self.slots.iter().map(|slot| Self::unit(slot).exists()).collect()
  }
}

/// 99 GiB, 111 once one slot lost its unit: past the 110 of the minimum and its margin
const ONE_SLOT_LIFTS: Volume = Volume {
  base: 99,
  per_unit: 12,
  per_slot: 0,
};

/// What a `ShrinkFixture` get that refused saw and freed
struct Refused {
  available: u64,
  freed: lease_pool::lease::FreedBy,
  message: String,
}

/// The free space a `ShrinkFixture` get sees, in GiB: `base`, `per_unit` more for each slot whose unit is gone and
/// `per_slot` more for each slot that is gone itself
#[derive(Clone, Copy)]
struct Volume {
  base: u64,
  per_unit: u64,
  per_slot: u64,
}

/// The paths of the slots a shrink cleaned, in its order
fn shrunk_paths(got: &lease_pool::lease::LeaseGot) -> Vec<String> {
  got
    .shrunk
    .as_ref()
    .map_or_else(Vec::new, |shrunk| shrunk.slots.iter().map(|slot| slot.path.clone()).collect())
}

/// Below the minimum free space, once the trim deleted nothing (the pool is within `branchdeck.leaseMaxSlots`), a get
/// deletes the build output of the free slots by their clean rules with keep 0, the one returned longest ago first,
/// measures after each and stops once the volume has the minimum and a tenth of it: the slot returned last keeps its
/// unit, and the get leases it, warm. The worktrees stay, free
#[test_log::test]
fn a_get_below_the_minimum_free_space_deletes_the_build_output_of_the_free_slots_returned_longest_ago_first() {
  let shrink = ShrinkFixture::new();
  let [first, second, third] = &shrink.slots;
  let steps = std::sync::Mutex::new(Vec::new());
  // 99 GiB, 105 after the first slot, 111 after the second: past the 110 of the minimum and its margin
  let got = shrink
    .get(
      Volume {
        base: 99,
        per_unit: 6,
        per_slot: 0,
      },
      &steps,
    )
    .unwrap();
  assert_eq!(shrunk_paths(&got), [first.clone(), second.clone()]);
  assert_eq!(shrink.units(), [false, false, true]);
  let shrunk = got.shrunk.as_ref().unwrap();
  assert!(shrunk.slots.iter().all(|slot| slot.freed_bytes() >= UNIT_BYTES as u64), "{shrunk:?}");
  assert!(got.trimmed.as_ref().is_some_and(lease_pool::trim::Trimmed::is_empty), "{:?}", got.trimmed);
  assert_eq!((&got.slot.path, got.free_bytes), (third, Some(111 * GIB)));
  assert_eq!(kinds(&steps)[..3], ["trim", "shrink", "reserve"]);
  assert_eq!(
    states(&shrink.fixture.status()),
    [
      (first.clone(), "free".to_string()),
      (second.clone(), "free".to_string()),
      (third.clone(), "ready".to_string())
    ]
  );
  // what the rules moved into the trash is deleted
  let trash = shrink.fixture.repo.path().join(".git/branch-deck/trash");
  assert_eq!(std::fs::read_dir(&trash).map_or(0, Iterator::count), 0);
}

/// A free slot with a process in it stays as it is (`busy`, with the process), and so does one without clean rules: the
/// shrink goes on to the next slot, here the one the get takes, and the answer names both, as brd prints it
#[cfg(unix)]
#[test_log::test]
fn a_shrink_passes_over_a_free_slot_with_a_process_in_it_and_one_without_rules() {
  let shrink = ShrinkFixture::new();
  let [first, second, third] = &shrink.slots;
  let _busy = Killed(busy_in(first));
  std::fs::remove_file(Path::new(second).join(".branchdeck.toml")).unwrap();
  let steps = std::sync::Mutex::new(Vec::new());
  let got = shrink.get(ONE_SLOT_LIFTS, &steps).unwrap();
  assert_eq!(shrunk_paths(&got), std::slice::from_ref(third));
  assert_eq!(shrink.units(), [true, true, false]);
  assert_eq!(got.slot.path, *third);
  let output = shrink.printed(&got);
  let shrunk = &output["shrunk"];
  assert_eq!(shrunk["busy"].as_array().map(Vec::len), Some(1), "{output}");
  assert_eq!(shrunk["busy"][0]["path"], json!(first), "{output}");
  assert_eq!(
    shrunk["busy"][0]["processes"][0]["command"].as_str().map(|command| command.contains("sleep")),
    Some(true),
    "{output}"
  );
  assert_eq!((&shrunk["slots"][0]["path"], &shrunk["processes_checked"]), (&json!(third), &json!(true)), "{output}");
  assert!(shrunk["freed_bytes"].as_u64().is_some_and(|freed| freed >= UNIT_BYTES as u64), "{output}");
}

/// The files below `dir` whose name `counted` admits, none when it is gone
fn files_below_matching(dir: &Path, counted: &dyn Fn(&std::ffi::OsStr) -> bool) -> u64 {
  std::fs::read_dir(dir).map_or(0, |entries| {
    entries
      .flatten()
      .map(|entry| {
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
          files_below_matching(&entry.path(), counted)
        } else {
          u64::from(counted(&entry.file_name()))
        }
      })
      .sum()
  })
}

/// The files named `name` below `dir`
fn files_named(dir: &Path, name: &str) -> u64 {
  files_below_matching(dir, &|file| file == name)
}

/// The files below `dir`
fn files_below(dir: &Path) -> u64 {
  files_below_matching(dir, &|_| true)
}

/// The delete of the shrink stops once the volume has the minimum and its margin, inside the build output of a slot of
/// many files (a slot of idea-1 is 1.4 million, minutes to delete): the rest waits in the trash for the next get below
/// the minimum (`left-…`), no other slot is cleaned, and the get leases at once
#[test_log::test]
fn a_shrink_stops_inside_the_delete_of_a_slot_once_the_volume_has_the_target() {
  let shrink = ShrinkFixture::new();
  let [first, second, third] = &shrink.slots;
  // the unit of the first slot, 1,000 files more
  let unit_dir = ShrinkFixture::unit(first).parent().unwrap().to_path_buf();
  for file in 0..1000 {
    std::fs::write(unit_dir.join(format!("f{file}")), "x").unwrap();
  }
  let output = files_below(&unit_dir);
  let trash = shrink.fixture.repo.path().join(".git/branch-deck/trash");
  // 99 GiB, and 1 GiB more for each 30 files of it deleted: the 110 of the minimum and its margin once 330 are gone
  let space = |_: &Path| {
    let deleted = output - files_below(&unit_dir) - files_below(&trash);
    Some(lease_pool::free_space::VolumeSpace {
      available: 99 * GIB + deleted * GIB / 30,
      total: VOLUME_BYTES,
    })
  };
  let got = shrink.get_on(&space, &|_| {}).unwrap();
  assert_eq!(shrunk_paths(&got), std::slice::from_ref(first));
  assert_eq!(shrink.units(), [false, true, true]);
  assert_eq!(got.slot.path, *third);
  assert!(got.free_bytes.is_some_and(|free| (110 * GIB..117 * GIB).contains(&free)), "{:?}", got.free_bytes);
  // asked once every 256 entries: stopped at the first ask past the 330 files, the rest left to a sweep
  let left: Vec<String> = std::fs::read_dir(&trash)
    .unwrap()
    .flatten()
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .collect();
  assert!(left.len() == 1 && left[0].starts_with("left-"), "{left:?}");
  let kept = files_below(&trash);
  assert!((output - 512..=output - 330).contains(&kept), "{kept} of {output}");
  assert!(ShrinkFixture::unit(second).exists());
}

/// A shrink stops at the first slot past the minimum and its margin: the others keep their build output
#[test_log::test]
fn a_shrink_that_one_slot_lifts_past_the_minimum_cleans_only_that_slot() {
  let shrink = ShrinkFixture::new();
  let steps = std::sync::Mutex::new(Vec::new());
  let got = shrink.get(ONE_SLOT_LIFTS, &steps).unwrap();
  assert_eq!(shrunk_paths(&got), std::slice::from_ref(&shrink.slots[0]));
  assert_eq!(shrink.units(), [false, true, true]);
  assert_eq!(got.slot.path, shrink.slots[2]);
}

/// When the test gives back the turn of the shrink it holds (`behind_held_turn`)
#[derive(Clone, Copy, PartialEq, Eq)]
enum Release {
  /// Once the holder deleted what it deletes, before the get answered
  BeforeAnswer,
  /// Only once the get answered: a get that waits for the turn never answers
  AfterAnswer,
}

/// A get of `shrink` on `volume` behind the turn of the shrink of the machine (`lock_pressure`), which the test holds:
/// once the get tells its wait, `holder` deletes what the shrink of the get whose turn it is would, and the turn is given
/// back as `release` says
fn behind_held_turn(shrink: &ShrinkFixture, volume: Volume, release: Release, holder: impl FnOnce()) -> lease_pool::lease::LeaseGot {
  let environment = shrink.fixture.environment(shrink.fixture.repo.path());
  let pool = lease_pool::lease::Pool::new(&shrink.fixture.repo.path().join(".git"));
  let mut held = Some(lease_pool::shrink::lock_pressure(environment.home.as_deref(), &pool, Duration::from_secs(10), &mut |_| {}, &|| false).unwrap());
  let (waiting, waits) = std::sync::mpsc::channel();
  let got = std::thread::scope(|scope| {
    let get = scope.spawn(move || {
      // dropped with the get: a get that never waits ends the test's wait with an error
      let waiting = std::sync::Mutex::new(waiting);
      shrink.get_reporting(volume, &|step| {
        if let GetStep::WaitForPressure { holder, .. } = step {
          waiting.lock().unwrap().send(holder.pid).unwrap();
        }
      })
    });
    assert_eq!(waits.recv().unwrap(), std::process::id());
    holder();
    if release == Release::BeforeAnswer {
      held = None;
    }
    get.join().unwrap().unwrap()
  });
  drop(held);
  got
}

/// The shrink and the eviction of the gets of the machine take turns (`lock_pressure`): a get below the minimum that
/// waits for the shrink of another tells the wait, measures once the volume has the minimum (in its wait, or after its
/// turn), finds the target that one left, and cleans nothing
#[test_log::test]
fn a_get_that_waited_for_the_shrink_of_another_cleans_nothing_once_the_volume_has_the_target() {
  let shrink = ShrinkFixture::new();
  let volume = Volume {
    base: 88,
    per_unit: 11,
    per_slot: 0,
  };
  let got = behind_held_turn(&shrink, volume, Release::BeforeAnswer, || {
    // what the shrink of the get that holds the turn deletes: 99 GiB after the first slot, below the minimum of 100,
    // then 110, the minimum and its margin; the get measures once it has the minimum, in its wait or after its turn
    for slot in &shrink.slots[..2] {
      std::fs::remove_file(ShrinkFixture::unit(slot)).unwrap();
    }
  });
  assert_eq!(shrunk_paths(&got), Vec::<String>::new());
  assert_eq!(shrink.units(), [false, false, true]);
  assert_eq!((&got.slot.path, got.free_bytes), (&shrink.slots[2], Some(110 * GIB)));
}

/// A get below the minimum that waits for the shrink of another stops waiting once the volume has the minimum (the
/// holder frees it slot by slot, and a slot of idea-1 takes minutes to delete): it cleans nothing and leases while the
/// other still holds the lock, which the test gives back only after the get answered
#[test_log::test]
fn a_get_that_waits_for_the_shrink_of_another_goes_on_once_the_volume_has_the_minimum_before_the_turn() {
  let shrink = ShrinkFixture::new();
  let volume = Volume {
    base: 99,
    per_unit: 6,
    per_slot: 0,
  };
  let got = behind_held_turn(&shrink, volume, Release::AfterAnswer, || {
    // the first slot the holder deletes: 105 GiB, the minimum of 100, short of the 110 of its margin
    std::fs::remove_file(ShrinkFixture::unit(&shrink.slots[0])).unwrap();
  });
  assert_eq!(shrunk_paths(&got), Vec::<String>::new());
  assert_eq!(shrink.units(), [false, true, true]);
  assert_eq!((&got.slot.path, got.free_bytes), (&shrink.slots[2], Some(105 * GIB)));
}

/// The paths of the slots the eviction of a get deleted, in its order
fn evicted_paths(got: &lease_pool::lease::LeaseGot) -> Vec<String> {
  got.evicted.as_ref().map_or_else(Vec::new, |evicted| {
    evicted.pools.iter().flat_map(|pool| &pool.removed).map(|slot| slot.path.clone()).collect()
  })
}

/// A get that the shrink leaves below the minimum deletes free slots below `branchdeck.leaseMaxSlots` (3 of 10 here),
/// the one returned longest ago first, one at a time, until the volume has the minimum, and leases the slot that is
/// left
#[test_log::test]
fn a_get_still_below_the_minimum_after_the_shrink_deletes_free_slots_below_the_limit_returned_longest_ago_first() {
  let shrink = ShrinkFixture::new();
  let [first, second, third] = &shrink.slots;
  let steps = std::sync::Mutex::new(Vec::new());
  // 53 GiB after the shrink, 83 once the first slot is gone, 113 once the second is
  let got = shrink
    .get(
      Volume {
        base: 50,
        per_unit: 1,
        per_slot: 30,
      },
      &steps,
    )
    .unwrap();
  assert_eq!(shrunk_paths(&got), shrink.slots);
  assert_eq!(evicted_paths(&got), [first.clone(), second.clone()]);
  let evicted = &got.evicted.as_ref().unwrap().pools[0];
  assert_eq!((evicted.max_slots, evicted.slots), (10, 3));
  assert!(evicted.removed.iter().all(|slot| slot.freed_bytes() > 0), "{evicted:?}");
  for slot in [first, second] {
    assert!(!Path::new(slot).parent().unwrap().exists(), "{slot}");
  }
  assert_eq!((&got.slot.path, got.free_bytes), (third, Some(113 * GIB)));
  assert_eq!(kinds(&steps)[..4], ["trim", "shrink", "evict", "reserve"]);
  assert_eq!(states(&shrink.fixture.status()), [(third.clone(), "ready".to_string())]);
  // as brd prints it: the shrink of the three slots, and the eviction as `brd lease trim` prints its slots
  let output = shrink.printed(&got);
  assert_eq!(output["shrunk"]["slots"].as_array().map(Vec::len), Some(3), "{output}");
  let removed: Vec<&Value> = output["evicted"]["pools"][0]["removed"].as_array().unwrap().iter().map(|slot| &slot["path"]).collect();
  assert_eq!(removed, [&json!(first), &json!(second)], "{output}");
}

/// The eviction stops at the minimum itself, not at the margin of the shrink: a slot is a whole worktree, and the one
/// that lifts the volume past the minimum is the last to go
#[test_log::test]
fn the_eviction_stops_at_the_minimum_without_the_margin_of_the_shrink() {
  let shrink = ShrinkFixture::new();
  let [first, second, third] = &shrink.slots;
  let steps = std::sync::Mutex::new(Vec::new());
  // 53 GiB after the shrink, 101 once the first slot is gone: past the minimum of 100, below the 110 of its margin
  let got = shrink
    .get(
      Volume {
        base: 50,
        per_unit: 1,
        per_slot: 48,
      },
      &steps,
    )
    .unwrap();
  assert_eq!(evicted_paths(&got), std::slice::from_ref(first));
  assert_eq!((&got.slot.path, got.free_bytes), (third, Some(101 * GIB)));
  assert_eq!(
    states(&shrink.fixture.status()),
    [(second.clone(), "free".to_string()), (third.clone(), "ready".to_string())]
  );
}

/// A get that the trim, the shrink and the eviction leave below the minimum refuses, with nothing leased, and names
/// what it freed: the slots whose build output it deleted, the slot of its next get last, and the slots it deleted,
/// every free slot but that one, which it would reuse rather than clone the same worktree again
#[test_log::test]
fn a_get_still_below_the_minimum_after_the_shrink_and_the_eviction_refuses_and_names_what_it_freed() {
  let shrink = ShrinkFixture::new();
  let steps = std::sync::Mutex::new(Vec::new());
  let Refused { available, freed, message } = shrink.refused(
    Volume {
      base: 50,
      per_unit: 1,
      per_slot: 0,
    },
    &steps,
  );
  assert_eq!((available, freed.trim.slots, freed.shrink.slots, freed.evict.slots), (53 * GIB, 0, 3, 2), "{message}");
  assert!(freed.shrink.bytes >= 3 * UNIT_BYTES as u64 && freed.evict.bytes > 0, "{message}");
  assert!(
    message.contains("the get deleted the build output of 3 free slot(s) of the pools of this machine")
      && message.contains("then it deleted 2 free slot(s) below `branchdeck.leaseMaxSlots`"),
    "{message}"
  );
  assert_eq!(kinds(&steps), ["trim", "shrink", "evict"]);
  assert_eq!(states(&shrink.fixture.status()), [(shrink.slots[2].clone(), "free".to_string())]);
}

/// The eviction keeps a free slot marked dirty, whose changes made without a lease may be their only copy, and one
/// never returned, such as a worktree `brd lease adopt` kept where it was: with the get's next slot kept too, it deletes
/// nothing here, and the get refuses
#[test_log::test]
fn the_eviction_keeps_dirty_and_never_returned_free_slots() {
  let shrink = ShrinkFixture::new();
  let mut state: Value = serde_json::from_str(&std::fs::read_to_string(shrink.fixture.state_file()).unwrap()).unwrap();
  let slots = state["slots"].as_array_mut().unwrap();
  slots[0]["dirty"] = json!(true);
  let adopted = slots[1].as_object_mut().unwrap();
  adopted.remove("returned");
  adopted.remove("returned_at");
  std::fs::write(shrink.fixture.state_file(), state.to_string()).unwrap();
  let steps = std::sync::Mutex::new(Vec::new());
  let Refused { freed, message, .. } = shrink.refused(
    Volume {
      base: 50,
      per_unit: 1,
      per_slot: 30,
    },
    &steps,
  );
  assert_eq!((freed.shrink.slots, freed.evict.slots), (3, 0), "{message}");
  assert_eq!(kinds(&steps), ["trim", "shrink", "evict"]);
  assert_eq!(states(&shrink.fixture.status()).len(), 3);
}

/// A get records the process it ran for, the nearest ancestor that is no shell, and the status tells whether it runs;
/// once it is gone, a get that adds a new slot names the lease, and the lease stays the holder's. On Windows too, where
/// the holder comes from a snapshot of the process list
#[test_log::test]
fn the_status_tells_whether_the_holder_of_a_lease_is_alive() {
  let fixture = Fixture::new();
  let first = fixture.get(&["--holder", "gone-agent"]);
  let (path, first_id) = (path_of(&first), &first["lease_id"]);
  let lease = &fixture.status()["slots"][0]["lease"];
  assert_eq!(lease["holder_alive"], json!(true), "{lease}");
  assert!(lease["holder_pid"].as_u64().unwrap() > 1, "{lease}");

  // the holder exits: a child's pid with its start time, once it has been reaped
  let (pid, started) = gone_process();
  let mut state: Value = serde_json::from_slice(&std::fs::read(fixture.state_file()).unwrap()).unwrap();
  state["slots"][0]["lease"]["holder_pid"] = json!(pid);
  state["slots"][0]["lease"]["holder_started"] = json!(started.unwrap_or(1));
  std::fs::write(fixture.state_file(), serde_json::to_vec(&state).unwrap()).unwrap();
  assert_eq!(fixture.status()["slots"][0]["lease"]["holder_alive"], json!(false));
  // got a moment ago, it is not quiet yet: the get adds a slot and names it, unchecked
  let second = fixture.get(&[]);
  assert_eq!(
    second["idle_leases"],
    json!([{ "path": path, "holder": "gone-agent", "lease_id": first_id, "idle": { "state": "idle" } }]),
    "{second}"
  );
  assert_eq!(states(&fixture.status())[0], (path, "ready".to_string()));
  assert_eq!(fixture.status()["slots"][0]["lease"]["idle"], json!({ "state": "idle" }));
}

/// Concurrent gets reserve their slots under the pool lock: never one slot twice, whether reused or new. The two new
/// slots are registered one after the other: a `worktree add` beside another read its half written admin directory
/// ("failed to read .git/worktrees/<name>/commondir"), and a get's `worktree prune` deleted it
#[test_log::test]
fn two_leases_never_get_the_same_slot() {
  let fixture = Fixture::new();
  let free: Vec<String> = (0..2).map(|_| path_of(&fixture.get(&[]))).collect();
  for path in &free {
    fixture.give_back(path);
  }
  let paths: Vec<String> = std::thread::scope(|scope| {
    let gets: Vec<_> = (0..4).map(|_| scope.spawn(|| fixture.run(&["lease", "get"]))).collect();
    gets
      .into_iter()
      .map(|get| {
        let (exit_code, output) = get.join().unwrap();
        assert_eq!(exit_code, 0, "{output}");
        path_of(&output)
      })
      .collect()
  });
  let unique: std::collections::BTreeSet<&String> = paths.iter().collect();
  assert_eq!(unique.len(), 4, "{paths:?}");
  assert!(free.iter().all(|path| paths.contains(path)), "{paths:?} reuse {free:?}");
  assert_eq!(states(&fixture.status()).iter().filter(|(_, state)| state == "ready").count(), 4);
}

/// The environment variable that makes `a_get_held_at_its_registration_in_a_child` run: `<repository>|<home of the
/// pool>|<global git config>|<the held git>`
#[cfg(unix)]
const HELD_GET_ENV: &str = "BD_HELD_GET_CHILD";

/// The child of `no_worktree_command_of_the_pool_runs_beside_the_registration_of_a_new_slot`: a `brd lease get` of
/// another process, held at the `worktree add` of its new slot by the git `held_environment` wrote; does nothing in a
/// normal run
#[cfg(unix)]
#[test_log::test]
fn a_get_held_at_its_registration_in_a_child() {
  let Ok(spec) = std::env::var(HELD_GET_ENV) else { return };
  let [repo, home, global_config, git] = spec.split('|').collect::<Vec<_>>().try_into().unwrap();
  let mut environment = pool_environment(Path::new(repo), Path::new(home), Path::new(global_config));
  environment.git = Some(git.to_string());
  let (exit_code, output) = run_json(&environment, &["lease", "get"]);
  assert_eq!(exit_code, 0, "{output}");
}

/// The `worktree add` that registers a new slot holds the lock of the pool's `worktree` commands (`worktrees.lock`), so
/// no other `worktree` command of the pool reads its admin directory half written: the move of an adopt beside it in
/// another process waits, and one that can't wait is refused, naming the lock (in one process the adopt would wait in
/// the line of the process instead). The pool lock stays free: that get reserved its slot. Another get that adds a
/// slot waits for the clones lock before any `worktree` command (`clones_lock_test.rs`)
#[cfg(unix)]
#[test_log::test]
fn no_worktree_command_of_the_pool_runs_beside_the_registration_of_a_new_slot() {
  let fixture = Fixture::new();
  let dir = tempfile::tempdir().unwrap();
  let (lane, old, held) = lane_beside_a_held(&fixture, dir.path(), "*\" worktree add \"*");
  let spec = [
    fixture.repo.path(),
    fixture.home.path(),
    fixture.global_config.path(),
    Path::new(held.git.as_deref().unwrap()),
  ]
  .map(|path| path.to_str().unwrap())
  .join("|");
  let child = std::process::Command::new(std::env::current_exe().unwrap())
    .args(["--exact", "lease_test::a_get_held_at_its_registration_in_a_child", "--nocapture"])
    .env(HELD_GET_ENV, spec)
    .spawn()
    .unwrap();
  let mut child = Killed(child);
  while !dir.path().join("held").exists() {
    assert!(child.0.try_wait().unwrap().is_none(), "the get ended before its registration");
    std::thread::sleep(Duration::from_millis(10));
  }
  let mut impatient = fixture.environment(fixture.repo.path());
  impatient.lock_timeout = Duration::from_millis(200);
  let (exit_code, output) = run_json(&impatient, &["lease", "adopt", "--move", &lane.path]);
  // released before any assertion, so a failure never leaves the held get waiting
  std::fs::write(dir.path().join("go"), "").unwrap();
  assert_eq!((exit_code, &output["refused"][0]["code"]), (4, &json!("git_error")), "{output}");
  assert!(output["refused"][0]["message"].as_str().unwrap().contains("worktrees.lock"), "{output}");
  assert!(child.0.wait().unwrap().success(), "the held get failed");
  // the lock is free once the registration is done
  let (exit_code, output) = run_json(&impatient, &["lease", "adopt", "--move", &lane.path]);
  assert_eq!((exit_code, &output["adopted"][0]["moved_from"]), (0, &json!(old)), "{output}");
}

/// A slot deleted by hand leaves its registration behind: the next new slot takes its path and the registration over
/// (`worktree add --force`). No get runs `worktree prune`, whose run beside the `worktree add` of another get deleted
/// the admin directory that add had just made, and that get failed (`two_leases_never_get_the_same_slot`)
#[test_log::test]
fn a_slot_deleted_by_hand_gives_its_path_to_the_next_new_slot() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  fixture.give_back(&path);
  std::fs::remove_dir_all(Path::new(&path).parent().unwrap()).unwrap();

  let got = fixture.get(&[]);
  assert_eq!((path_of(&got), &got["reused"]), (path.clone(), &json!(false)), "{got}");
  let listed = git(&fixture.repo, &["worktree", "list", "--porcelain"]);
  let registered = format!("worktree {}", git_path(&path));
  assert_eq!(listed.lines().filter(|line| *line == registered).count(), 1, "{listed}");
}

/// A free slot with an untracked top-level `.gitignore` that ignores the untracked `left.txt`
#[cfg(target_os = "macos")]
fn free_slot_with_an_untracked_gitignore(fixture: &Fixture) -> String {
  let slot = path_of(&fixture.get(&[]));
  fixture.give_back(&slot);
  write_a_hidden_file(&slot);
  slot
}

/// An untracked top-level `.gitignore` in `slot` that ignores the untracked `left.txt`
fn write_a_hidden_file(slot: &str) {
  std::fs::write(Path::new(slot).join(".gitignore"), "/left.txt\n").unwrap();
  std::fs::write(Path::new(slot).join("left.txt"), "left\n").unwrap();
}

/// The slot at `path` holds its commit's tree and nothing else: no change, no untracked file, no ignored one
fn assert_pristine(path: &str) {
  assert_eq!(slot_git(path, &["status", "--porcelain", "--untracked-files=all", "--ignored"]), "", "{path}");
}

/// One `clean -fd` judges what is ignored by the ignore files as it finds them when it starts: it deleted an untracked
/// `.gitignore` and kept the file it ignored, untracked from then on, and the clone of such a template was rolled back.
/// The clone removes the untracked `.gitignore` first: the new slot is clean, and the template keeps its files
#[cfg(target_os = "macos")]
#[test_log::test]
fn a_clone_of_a_template_with_an_untracked_gitignore_is_clean() {
  let fixture = Fixture::new();
  let template = free_slot_with_an_untracked_gitignore(&fixture);
  let got = fixture.get(&[]);
  assert_eq!((&got["created_by"], &got["cloned_from"]), (&json!("clone"), &json!(template)), "{got}");
  assert_pristine(&path_of(&got));
  assert!(Path::new(&template).join(".gitignore").exists() && Path::new(&template).join("left.txt").exists());
}

/// A return with `--confirm-preserved` discards an untracked `.gitignore` and the file it ignored: the slot is clean
#[test_log::test]
fn a_confirmed_return_discards_the_file_an_untracked_gitignore_hid() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  write_a_hidden_file(&path);
  let (exit_code, returned) = fixture.run(&["lease", "return", "--workspace", &path, "--confirm-preserved"]);
  assert_eq!((exit_code, &returned["discarded"]), (0, &json!([".gitignore"])), "{returned}");
  assert_pristine(&path);
}

/// A return refuses unlanded commits, a dirty tree and a busy slot, each with nothing changed; --confirm-preserved
/// discards the changes and gives up the commits
#[test_log::test]
fn a_return_refuses_unlanded_commits_a_dirty_tree_and_a_busy_slot() {
  let fixture = Fixture::new();
  let got = fixture.get(&[]);
  let path = path_of(&got);
  std::fs::write(Path::new(&path).join("c.txt"), "c\n").unwrap();
  slot_git(&path, &["add", "c.txt"]);
  slot_git(&path, &["commit", "-q", "-m", "(lane) Add c"]);
  let commit = slot_git(&path, &["rev-parse", "HEAD"]);

  let (exit_code, output) = fixture.run_in(Path::new(&path), &["lease", "return"]);
  assert_error(&output, exit_code, (4, "unlanded_commits"));
  assert_eq!(output["error"]["commits"], json!([commit]));
  assert!(output["error"]["message"].as_str().unwrap().contains("lease pick`"), "{output}");

  // a branch holds the commit now; a local change refuses next
  slot_git(&path, &["branch", "keep", "HEAD"]);
  std::fs::write(Path::new(&path).join("a.txt"), "changed\n").unwrap();
  let (exit_code, output) = fixture.run(&["lease", "return", "--workspace", &path]);
  assert_error(&output, exit_code, (4, "dirty_worktree"));
  assert_eq!(output["error"]["paths"], json!(["a.txt"]));

  #[cfg(unix)]
  {
    let mut sleep = busy_in(&path);
    let (exit_code, output) = fixture.run(&["lease", "return", "--workspace", &path, "--confirm-preserved"]);
    sleep.kill().unwrap();
    sleep.wait().unwrap();
    assert_error(&output, exit_code, (4, "slot_busy"));
    assert!(
      output["error"]["message"].as_str().unwrap().contains(&format!("sleep ({}: sleep 30)", sleep.id())),
      "{output}"
    );
  }
  assert_eq!(states(&fixture.status())[0].1, "ready");

  // another lease id, and a worktree without a lease
  let (exit_code, output) = fixture.run(&["lease", "return", "--workspace", &path, "--lease-id", "other"]);
  assert_error(&output, exit_code, (2, "lease_mismatch"));
  let (exit_code, output) = fixture.run(&["lease", "return"]);
  assert_error(&output, exit_code, (2, "no_lease"));
  // an id no slot holds, from a checkout without a lease
  let (exit_code, output) = fixture.run(&["lease", "return", "--lease-id", "other"]);
  assert_error(&output, exit_code, (2, "no_lease"));

  slot_git(&path, &["branch", "-D", "keep"]);
  let (exit_code, output) = fixture.run(&[
    "lease",
    "return",
    "--workspace",
    &path,
    "--confirm-preserved",
    "--lease-id",
    got["lease_id"].as_str().unwrap(),
  ]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["discarded"], &output["given_up"]),
    (&json!(["a.txt"]), &json!([{ "commit": commit, "subject": "(lane) Add c" }]))
  );
  assert_eq!(std::fs::read_to_string(Path::new(&path).join("a.txt")).unwrap(), "a\n");
  assert_eq!(slot_git(&path, &["status", "--porcelain"]), "");
  assert_eq!(states(&fixture.status())[0].1, "free");
}

/// `--branch` creates the branch at the source HEAD and checks it out; the return detaches HEAD and the branch stays
#[test_log::test]
fn a_lease_on_a_new_branch_keeps_the_branch_after_the_return() {
  let fixture = Fixture::new();
  let got = fixture.get(&["-b", "feature"]);
  let path = path_of(&got);
  assert_eq!(got["branch"], json!("feature"));
  assert_eq!(slot_git(&path, &["symbolic-ref", "HEAD"]), "refs/heads/feature");
  let (exit_code, output) = fixture.run(&["lease", "get", "--branch", "feature"]);
  assert_error(&output, exit_code, (4, "branch_exists"));
  let (exit_code, output) = fixture.run(&["lease", "get", "--branch", "bad..name"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  let returned = fixture.give_back(&path);
  assert_eq!(returned["detached_branch"], json!("feature"));
  assert_eq!(git(&fixture.repo, &["rev-parse", "refs/heads/feature"]), fixture.repo.head());
  let (_, attached) = GitCommandExecutor::new().execute_command_with_status(&["symbolic-ref", "-q", "HEAD"], &path).unwrap();
  assert_ne!(attached, 0);
  // a reused slot on a new branch
  let again = fixture.get(&["-b", "second"]);
  assert_eq!((path_of(&again), &again["reused"]), (path.clone(), &json!(true)));
  assert_eq!(slot_git(&path, &["symbolic-ref", "HEAD"]), "refs/heads/second");
}

/// The ignored files `.worktreeinclude` names are copied into the slot; the return removes them. A tracked file and
/// a file git does not ignore are never seeded
#[test_log::test]
fn the_seed_copies_the_included_ignored_files_and_the_return_removes_them() {
  let fixture = Fixture::new();
  let repo = &fixture.repo;
  repo.create_commit_with_files(
    "Add the seed lists",
    &[
      (".gitignore", ".claude/settings.local.json\n"),
      (".worktreeinclude", "# local settings\n/.claude/settings.local.json\n/a.txt\n/plain.txt\n"),
    ],
  );
  std::fs::create_dir_all(repo.path().join(".claude")).unwrap();
  std::fs::write(repo.path().join(".claude/settings.local.json"), "{\"local\":true}\n").unwrap();
  std::fs::write(repo.path().join("plain.txt"), "not ignored\n").unwrap();

  let got = fixture.get(&[]);
  let path = path_of(&got);
  assert_eq!(got["seeded"], json!([".claude/settings.local.json"]));
  let seeded = Path::new(&path).join(".claude/settings.local.json");
  assert_eq!(std::fs::read_to_string(&seeded).unwrap(), "{\"local\":true}\n");
  assert!(!Path::new(&path).join("plain.txt").exists());

  let returned = fixture.give_back(&path);
  assert_eq!(returned["removed"], json!([".claude/settings.local.json"]));
  assert!(!seeded.exists());
}

/// `brd lease adopt` adds a linked worktree of this repository where it is, and refuses the main worktree, a worktree
/// of another repository, one the pool holds, a dirty one and a busy one; the next get reuses the adopted slot
#[test_log::test]
fn adopt_adds_clean_linked_worktrees_and_refuses_the_rest() {
  let fixture = Fixture::new();
  let lane = TestWorktree::detached(&fixture.repo);
  let dirty = TestWorktree::detached(&fixture.repo);
  dirty.write("left.txt", "left\n");
  let other_repo = repo_with_baseline(&[("x.txt", "x\n")]);
  let other = TestWorktree::detached(&other_repo);
  let main = fixture.repo.path().to_str().unwrap().to_string();

  let (exit_code, output) = fixture.run(&["lease", "adopt", &lane.path, &main, &other.path, &dirty.path]);
  assert_eq!(exit_code, 4, "{output}");
  assert_eq!(output["adopted"], json!([{ "slot": "lane", "path": slot_path(&lane.path) }]));
  let refused: Vec<(&Value, &Value)> = output["refused"].as_array().unwrap().iter().map(|refused| (&refused["path"], &refused["code"])).collect();
  assert_eq!(
    refused,
    [
      (&json!(canonical(fixture.repo.path()).to_string_lossy()), &json!("not_a_slot")),
      (&json!(slot_path(&other.path)), &json!("not_a_slot")),
      (&json!(slot_path(&dirty.path)), &json!("dirty_worktree")),
    ]
  );
  let (exit_code, output) = fixture.run(&["lease", "adopt", &lane.path]);
  assert_eq!((exit_code, &output["refused"][0]["code"]), (4, &json!("slot_in_pool")), "{output}");

  #[cfg(unix)]
  {
    let busy = TestWorktree::detached(&fixture.repo);
    let mut sleep = busy_in(&busy.path);
    let (exit_code, output) = fixture.run(&["lease", "adopt", &busy.path]);
    sleep.kill().unwrap();
    sleep.wait().unwrap();
    assert_eq!((exit_code, &output["refused"][0]["code"]), (4, &json!("slot_busy")), "{output}");
  }

  let got = fixture.get(&[]);
  assert_eq!((path_of(&got), &got["reused"], &got["slot"]), (slot_path(&lane.path), &json!(true), &json!("lane")));
}

/// `brd lease adopt --move` moves a linked worktree into the pool directory with `git worktree move`: git knows the new
/// path, the old directory and its empty parent are gone, and the next get reuses the slot there
#[test_log::test]
fn adopt_with_move_moves_the_worktree_into_the_pool() {
  let fixture = Fixture::new();
  let lane = TestWorktree::detached(&fixture.repo);
  let old = slot_path(&lane.path);
  let (exit_code, output) = fixture.run(&["lease", "adopt", "--move", &lane.path]);
  assert_eq!(exit_code, 0, "{output}");
  let adopted = &output["adopted"][0];
  let path = adopted["path"].as_str().unwrap().to_string();
  assert_eq!((&adopted["slot"], &adopted["moved_from"]), (&json!("lane"), &json!(old)));
  assert!(Path::new(&path).ends_with(Path::new("1").join(fixture.repo.path().file_name().unwrap())), "{path}");
  assert!(!Path::new(&old).exists() && !Path::new(&old).parent().unwrap().exists());
  let listed = git(&fixture.repo, &["worktree", "list", "--porcelain"]);
  assert!(listed.lines().any(|line| line == format!("worktree {}", git_path(&path))), "{listed}");
  assert_eq!(slot_git(&path, &["status", "--porcelain"]), "");
  let got = fixture.get(&[]);
  assert_eq!((path_of(&got), &got["reused"]), (path, &json!(true)));
}

/// `brd lease adopt --move` holds no pool lock across `git worktree move`, only the lock of the pool's `worktree`
/// commands: while the move is held, the state can be written at once, and the target path is `preparing` under an
/// entry of the adopt, so no get takes it; once the move is done the entry is the free slot
#[cfg(unix)]
#[test_log::test]
fn adopt_with_move_holds_no_pool_lock_across_the_move() {
  let fixture = Fixture::new();
  let dir = tempfile::tempdir().unwrap();
  let (lane, old, held) = lane_beside_a_held(&fixture, dir.path(), "*\" worktree move \"*");
  let pool = lease_pool::lease::Pool::new(Path::new(&slot_path(&fixture.repo.path().join(".git").to_string_lossy())));
  let ((exit_code, output), written, states_during) = std::thread::scope(|scope| {
    let adopt = scope.spawn(|| run_json(&held, &["lease", "adopt", "--move", &lane.path]));
    wait_until_held(dir.path(), &adopt);
    let written = pool.update(Duration::from_millis(500), |_| Ok(()));
    let states_during = pool
      .read()
      .map(|store| store.slots.into_iter().map(|slot| (slot.path, slot.lease.map(|lease| lease.state))).collect::<Vec<_>>());
    // released before any assertion, so a failure never leaves the held adopt waiting
    std::fs::write(dir.path().join("go"), "").unwrap();
    (adopt.join().unwrap(), written, states_during)
  });
  assert!(written.is_ok(), "the pool lock is free during the move: {written:?}");
  assert_eq!(exit_code, 0, "{output}");
  let adopted = &output["adopted"][0];
  let path = adopted["path"].as_str().unwrap().to_string();
  assert_eq!(adopted["moved_from"], json!(old));
  assert_eq!(states_during.unwrap(), [(path.clone(), Some(lease_pool::lease::LeaseState::Preparing))]);
  assert_eq!(states(&fixture.status()), [(slot_path(&path), "free".to_string())]);
}

/// `path` as `git worktree list` names it: `/`-separated on Windows too
fn git_path(path: &str) -> String {
  path.replace('\\', "/")
}

/// `brd lease status` reads the state file: the repository and the pool config, no `status` of any slot
#[test_log::test]
fn the_status_reads_the_state_without_a_git_status() {
  let fixture = Fixture::new();
  let leased = path_of(&fixture.get(&["--holder", "lane-1"]));
  let free = path_of(&fixture.get(&[]));
  fixture.give_back(&free);

  let environment = fixture.environment(fixture.repo.path());
  let context = LeaseContext::resolve(&environment, None).unwrap();
  let status = lease_status(&context, &environment, &LeaseStatusArgs::default()).unwrap();
  let counted: Vec<(String, u32)> = context
    .git
    .process_stats()
    .unwrap()
    .by_command()
    .into_iter()
    .map(|(subcommand, stats)| (subcommand, stats.count))
    .collect();
  assert_eq!(counted, [("config".to_string(), 1), ("rev-parse".to_string(), 1)]);
  let slots: Vec<(&str, String, Option<&str>)> = status
    .slots
    .iter()
    .map(|slot| (slot.path.as_str(), format!("{:?}", slot.state), slot.lease.as_ref().map(|lease| lease.holder.as_str())))
    .collect();
  assert_eq!(slots, [(leased.as_str(), "Ready".to_string(), Some("lane-1")), (free.as_str(), "Free".to_string(), None)]);
}

/// A get that died while preparing a lease leaves it `preparing`; once its process is gone the slot is abandoned, and
/// the next get takes it: on Windows too, which tells the start of a process though it has no scan
#[test_log::test]
fn a_preparing_lease_of_a_gone_process_is_taken_by_the_next_get() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  let mut gone = std::process::Command::new(if cfg!(windows) { "cmd" } else { "true" });
  if cfg!(windows) {
    gone.args(["/c", "exit"]);
  }
  let mut gone = gone.spawn().unwrap();
  let pid = gone.id();
  gone.wait().unwrap();
  let mut state: Value = serde_json::from_slice(&std::fs::read(fixture.state_file()).unwrap()).unwrap();
  let lease = &mut state["slots"][0]["lease"];
  lease["state"] = json!("preparing");
  lease["pid"] = json!(pid);
  lease["process_started"] = json!(1);
  std::fs::write(fixture.state_file(), state.to_string()).unwrap();
  assert_eq!(states(&fixture.status()), [(path.clone(), "abandoned".to_string())]);
  let got = fixture.get(&[]);
  assert_eq!((path_of(&got), &got["reused"]), (path, &json!(true)));
}

/// A get that fails after it recorded the lease gives the slot back and says so
#[test_log::test]
fn a_failed_get_rolls_the_lease_back() {
  let fixture = Fixture::new();
  let got = fixture.get(&[]);
  let path = path_of(&got);
  fixture.give_back(&path);
  // an index git can't read fails the `status` of the get
  let index = fixture.repo.path().join(".git/worktrees").join(got["slot"].as_str().unwrap()).join("index");
  std::fs::write(&index, "not an index").unwrap();
  let (exit_code, output) = fixture.run(&["lease", "get"]);
  assert_error(&output, exit_code, (5, "git_error"));
  assert!(output["error"]["message"].as_str().unwrap().contains("The lease was rolled back"), "{output}");
  assert_eq!(states(&fixture.status()), [(path, "free".to_string())]);
}

/// A git killed in a slot leaves its `index.lock`, which no process owns: the move of the next get waits for it, removes
/// it, and moves the slot by the diff. Windows has no process scan to tell that no process owns it, so the lock stays
/// there (`index_lock_test.rs`, `a_lock_that_no_process_owns_is_removed_and_the_step_runs_again`)
#[cfg(unix)]
#[test_log::test]
fn a_get_removes_the_index_lock_a_killed_git_left_in_the_slot() {
  let fixture = Fixture::new();
  let got = fixture.get(&[]);
  let path = path_of(&got);
  fixture.give_back(&path);
  let head = fixture.repo.create_commit("Add c", "c.txt", "c\n");
  let lock = fixture.repo.path().join(".git/worktrees").join(got["slot"].as_str().unwrap()).join("index.lock");
  std::fs::write(&lock, "").unwrap();
  let again = fixture.get(&[]);
  assert_eq!((path_of(&again), &again["reused"]), (path.clone(), &json!(true)), "{again}");
  assert!(!lock.exists());
  assert_eq!(std::fs::read_to_string(Path::new(&path).join("c.txt")).unwrap(), "c\n");
  assert_eq!(slot_git(&path, &["rev-parse", "HEAD"]), head);
}

/// The outputs match `brd schema`, with their optional parts
#[test_log::test]
fn lease_outputs_match_the_schema() {
  let fixture = Fixture::new();
  let (_, schema) = fixture.run(&["schema", "--all"]);
  let outputs = &schema["outputs"];
  let first = path_of(&fixture.get(&[]));
  fixture.give_back(&first);
  std::fs::write(Path::new(&first).join("left.txt"), "left\n").unwrap();
  let got = fixture.get(&["-b", "feature"]);
  assert_eq!(got["skipped"].as_array().map(Vec::len), Some(1), "{got}");
  assert_covered(&outputs["lease_get"], &outputs["lease_get"], &got, "lease_get");

  let (_, status) = fixture.run(&["lease", "status", "--processes"]);
  assert_eq!(status["processes_checked"], json!(cfg!(unix)), "{status}");
  assert_covered(&outputs["lease_status"], &outputs["lease_status"], &status, "lease_status");

  // on a branch with a local change: `discarded` and `detached_branch`
  let path = path_of(&got);
  std::fs::write(Path::new(&path).join("a.txt"), "changed\n").unwrap();
  let (exit_code, returned) = fixture.run(&["lease", "return", "--workspace", &path, "--confirm-preserved"]);
  assert_eq!(exit_code, 0, "{returned}");
  assert!(returned.get("discarded").is_some() && returned.get("detached_branch").is_some(), "{returned}");
  assert_covered(&outputs["lease_return"], &outputs["lease_return"], &returned, "lease_return");
  // detached with an unlanded commit: `given_up`
  let path = path_of(&fixture.get(&[]));
  std::fs::write(Path::new(&path).join("c.txt"), "c\n").unwrap();
  slot_git(&path, &["add", "c.txt"]);
  slot_git(&path, &["commit", "-q", "-m", "Add c"]);
  let (exit_code, returned) = fixture.run(&["lease", "return", "--workspace", &path, "--confirm-preserved"]);
  assert_eq!((exit_code, returned["given_up"].as_array().map(Vec::len)), (0, Some(1)), "{returned}");
  assert_covered(&outputs["lease_return"], &outputs["lease_return"], &returned, "lease_return");

  let (_, adopt) = fixture.run(&["lease", "adopt", &path, fixture.repo.path().to_str().unwrap()]);
  assert_eq!(adopt["refused"].as_array().map(Vec::len), Some(2), "{adopt}");
  assert_covered(&outputs["lease_adopt"], &outputs["lease_adopt"], &adopt, "lease_adopt");
  let (_, error) = fixture.run(&["lease", "return"]);
  assert_covered(&outputs["error"], &outputs["error"], &error, "error");
}

/// brd of the fixture in `cwd` with a git that holds its first command whose arguments match `pattern` (a `case`
/// pattern of `" $* "`) until the test writes `<dir>/go`; `<dir>/held` tells it is held. It waits outside the slot, so
/// the process scan of another command finds no process in it
#[cfg(unix)]
pub(crate) fn held_environment(fixture: &Fixture, cwd: &Path, dir: &Path, pattern: &str) -> Environment {
  let dir_text = test_utils::fake_executable::shell_path(dir);
  let script = test_utils::fake_executable::fake_executable(
    dir,
    "git",
    &format!(
      "case \" $* \" in {pattern}) if [ ! -f '{dir_text}/held' ]; then here=$(pwd); cd /; touch '{dir_text}/held'; while [ ! -f '{dir_text}/go' ]; do sleep 0.01; done; cd \"$here\"; fi;; esac\nexec git \"$@\""
    ),
  );
  let mut environment = fixture.environment(cwd);
  environment.git = Some(script.to_str().unwrap().to_string());
  environment
}

/// Until `run` reached the command that `held_environment` in `dir` holds; it must not end first
#[cfg(unix)]
pub(crate) fn wait_until_held<T>(dir: &Path, run: &std::thread::ScopedJoinHandle<'_, T>) {
  while !dir.join("held").exists() && !run.is_finished() {
    std::thread::yield_now();
  }
  assert!(dir.join("held").exists(), "the return reached the held command");
}

/// A detached linked worktree of the repository to adopt, its canonical path, and an environment whose git holds at
/// the first command `pattern` matches (`held_environment` in `dir`)
#[cfg(unix)]
fn lane_beside_a_held(fixture: &Fixture, dir: &Path, pattern: &str) -> (TestWorktree, String, Environment) {
  let lane = TestWorktree::detached(&fixture.repo);
  let old = slot_path(&lane.path);
  (lane, old, held_environment(fixture, fixture.repo.path(), dir, pattern))
}

/// A directory of 1 MiB in the trash of the pool, as a run leaves it: of a process that is gone (`gone`), else of
/// this one, which still runs
pub(crate) fn trash_of_a_run(fixture: &Fixture, gone: bool) -> PathBuf {
  let (pid, started, name) = if gone {
    let (pid, started) = gone_process();
    (pid, started, "gone")
  } else {
    (std::process::id(), sync_utils::process_scan::process_started(std::process::id()), "running")
  };
  let started = started.unwrap_or(1);
  let dir = fixture.repo.path().join(".git/branch-deck/trash").join(format!("{pid}-{started}-{name}"));
  std::fs::create_dir_all(dir.join("0")).unwrap();
  std::fs::write(dir.join("0").join("out.bin"), vec![1; 1 << 20]).unwrap();
  dir
}

/// `brd lease return` in `path`, held as `held_environment` holds it, while `during` runs; its exit code and output
#[cfg(unix)]
fn return_held_while(fixture: &Fixture, path: &str, pattern: &str, during: impl FnOnce()) -> (i32, Value) {
  let dir = tempfile::tempdir().unwrap();
  let environment = held_environment(fixture, Path::new(path), dir.path(), pattern);
  std::thread::scope(|scope| {
    let held_return = scope.spawn(|| run_json(&environment, &["lease", "return"]));
    wait_until_held(dir.path(), &held_return);
    during();
    std::fs::write(dir.path().join("go"), "").unwrap();
    held_return.join().unwrap()
  })
}

/// An agent returns its lease again while its first return still checks the slot, and the retry wins; another agent
/// leases the freed slot at once. The first return finds the lease it checked gone: it answers that the lease was
/// returned (`joined`) and changes nothing of the slot of the next lease: no detach of its branch, no discard, no clean
#[cfg(unix)]
#[test_log::test]
fn a_return_that_a_retry_overtook_answers_joined_and_leaves_the_next_lease_of_the_slot_alone() {
  let fixture = Fixture::new();
  let first = fixture.get(&["-b", "first"]);
  let path = path_of(&first);
  let mut lease_id = Value::Null;
  let (exit_code, output) = return_held_while(&fixture, &path, "*\" status \"*", || {
    lease_id = fixture.give_back(&path)["lease_id"].clone();
    let next = fixture.get(&["-b", "second"]);
    assert_eq!(path_of(&next), path, "the freed slot");
    // the next holder starts to work: a check of the overtaken return finds its change
    std::fs::write(Path::new(&path).join("next.txt"), "next\n").unwrap();
  });
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["joined"], &output["lease_id"], &output["detached_branch"]),
    (&json!(true), &lease_id, &Value::Null),
    "{output}"
  );
  assert!(Path::new(&path).join("next.txt").exists(), "the change of the next holder stays");
  assert_eq!(slot_git(&path, &["symbolic-ref", "HEAD"]), "refs/heads/second");
  assert_eq!(states(&fixture.status()), [(slot_path(&path), "ready".to_string())]);
}

/// An agent's return is slow and its tool call times out, so it returns the lease again: the retry passes its checks
/// while the first return acts on the slot, waits for it instead of failing, and answers that the lease was returned
/// (`joined`); the first return frees the slot. Neither fails
#[cfg(unix)]
#[test_log::test]
fn a_return_retried_while_the_first_acts_on_the_slot_waits_for_it_and_answers_joined() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&["-b", "feature"]));
  let (retry_dir, first_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
  // the retry in its checks, before its reservation; the first return at the detach of HEAD, after its reservation
  let retry_environment = held_environment(&fixture, Path::new(&path), retry_dir.path(), "*\" status \"*");
  let first_environment = held_environment(&fixture, Path::new(&path), first_dir.path(), "*\" update-ref \"*");
  let (first, retry) = std::thread::scope(|scope| {
    let retry = scope.spawn(|| run_json(&retry_environment, &["lease", "return"]));
    wait_until_held(retry_dir.path(), &retry);
    let first = scope.spawn(|| run_json(&first_environment, &["lease", "return"]));
    wait_until_held(first_dir.path(), &first);
    assert_eq!(states(&fixture.status()), [(slot_path(&path), "returning".to_string())]);
    // the retry reaches its reservation while the first return runs, or after it freed the slot: it joins either way
    std::fs::write(retry_dir.path().join("go"), "").unwrap();
    std::fs::write(first_dir.path().join("go"), "").unwrap();
    (first.join().unwrap(), retry.join().unwrap())
  });
  assert_eq!(first.0, 0, "{}", first.1);
  assert_eq!(first.1["detached_branch"], json!("feature"));
  assert_eq!(retry.0, 0, "{}", retry.1);
  assert_eq!((&retry.1["joined"], &retry.1["lease_id"]), (&json!(true), &first.1["lease_id"]), "{}", retry.1);
  assert_eq!(retry.1["detached_branch"], Value::Null, "the retry changed nothing");
  assert_eq!(states(&fixture.status()), [(slot_path(&path), "free".to_string())]);
}

/// While a return acts on the slot (`returning`), a get leases neither it nor a new slot: it waits for the return
/// (`waiting for the return of <slot>`), the return frees the slot, and the get leases that slot
#[cfg(unix)]
#[test_log::test]
fn a_running_return_reserves_the_slot_against_a_get() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&["-b", "feature"]));
  let steps = std::sync::Mutex::new(Vec::new());
  let (got, (exit_code, output)) = std::thread::scope(|scope| {
    let mut get = None;
    // the detach of HEAD, after the reservation
    let returned = return_held_while(&fixture, &path, "*\" update-ref \"*", || {
      assert_eq!(states(&fixture.status()), [(slot_path(&path), "returning".to_string())]);
      let waiting = scope.spawn(|| get_with_steps(&fixture, "other", &steps));
      let wait = wait_for_the_slot(&steps, &waiting);
      assert!(matches!(&wait, GetStep::WaitForSlot { slot, clean: false, .. } if *slot == path), "{wait:?}");
      assert_eq!(states(&fixture.status()), [(slot_path(&path), "returning".to_string())], "the get reserved nothing");
      get = Some(waiting);
    });
    (get.unwrap().join().unwrap().unwrap(), returned)
  });
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["detached_branch"], json!("feature"));
  assert_eq!((&got.path, got.reused), (&path, true));
  assert_eq!(kinds(&steps), ["reserve", "reserve adding", "wait for slot", "reserve adding", "status", "checkout"]);
  assert_eq!(states(&fixture.status()), [(slot_path(&path), "ready".to_string())]);
}

/// An agent leases a worktree and gives it back while another agent's `brd sync` of the repository is held in its
/// merges: a sync takes neither the repository lock nor the pool lock, so the get and the return end while that sync is
/// still held, which then ends too
#[cfg(unix)]
#[test_log::test]
fn a_get_and_a_return_beside_a_held_sync_do_not_wait_for_it() {
  let fixture = Fixture::new();
  // below it a commit of no branch: the sync merges `own` onto the baseline
  fixture.repo.create_commit("(own) Add c", "c.txt", "c\n");
  let head = fixture.repo.head();
  let mut path = String::new();
  beside_a_held_sync(&fixture.repo, || {
    path = path_of(&fixture.get(&["-b", "lane"]));
    assert_eq!(slot_git(&path, &["rev-parse", "HEAD"]).trim(), head, "the lease is cut at the HEAD the sync reads");
    fixture.give_back(&path);
  });
  assert_eq!(states(&fixture.status()), [(slot_path(&path), "free".to_string())]);
}
