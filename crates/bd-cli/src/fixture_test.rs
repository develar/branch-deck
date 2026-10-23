//! Repositories and helpers shared by the tests (named `_test` so the worktree safety scan treats it as test code)

use crate::run_with;
use bd_core::context::{Environment, RepoContext, RepoOptions};
use git_ops::revs::short_id;
use history_rewrite::repository_lock::{RepositoryLock, lock_repository};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::fmt::Write;
use std::path::Path;
use test_utils::git_test_utils::TestRepo;

pub const PREFIX: &str = "test-user";

/// No app settings, git from `PATH`, started in `cwd`
pub fn environment(cwd: &Path) -> Environment {
  Environment {
    cwd: cwd.to_path_buf(),
    settings_file: None,
    git: None,
    lock_timeout: std::time::Duration::from_millis(200),
    server_stop_timeout: std::time::Duration::from_secs(30),
    maintenance_timeout: std::time::Duration::from_secs(60),
    home: None,
    executable: None,
    appimage: None,
    path: None,
    // nothing to answer: a test of `bd safe-push` starts its own Patronus
    patronus_url: "http://127.0.0.1:9".to_string(),
    patronus_new_api: false,
    space_token: None,
    // nothing to answer and no token: a test of a GitHub pull request starts its own GitHub
    github_api_url: "http://127.0.0.1:9".to_string(),
    github_tokens: github::token::TokenSources::default(),
    // no YouTrack: the text of a pull request names its issues without their summaries
    youtrack_environment: None,
    youtrack_token_file: None,
    // no `teamcity` CLI: a test of `bd robot --full` writes a fake one
    teamcity: Some(cwd.join("no-teamcity")),
  }
}

/// Run `bd` and parse its single JSON line
/// The context of `bd` without `--prefix` in `repo`, whose config sets `branchdeck.branchPrefix` to `PREFIX`
pub fn context_with_the_config_prefix(repo: &TestRepo) -> RepoContext {
  repo.set_config("branchdeck.branchPrefix", PREFIX).unwrap();
  let options = RepoOptions {
    repo: None,
    prefix: None,
    full_ids: false,
    no_cache: false,
  };
  let context = RepoContext::resolve(&environment(repo.path()), &options).unwrap();
  assert_eq!(context.prefix, PREFIX);
  context
}

pub fn run_json(environment: &Environment, args: &[&str]) -> (i32, Value) {
  let mut out = Vec::new();
  let exit_code = run_with(args.iter().map(OsString::from), environment, &mut out);
  let text = String::from_utf8(out).expect("UTF-8 output");
  assert_eq!(text.lines().count(), 1, "one line of JSON, got: {text}");
  let value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("invalid JSON ({e}): {text}"));
  (exit_code, value)
}

/// The output is the error `expected.1` with the exit code `expected.0`
pub fn assert_error(output: &Value, exit_code: i32, expected: (i32, &str)) {
  assert_eq!((exit_code, &output["error"]["code"]), (expected.0, &serde_json::json!(expected.1)), "{output}");
}

/// A cherry-pick in progress in `worktree` of `repo`, as one that stopped leaves it: `CHERRY_PICK_HEAD` names its HEAD
pub fn start_cherry_pick(repo: &TestRepo, worktree: &Path) {
  let git = repo.git_executor();
  let worktree_path = worktree.to_str().unwrap();
  // relative to the worktree in the main one, absolute in a linked one
  let marker = git.execute_command(&["rev-parse", "--git-path", "CHERRY_PICK_HEAD"], worktree_path).unwrap();
  let head = git.execute_command(&["rev-parse", "HEAD"], worktree_path).unwrap();
  std::fs::write(worktree.join(marker), format!("{head}\n")).unwrap();
}

/// `run(&[])` and its dry run `run(&["--dry-run"])` are refused as `operation_in_progress` (exit 4) naming
/// `operation`, and HEAD stays
pub fn assert_refused_during(repo: &TestRepo, operation: &str, run: impl Fn(&[&str]) -> (i32, Value)) {
  let head = repo.head();
  for extra in [&[][..], &["--dry-run"]] {
    let (exit_code, output) = run(extra);
    assert_error(&output, exit_code, (4, "operation_in_progress"));
    let message = output["error"]["message"].as_str().unwrap_or_default();
    assert!(message.contains(&format!("{operation} is in progress")), "{extra:?}: {output}");
    assert_eq!(repo.head(), head, "{extra:?}");
  }
}

