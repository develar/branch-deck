//! A remote whose first fetch waits at the remote until the test lets it go: the upload-pack of a remote of a clone
//! (`remote.<name>.uploadpack`) replaced by a script that counts its runs and holds the first one. A test knows then
//! that a fetch is in flight, holding whatever it took before its transfer (the fetch lock of the baseline), while it
//! starts another fetch beside it, in a thread or in another process, and counts the transfers that reached the remote.

use crate::fake_executable::{fake_executable, shell_path};
use crate::git_test_utils::TestRepo;
use std::fs;
use tempfile::TempDir;

/// What the first run does once the test let it go: serve the fetch
pub const SERVE: &str = "exec git upload-pack \"$@\"";
/// What the first run does once the test let it go: fail the fetch, as a remote that hung up
pub const FAIL: &str = "echo 'the remote hung up' >&2; exit 1";

/// The fake upload-pack and its marks: `runs` (a line per run), `held` (the first run waits), `go` (it goes on)
pub struct HeldUploadPack {
  dir: TempDir,
}

impl HeldUploadPack {
  /// The upload-pack of the remote `remote` of `repo`: its first run waits until `release`, then does `first` (`SERVE`,
  /// `FAIL`); the runs after it serve at once
  ///
  /// # Panics
  /// When the script or the config can't be written
  #[must_use]
  pub fn install(repo: &TestRepo, remote: &str, first: &str) -> Self {
    let dir = tempfile::tempdir().unwrap();
    let mark = |name: &str| shell_path(&dir.path().join(name));
    let body = format!(
      "echo run >> '{runs}'\nif [ ! -f '{held}' ]; then touch '{held}'; until [ -f '{go}' ]; do sleep 0.01; done; {first}; fi\n{SERVE}",
      runs = mark("runs"),
      held = mark("held"),
      go = mark("go"),
    );
    let upload_pack = fake_executable(dir.path(), "upload-pack", &body);
    repo.git(&["config", &format!("remote.{remote}.uploadpack"), &shell_path(&upload_pack)]);
    Self { dir }
  }

  /// How many fetches reached the remote
  #[must_use]
  pub fn runs(&self) -> usize {
    fs::read_to_string(self.dir.path().join("runs")).map_or(0, |runs| runs.lines().count())
  }

  /// Waits until the first run holds, while `running` says the fetch that should reach it still runs
  ///
  /// # Panics
  /// When the fetch ended without reaching the remote
  pub fn wait_held(&self, mut running: impl FnMut() -> bool) {
    let held = self.dir.path().join("held");
    while !held.exists() && running() {
      std::thread::yield_now();
    }
    assert!(held.exists(), "the first fetch reached the remote");
  }

  /// Lets the first run go on; before it reached the remote, the first run does not wait
  ///
  /// # Panics
  /// When the mark can't be written
  pub fn release(&self) {
    fs::write(self.dir.path().join("go"), "").unwrap();
  }
}
