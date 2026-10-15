use crate::git_command_executor::GitCommandExecutor;
use std::path::Path;
use std::time::Duration;

/// The lock files under `.git` and `.git/refs/heads`
fn lock_files(repo: &Path) -> Vec<String> {
  [repo.join(".git"), repo.join(".git/refs/heads")]
    .iter()
    .flat_map(|dir| std::fs::read_dir(dir).unwrap().flatten())
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .filter(|name| Path::new(name).extension().is_some_and(|extension| extension == "lock"))
    .collect()
}

/// A dropped `update-ref --stdin` after `prepare` holds `HEAD.lock` and the branch lock: dropping the handle must let
/// git abort and remove them, not kill it while it holds them
#[test]
fn a_handle_dropped_after_prepare_leaves_no_lock_files() {
  let directory = std::env::temp_dir().join(format!("interactive-command-{}", std::process::id()));
  std::fs::create_dir_all(&directory).unwrap();
  let repo = directory.to_str().unwrap();
  let git = GitCommandExecutor::new();
  let env = [
    ("GIT_AUTHOR_NAME", "Test"),
    ("GIT_AUTHOR_EMAIL", "test@example.com"),
    ("GIT_COMMITTER_NAME", "Test"),
    ("GIT_COMMITTER_EMAIL", "test@example.com"),
  ];
  git.execute_command(&["init", "-q", "-b", "master"], repo).unwrap();
  for message in ["a", "b"] {
    git.execute_command_with_env(&["commit", "-q", "--allow-empty", "-m", message], repo, &env).unwrap();
  }
  let ids = git.execute_command_lines(&["rev-parse", "HEAD~1", "HEAD"], repo).unwrap();

  let mut update_ref = git.spawn_interactive(&["update-ref", "--stdin"], repo).unwrap();
  update_ref.send("start").unwrap();
  assert_eq!(update_ref.read_line(Duration::from_secs(10)).unwrap(), "start: ok");
  update_ref.send(&format!("update HEAD {} {}", ids[0], ids[1])).unwrap();
  update_ref.send("prepare").unwrap();
  assert_eq!(update_ref.read_line(Duration::from_secs(10)).unwrap(), "prepare: ok");
  assert_eq!(lock_files(&directory).len(), 2, "prepare holds HEAD.lock and master.lock");

  drop(update_ref);
  assert_eq!(lock_files(&directory), Vec::<String>::new());
  assert_eq!(git.execute_command(&["rev-parse", "HEAD"], repo).unwrap(), ids[1], "the transaction was aborted");
  std::fs::remove_dir_all(&directory).unwrap();
}