/// `fields` after the `schema`, `repository` and `prefix` every output of `bd` in `repo` starts with
pub fn with_header(repo: &TestRepo, fields: Value) -> Value {
  let mut output = serde_json::json!({ "schema": 2, "repository": repository(repo), "prefix": PREFIX });
  let Value::Object(fields) = fields else { panic!("fields must be an object") };
  output.as_object_mut().unwrap().extend(fields);
  output
}

/// The repository path as `bd` prints it: the top level that git names, `C:/Users/...` on Windows
pub fn repository(repo: &TestRepo) -> String {
  git(repo, &["rev-parse", "--show-toplevel"])
}

/// `path` resolved like `std::fs::canonicalize`, without the `\\?\` prefix that it gives on Windows
pub fn canonical(path: &Path) -> std::path::PathBuf {
  std::path::PathBuf::from(sync_utils::paths::display_path(&std::fs::canonicalize(path).unwrap()))
}

/// The `undo_token` of `output` is `<head>..<HEAD now>`, and `bd undo` with it moves HEAD back to `head`
pub fn assert_undo_restores(repo: &TestRepo, output: &Value, head: &str) {
  let token = output["undo_token"].as_str().unwrap_or_else(|| panic!("no undo token: {output}"));
  assert_eq!(token, format!("{head}..{}", repo.head()));
  let (exit_code, undo) = run_in(repo, &["undo", token]);
  assert_eq!(exit_code, 0, "{undo}");
  assert_eq!(repo.head(), head);
}

/// `(name, status)` of every branch of the `sync` part of a rewrite output
pub fn sync_statuses(output: &Value) -> Vec<(&Value, &Value)> {
  output["sync"]["branches"]
    .as_array()
    .unwrap_or_else(|| panic!("no sync: {output}"))
    .iter()
    .map(|branch| (&branch["name"], &branch["state"]))
    .collect()
}

/// Run `bd` in `repo` with `--prefix test-user`
pub fn run_in(repo: &TestRepo, args: &[&str]) -> (i32, Value) {
  let mut all_args = vec!["--prefix", PREFIX];
  all_args.extend_from_slice(args);
  run_json(&environment(repo.path()), &all_args)
}

/// bd with a git wrapper that logs every command with the state of the repository lock when it started: `locked` while
/// the holder record of `$GIT_COMMON_DIR/branch-deck.lock` exists (a holder writes it once it has the lock and deletes
/// it before the release), so a test reads the lock span of a command. `arms` are `case " $* " in` arms that run before
/// the real git, with `{dir}` replaced by the directory of the wrapper (a barrier, a failure)
#[cfg(unix)]
pub struct LockSpanGit {
  directory: tempfile::TempDir,
  pub environment: Environment,
}

#[cfg(unix)]
impl LockSpanGit {
  pub fn new(repo: &TestRepo, arms: &str) -> Self {
    let directory = tempfile::tempdir().unwrap();
    let dir = directory.path().to_str().unwrap();
    let holder = repo.path().join(".git").join("branch-deck.lock.holder");
    let script = directory.path().join("git");
    let text = format!(
      "#!/bin/sh\nif [ -f '{holder}' ]; then state=locked; else state=free; fi\necho \"$state $*\" >> '{dir}/log'\ncase \" $* \" in {arms}\nesac\nexec git \"$@\"\n",
      holder = holder.display(),
      arms = arms.replace("{dir}", dir),
    );
    std::fs::write(&script, text).unwrap();
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let mut environment = environment(repo.path());
    environment.git = Some(script.to_str().unwrap().to_string());
    Self { directory, environment }
  }

  /// The directory of the wrapper, where its arms keep their marks
  pub fn dir(&self) -> &Path {
    self.directory.path()
  }

  /// Run `bd` with `--prefix test-user`; the log starts empty
  pub fn run(&self, args: &[&str]) -> (i32, Value) {
    let mut all = vec!["--prefix", PREFIX];
    all.extend_from_slice(args);
    let _ = std::fs::remove_file(self.directory.path().join("log"));
    run_json(&self.environment, &all)
  }

