use crate::history::{HistoryCommit, HistoryStream, numbered_lines};
use git_executor::git_command_executor::GitCommandExecutor;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

// Constants for test Git user configuration
const TEST_USER_NAME: &str = "Test User";
const TEST_USER_EMAIL: &str = "test@example.com";

/// Config for a fixture repository: no background `git maintenance` or auto gc. A commit or a push would otherwise
/// start one that holds `.git/objects/maintenance.lock`, and a clone of the repository at that moment fails with
/// "failed to copy file to '.git/objects/maintenance.lock'"
pub const NO_BACKGROUND_MAINTENANCE: [(&str, &str); 2] = [("maintenance.auto", "false"), ("gc.auto", "0")];

/// The config of a `TestRepo`: `NO_BACKGROUND_MAINTENANCE`, the test user, `zdiff3` conflicts, no signing (a global
/// signing setup must not make test commits prompt or fail) and no `core.autocrlf` (the system config of Git for
/// Windows sets it to `true`, and the tests compare checked-out text with LF)
const TEST_REPO_CONFIG: [(&str, &str); 7] = [
  NO_BACKGROUND_MAINTENANCE[0],
  NO_BACKGROUND_MAINTENANCE[1],
  ("user.name", TEST_USER_NAME),
  ("user.email", TEST_USER_EMAIL),
  ("merge.conflictstyle", "zdiff3"),
  ("commit.gpgsign", "false"),
  ("core.autocrlf", "false"),
];

/// The repository-local variables of git (`git rev-parse --local-env-vars`). A test run inherits them from
/// `git rebase --exec` (`GIT_DIR`) or a hook (`GIT_INDEX_FILE`), and a fixture command would then write into that
/// repository instead of the fixture: `pnpm e2e` under `rebase --exec` once set `core.bare`, the test user and the
/// template refs in the developer's clone
const REPOSITORY_ENV: [&str; 15] = [
  "GIT_ALTERNATE_OBJECT_DIRECTORIES",
  "GIT_CONFIG",
  "GIT_CONFIG_PARAMETERS",
  "GIT_CONFIG_COUNT",
  "GIT_OBJECT_DIRECTORY",
  "GIT_DIR",
  "GIT_WORK_TREE",
  "GIT_IMPLICIT_WORK_TREE",
  "GIT_GRAFT_FILE",
  "GIT_INDEX_FILE",
  "GIT_NO_REPLACE_OBJECTS",
  "GIT_REPLACE_REF_BASE",
  "GIT_PREFIX",
  "GIT_SHALLOW_FILE",
  "GIT_COMMON_DIR",
];

/// `git` without the inherited `REPOSITORY_ENV`, so it finds the repository from its `current_dir`. Fixture commands
/// start here, never from `Command::new("git")` (`git_command_test` scans for it)
pub fn git_command() -> Command {
  let mut command = Command::new("git");
  for name in REPOSITORY_ENV {
    command.env_remove(name);
  }
  command
}

/// Apply `NO_BACKGROUND_MAINTENANCE` to the repository (bare or not) at `repo_path`, right after it is created or cloned
pub fn disable_background_maintenance(repo_path: &Path) {
  append_config(repo_path, &NO_BACKGROUND_MAINTENANCE);
}

/// Append `entries` (`section.key` or `section.subsection.key`, value) to the config file of the repository (bare or
/// not) at `repo_path`, as `git config <key> <value>` of each would, without a git process per key: a fixture repository
/// is set up by the thousand, and a process costs 5–10 ms on macOS and far more on Windows. Only for a repository no git
/// process uses meanwhile, right after it is created or cloned
///
/// # Panics
/// When `repo_path` has neither `.git/config` nor `config`, or the file cannot be written
pub fn append_config(repo_path: &Path, entries: &[(&str, &str)]) {
  let dot_git = repo_path.join(".git");
  let file = if dot_git.is_dir() { dot_git.join("config") } else { repo_path.join("config") };
  assert!(file.is_file(), "no config file of a repository at {}", repo_path.display());
  let mut text = String::new();
  for (key, value) in entries {
    let (section, name) = key.rsplit_once('.').unwrap_or_else(|| panic!("not a config key: {key}"));
    let header = match section.split_once('.') {
      Some((section, subsection)) => {
        assert!(!subsection.contains('\n'), "a config subsection with a newline: {subsection:?}");
        format!("[{section} \"{}\"]", quoted(subsection))
      }
      None => format!("[{section}]"),
    };
    // a value may have lines (a branch description): `\n` inside the quotes, as `git config` writes it
    let _ = writeln!(text, "{header}\n\t{name} = \"{}\"", quoted(value).replace('\n', "\\n"));
  }
  let mut config = fs::OpenOptions::new()
    .append(true)
    .open(&file)
    .unwrap_or_else(|e| panic!("cannot open {}: {e}", file.display()));
  std::io::Write::write_all(&mut config, text.as_bytes()).unwrap_or_else(|e| panic!("cannot write {}: {e}", file.display()));
}

