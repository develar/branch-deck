//! The files of Git LFS of a new slot: its checkout under the clones lock writes their pointers, so no smudge filter
//! waits for the network while every get that adds a slot waits for the lock, and `git lfs pull` writes their content
//! after the release. A fake smudge filter and a fake `git lfs pull` log which locks of the pool were held as they ran:
//! a lock is held while its holder record (`<lock>.holder`) exists, written before the file lock counts as taken and
//! deleted before the release

use crate::clones_lock_test::kinds;
use crate::lease_command::{LeaseContext, LeaseGetArgs, LeaseGetOutput, lease_get_reporting};
use crate::lease_test::{Fixture, path_of};
use bd_core::context::Environment;
use bd_core::error::CliError;
use git_ops::lease_slot::STATE_DIR;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tempfile::TempDir;
use test_utils::fake_executable::{fake_executable, shell_path};
use test_utils::git_test_utils::append_config;
use test_utils::hermetic_git::HermeticGit;

/// The blob of the file of Git LFS, as the commit holds it: what the checkout of a new slot writes
const POINTER: &str = "the pointer of the model\n";

/// A repository whose `*.bin` files go through the filter `lfs`, as `git lfs install` configures it in the repository:
/// its smudge command a fake that logs `smudge` with the locks of the pool held as it ran, then passes the blob through.
/// brd's git is a wrapper whose `git lfs pull` logs `pull` with those locks instead, and fails while the file `fail`
/// exists, and whose `git lfs checkout` logs `checkout` with them; it runs the real git for the rest
struct LfsPool {
  fixture: Fixture,
  directory: TempDir,
  git: PathBuf,
  _hermetic: HermeticGit,
}

impl LfsPool {
  fn new() -> Self {
    // the filter of a `git lfs install` of the developer or of the runner stays out of every git of the test: brd's
    // reads the global config of the fixture already
    let hermetic = HermeticGit::new();
    let fixture = Fixture::new();
    let repo = &fixture.repo;
    let directory = tempfile::tempdir().unwrap();
    let log = shell_path(&directory.path().join("log"));
    let fail = shell_path(&directory.path().join("fail"));
    let state = shell_path(&repo.path().join(".git").join(STATE_DIR));
    let held = format!("held=; for lock in clones worktrees leases; do [ -f '{state}/'$lock'.lock.holder' ] && held=\"$held $lock\"; done");
    let smudge = fake_executable(directory.path(), "smudge", &format!("{held}\necho \"smudge$held\" >> '{log}'\nexec cat"));
    let git = fake_executable(
      directory.path(),
      "git",
      &format!(
        "case \" $* \" in\n*\" lfs pull \"*) {held}; echo \"pull$held\" >> '{log}'; [ ! -f '{fail}' ];;\n*\" lfs checkout \"*) {held}; echo \"checkout$held\" >> '{log}';;\n*) exec git \"$@\";;\nesac"
      ),
    );
    repo.create_commit_with_files("Add a model", &[(".gitattributes", "*.bin filter=lfs\n"), ("model.bin", POINTER)]);
    // appended, not a process per key
    let mut config = std::fs::OpenOptions::new().append(true).open(repo.path().join(".git").join("config")).unwrap();
    writeln!(config, "[filter \"lfs\"]\n\tclean = cat\n\tsmudge = {}\n\trequired = true", shell_path(&smudge)).unwrap();
    // the template of a clone (macOS) differs in the model, so the checkout of the new slot writes it, as the checkout of
    // a new slot without a clone does
    std::fs::write(repo.path().join("model.bin"), "edited in the source\n").unwrap();
    // the store of Git LFS, which keeps the objects of the repository
    std::fs::create_dir_all(repo.path().join(".git").join("lfs").join("objects")).unwrap();
    Self {
      fixture,
      directory,
      git,
      _hermetic: hermetic,
    }
  }

  fn environment(&self) -> Environment {
    let mut environment = self.fixture.environment(self.fixture.repo.path());
    environment.git = Some(self.git.to_str().unwrap().to_string());
    environment
  }

  /// `brd lease get --holder <holder>` with the wrapped git, and the kind of each of its steps
  fn get(&self, holder: &str) -> (Result<LeaseGetOutput, CliError>, Vec<&'static str>) {
    let environment = self.environment();
    let context = LeaseContext::resolve(&environment, None).unwrap();
    let args = LeaseGetArgs {
      holder: Some(holder.to_string()),
      ..LeaseGetArgs::default()
    };
    let steps = Mutex::new(Vec::new());
    let got = lease_get_reporting(&context, &environment, &args, &|step| steps.lock().unwrap().push(step));
    (got, kinds(&steps))
  }

  /// The path of a new slot of `brd lease get`, whose checkout wrote the pointer of the model and whose last step was the
  /// one `git lfs pull`, which writes nothing here
  fn new_slot_with_its_pointers(&self) -> String {
    self.new_slot_logging(&["pull"])
  }