  /// `(locked, arguments)` of every git command of the last run, in the order they started
  pub fn commands(&self) -> Vec<(bool, String)> {
    let log = std::fs::read_to_string(self.directory.path().join("log")).unwrap_or_default();
    log
      .lines()
      .filter_map(|line| line.split_once(' '))
      .map(|(state, args)| (state == "locked", args.to_string()))
      .collect()
  }

  /// The commands of the last run that started under the repository lock
  pub fn under_lock(&self) -> Vec<String> {
    self.commands().into_iter().filter(|(locked, _)| *locked).map(|(_, args)| args).collect()
  }
}

/// The subcommand of git arguments as the wrapper logs them: the first word that is neither an option nor the value of
/// a `-c` option
pub fn subcommand(args: &str) -> &str {
  args.split(' ').find(|word| !word.starts_with('-') && !word.contains('=')).unwrap_or_default()
}

/// A repository whose `origin/master` is its first commit, so every later commit is ahead of the baseline
pub fn repo_with_baseline(files: &[(&str, &str)]) -> TestRepo {
  let repo = TestRepo::new();
  let initial = repo.create_commit_with_files("Initial commit", files);
  git(&repo, &["remote", "add", "origin", "."]);
  git(&repo, &["update-ref", "refs/remotes/origin/master", &initial]);
  repo
}

pub fn git(repo: &TestRepo, args: &[&str]) -> String {
  repo
    .git_executor()
    .execute_command(args, repo.path().to_str().unwrap())
    .unwrap_or_else(|e| panic!("git {args:?} failed: {e}"))
}

/// Commit everything in the working tree, with a fixed author and committer time
pub fn commit_all(repo: &TestRepo, message: &str, timestamp: i64) -> String {
  git(repo, &["add", "-A"]);
  let date = format!("{timestamp} +0000");
  repo
    .git_executor()
    .cmd(&["commit", "-m", message], repo.path().to_str().unwrap())
    .env(&[("GIT_AUTHOR_DATE", &date), ("GIT_COMMITTER_DATE", &date)])
    .text()
    .unwrap_or_else(|e| panic!("commit {message:?} failed: {e}"));
  repo.head()
}

/// The repository lock, held as another process would hold it, until the guard is dropped
pub fn hold_the_lock(repo: &TestRepo) -> RepositoryLock {
  lock_repository(repo.git_executor(), repo.path().to_str().unwrap(), std::time::Duration::from_millis(100)).unwrap()
}

/// A dry run's output: it says so, its `new_head` is `head` (abbreviated) and it has no undo token
pub fn assert_dry_run(output: &Value, head: &str) {
  assert_eq!(
    (&output["dry_run"], &output["new_head"], output.get("undo_token")),
    (&json!(true), &json!(short_id(head)), None),
    "{output}"
  );
}

/// One commit per subject, each adding its own file, 100 s apart; returns their ids
pub fn commit_subjects(repo: &TestRepo, subjects: &[&str]) -> Vec<String> {
  let mut ids = Vec::new();
  for (index, subject) in subjects.iter().enumerate() {
    write(repo, &format!("f{index}.txt"), *subject);
    ids.push(commit_all(repo, subject, 1_700_000_000 + i64::try_from(index).unwrap() * 100));
  }
  ids
}

pub fn write(repo: &TestRepo, path: &str, content: impl AsRef<[u8]>) {
  let path = repo.path().join(path);
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, content).unwrap();
}

/// Lines `line 1` ..= `line n`
pub fn numbered_lines(n: usize) -> String {
  (1..=n).fold(String::new(), |mut text, i| {
    let _ = writeln!(text, "line {i}");
    text
  })
}

/// The paths of `commit_dangling_reference`
pub const ACP_AGENT: &str = "plugins/air/src/acp/AcpScenarioAgent.java";
pub const ACP_PROTOCOL: &str = "plugins/air/src/acp/AcpControlProtocol.java";
pub const ACP_HOST: &str = "plugins/air/tests/flow/AirFlowLaneHost.kt";

