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

  /// The body of an arm that runs a `fast-import` (its stream on stdin, ending with `done`) with `other` in the middle:
  /// after git parsed every command but the `done` (a `progress` line it prints once there), before it moves the refs,
  /// as another writer that moves a ref between the `from <ref>^0` of the stream and its update. The exit code and the
  /// output (the marks a `get-mark` asked) are git's
  pub fn during_fast_import(other: &str) -> String {
    format!(
      "cat > '{{dir}}/stream'; sed '$d' '{{dir}}/stream' > '{{dir}}/body'; rm -f '{{dir}}/in' '{{dir}}/out'; mkfifo '{{dir}}/in'; \
       git \"$@\" < '{{dir}}/in' > '{{dir}}/out' & pid=$!; exec 3> '{{dir}}/in'; cat '{{dir}}/body' >&3; echo 'progress parsed' >&3; \
       until grep -q 'parsed' '{{dir}}/out' 2>/dev/null || ! kill -0 $pid 2>/dev/null; do :; done; {other}; echo done >&3; exec 3>&-; wait $pid; code=$?; \
       grep -v '^progress parsed$' '{{dir}}/out'; exit $code"
    )
  }

  pub const fn executor(&self) -> &GitCommandExecutor {
    &self.executor
  }

  /// Whether an arm left the mark `name` in the directory of the wrapper (`touch '{dir}/name'`)
  pub fn marked(&self, name: &str) -> bool {
    self.directory.path().join(name).exists()
  }
}
