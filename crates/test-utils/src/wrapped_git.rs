//! A git with a hook before each command, for a test of what another writer does in the middle of an operation: a
//! `GitCommandExecutor` whose git is a script that runs `case " $* " in` arms before the real git.

use crate::fake_executable::{fake_executable, shell_path};
use git_executor::git_command_executor::GitCommandExecutor;
use git_executor::git_info::GitInfo;
use tempfile::TempDir;

/// The wrapper and its directory, where its arms keep their marks (`{dir}`)
pub struct WrappedGit {
  directory: TempDir,
  executor: GitCommandExecutor,
}

impl WrappedGit {
  /// A git that runs `arms`, `case " $* " in` arms with `{dir}` replaced by the directory of the wrapper, before the
  /// real git: `*" fast-import --quiet --done ")` matches the arguments of one command. An arm runs the real git as
  /// `git`, with the environment of the command (`GIT_DIR` and the like are stripped by the executor)
  pub fn new(arms: &str) -> Self {
    let directory = tempfile::tempdir().unwrap();
    let dir = shell_path(directory.path());
    let script = fake_executable(directory.path(), "git", &format!("case \" $* \" in {}\nesac\nexec git \"$@\"", arms.replace("{dir}", &dir)));
    let version = GitCommandExecutor::new().get_info().unwrap().version.clone();
    let executor = GitCommandExecutor::with_info(GitInfo {
      version,
      path: script.to_str().unwrap().to_string(),
    });
    Self { directory, executor }
  }

  pub const fn executor(&self) -> &GitCommandExecutor {
    &self.executor
  }

  /// Whether an arm left the mark `name` in the directory of the wrapper (`touch '{dir}/name'`)
  pub fn marked(&self, name: &str) -> bool {
    self.directory.path().join(name).exists()
  }
}