/// The Kotlin user of the constant, qualified by `class_name`; the use is on line 7
pub fn acp_host(class_name: &str) -> String {
  format!(
    "package flow\n\nimport acp.{class_name}\n\nclass AirFlowLaneHost {{\n  fun environment(url: String): Map<String, String> {{\n    return mapOf({class_name}.CONTROL_URL_ENV to url)\n  }}\n}}\n"
  )
}

pub const MANAGER_TEST: &str = "impl/testSrc/p/ManagerTest.kt";

/// The Safe Push failure of 2026-10-17 (IJAI-4074) in `repo` (from `repo_with_baseline`): the baseline gets `Manager`
/// with `buttons(window: Window)` in the module `impl`; `(abc)` adds the overload `buttons(paneId: String)`, and
/// `(xyz)` adds a test in another file whose object expression overrides both, the new one on line 7. No line links
/// the two. Returns `(the commit that adds the overload, the commit that overrides it)`
pub fn commit_override_of_overload(repo: &TestRepo) -> (String, String) {
  let manager =
    |overload: &str| format!("package p\n\nopen class Manager {{\n  internal open fun buttons(window: Window): Buttons = Buttons()\n{overload}}}\n\nclass Window\nclass Buttons\n");
  write(repo, "impl/BUILD.bazel", "");
  write(repo, "impl/src/p/Manager.kt", manager(""));
  let base = commit_all(repo, "Add the manager", 1_700_000_050);
  git(repo, &["update-ref", "refs/remotes/origin/master", &base]);
  write(
    repo,
    "impl/src/p/Manager.kt",
    manager("\n  internal open fun buttons(paneId: String): Buttons = Buttons()\n"),
  );
  let declares = commit_all(repo, "(abc) Add the pane overload", 1_700_000_100);
  write(
    repo,
    MANAGER_TEST,
    "package p\n\nclass ManagerTest {\n  fun create(buttons: Buttons): Manager = object : Manager() {\n    override fun buttons(window: Window): Buttons = buttons\n\n    override fun buttons(paneId: String): Buttons = buttons\n  }\n}\n",
  );
  let overrides = commit_all(repo, "(xyz) Test the manager", 1_700_000_200);
  (declares, overrides)
}

pub const LEDGER_TEST: &str = "plugins/air/tests/contract/AcpJourneyLifecycleTest.kt";
pub const LEDGER: &str = "plugins/air/tests/bridge/AcpOwnershipLedger.kt";

/// The Safe Push failure of IJAI-4185 (2026-10-17) in `repo` (from `repo_with_baseline`): the baseline gets a ledger and
/// a test that calls its `cleanup`; `(journey)` deletes the test, then `(suspend)` makes `cleanup` suspend. HEAD
/// compiles, the tree of `(suspend)` has the call outside a coroutine. Returns `(the commit that deletes the test, the
/// commit that changes the function)`
pub fn commit_changed_declaration(repo: &TestRepo) -> (String, String) {
  commit_changed_member(repo, "suspend fun cleanup()")
}

/// `commit_changed_declaration` with `cleanup` changed to `header` instead
pub fn commit_changed_member(repo: &TestRepo, header: &str) -> (String, String) {
  let ledger = |header: &str| format!("package bridge\n\nclass AcpOwnershipLedger {{\n  {header} {{\n  }}\n}}\n");
  write(repo, LEDGER, ledger("fun cleanup()"));
  write(
    repo,
    LEDGER_TEST,
    "package contract\n\nimport bridge.AcpOwnershipLedger\n\nclass AcpJourneyLifecycleTest {\n  fun cleans() {\n    val ledger = AcpOwnershipLedger()\n    ledger.cleanup()\n  }\n}\n",
  );
  let base = commit_all(repo, "Add the ledger", 1_700_000_050);
  git(repo, &["update-ref", "refs/remotes/origin/master", &base]);
  git(repo, &["rm", "-q", "--", LEDGER_TEST]);
  let deleted_by = commit_all(repo, "(journey) IJAI-4182 The journey support goes with its tests", 1_700_000_100);
  write(repo, LEDGER, ledger(header));
  let changed_by = commit_all(repo, "(suspend) IJAI-4185 The cleanups may suspend", 1_700_000_200);
  (deleted_by, changed_by)
}