/// `text` inside the double quotes of a config value or subsection, its backslashes and quotes escaped
fn quoted(text: &str) -> String {
  text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// A repository whose initial commit is the baseline; `stacks` turns on stacked branches
///
/// # Panics
/// When git cannot write the config
#[must_use]
pub fn baseline_repo(stacks: bool) -> TestRepo {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  if stacks {
    repo.set_config("branchdeck.stacks", "true").unwrap();
  }
  repo
}

/// A repository on a bare `origin` (`.git/origin.git`, no background maintenance) that has only its `master`, with
/// `alpha` and `beta` built on it committed ahead and their virtual branches (`test/virtual/…`) at their commits: what
/// the tests of the app's stack push push
pub fn stack_ahead_of_bare_origin() -> TestRepo {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test Project\n");
  let origin = repo.path().join(".git/origin.git");
  let origin = origin.to_str().expect("a UTF-8 path");
  repo.git(&["init", "--bare", "-q", "--initial-branch=master", origin]);
  disable_background_maintenance(Path::new(origin));
  repo.git(&["remote", "add", "origin", origin]);
  repo.git(&["push", "-q", "origin", "master"]);
  let alpha = repo.create_commit("(alpha) Alpha", "alpha.txt", "alpha\n");
  let beta = repo.create_commit("(alpha/beta) Beta", "beta.txt", "beta\n");
  repo.git(&["update-ref", "refs/heads/test/virtual/alpha", &alpha]);
  repo.git(&["update-ref", "refs/heads/test/virtual/beta", &beta]);
  repo
}

/// A `pre-receive` hook in the bare repository at `bare_path` that refuses every push, as a protected branch does
pub fn refuse_every_push(bare_path: &Path) {
  pre_receive_hook(bare_path, "echo 'protected branch' >&2\nexit 1");
}

/// The `pre-receive` hook `script` (sh) in the bare repository at `bare_path`: it runs while a push to it goes
pub fn pre_receive_hook(bare_path: &Path, script: &str) {
  let hook = bare_path.join("hooks/pre-receive");
  fs::write(&hook, format!("#!/bin/sh\n{script}\n")).unwrap();
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
  }
}

/// The directory `path` as git prints it (`rev-parse --show-toplevel`, `%(worktreepath)`): canonical, because git
/// resolves symlinks (the temporary directory `/var/…` of macOS is `/private/var/…`), and on Windows in git's form
/// (`windows_git_form`), not the verbatim `\\?\C:\…` that `canonicalize` gives
pub fn git_canonical_path(path: &Path) -> String {
  let canonical = fs::canonicalize(path).unwrap_or_else(|e| panic!("cannot canonicalize {}: {e}", path.display()));
  let text = canonical.to_str().unwrap_or_else(|| panic!("not UTF-8: {}", canonical.display()));
  if cfg!(windows) { windows_git_form(text) } else { text.to_string() }
}

