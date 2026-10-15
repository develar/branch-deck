use crate::git_test_utils::git_command;
use crate::repo_template::{RepoTemplate, templates};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn git_output(dir: &Path, args: &[&str]) -> std::process::Output {
  git_command().args(args).current_dir(dir).output().unwrap()
}

/// Fixtures built under `git rebase --exec` (`GIT_DIR` and `GIT_INDEX_FILE` of the developer's clone in the
/// environment) go into their own directories and leave the clone alone; the bare template once set `core.bare` there
#[test]
fn fixtures_ignore_the_repository_of_the_environment() {
  let clone = TempDir::new().unwrap();
  assert!(git_output(clone.path(), &["init", "-q"]).status.success());
  let git_dir = clone.path().join(".git");
  let config_before = fs::read_to_string(git_dir.join("config")).unwrap();

  let fixtures = TempDir::new().unwrap();
  // SAFETY: nextest runs every test in its own process, no other thread reads the environment
  unsafe {
    std::env::set_var("GIT_DIR", &git_dir);
    std::env::set_var("GIT_INDEX_FILE", git_dir.join("index"));
  }
  let built = RepoTemplate::new("leak")
    .branch_prefix("user-name")
    .commit("first", &[("a.txt", "a")])
    .build(&fixtures.path().join("work"))
    .and_then(|()| templates::bare().build(&fixtures.path().join("bare")));
  unsafe {
    std::env::remove_var("GIT_DIR");
    std::env::remove_var("GIT_INDEX_FILE");
  }
  built.unwrap();

  assert_eq!(fs::read_to_string(git_dir.join("config")).unwrap(), config_before);
  assert!(
    !git_output(clone.path(), &["rev-parse", "--verify", "-q", "HEAD"]).status.success(),
    "the clone got a commit"
  );
  let subjects = git_output(&fixtures.path().join("work"), &["log", "--format=%s"]);
  assert_eq!(String::from_utf8(subjects.stdout).unwrap(), "first\n");
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
  for entry in fs::read_dir(dir).unwrap().flatten() {
    let path = entry.path();
    if path.is_dir() {
      if path.file_name().is_some_and(|name| name != "target" && name != "node_modules") {
        collect_rust_files(&path, files);
      }
    } else if path.extension().is_some_and(|e| e == "rs") {
      files.push(path);
    }
  }
}

/// Test code spawns git through `git_command` (or `GitCommandExecutor`), so an inherited `GIT_DIR` can't redirect it
#[test]
fn test_code_spawns_git_only_through_git_command() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
  let mut files = Vec::new();
  for dir in ["crates", "src-tauri/src"] {
    collect_rust_files(&root.join(dir), &mut files);
  }
  let raw_spawn = concat!("Command::new(", "\"git\")");
  let offenders: Vec<_> = files
    .iter()
    .filter(|path| !path.ends_with("crates/test-utils/src/git_test_utils.rs"))
    .filter(|path| fs::read_to_string(path).unwrap().contains(raw_spawn))
    .map(|path| path.strip_prefix(root).unwrap().display().to_string())
    .collect();
  assert!(offenders.is_empty(), "use test_utils::git_test_utils::git_command instead of {raw_spawn} in {offenders:?}");
}