/// The Safe Push failure of 2026-10-16 in `repo` (from `repo_with_baseline`): the baseline gets a Java class with a
/// constant and a Kotlin file that uses it; `(fake-agent)` moves the constant into a new class, and `(lane-document)`
/// changes the use. Returns `(the commit that moves it, the commit that changes the use)`
pub fn commit_dangling_reference(repo: &TestRepo) -> (String, String) {
  write(
    repo,
    ACP_AGENT,
    "package acp;\n\npublic final class AcpScenarioAgent {\n  public static final String CONTROL_URL_ENV = \"AIR_CONTROL_URL\";\n}\n",
  );
  write(repo, ACP_HOST, acp_host("AcpScenarioAgent"));
  let base = commit_all(repo, "Add the scenario agent", 1_700_000_050);
  git(repo, &["update-ref", "refs/remotes/origin/master", &base]);
  write(repo, ACP_AGENT, "package acp;\n\npublic final class AcpScenarioAgent {\n}\n");
  write(
    repo,
    ACP_PROTOCOL,
    "package acp;\n\npublic final class AcpControlProtocol {\n  public static final String CONTROL_URL_ENV = \"AIR_CONTROL_URL\";\n}\n",
  );
  let removed_by = commit_all(repo, "(fake-agent) IJAI-3880 Move the control constants", 1_700_000_100);
  write(repo, ACP_HOST, acp_host("AcpControlProtocol"));
  let fixed_by = commit_all(repo, "(lane-document) IJAI-3878 Use the control protocol", 1_700_000_200);
  (removed_by, fixed_by)
}

/// The object or array schema behind a `$ref` into `$defs` or the non-null alternative of an `anyOf` (an `Option`)
fn resolve<'a>(root: &'a Value, schema: &'a Value) -> &'a Value {
  if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
    return resolve(root, &root["$defs"][reference.trim_start_matches("#/$defs/")]);
  }
  if let Some(alternatives) = schema.get("anyOf").and_then(Value::as_array)
    && let Some(alternative) = alternatives.iter().find(|alternative| alternative["type"] != json!("null"))
  {
    return resolve(root, alternative);
  }
  schema
}

/// Every key of `value` is a property of `schema` (or a key of a map), and every key the schema requires is in `value`,
/// recursively through objects and arrays
pub fn assert_covered(root: &Value, schema: &Value, value: &Value, path: &str) {
  let schema = resolve(root, schema);
  match value {
    Value::Object(object) => {
      // an output leaves an empty optional field out, so the schema must not require it
      for key in schema["required"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        assert!(object.contains_key(key), "{path}.{key} is required by the schema but missing");
      }
      for (key, child) in object {
        // a map declares its values with `additionalProperties`
        let property = schema["properties"].get(key).unwrap_or_else(|| &schema["additionalProperties"]);
        assert!(!property.is_null(), "{path}.{key} is not in the schema {schema}");
        assert_covered(root, property, child, &format!("{path}.{key}"));
      }
    }
    Value::Array(items) => {
      for (index, item) in items.iter().enumerate() {
        assert_covered(root, &schema["items"], item, &format!("{path}[{index}]"));
      }
    }
    _ => {}
  }
}

/// The branch `name` of the `branches` of an output (`bd check`, `bd push`)
pub fn branch<'a>(output: &'a Value, name: &str) -> &'a Value {
  output["branches"]
    .as_array()
    .unwrap()
    .iter()
    .find(|branch| branch["name"] == json!(name))
    .unwrap_or_else(|| panic!("no {name} in {output}"))
}

/// HEAD, the refs, the local config, the index and the working tree status: what a read-only command must not change
pub fn writable_state(repo: &TestRepo) -> (String, String, String, Vec<u8>, String) {
  (
    format!("{}{}", git(repo, &["symbolic-ref", "HEAD"]), git(repo, &["rev-parse", "HEAD"])),
    git(repo, &["for-each-ref", "--format=%(refname) %(objectname)"]),
    git(repo, &["config", "--local", "--list"]),
    std::fs::read(repo.path().join(".git/index")).unwrap(),
    git(repo, &["status", "--porcelain=v1", "-z", "--untracked-files=all"]),
  )
}
