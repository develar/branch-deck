//! The objects of Git LFS of a move of the working tree are fetched before the repository lock
//! (`git_ops::lfs::prefetch_lfs_objects`), so the smudge filter of the move under the lock reads the store only: for
//! Update Project, an unapply and a reapply, a drop, a pick, and the undo of each. The filter is a fake that logs whether
//! the lock was held (its holder record exists) and whether the store had the object; `git lfs fetch` is an arm of the
//! git of `LockSpanGit`, which logs the lock state of every git, that puts the objects of `fetchable` into the store

use bd_cli::fixture_test::{LockSpanGit, repo_on_baseline, run_json};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::PathBuf;
use tempfile::TempDir;
use test_utils::env_vars::EnvVars;
use test_utils::git_test_utils::{TestRepo, TestWorktree, append_config};
use test_utils::history::HistoryCommit;

const ATTRIBUTES: (&str, &str) = (".gitattributes", "*.bin filter=lfs\n");

/// The object id of the object `n` of a test: 64 hex digits
fn oid(n: u8) -> String {
  format!("{n:02x}").repeat(32)
}

/// The pointer file of Git LFS of the object `n`
fn pointer(n: u8) -> String {
  format!("version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize 12\n", oid(n))
}

/// A repository whose `*.bin` files go through the filter `lfs`, with its store, and brd's git that logs the lock state
/// of each command and answers `git lfs fetch`
struct Lfs {
  wrapped: LockSpanGit,
  directory: TempDir,
  objects: PathBuf,
}

impl Lfs {
  /// The filter as `git lfs install` sets it, in the config of `repo`: the smudge command logs `locked` or `free`, then
  /// `store` or `network` for its object, and passes the pointer through; `clean` passes it through as well
  fn new(repo: &TestRepo) -> Self {
    Self::with_store(repo, "lfs")
  }