/// A Windows path as Git for Windows prints it: without the verbatim prefix (`\\?\C:\…` is `C:/…`,
/// `\\?\UNC\server\…` is `//server/…`) and with `/`
fn windows_git_form(text: &str) -> String {
  let plain = text
    .strip_prefix(r"\\?\UNC\")
    .map_or_else(|| text.strip_prefix(r"\\?\").unwrap_or(text).to_string(), |share| format!(r"\\{share}"));
  plain.replace('\\', "/")
}

/// Commits `files` onto HEAD of the repository at `repo_path` as a commit in a terminal would (`git add`, `git commit`),
/// authored and committed at `timestamp` (Unix seconds): lets the E2E mock server change a repository behind the app's
/// back. Returns the new commit id.
pub fn commit_files_on_head(repo_path: &Path, message: &str, files: &[(&str, &str)], timestamp: i64) -> Result<String, String> {
  let git = GitCommandExecutor::new();
  let path = repo_path.to_str().ok_or("the repository path is not UTF-8")?;
  let mut add_args = vec!["add", "--"];
  for (name, content) in files {
    let file = repo_path.join(name);
    if let Some(parent) = file.parent() {
      fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&file, content).map_err(|e| e.to_string())?;
    add_args.push(name);
  }
  git.execute_command(&add_args, path).map_err(|e| e.to_string())?;
  let date = format!("{timestamp} +0000");
  commit_in(&git, path, &["-m", message], &[("GIT_AUTHOR_DATE", date.as_str()), ("GIT_COMMITTER_DATE", date.as_str())])
}

/// The bare remote of its own a fixture repository gets for `push_from_elsewhere` (and the templates of Update Project
/// have), relative to the repository
pub const OWN_REMOTE: &str = ".git/upstream.git";

/// Someone else pushes a commit of `file` with `content` onto `branch` (a full name, `user-name/virtual/auth`) of the
/// remote of the repository at `repo_path`, from a clone of their own, as `author` (`Name <email>`) at `timestamp`: lets
/// the E2E mock server put a colleague's commit on a pushed branch, which this clone has not fetched. A template's
/// `origin` is the repository itself (`.`), whose branches a sync rewrites, so the first call gives it a bare remote of
/// its own (`.git/upstream.git`) with what this clone has of `origin` and the branch as it is here, its remote-tracking
/// ref at it. Returns the id of the pushed commit
pub fn push_from_elsewhere(repo_path: &Path, branch: &str, author: &str, file: &str, content: &str, timestamp: i64) -> Result<String, String> {
  let git = GitCommandExecutor::new();
  let path = repo_path.to_str().ok_or("the repository path is not UTF-8")?;
  let (name, email) = author
    .strip_suffix('>')
    .and_then(|author| author.split_once(" <"))
    .ok_or_else(|| format!("the author is not `Name <email>`: {author}"))?;
  let upstream = repo_path.join(OWN_REMOTE);
  let upstream_path = upstream.to_str().ok_or("the upstream path is not UTF-8")?;
  let run = |args: &[&str]| git.execute_command(args, path).map(drop).map_err(|e| e.to_string());
  if !upstream.exists() {
    run(&["init", "-q", "--bare", "--initial-branch=master", upstream_path])?;
    disable_background_maintenance(&upstream);
    run(&["push", "-q", upstream_path, "refs/remotes/origin/*:refs/heads/*"])?;
    run(&["config", "remote.origin.url", upstream_path])?;
  }
  let branch_ref = format!("refs/heads/{branch}");
  let tracking = format!("+{branch_ref}:refs/remotes/origin/{branch}");
  let (_, missing) = git
    .execute_command_with_status(&["ls-remote", "--exit-code", upstream_path, &branch_ref], path)
    .map_err(|e| e.to_string())?;
  if missing != 0 {
    run(&["push", "-q", upstream_path, &format!("{branch_ref}:{branch_ref}")])?;
  }
  run(&["fetch", "-q", "origin", &tracking])?;

  let other = TestRepo::new_empty();
  other.clone_from(&upstream)?;
  other.checkout(branch)?;
  other.set_config("user.name", name)?;
  other.set_config("user.email", email)?;
  let id = other.create_commit_with_timestamp(&format!("Change {file}"), file, content, Some(timestamp));
  other.push("origin", branch)?;
  Ok(id)
}

/// Runs `git commit` with `args` and `env` in `path` and returns the id of the new commit, which git prints in its
/// summary line (in full with `core.abbrev=no`): no `rev-parse HEAD` after each commit of a fixture. Never `--quiet`,
/// which prints no summary
fn commit_in(git: &GitCommandExecutor, path: &str, args: &[&str], env: &[(&str, &str)]) -> Result<String, String> {
  let mut all_args = vec!["-c", "core.abbrev=no", "commit"];
  all_args.extend_from_slice(args);
  let output = git.cmd(&all_args, path).env(env).text().map_err(|e| e.to_string())?;
  commit_id_of_summary(&output).ok_or_else(|| format!("no commit id in the output of git commit: {output}"))
}

/// The full commit id of the summary line `git commit` prints, `[<branch>[ (root-commit)] <id>] <subject>`: the branch
/// and the root-commit mark may be translated, the id is never abbreviated with `core.abbrev=no`
pub(crate) fn commit_id_of_summary(output: &str) -> Option<String> {
  let summary = output.lines().find_map(|line| line.strip_prefix('['))?;
  let is_id = |word: &&str| matches!(word.len(), 40 | 64) && word.bytes().all(|byte| byte.is_ascii_hexdigit());
  summary.split([' ', ']']).find(is_id).map(str::to_string)
}

/// Git test repository wrapper with helper methods
pub struct TestRepo {
  dir: TempDir,
  git_executor: GitCommandExecutor,
}

impl Default for TestRepo {
  fn default() -> Self {
    Self::new()
  }
}

impl TestRepo {
  /// Creates a new test repository
  pub fn new() -> Self {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path();
    let git_executor = GitCommandExecutor::new();

    let repo_path = repo_path.to_str().unwrap();

    // Pin the initial branch: tests assume `master` regardless of the user's `init.defaultBranch`. No template: the
    // sample hooks, `description` and `info/exclude` are 17 files written and deleted per repository, used by no test
    git_executor
      .execute_command(&["init", "--initial-branch=master", "--template="], repo_path)
      .unwrap_or_else(|e| panic!("Git init failed: {e}"));
    append_config(dir.path(), &TEST_REPO_CONFIG);

    Self { dir, git_executor }
  }

  /// A repository whose `master` has `commits`, written by one `fast-import` and checked out by one `read-tree`
  /// (`history`): two processes for any number of commits, where `create_commit` costs two per commit. The ids of the
  /// commits, in order
  ///
  /// # Panics
  /// When git refuses the history, or a commit changes nothing
  pub fn with_history(commits: &[HistoryCommit]) -> (Self, Vec<String>) {
    let repo = Self::new();
    let mut history = HistoryStream::new("refs/heads/master");
    for commit in commits {
      history.commit(commit);
    }
    let ids = history.write(repo.path()).unwrap_or_else(|e| panic!("cannot write the history: {e:#}"));
    let ids = ids.into_iter().map(|id| id.expect("every commit of a history changes a file")).collect();
    (repo, ids)
  }

  /// Commits `commits` on top of `branch` (`refs/heads/master`) with one `fast-import`, then checks HEAD out with one
  /// `read-tree` (`history`): what `create_commit` of each does in two processes per commit. The ids of the commits
  ///
  /// # Panics
  /// When git refuses the history
  pub fn commit_history(&self, branch: &str, commits: &[HistoryCommit]) -> Vec<String> {
    let mut history = HistoryStream::on_branch(branch);
    for commit in commits {
      history.commit(commit);
    }
    let ids = history.write(self.path()).unwrap_or_else(|e| panic!("cannot write the history: {e:#}"));
    ids.into_iter().map(|id| id.expect("a commit of a continued history is always written")).collect()
  }

  /// Creates an empty temporary directory without initializing git
  /// Useful for cloning into
  pub fn new_empty() -> Self {
    let dir = tempfile::tempdir().unwrap();
    let git_executor = GitCommandExecutor::new();
    Self { dir, git_executor }
  }

  /// Get the repository path
  pub fn path(&self) -> &Path {
    self.dir.path()
  }

  /// Get the repository path as a string
  pub fn path_str(&self) -> &str {
    self.dir.path().to_str().unwrap()
  }

  /// Runs git in the repository; panics on a failure
  pub fn git(&self, args: &[&str]) -> String {
    self.git_executor.execute_command(args, self.path_str()).unwrap()
  }

  /// Get a reference to the git executor
  pub const fn git_executor(&self) -> &GitCommandExecutor {
    &self.git_executor
  }

  /// Runs `git commit` with `args` and `env` and returns the id of the commit (`commit_in`)
  ///
  /// # Panics
  /// When the commit fails or its output names no full commit id
  pub fn commit_with(&self, args: &[&str], env: &[(&str, &str)]) -> String {
    commit_in(&self.git_executor, self.path_str(), args, env).unwrap_or_else(|e| panic!("Git commit failed: {e}"))
  }

  /// Creates a commit with a file
  pub fn create_commit(&self, message: &str, filename: &str, content: &str) -> String {
    self.create_commit_with_timestamp(message, filename, content, None)
  }

  /// Write `content` to `filename` in the working tree, creating its directories
  fn write_test_file(&self, filename: &str, content: &str) {
    let file_path = self.path().join(filename);
    if let Some(parent) = file_path.parent() {
      fs::create_dir_all(parent).unwrap();
    }
    fs::write(&file_path, content).unwrap();
  }

  /// Creates a commit with a file and fixed timestamp
  pub fn create_commit_with_timestamp(&self, message: &str, filename: &str, content: &str, timestamp: Option<i64>) -> String {
    self.write_test_file(filename, content);

    // Add to git
    self
      .git_executor
      .execute_command(&["add", filename], self.path_str())
      .unwrap_or_else(|e| panic!("Git add failed: {e}"));

    // Commit with optional fixed timestamp
    let date = timestamp.map(|ts| format!("{ts} +0000"));
    let env: Vec<(&str, &str)> = date
      .iter()
      .flat_map(|date| [("GIT_AUTHOR_DATE", date.as_str()), ("GIT_COMMITTER_DATE", date.as_str())])
      .collect();
    let args: &[&str] = if message.is_empty() { &["--allow-empty-message", "-m", ""] } else { &["-m", message] };
    self.commit_with(args, &env)
  }

  /// Creates a branch pointing to the current HEAD
  pub fn create_branch(&self, branch_name: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["branch", branch_name], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Creates a branch pointing to a specific commit
  pub fn create_branch_at(&self, branch_name: &str, commit_hash: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["branch", branch_name, commit_hash], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Checkout a branch or commit
  pub fn checkout(&self, ref_name: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["checkout", ref_name], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Hard reset to a commit
  pub fn reset_hard(&self, commit_hash: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["reset", "--hard", commit_hash], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Get the current HEAD commit hash
  pub fn head(&self) -> String {
    self.git_executor.execute_command(&["rev-parse", "HEAD"], self.path_str()).unwrap().trim().to_string()
  }

  /// Get the commit hash of a reference
  pub fn rev_parse(&self, ref_name: &str) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["rev-parse", ref_name], self.path_str())
      .map(|output| output.trim().to_string())
      .map_err(|e| e.to_string())
  }

  /// Creates a commit with multiple files
  pub fn create_commit_with_files(&self, message: &str, files: &[(&str, &str)]) -> String {
    let mut add_args = Vec::with_capacity(files.len() + 2);
    add_args.extend(["add", "--"]);
    for (filename, content) in files {
      self.write_test_file(filename, content);
      add_args.push(filename);
    }
    self.git_executor.execute_command(&add_args, self.path_str()).unwrap();
    self.commit_with(&["-m", message], &[])
  }

  /// Commits the index with `files` added to it, written to the index only, never to the working tree, so that a name
  /// the file system refuses (`*`, `:`, `"`, a tab or a newline on Windows) can be committed
  pub fn create_commit_in_index(&self, message: &str, files: &[(&str, &str)]) -> String {
    if cfg!(windows) {
      // Git for Windows refuses such a name in the index ("Invalid path": `core.protectNTFS`, on by default), while a
      // commit made on Linux has it. Off in this repository's own config, not only for the two commands below: a later
      // checkout or merge of the fixture rebuilds the index and checks every name in it again (it keeps the entry and
      // never writes the file), and so does a git of the code under test that writes an index. Never in `new`: off, it
      // also lifts git's checks of the NTFS aliases of `.git` that the other tests keep
      self.git(&["config", "core.protectNTFS", "false"]);
    }
    for (filename, content) in files {
      let blob = self
        .git_executor
        .cmd(&["hash-object", "-w", "--stdin"], self.path_str())
        .stdin(content.as_bytes())
        .text()
        .unwrap();
      self.git(&["update-index", "--add", "--cacheinfo", &format!("100644,{blob},{filename}")]);
    }
    self.commit_with(&["-m", message], &[])
  }

  /// Set config value
  pub fn set_config(&self, key: &str, value: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["config", key, value], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Check if branch exists
  pub fn branch_exists(&self, branch_name: &str) -> bool {
    let ref_path = format!("refs/heads/{branch_name}");
    self
      .git_executor
      .execute_command_with_status(&["show-ref", "--verify", "--quiet", &ref_path], self.path_str())
      .is_ok_and(|(_, exit_code)| exit_code == 0)
  }

  /// Get list of files in a commit
  pub fn get_files_in_commit(&self, commit_hash: &str) -> Result<Vec<String>, String> {
    self
      .git_executor
      .execute_command_lines(&["ls-tree", "-r", "--name-only", commit_hash], self.path_str())
      .map_err(|e| e.to_string())
  }

  /// Get the last N commit messages from HEAD
  pub fn get_commit_messages(&self, count: usize) -> Vec<String> {
    let count_arg = format!("-{count}");
    self
      .git_executor
      .execute_command_lines(&["log", &count_arg, "--pretty=format:%s"], self.path_str())
      .unwrap_or_default()
  }

  /// Cherry-pick a commit and return the new commit hash
  pub fn cherry_pick(&self, commit: &str) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["cherry-pick", commit], self.path_str())
      .map(|_| self.head())
      .map_err(|e| e.to_string())
  }

  /// Rebase current branch onto another branch
  pub fn rebase(&self, onto: &str) -> Result<(), String> {
    self.git_executor.execute_command(&["rebase", onto], self.path_str()).map(|_| ()).map_err(|e| e.to_string())
  }

  /// Merge a branch with --no-ff
  pub fn merge_no_ff(&self, branch: &str, message: &str) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["merge", "--no-ff", branch, "-m", message], self.path_str())
      .map(|_| self.head())
      .map_err(|e| e.to_string())
  }

  /// Merge a branch with --no-ff and specific timestamp
  pub fn merge_no_ff_with_timestamp(&self, branch: &str, message: &str, timestamp: i64) -> Result<String, String> {
    let date_str = format!("{timestamp} +0000");
    let env_vars = vec![("GIT_COMMITTER_DATE", date_str.as_str())];

    self
      .git_executor
      .cmd(&["merge", "--no-ff", branch, "-m", message], self.path_str())
      .env(&env_vars)
      .text()
      .map(|_| self.head())
      .map_err(|e| e.to_string())
  }

  /// Squash merge a branch (creates a single commit with all changes)
  pub fn merge_squash(&self, branch: &str, message: &str) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["merge", "--squash", branch], self.path_str())
      .map_err(|e| e.to_string())?;

    // After squash merge, we need to commit
    self
      .git_executor
      .execute_command(&["commit", "-m", message], self.path_str())
      .map(|_| self.head())
      .map_err(|e| e.to_string())
  }

  /// Squash merge a branch with specific timestamp
  pub fn merge_squash_with_timestamp(&self, branch: &str, message: &str, timestamp: i64) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["merge", "--squash", branch], self.path_str())
      .map_err(|e| e.to_string())?;

    // After squash merge, we need to commit with timestamp
    let date_str = format!("{timestamp} +0000");
    let env_vars = vec![("GIT_COMMITTER_DATE", date_str.as_str())];

    self
      .git_executor
      .cmd(&["commit", "-m", message], self.path_str())
      .env(&env_vars)
      .text()
      .map(|_| self.head())
      .map_err(|e| e.to_string())
  }

  /// Get the committer timestamp of a commit
  pub fn get_commit_timestamp(&self, commit: &str) -> Result<u32, String> {
    self
      .git_executor
      .execute_command(&["show", "-s", "--format=%ct", commit], self.path_str())
      .map_err(|e| e.to_string())
      .and_then(|output| output.trim().parse::<u32>().map_err(|e| format!("Failed to parse timestamp: {e}")))
  }

  /// Delete a branch
  pub fn delete_branch(&self, branch: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["branch", "-D", branch], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// An upstream repository with one commit of `files`, and a clone of it whose `master` tracks `origin/master`
  pub fn upstream_and_clone(files: &[(&str, &str)]) -> (Self, Self) {
    let upstream = Self::new();
    upstream.create_commit_with_files("Initial", files);
    let clone = Self::new_empty();
    clone.clone_from(upstream.path()).unwrap();
    (upstream, clone)
  }

  /// `upstream_and_clone` with local commits that landed upstream in another order (spec/repository/landed-commits.spec.md,
  /// "The proof"): `f.txt` after an upstream change of a line of its context, so its cherry-pick writes the upstream
  /// commit's tree; with `fixup`, a local fixup of it; `g.txt` after an upstream change of the line next to it, so its
  /// cherry-pick conflicts and only the changed lines are the same; then a real conflict of `other.txt`. The ids of the
  /// local commits and of the upstream ones, oldest first
  pub fn upstream_and_clone_with_twins(fixup: bool) -> (Self, Self, Vec<String>, Vec<String>) {
    let (upstream, clone) = Self::upstream_and_clone(&[("f.txt", &numbered_lines(&[])), ("g.txt", &numbered_lines(&[])), ("other.txt", "other\n")]);
    let change = |message: &str, file: &str, changes: &[(usize, &str)], time: i64| HistoryCommit::new(message, &[(file, &numbered_lines(changes))]).at(time);
    let mut local = vec![change("(auth) Change line 5", "f.txt", &[(5, "five")], 1_700_000_200)];
    if fixup {
      local.push(change("fixup! (auth) Change line 5", "f.txt", &[(5, "five, fixed")], 1_700_000_300));
    }
    local.push(change("(auth) Change line 5 of g", "g.txt", &[(5, "five")], 1_700_000_250));
    local.push(HistoryCommit::new("(ui) Change other", &[("other.txt", "mine\n")]).at(1_700_000_400));
    let local = clone.commit_history("refs/heads/master", &local);
    let twins = upstream.commit_history(
      "refs/heads/master",
      &[
        change("Other change", "f.txt", &[(2, "two")], 1_700_000_100),
        change("Change line 5", "f.txt", &[(2, "two"), (5, "five")], 1_700_000_200),
        change("Change line 4 of g", "g.txt", &[(4, "four")], 1_700_000_110),
        change("Change line 5 of g", "g.txt", &[(4, "four"), (5, "five")], 1_700_000_250),
        HistoryCommit::new("Change other upstream", &[("other.txt", "theirs\n")]),
      ],
    );
    (upstream, clone, local, twins)
  }

  /// Clone a repository into this directory, with the config of a `TestRepo` but `merge.conflictstyle` and
  /// `commit.gpgsign`, which `git clone -c` writes into the clone before it fetches
  pub fn clone_from(&self, source_path: &Path) -> Result<(), String> {
    let config: Vec<String> = TEST_REPO_CONFIG
      .iter()
      .filter(|(key, _)| !matches!(*key, "merge.conflictstyle" | "commit.gpgsign"))
      .map(|(key, value)| format!("{key}={value}"))
      .collect();
    let mut args = vec!["clone"];
    for entry in &config {
      args.extend(["-c", entry.as_str()]);
    }
    args.extend([source_path.to_str().unwrap(), "."]);
    self.git_executor.execute_command(&args, self.path_str()).map(|_| ()).map_err(|e| e.to_string())
  }

  /// Create and checkout a new branch
  pub fn checkout_new_branch(&self, branch_name: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["checkout", "-b", branch_name], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Push a branch to a remote
  pub fn push(&self, remote: &str, branch: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["push", remote, branch], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Fetch from a remote with prune option
  pub fn fetch_prune(&self, remote: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["fetch", "--prune", remote], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Pull from a remote
  pub fn pull(&self) -> Result<(), String> {
    self.git_executor.execute_command(&["pull"], self.path_str()).map(|_| ()).map_err(|e| e.to_string())
  }

  /// Merge with fast-forward only
  pub fn merge_ff_only(&self, branch: &str) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["merge", "--ff-only", branch], self.path_str())
      .map(|_| self.head())
      .map_err(|e| e.to_string())
  }

  /// Merge a branch (standard merge)
  pub fn merge(&self, branch: &str, message: &str) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["merge", branch, "-m", message], self.path_str())
      .map(|_| self.head())
      .map_err(|e| e.to_string())
  }

  /// List branches matching a pattern
  pub fn list_branches(&self, pattern: &str) -> Result<Vec<String>, String> {
    self
      .git_executor
      .execute_command_lines(&["branch", "--list", pattern], self.path_str())
      .map(|lines| lines.into_iter().map(|line| line.trim().trim_start_matches("* ").to_string()).collect())
      .map_err(|e| e.to_string())
  }

  /// Get git log output
  pub fn log(&self, args: &[&str]) -> Result<String, String> {
    let mut cmd_args = vec!["log"];
    cmd_args.extend_from_slice(args);

    self.git_executor.execute_command(&cmd_args, self.path_str()).map_err(|e| e.to_string())
  }

  /// Get current branch name
  pub fn current_branch(&self) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["branch", "--show-current"], self.path_str())
      .map(|output| output.trim().to_string())
      .map_err(|e| e.to_string())
  }

  /// Add a git note to a commit
  pub fn add_note(&self, commit_hash: &str, note_content: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["notes", "add", "-f", "-m", note_content, commit_hash], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Show the note for a commit
  pub fn show_note(&self, commit_hash: &str) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["notes", "show", commit_hash], self.path_str())
      .map_err(|e| e.to_string())
  }

  /// List all notes in a ref
  pub fn list_notes_with_ref(&self, notes_ref: &str) -> Result<Vec<String>, String> {
    self
      .git_executor
      .execute_command_lines(&["notes", "--ref", notes_ref, "list"], self.path_str())
      .map_err(|e| e.to_string())
  }

  /// Rename a branch
  pub fn rename_branch(&self, old_name: &str, new_name: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["branch", "-m", old_name, new_name], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Add a remote
  pub fn add_remote(&self, name: &str, url: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["remote", "add", name, url], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }
}

