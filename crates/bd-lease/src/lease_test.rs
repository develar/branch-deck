use crate::lease_command::{LeaseContext, LeaseStatusArgs, lease_status};
use bd_cli::fixture_test::assert_covered;
use bd_cli::fixture_test::{PREFIX, assert_error, canonical, environment, git, repo_with_baseline, run_json};
use bd_core::context::Environment;
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use test_utils::git_test_utils::{TestRepo, TestWorktree};

/// A repository with a commit on master, the home directory its pool lives in and the global git config of bd
pub(crate) struct Fixture {
  pub(crate) repo: TestRepo,
  pub(crate) home: tempfile::TempDir,
  /// Empty unless a test writes it
  pub(crate) global_config: tempfile::NamedTempFile,
}

impl Fixture {
  pub(crate) fn new() -> Self {
    let repo = repo_with_baseline(&[("a.txt", "a\n")]);
    repo.create_commit("Add b", "b.txt", "b\n");
    Self {
      repo,
      home: tempfile::tempdir().unwrap(),
      global_config: tempfile::NamedTempFile::new().unwrap(),
    }
  }

  /// bd started in `cwd`, the pool in the fixture's home
  fn environment(&self, cwd: &Path) -> Environment {
    let mut environment = environment(cwd);
    environment.home = Some(self.home.path().to_path_buf());
    environment.git_config_global = Some(self.global_config.path().to_path_buf());
    environment.lock_timeout = Duration::from_secs(10);
    environment
  }

  /// `bd <args>` in the repository
  pub(crate) fn run(&self, args: &[&str]) -> (i32, Value) {
    self.run_in(self.repo.path(), args)
  }

  pub(crate) fn run_in(&self, cwd: &Path, args: &[&str]) -> (i32, Value) {
    run_json(&self.environment(cwd), args)
  }

  /// `bd lease get <args>`, which must succeed
  pub(crate) fn get(&self, args: &[&str]) -> Value {
    let (exit_code, output) = self.run(&[&["lease", "get"][..], args].concat());
    assert_eq!(exit_code, 0, "{output}");
    output
  }

