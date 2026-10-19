use crate::context::Environment;
use crate::fixture_test::{assert_error, canonical, environment, git, repo_with_baseline, run_json};
use crate::lease_command::{LeaseContext, LeaseStatusArgs, lease_status};
use crate::schema_test::assert_covered;
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use test_utils::git_test_utils::{TestRepo, TestWorktree};

/// A repository with a commit on master and the home directory its pool lives in
struct Fixture {
  repo: TestRepo,
  home: tempfile::TempDir,
}

impl Fixture {
  fn new() -> Self {
    let repo = repo_with_baseline(&[("a.txt", "a\n")]);
    repo.create_commit("Add b", "b.txt", "b\n");
    Self {
      repo,
      home: tempfile::tempdir().unwrap(),
    }
  }

  /// bd started in `cwd`, the pool in the fixture's home
  fn environment(&self, cwd: &Path) -> Environment {
    let mut environment = environment(cwd);
    environment.home = Some(self.home.path().to_path_buf());
    environment.lock_timeout = Duration::from_secs(10);
    environment
  }

  /// `bd <args>` in the repository
  fn run(&self, args: &[&str]) -> (i32, Value) {
    self.run_in(self.repo.path(), args)
  }

  fn run_in(&self, cwd: &Path, args: &[&str]) -> (i32, Value) {
    run_json(&self.environment(cwd), args)
  }

  /// `bd lease get <args>`, which must succeed
  fn get(&self, args: &[&str]) -> Value {
    let (exit_code, output) = self.run(&[&["lease", "get"][..], args].concat());
    assert_eq!(exit_code, 0, "{output}");
    output
  }

  /// `bd lease return` run inside the slot, which must succeed
  fn give_back(&self, path: &str) -> Value {
    let (exit_code, output) = self.run_in(Path::new(path), &["lease", "return"]);
    assert_eq!(exit_code, 0, "{output}");
    output
  }

  fn status(&self) -> Value {
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
fn slot_path(path: &str) -> String {
  canonical(Path::new(path)).to_string_lossy().into_owned()
}

/// git in a slot; panics on a failure
fn slot_git(path: &str, args: &[&str]) -> String {
  GitCommandExecutor::new()
    .execute_command(args, path)
    .unwrap_or_else(|e| panic!("git {args:?} in {path}: {e}"))
}

fn path_of(output: &Value) -> String {
  output["path"].as_str().unwrap_or_else(|| panic!("no path: {output}")).to_string()
}

/// The `state` of each slot of `bd lease status`, by path
fn states(status: &Value) -> Vec<(String, String)> {
  status["slots"]
    .as_array()
    .unwrap()
    .iter()
    .map(|slot| (slot["path"].as_str().unwrap().to_string(), slot["state"].as_str().unwrap().to_string()))
    .collect()
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
  assert!(
    Path::new(&path).starts_with(fixture.home.path().canonicalize().unwrap().join(".branch-deck/pool")),
    "{path}"
  );
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
  let mut sleep = std::process::Command::new("sleep").arg("30").current_dir(&first).spawn().unwrap();
  let (exit_code, second) = fixture.run(&["lease", "get"]);
  sleep.kill().unwrap();
  sleep.wait().unwrap();
  assert_eq!(exit_code, 0, "{second}");
  assert_ne!(path_of(&second), first);
  let skipped = &second["skipped"][0];
  assert_eq!(
    (&skipped["path"], &skipped["reason"], &skipped["processes"][0]["pid"]),
    (&json!(first), &json!("busy"), &json!(sleep.id()))
  );
}

/// Concurrent gets reserve their slots under the pool lock: never one slot twice, whether reused or new
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
    let mut sleep = std::process::Command::new("sleep").arg("30").current_dir(&path).spawn().unwrap();
    let (exit_code, output) = fixture.run(&["lease", "return", "--workspace", &path, "--confirm-preserved"]);
    sleep.kill().unwrap();
    sleep.wait().unwrap();
    assert_error(&output, exit_code, (4, "slot_busy"));
    assert!(output["error"]["message"].as_str().unwrap().contains(&format!("sleep ({})", sleep.id())), "{output}");
  }
  assert_eq!(states(&fixture.status())[0].1, "ready");

  // another lease id, and a worktree without a lease
  let (exit_code, output) = fixture.run(&["lease", "return", "--workspace", &path, "--lease-id", "other"]);
  assert_error(&output, exit_code, (2, "lease_mismatch"));
  let (exit_code, output) = fixture.run(&["lease", "return"]);
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
    let mut sleep = std::process::Command::new("sleep").arg("30").current_dir(&busy.path).spawn().unwrap();
    let (exit_code, output) = fixture.run(&["lease", "adopt", &busy.path]);
    sleep.kill().unwrap();
    sleep.wait().unwrap();
    assert_eq!((exit_code, &output["refused"][0]["code"]), (4, &json!("slot_busy")), "{output}");
  }

  let got = fixture.get(&[]);
  assert_eq!((path_of(&got), &got["reused"], &got["slot"]), (slot_path(&lane.path), &json!(true), &json!("lane")));
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
/// the next get takes it
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