/// A linked worktree of a `TestRepo`, in a temporary directory of its own: the lease of an agent session
pub struct TestWorktree {
  _dir: TempDir,
  /// Its top level, as git names it (`git_canonical_path`: macOS `/private/var/…`, Windows `C:/Users/…`)
  pub path: String,
  git_executor: GitCommandExecutor,
}

impl TestWorktree {
  /// A worktree of `repo`, detached at its HEAD
  pub fn detached(repo: &TestRepo) -> Self {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lane");
    repo.git(&["worktree", "add", "--detach", path.to_str().unwrap(), "HEAD"]);
    let path = git_canonical_path(&path);
    Self {
      _dir: dir,
      path,
      git_executor: GitCommandExecutor::new(),
    }
  }

  /// Runs git in the worktree; panics on a failure
  pub fn git(&self, args: &[&str]) -> String {
    self.git_executor.execute_command(args, &self.path).unwrap()
  }

  pub fn write(&self, file: &str, content: &str) {
    fs::write(Path::new(&self.path).join(file), content).unwrap();
  }

  pub fn read(&self, file: &str) -> String {
    fs::read_to_string(Path::new(&self.path).join(file)).unwrap()
  }

  /// Commits `file` with `content`; returns the commit
  pub fn commit(&self, message: &str, file: &str, content: &str) -> String {
    self.write(file, content);
    self.git(&["add", "--", file]);
    commit_in(&self.git_executor, &self.path, &["-m", message], &[]).unwrap_or_else(|e| panic!("Git commit failed: {e}"))
  }