  /// `bd lease return` run inside the slot, which must succeed
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
  fn state_file(&self) -> PathBuf {
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

/// The `state` of each slot of `bd lease status`, by path
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

/// A free slot with local changes is skipped, marked dirty and tried last; with no other free slot the pool grows
#[test_log::test]
fn a_dirty_free_slot_is_skipped_and_a_new_slot_is_added() {
  let fixture = Fixture::new();
  let first = path_of(&fixture.get(&[]));
  fixture.give_back(&first);
  std::fs::write(Path::new(&first).join("left.txt"), "left behind\n").unwrap();

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

/// The checkouts of the pool (a reused slot, a cloned one, a new one after its `worktree add --no-checkout`) run with
/// `checkout.workers=0`, one worker per core, unless the repository sets `checkout.workers` itself
#[cfg(unix)]
#[test_log::test]
fn every_checkout_of_the_pool_runs_with_a_worker_per_core_unless_the_repository_sets_it() {
  let fixture = Fixture::new();
  let trace = fixture.home.path().join("trace2.json");
  let _env = test_utils::env_vars::EnvVars::set(&[("GIT_TRACE2_EVENT", &trace)]);
  let parallel = |argv: &&Vec<String>| argv.windows(2).any(|pair| pair[0] == "-c" && pair[1] == "checkout.workers=0");

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
  if cfg!(target_os = "macos") {
    assert_eq!(cloned["created_by"], json!("clone"), "{cloned}");
    assert!(checkouts.len() >= 2 && checkouts.iter().all(parallel), "{gits:?}");
  } else {
    assert!(!checkouts.is_empty() && checkouts.iter().all(parallel) && registered(&gits), "{gits:?}");
  }

  fixture.give_back(&path);
  fixture.repo.set_config("checkout.workers", "2").unwrap();
  fixture.repo.create_commit("Add d", "d.txt", "d\n");
  started_gits(&trace);
  fixture.get(&[]);
  let gits = started_gits(&trace);
  let checkouts = gits_of(&gits, "checkout");
  assert!(!checkouts.is_empty() && !checkouts.iter().any(parallel), "{gits:?}");
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
/// commit the closest; the order they joined the pool decides only among the slots never returned
#[test_log::test]
fn the_free_slot_returned_last_is_leased_first() {
  let fixture = Fixture::new();
  let first = path_of(&fixture.get(&[]));
  let second = path_of(&fixture.get(&[]));
  fixture.give_back(&first);
  fixture.give_back(&second);
  assert_eq!(path_of(&fixture.get(&[])), second);
  assert_eq!(path_of(&fixture.get(&[])), first);
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

/// Commit `file` in the slot at `path`
fn commit_in(path: &str, subject: &str, file: &str) -> String {
  std::fs::write(Path::new(path).join(file), format!("{subject}\n")).unwrap();
  slot_git(path, &["add", file]);
  slot_git(path, &["commit", "-q", "-m", subject]);
  slot_git(path, &["rev-parse", "HEAD"])
}

/// A lease whose commits landed by `bd pick --follow`, on master with a peer commit, after which a peer rewrote master:
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
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  let base = fixture.repo.head();
  let commit = commit_in(&path, "(lane) Add c", "lane-c.txt");
  fixture.repo.git(&["update-ref", "-m", "branch-deck: pick", "refs/heads/master", &commit, &base]);
  fixture
    .repo
    .git(&["update-ref", "-m", "branch-deck: pick rolled back", "refs/heads/master", &base, &commit]);

  let (exit_code, output) = fixture.run_in(Path::new(&path), &["lease", "return"]);
  assert_error(&output, exit_code, (4, "unlanded_commits"));
  assert_eq!(output["error"]["commits"], json!([commit]));
}

/// A session lands a commit with `bd pick --follow`, then fixes its message on the shared branch from its own worktree
/// (`bd -C <shared checkout> reword`): the worktree needs nothing, its old tip is landed by content, a later pick
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
}

#[cfg(unix)]
impl FakeServer {
  fn start(workspace: &str, cwd: &Path, ignore_term: bool) -> Self {
    use std::os::unix::process::CommandExt;
    let script = if ignore_term { "trap '' TERM; read line" } else { "read line" };
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
      .current_dir(cwd)
      .stdin(std::process::Stdio::piped())
      .spawn()
      .unwrap();
    // its argv is the one of `sh` once it runs: wait until a scan sees the workspace
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let pid = child.id();
    while !sync_utils::process_scan::ProcessScan::scan()
      .inside(Path::new(workspace))
      .iter()
      .any(|process| process.pid == pid)
    {
      assert!(std::time::Instant::now() < deadline, "the fake server never showed up");
      std::thread::sleep(Duration::from_millis(20));
    }
    Self { child }
  }

  fn pid(&self) -> u32 {
    self.child.id()
  }

  /// Whether it still runs; reaps it once it exited
  fn running(&mut self) -> bool {
    self.child.try_wait().unwrap().is_none()
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
  assert!(!root.running() && !community.running());
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
  assert!(!server.running());
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

/// A process in the slot that runs in the process group of bd (the `jq` of `bd lease return | jq`, the `caffeinate` of
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
  let deadline = std::time::Instant::now() + Duration::from_secs(10);
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
/// slots, with nothing leased; `--min-free 0` checks nothing; a size that does not parse is an invalid argument
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
    message.contains("1048576.0 GiB") && message.contains("1 free slot(s)") && message.contains("--min-free 0"),
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

/// A get records the process it ran for, the nearest ancestor that is no shell, and the status tells whether it runs;
/// once it is gone, a get that adds a new slot names the lease, and the lease stays the holder's
#[cfg(unix)]
#[test_log::test]
fn the_status_tells_whether_the_holder_of_a_lease_is_alive() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&["--holder", "gone-agent"]));
  let lease = &fixture.status()["slots"][0]["lease"];
  assert_eq!(lease["holder_alive"], json!(true), "{lease}");
  assert!(lease["holder_pid"].as_u64().unwrap() > 1, "{lease}");

  // the holder exits: a child's pid with its start time, once it has been reaped
  let mut child = std::process::Command::new("true").spawn().unwrap();
  let (pid, started) = (child.id(), sync_utils::process_scan::process_started(child.id()));
  child.wait().unwrap();
  let mut state: Value = serde_json::from_slice(&std::fs::read(fixture.state_file()).unwrap()).unwrap();
  state["slots"][0]["lease"]["holder_pid"] = json!(pid);
  state["slots"][0]["lease"]["holder_started"] = json!(started.unwrap_or(1));
  std::fs::write(fixture.state_file(), serde_json::to_vec(&state).unwrap()).unwrap();
  assert_eq!(fixture.status()["slots"][0]["lease"]["holder_alive"], json!(false));
  let second = fixture.get(&[]);
  assert_eq!(second["idle_leases"], json!([{ "path": path, "holder": "gone-agent" }]), "{second}");
  assert_eq!(states(&fixture.status())[0], (path, "ready".to_string()));
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
    #[expect(clippy::needless_collect, reason = "every get starts before the first one is joined")]
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

/// The `worktree add` that registers a new slot holds the lock of the pool's `worktree` commands (`worktrees.lock`), so
/// no other `worktree` command of the pool reads its admin directory half written: a get that registers beside it
/// waits, and one that can't wait is `locked`. The pool lock stays free: that get reserved its slot
#[cfg(unix)]
#[test_log::test]
fn no_worktree_command_of_the_pool_runs_beside_the_registration_of_a_new_slot() {
  let fixture = Fixture::new();
  let dir = tempfile::tempdir().unwrap();
  let held = held_environment(&fixture, fixture.repo.path(), dir.path(), "*\" worktree add \"*");
  let mut impatient = fixture.environment(fixture.repo.path());
  impatient.lock_timeout = Duration::from_millis(200);
  std::thread::scope(|scope| {
    let get = scope.spawn(|| run_json(&held, &["lease", "get"]));
    wait_until_held(dir.path(), &get);
    let (exit_code, output) = run_json(&impatient, &["lease", "get"]);
    // released before any assertion, so a failure never leaves the held get waiting
    std::fs::write(dir.path().join("go"), "").unwrap();
    assert_error(&output, exit_code, (3, "locked"));
    assert!(output["error"]["message"].as_str().unwrap().contains("worktrees.lock"), "{output}");
    let (exit_code, output) = get.join().unwrap();
    assert_eq!(exit_code, 0, "{output}");
  });
  // the lock is free once the registration is done
  let (exit_code, output) = run_json(&impatient, &["lease", "get"]);
  assert_eq!(exit_code, 0, "{output}");
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
  assert!(output["error"]["message"].as_str().unwrap().contains("bd pick --from"), "{output}");

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

/// `bd lease adopt` adds a linked worktree of this repository where it is, and refuses the main worktree, a worktree
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

/// `bd lease adopt --move` moves a linked worktree into the pool directory with `git worktree move`: git knows the new
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

/// `path` as `git worktree list` names it: `/`-separated on Windows too
fn git_path(path: &str) -> String {
  path.replace('\\', "/")
}

/// `bd lease status` reads the state file: the repository and the pool config, no `status` of any slot
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
  fixture.repo.create_commit("Add c", "c.txt", "c\n");
  // a held index lock fails the checkout; `status` reads without it
  let lock = fixture.repo.path().join(".git/worktrees").join(got["slot"].as_str().unwrap()).join("index.lock");
  std::fs::write(&lock, "").unwrap();
  let (exit_code, output) = fixture.run(&["lease", "get"]);
  std::fs::remove_file(&lock).unwrap();
  assert_error(&output, exit_code, (5, "git_error"));
  assert!(output["error"]["message"].as_str().unwrap().contains("The lease was rolled back"), "{output}");
  assert_eq!(states(&fixture.status()), [(path, "free".to_string())]);
}

/// The outputs match `bd schema`, with their optional parts
#[test_log::test]
fn lease_outputs_match_the_schema() {
  let fixture = Fixture::new();
  let (_, schema) = fixture.run(&["schema"]);
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

/// bd of the fixture in `cwd` with a git that holds its first command whose arguments match `pattern` (a `case`
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
fn wait_until_held<T>(dir: &Path, run: &std::thread::ScopedJoinHandle<'_, T>) {
  while !dir.join("held").exists() && !run.is_finished() {
    std::thread::yield_now();
  }
  assert!(dir.join("held").exists(), "the return reached the held command");
}

/// `bd lease return` in `path`, held as `held_environment` holds it, while `during` runs; its exit code and output
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

/// While a return acts on the slot (`returning`), a get passes the slot over; the return then frees the slot
#[cfg(unix)]
#[test_log::test]
fn a_running_return_reserves_the_slot_against_a_get() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&["-b", "feature"]));
  let mut other = String::new();
  // the detach of HEAD, after the reservation
  let (exit_code, output) = return_held_while(&fixture, &path, "*\" update-ref \"*", || {
    assert_eq!(states(&fixture.status()), [(slot_path(&path), "returning".to_string())]);
    other = path_of(&fixture.get(&[]));
    assert_ne!(other, path, "a get passes the returning slot over");
  });
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["detached_branch"], json!("feature"));
  let mut expected = vec![(slot_path(&path), "free".to_string()), (slot_path(&other), "ready".to_string())];
  let mut actual = states(&fixture.status());
  expected.sort();
  actual.sort();
  assert_eq!(actual, expected);
}