  /// `new_slot_with_its_pointers`, the fakes having logged `log`
  fn new_slot_logging(&self, log: &[&str]) -> String {
    let (got, steps) = self.get("first");
    let got = serde_json::to_value(got.unwrap()).unwrap();
    assert_eq!(got["reused"], json!(false), "{got}");
    assert_eq!(
      (self.log(), steps.last().copied()),
      (log.iter().map(|line| (*line).to_string()).collect(), Some("lfs pull")),
      "{got}"
    );
    let slot = path_of(&got);
    assert_eq!(std::fs::read_to_string(Path::new(&slot).join("model.bin")).unwrap(), POINTER);
    slot
  }

  /// What the fakes logged, in order: `smudge` or `pull`, then each lock of the pool held as it ran
  fn log(&self) -> Vec<String> {
    std::fs::read_to_string(self.directory.path().join("log"))
      .unwrap_or_default()
      .lines()
      .map(str::to_string)
      .collect()
  }
}

/// A new slot is checked out under the clones lock with the pointers of its files of Git LFS: no smudge filter runs
/// there, nor under the lock of the pool's `worktree` commands (the registration of a slot checks nothing out) or the
/// pool lock, and `git lfs pull` writes the content once the get released the clones lock, before the seed. A reused
/// slot moves without a lock of the pool: the smudge filter runs in its move, and no pull follows
#[test_log::test]
fn a_new_slot_gets_the_content_of_its_lfs_files_after_the_clones_lock() {
  let pool = LfsPool::new();
  let slot = pool.new_slot_with_its_pointers();

  pool.fixture.give_back(&slot);
  pool
    .fixture
    .repo
    .create_commit_with_files("Change the model", &[("model.bin", "the pointer of the next model\n")]);
  let (reused, steps) = pool.get("second");
  let reused = serde_json::to_value(reused.unwrap()).unwrap();
  assert_eq!((path_of(&reused), &reused["reused"]), (slot, &json!(true)), "{reused}");
  assert_eq!(pool.log(), ["pull", "smudge"]);
  assert_eq!(steps, ["reserve", "status", "checkout"]);
}

/// A new slot whose `git lfs pull` failed has the pointers of its files of Git LFS still, which a later move by the diff
/// would keep as their content: the get fails, and the slot goes back unfinished and without its index, so no get takes
/// it, and the next one removes it before it adds another
#[test_log::test]
fn a_new_slot_whose_lfs_pull_failed_is_never_reused() {
  let pool = LfsPool::new();
  let fail = pool.directory.path().join("fail");
  std::fs::write(&fail, "").unwrap();
  let error = pool.get("failed").0.unwrap_err();
  assert!(format!("{error:?}").contains("Cannot pull the files of Git LFS"), "{error:?}");
  std::fs::remove_file(&fail).unwrap();

  // the failed get gave the slot back unfinished: the next get removes it and adds another, never moves it by the diff
  let got: Value = serde_json::to_value(pool.get("next").0.unwrap()).unwrap();
  assert_eq!(got["reused"], json!(false), "{got}");
  assert_eq!(got.get("skipped"), None, "{got}");
  assert_eq!(got["removed_unfinished"].as_array().map(Vec::len), Some(1), "{got}");
  assert_eq!(pool.log(), ["pull", "pull"]);
}

/// A repository whose filter of Git LFS is configured but that keeps no objects of LFS (Git for Windows configures the
/// filter in its system config for every repository): the checkout of a new slot runs no filter of LFS, and no pull
/// follows
#[test_log::test]
fn a_repository_without_objects_of_git_lfs_pulls_nothing() {
  let pool = LfsPool::new();
  std::fs::remove_dir_all(pool.fixture.repo.path().join(".git").join("lfs")).unwrap();
  let (got, steps) = pool.get("first");
  let got = serde_json::to_value(got.unwrap()).unwrap();
  assert_eq!(got["reused"], json!(false), "{got}");
  assert_eq!(pool.log(), Vec::<String>::new());
  assert!(!steps.contains(&"lfs pull"), "{steps:?}");
  assert_eq!(std::fs::read_to_string(Path::new(&path_of(&got)).join("model.bin")).unwrap(), POINTER);
}

/// A repository that lives on the pointers of what Git LFS can not download (`lfs.skipdownloaderrors`, as its smudge
/// writes them) keeps them in a new slot whose pull failed: the get goes on, its `git lfs checkout`, no lock of the pool
/// held, writes the content of every object that the pull got or the store had, as a pull that ended early may not, and
/// the slot is reused, not dirty
#[test_log::test]
fn a_repository_that_skips_download_errors_keeps_the_pointers_of_a_failed_pull() {
  let pool = LfsPool::new();
  append_config(pool.fixture.repo.path(), &[("lfs.skipdownloaderrors", "true")]);
  std::fs::write(pool.directory.path().join("fail"), "").unwrap();
  let slot = pool.new_slot_logging(&["pull", "checkout"]);

  pool.fixture.give_back(&slot);
  let reused = serde_json::to_value(pool.get("second").0.unwrap()).unwrap();
  assert_eq!((path_of(&reused), &reused["reused"]), (slot, &json!(true)), "{reused}");
}