  pub fn head(&self) -> String {
    self.git(&["rev-parse", "HEAD"])
  }
}

/// Builder for creating conflict test scenarios
pub struct ConflictTestBuilder<'a> {
  repo: &'a TestRepo,
  initial_files: Vec<(&'a str, &'a str)>,
  initial_message: &'a str,
  target_files: Vec<(&'a str, &'a str)>,
  target_message: &'a str,
  cherry_files: Vec<(&'a str, &'a str)>,
  cherry_message: &'a str,
}

impl<'a> ConflictTestBuilder<'a> {
  /// Create a new conflict test builder
  pub const fn new(repo: &'a TestRepo) -> Self {
    Self {
      repo,
      initial_files: vec![],
      initial_message: "Initial commit",
      target_files: vec![],
      target_message: "Target branch changes",
      cherry_files: vec![],
      cherry_message: "Cherry-pick changes",
    }
  }

  /// Set initial state files and commit message
  #[must_use]
  pub fn with_initial_state(mut self, files: Vec<(&'a str, &'a str)>, message: &'a str) -> Self {
    self.initial_files = files;
    self.initial_message = message;
    self
  }

  /// Set target branch changes
  #[must_use]
  pub fn with_target_changes(mut self, files: Vec<(&'a str, &'a str)>, message: &'a str) -> Self {
    self.target_files = files;
    self.target_message = message;
    self
  }