  /// `new` with the store in `store` of the git dir (`lfs.storage` when it is not `lfs`)
  fn with_store(repo: &TestRepo, store: &str) -> Self {
    let directory = tempfile::tempdir().unwrap();
    let objects = repo.path().join(".git").join(store).join("objects");
    std::fs::create_dir_all(&objects).unwrap();
    let holder = repo.path().join(".git").join("branch-deck.lock.holder");
    let (objects_text, holder_text, log) = (objects.display(), holder.display(), directory.path().join("smudge.log"));
    let object_of = |oid: &str| format!("'{objects_text}'/$(printf %s \"{oid}\" | cut -c1-2)/$(printf %s \"{oid}\" | cut -c3-4)/{oid}");
    let smudge = directory.path().join("smudge");
    let script = format!(
      "#!/bin/sh\npointer=$(cat)\noid=$(printf '%s\\n' \"$pointer\" | sed -n 's/^oid sha256://p')\nobject={}\nif [ -f '{holder_text}' ]; then state=locked; else state=free; fi\nif [ -f \"$object\" ]; then from=store; else from=network; fi\necho \"$state $from\" >> '{}'\nprintf '%s\\n' \"$pointer\"\n",
      object_of("$oid"),
      log.display()
    );
    std::fs::write(&smudge, script).unwrap();
    std::fs::set_permissions(&smudge, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    append_config(
      repo.path(),
      &[
        ("filter.lfs.smudge", smudge.to_str().unwrap()),
        ("filter.lfs.clean", "cat"),
        ("filter.lfs.required", "true"),
      ],
    );
    let fetch = format!(
      "*\" lfs fetch \"*) [ -f '{{dir}}/fetchable' ] || exit 1; while read -r oid; do object={}; mkdir -p \"$(dirname \"$object\")\"; : > \"$object\"; done < '{{dir}}/fetchable'; exit 0;;",
      object_of("$oid")
    );
    Self {
      wrapped: LockSpanGit::new(repo, &fetch),
      directory,
      objects,
    }
  }

  fn object(&self, n: u8) -> PathBuf {
    let oid = oid(n);
    self.objects.join(&oid[..2]).join(&oid[2..4]).join(&oid)
  }

  /// The object `n` in the store, as a smudge or a fetch before left it
  fn store(&self, n: u8) {
    std::fs::create_dir_all(self.object(n).parent().unwrap()).unwrap();
    std::fs::write(self.object(n), "").unwrap();
  }

  /// The object `n` gone from the store (`git lfs prune`)
  fn forget(&self, n: u8) {
    std::fs::remove_file(self.object(n)).unwrap();
  }

  /// What the remote of Git LFS has for the next fetch: the object `n`
  fn fetchable(&self, n: u8) {
    std::fs::write(self.wrapped.dir().join("fetchable"), format!("{}\n", oid(n))).unwrap();
  }

  /// `brd <args>`, which must succeed, the logs of the git and of the smudge filter starting empty
  fn run(&self, args: &[&str]) -> Value {
    self.run_exiting(args, 0)
  }

  /// `brd <args>`, which must exit with `exit_code`, the logs starting empty
  fn run_exiting(&self, args: &[&str], exit_code: i32) -> Value {
    let _ = std::fs::remove_file(self.directory.path().join("smudge.log"));
    let (code, output) = self.wrapped.run(args);
    assert_eq!(code, exit_code, "{output}");
    output
  }

  /// What the smudge filter logged in the last run
  fn smudges(&self) -> Vec<String> {
    let log = std::fs::read_to_string(self.directory.path().join("smudge.log")).unwrap_or_default();
    log.lines().map(str::to_string).collect()
  }

  /// The last run fetched the object `n` once, before the lock, the file that changes at the commit HEAD is at now
  fn assert_fetched_before_the_lock(&self, repo: &TestRepo, n: u8) {
    self.assert_fetched_before_the_lock_at(&repo.head(), n);
  }

  /// The last run fetched the object `n` once, before the lock, the file that changes at the commit `to`
  fn assert_fetched_before_the_lock_at(&self, to: &str, n: u8) {
    assert_eq!(self.wrapped.locked_runs("lfs"), [false], "one fetch, before the lock: {:#?}", self.wrapped.commands());
    let fetch = format!(" lfs fetch --include model.bin origin {to}");
    assert!(
      self.wrapped.commands().iter().any(|(_, args)| args.ends_with(&fetch)),
      "{fetch}: {:#?}",
      self.wrapped.commands()
    );
    assert!(self.object(n).is_file(), "the fetch stored the object {n}");
  }

  /// `assert_fetched_before_the_lock`, and the move under the lock smudged the file from the store: never from the
  /// network under the lock
  fn assert_prefetched(&self, repo: &TestRepo, n: u8) {
    self.assert_fetched_before_the_lock(repo, n);
    assert_eq!(self.smudges(), ["locked store"], "{:#?}", self.wrapped.commands());
  }
}

fn model(repo: &TestRepo) -> String {
  std::fs::read_to_string(repo.path().join("model.bin")).unwrap()
}

fn undo_token(output: &Value) -> String {
  output["undo_token"].as_str().unwrap_or_else(|| panic!("no undo token: {output}")).to_string()
}

/// No system config and an empty global one for every git of the test, the fixtures' and brd's: the filter of a
/// `git lfs install` of the developer or of the runner (`filter.lfs.required=true`, the real smudge) stays out, and only
/// the fake filter of `Lfs` is configured
struct Hermetic {
  _vars: EnvVars,
  _global: tempfile::NamedTempFile,
}

impl Hermetic {
  fn new() -> Self {
    let global = tempfile::NamedTempFile::new().unwrap();
    let vars = EnvVars::set(&[("GIT_CONFIG_NOSYSTEM", "1".as_ref()), ("GIT_CONFIG_GLOBAL", global.path().as_os_str())]);
    Self { _vars: vars, _global: global }
  }
}

/// A repository whose baseline has the model `1` and whose branch `feature` changes it to `2`, the store with `2`
fn feature_changes_the_model() -> (TestRepo, Lfs, String) {
  feature_changes_the_model_in("lfs")
}

/// `feature_changes_the_model` with the store in `store` of the git dir
fn feature_changes_the_model_in(store: &str) -> (TestRepo, Lfs, String) {
  let (repo, ids) = repo_on_baseline(
    &[ATTRIBUTES, ("model.bin", &pointer(1))],
    &[HistoryCommit::new("(feature) Change the model", &[("model.bin", &pointer(2))])],
    ".",
  );
  let lfs = Lfs::with_store(&repo, store);
  lfs.store(2);
  (repo, lfs, ids[0].clone())
}

/// Update Project fetches the objects of the upstream's files before its lock, and so does its undo
#[test_log::test]
fn an_update_and_its_undo_fetch_the_objects_of_git_lfs_before_the_lock() {
  let _hermetic = Hermetic::new();
  let (upstream, clone) = TestRepo::upstream_and_clone(&[ATTRIBUTES, ("model.bin", &pointer(1))]);
  upstream.create_commit("Change the model", "model.bin", &pointer(2));
  clone.git(&["fetch", "-q", "origin"]);
  let lfs = Lfs::new(&clone);
  lfs.store(1);

  lfs.fetchable(2);
  let updated = lfs.run(&["update", "--no-fetch"]);
  assert_eq!(updated["state"], json!("fast_forward"), "{updated}");
  lfs.assert_prefetched(&clone, 2);
  assert_eq!(model(&clone), pointer(2));

  lfs.forget(1);
  lfs.fetchable(1);
  lfs.run(&["undo", &undo_token(&updated)]);
  lfs.assert_prefetched(&clone, 1);
  assert_eq!(model(&clone), pointer(1));
}

/// An unapply fetches the objects of the files it brings back before its lock, and so does the reapply
#[test_log::test]
fn an_unapply_and_a_reapply_fetch_the_objects_of_git_lfs_before_the_lock() {
  let _hermetic = Hermetic::new();
  let (repo, lfs, _) = feature_changes_the_model();
  // an unapply takes the commits of the virtual branch a sync built
  lfs.run(&["sync"]);
  lfs.fetchable(1);
  lfs.run(&["unapply", "feature"]);
  lfs.assert_prefetched(&repo, 1);
  assert_eq!(model(&repo), pointer(1));

  lfs.forget(2);
  lfs.fetchable(2);
  lfs.run(&["reapply", "feature"]);
  lfs.assert_prefetched(&repo, 2);
  assert_eq!(model(&repo), pointer(2));
}

/// A drop fetches the objects of the files it brings back before its lock, and so does its undo
#[test_log::test]
fn a_drop_and_its_undo_fetch_the_objects_of_git_lfs_before_the_lock() {
  let _hermetic = Hermetic::new();
  let (repo, lfs, commit) = feature_changes_the_model();
  lfs.fetchable(1);
  let dropped = lfs.run(&["drop", &commit]);
  lfs.assert_prefetched(&repo, 1);
  assert_eq!(model(&repo), pointer(1));

  lfs.forget(2);
  lfs.fetchable(2);
  lfs.run(&["undo", &undo_token(&dropped)]);
  lfs.assert_prefetched(&repo, 2);
  assert_eq!(model(&repo), pointer(2));
}

/// A pick fetches the objects of the files it brings before its lock, and so does its undo
#[test_log::test]
fn a_pick_and_its_undo_fetch_the_objects_of_git_lfs_before_the_lock() {
  let _hermetic = Hermetic::new();
  let (repo, _) = repo_on_baseline(&[ATTRIBUTES, ("model.bin", &pointer(1))], &[], ".");
  let lane = TestWorktree::detached(&repo);
  lane.commit("(lane) Change the model", "model.bin", &pointer(2));
  let lfs = Lfs::new(&repo);
  lfs.store(1);

  lfs.fetchable(2);
  let picked = lfs.run(&["pick", "--from", &lane.path, "--follow"]);
  assert_eq!(picked["state"], json!("picked"), "{picked}");
  lfs.assert_prefetched(&repo, 2);
  assert_eq!(model(&repo), pointer(2));

  lfs.forget(1);
  lfs.fetchable(1);
  lfs.run(&["undo", &undo_token(&picked)]);
  lfs.assert_prefetched(&repo, 1);
  assert_eq!(model(&repo), pointer(1));
}

/// Objects the store has are fetched for no move, and a fetch that fails fails the move before its lock: nothing moved
#[test_log::test]
fn a_move_whose_objects_are_stored_fetches_nothing_and_a_failed_fetch_moves_nothing() {
  let _hermetic = Hermetic::new();
  let (repo, lfs, commit) = feature_changes_the_model();
  let head = repo.head();
  // the fetch of the arm reads `fetchable`, which the test never wrote: the fetch fails
  let (exit_code, output) = lfs.wrapped.run(&["drop", &commit]);
  assert_ne!(exit_code, 0, "{output}");
  assert!(
    output["error"]["message"]
      .as_str()
      .is_some_and(|message| message.contains("Cannot fetch the files of Git LFS")),
    "{output}"
  );
  assert_eq!(lfs.wrapped.locked_runs("lfs"), [false], "{:#?}", lfs.wrapped.commands());
  assert!(lfs.wrapped.under_lock().is_empty(), "no lock: {:#?}", lfs.wrapped.commands());
  assert_eq!((repo.head(), model(&repo)), (head, pointer(2)));

  lfs.store(1);
  lfs.run(&["drop", &commit]);
  assert_eq!(lfs.wrapped.locked_runs("lfs"), Vec::<bool>::new(), "{:#?}", lfs.wrapped.commands());
  assert_eq!(model(&repo), pointer(1));
}

/// A repository that lives on the pointers of what Git LFS can not download (`lfs.skipdownloaderrors`) moves when the
/// fetch before the lock failed: its smudge writes the pointers as the repository asks, and tries the object once more
/// under the lock
#[test_log::test]
fn a_repository_that_skips_download_errors_moves_when_the_fetch_failed() {
  let _hermetic = Hermetic::new();
  let (repo, lfs, commit) = feature_changes_the_model();
  append_config(repo.path(), &[("lfs.skipdownloaderrors", "true")]);
  lfs.run(&["drop", &commit]);
  assert_eq!(lfs.wrapped.locked_runs("lfs"), [false], "{:#?}", lfs.wrapped.commands());
  assert_eq!(model(&repo), pointer(1));
}

/// An update that conflicts starts a resolve session, whose start may finish under the lock when the words resolve every
/// round, and `brd update --continue` replays and finishes under the lock: both fetch the objects of the files the
/// upstream changes (from HEAD to the upstream tip the check read) before the lock, and the finish smudges from the store
#[test_log::test]
fn a_session_start_and_its_continue_fetch_the_objects_of_git_lfs_before_the_lock() {
  let _hermetic = Hermetic::new();
  let (upstream, clone) = TestRepo::upstream_and_clone(&[ATTRIBUTES, ("model.bin", &pointer(1)), ("shared.txt", "one\ntwo\nthree\n")]);
  clone.create_commit("(auth) Change line 2", "shared.txt", "one\nmine\nthree\n");
  let upstream_tip = upstream.create_commit_with_files(
    "Change line 2 and the model upstream",
    &[("shared.txt", "one\ntheirs\nthree\n"), ("model.bin", &pointer(2))],
  );
  clone.git(&["fetch", "-q", "origin"]);
  let lfs = Lfs::new(&clone);
  lfs.store(1);

  lfs.fetchable(2);
  let round = lfs.run_exiting(&["update", "--no-fetch"], 1);
  assert_eq!(round["state"], json!("conflict"), "{round}");
  lfs.assert_fetched_before_the_lock_at(&upstream_tip, 2);
  assert_eq!(lfs.smudges(), Vec::<String>::new(), "a round moves no file");

  lfs.forget(2);
  lfs.fetchable(2);
  let file = round["session"]["files"]
    .as_array()
    .unwrap()
    .iter()
    .find(|file| file["path"] == json!("shared.txt"))
    .unwrap();
  std::fs::write(file["edit"].as_str().unwrap(), "one\ntheirs and mine\nthree\n").unwrap();
  let done = lfs.run(&["update", "--continue", "--round", "1"]);
  assert_eq!(done["state"], json!("rebase"), "{done}");
  lfs.assert_fetched_before_the_lock_at(&upstream_tip, 2);
  assert_eq!(lfs.smudges(), ["locked store"], "{:#?}", lfs.wrapped.commands());
  assert_eq!(model(&clone), pointer(2));
}

/// A store that `lfs.storage` keeps away from `$GIT_COMMON_DIR/lfs` is checked where the config that brd reads anyway
/// puts it (`ConfigSnapshot`, a command without `--prefix`), with no process more, so the drop fetches what that store
/// lacks before the lock
#[test_log::test]
fn a_store_that_lfs_storage_moves_is_checked_where_the_config_puts_it() {
  let _hermetic = Hermetic::new();
  let (repo, lfs, commit) = feature_changes_the_model_in("store");
  append_config(repo.path(), &[("lfs.storage", "store"), ("branchdeck.branchPrefix", "test-user")]);
  assert!(!repo.path().join(".git").join("lfs").exists());
  lfs.fetchable(1);
  let _ = std::fs::remove_file(lfs.wrapped.dir().join("log"));
  let (exit_code, output) = run_json(&lfs.wrapped.environment, &["drop", &commit]);
  assert_eq!(exit_code, 0, "{output}");
  lfs.assert_fetched_before_the_lock(&repo, 1);
  assert_eq!(lfs.smudges(), ["locked store"], "{:#?}", lfs.wrapped.commands());
}
