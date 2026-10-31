//! A git that logs every command with the state of the repository lock when it started, for the test of what a core
//! runs under the lock (the lock span, C11 of docs/design/rust-domain-authority.md): a `GitCommandExecutor` whose git is
//! a script that writes `locked` or `free` and the arguments to a log, then runs the real git. The lock is held while
//! its holder record (`$GIT_COMMON_DIR/branch-deck.lock.holder`) exists: every lock writes it before the file lock
//! counts as taken and deletes it before the release.

use crate::fake_executable::{fake_executable, shell_path};
use git_executor::git_command_executor::GitCommandExecutor;
use git_executor::git_info::GitInfo;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// The logging git of one repository and its log
pub struct LockSpan {
  _directory: TempDir,
  log: PathBuf,
  executor: GitCommandExecutor,
}

impl LockSpan {
  /// A git that logs the lock state of the repository whose `$GIT_COMMON_DIR` is `git_common_dir` (`<repo>/.git`)
  ///
  /// # Panics
  /// When the script cannot be written or the real git cannot be found
  #[must_use]
  pub fn new(git_common_dir: &Path) -> Self {
    let directory = tempfile::tempdir().unwrap();
    let log = directory.path().join("log");
    let holder = shell_path(&git_common_dir.join("branch-deck.lock.holder"));
    let body = format!(
      "state=free\n[ -f '{holder}' ] && state=locked\necho \"$state $*\" >> '{}'\nexec git \"$@\"",
      shell_path(&log)
    );
    let script = fake_executable(directory.path(), "git", &body);
    let version = GitCommandExecutor::new().get_info().unwrap().version.clone();
    let executor = GitCommandExecutor::with_info(GitInfo {
      version,
      path: script.to_str().unwrap().to_owned(),
    });
    Self {
      _directory: directory,
      log,
      executor,
    }
  }

  /// The git to run the core with
  #[must_use]
  pub const fn executor(&self) -> &GitCommandExecutor {
    &self.executor
  }

  /// Every command so far, in the order they started: whether the lock was held, and the arguments
  ///
  /// # Panics
  /// When the log cannot be read
  #[must_use]
  pub fn commands(&self) -> Vec<(bool, String)> {
    let text = std::fs::read_to_string(&self.log).unwrap_or_default();
    text
      .lines()
      .filter_map(|line| line.split_once(' ').map(|(state, args)| (state == "locked", args.to_owned())))
      .collect()
  }

  /// The subcommands that started between the first and the last command under the lock, sorted, each once: what the
  /// span of the lock is made of. Empty when no command ran under it
  #[must_use]
  pub fn commands_under_the_lock(&self) -> Vec<String> {
    let commands = self.commands();
    let (Some(first), Some(last)) = (commands.iter().position(|(locked, _)| *locked), commands.iter().rposition(|(locked, _)| *locked)) else {
      return Vec::new();
    };
    let mut under: Vec<String> = commands[first..=last].iter().map(|(_, args)| subcommand(args)).collect();
    under.sort_unstable();
    under.dedup();
    under
  }

  /// The subcommand of every command that started under the lock, sorted, once per run: how many processes of each the
  /// span holds
  #[must_use]
  pub fn runs_under_the_lock(&self) -> Vec<String> {
    let mut runs: Vec<String> = self.commands().into_iter().filter(|(locked, _)| *locked).map(|(_, args)| subcommand(&args)).collect();
    runs.sort_unstable();
    runs
  }

  /// The subcommands that started before the first command under the lock, in order
  #[must_use]
  pub fn commands_before_the_lock(&self) -> Vec<String> {
    let commands = self.commands();
    let first = commands.iter().position(|(locked, _)| *locked).unwrap_or(commands.len());
    commands[..first].iter().map(|(_, args)| subcommand(args)).collect()
  }

  /// Asserts that each of `subcommands` started before the first command under the lock: what a core computes before it
  ///
  /// # Panics
  /// When one did not
  pub fn assert_before_the_lock(&self, subcommands: &[&str]) {
    let before = self.commands_before_the_lock();
    for subcommand in subcommands {
      assert!(before.iter().any(|command| command == subcommand), "{subcommand} before the lock: {:#?}", self.commands());
    }
  }

  /// Asserts that exactly `under` ran under the lock (`commands_under_the_lock`) and each of `after` started after it
  ///
  /// # Panics
  /// When another set ran under the lock, or one of `after` did not start after it
  pub fn assert_span(&self, under: &[&str], after: &[&str]) {
    assert_eq!(self.commands_under_the_lock(), under, "{:#?}", self.commands());
    let later = self.commands_after_the_lock();
    for subcommand in after {
      assert!(later.iter().any(|command| command == subcommand), "{subcommand} after the lock: {:#?}", self.commands());
    }
  }

  /// The subcommands that started after the last command under the lock, in order
  #[must_use]
  pub fn commands_after_the_lock(&self) -> Vec<String> {
    let commands = self.commands();
    commands
      .iter()
      .rposition(|(locked, _)| *locked)
      .map_or_else(Vec::new, |last| commands[last + 1..].iter().map(|(_, args)| subcommand(args)).collect())
  }
}

/// The git subcommand of `args`: the first word that is no option and no `-c` value
fn subcommand(args: &str) -> String {
  args.split(' ').find(|word| !word.starts_with('-') && !word.contains('=')).unwrap_or_default().to_owned()
}