  /// Set cherry-pick changes
  #[must_use]
  pub fn with_cherry_changes(mut self, files: Vec<(&'a str, &'a str)>, message: &'a str) -> Self {
    self.cherry_files = files;
    self.cherry_message = message;
    self
  }

  /// Build the conflict scenario and return the commit hashes
  pub fn build(self) -> ConflictScenario {
    // Create initial commit
    let initial_hash = if self.initial_files.is_empty() {
      self.repo.create_commit(self.initial_message, "README.md", "Initial content")
    } else {
      self.repo.create_commit_with_files(self.initial_message, &self.initial_files)
    };

    // Create target branch changes
    let target_hash = if self.target_files.is_empty() {
      // For deletion scenarios, create a commit that deletes the specific files from initial commit
      // This represents the "target branch deletes the file" scenario

      // Remove each file that was in the initial state and stage all deletions at once
      let mut rm_args = vec!["rm", "--quiet", "--ignore-unmatch", "--"];
      for (filename, _) in &self.initial_files {
        let file_path = self.repo.path().join(filename);
        if file_path.exists() {
          fs::remove_file(&file_path).unwrap();
        }
        rm_args.push(filename);
      }
      if !self.initial_files.is_empty() {
        self
          .repo
          .git_executor
          .execute_command(&rm_args, self.repo.path_str())
          .unwrap_or_else(|e| panic!("Failed to stage deletions: {e}"));
      }

      // Create a commit with the deletions (may be empty, which is fine for deletion)
      self.repo.commit_with(&["--allow-empty", "-m", self.target_message], &[])
    } else {
      self.repo.create_commit_with_files(self.target_message, &self.target_files)
    };

    // Reset to initial commit
    self.repo.reset_hard(&initial_hash).unwrap();

    // Create cherry-pick changes
    let cherry_hash = if self.cherry_files.is_empty() {
      panic!("Cherry-pick changes must be provided for conflict scenario");
    } else {
      self.repo.create_commit_with_files(self.cherry_message, &self.cherry_files)
    };

    ConflictScenario {
      target_commit: target_hash,
      cherry_commit: cherry_hash,
    }
  }
}

/// Result of building a conflict scenario
pub struct ConflictScenario {
  pub target_commit: String,
  pub cherry_commit: String,
}

/// Create a file deletion conflict test setup
pub fn setup_deletion_conflict(repo: &TestRepo) -> ConflictScenario {
  ConflictTestBuilder::new(repo)
    .with_initial_state(vec![("delete_me.txt", "content to delete")], "Initial commit")
    .with_target_changes(vec![], "Target branch (file deleted)")
    .with_cherry_changes(vec![("delete_me.txt", "modified content")], "Cherry modifies file")
    .build()
}

#[cfg(test)]
#[path = "git_test_utils_test.rs"]
mod git_test_utils_test;
