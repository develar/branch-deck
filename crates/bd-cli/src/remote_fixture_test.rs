//! The fakes of the tests of the remote commands, shared by bd-remote and the process budgets of bd-cli: a fake Patronus
//! with a branch to land (`Space`), a fake GitHub with pull requests, and a repository with a remote (named `_test`
//! so the worktree safety scan treats it as test code)

use crate::fixture_test::{PREFIX, assert_covered, commit_all, commit_subjects, environment, git, repo_with_baseline, run_json, write};
use bd_core::context::Environment;
use github::fake_github::{FakeAnswer, FakeGitHub, start_fake as start_fake_github};
use github::token::TokenSources;
use patronus::fake_patronus::{FakePatronus, robot_json, start_fake};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use sync_utils::timestamp::{format_utc_timestamp, unix_now};
use tempfile::TempDir;
use test_utils::fake_executable::{fake_executable, shell_path};
use test_utils::git_test_utils::{TestRepo, disable_background_maintenance, git_command};
use tokio::runtime::Runtime;
pub const AUTH: &str = "test-user/virtual/auth";

/// A repository whose `origin` is on Space (`ssh://git@git.jetbrains.team/ij/ultimate.git`), served by a local bare
/// repository through a fake `ssh`, and a Patronus that answers for it
pub struct Space {
  pub repo: TestRepo,
  pub remote: TempDir,
  pub home: TempDir,
  pub fake: Arc<FakePatronus>,
  pub patronus_url: String,
  /// Serves the fake Patronus (and a fake GitHub) while the test runs
  pub runtime: Runtime,
}

impl Space {
  pub fn new(subjects: &[&str]) -> Self {
    let (repo, remote) = repo_with_remote(subjects);
    let home = tempfile::tempdir().unwrap();
    // git runs `<ssh> [options] git@git.jetbrains.team "git-receive-pack '/ij/ultimate.git'"`: serve the bare repository
    let ssh = fake_executable(
      home.path(),
      "ssh",
      &format!(
        "for last; do :; done\ncase \"$last\" in\n  *receive-pack*) exec git receive-pack '{0}' ;;\n  *) exec git upload-pack '{0}' ;;\nesac",
        shell_path(remote.path())
      ),
    );
    git(&repo, &["config", "core.sshCommand", &shell_path(&ssh)]);
    git(&repo, &["remote", "set-url", "origin", "ssh://git@git.jetbrains.team/ij/ultimate.git"]);

    let token = home.path().join(".patronus/code.jetbrains.team-identity-token.json");
    std::fs::create_dir_all(token.parent().unwrap()).unwrap();
    std::fs::write(&token, r#"{"name": "me", "token": "t", "expires": null}"#).unwrap();

    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let (fake, client) = runtime.block_on(start_fake());
    Self {
      repo,
      remote,
      home,
      fake,
      patronus_url: client.base_url().to_string(),
      runtime,
    }
  }

  pub fn environment(&self) -> Environment {
    Environment {
      home: Some(self.home.path().to_path_buf()),
      patronus_url: self.patronus_url.clone(),
      ..environment(self.repo.path())
    }
  }

  pub fn run(&self, args: &[&str]) -> (i32, Value) {
    safe_push(&self.environment(), args)
  }

  pub fn remote_tip(&self, branch: &str) -> String {
    let output = git_command()
      .args(["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")])
      .current_dir(self.remote.path())
      .output()
      .unwrap();
    String::from_utf8(output.stdout).unwrap().trim().to_string()
  }

  pub fn local_tip(&self, branch: &str) -> String {
    git(&self.repo, &["rev-parse", &format!("refs/heads/{branch}")])
  }

  pub fn set_robots(&self, robots: Vec<Value>) {
    *self.fake.robots.lock().unwrap() = Some(robots);
  }

  pub fn last_start(&self) -> Value {
    self.fake.last_start()
  }

  /// Puts the baseline on the remote's `master`, the target the robots push into
  pub fn publish_target(&self) {
    git(&self.repo, &["push", "--quiet", "origin", "refs/remotes/origin/master:refs/heads/master"]);
  }

  /// A commit of another developer on the remote's `master`, which the repository hasn't fetched
  pub fn land_on_target(&self, file: &str, content: &str, subject: &str) -> String {
    let clone = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
      let output = git_command().args(args).current_dir(clone.path()).output().unwrap();
      assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
      String::from_utf8(output.stdout).unwrap().trim().to_string()
    };
    run(&["clone", "--quiet", self.remote.path().to_str().unwrap(), "."]);
    std::fs::write(clone.path().join(file), content).unwrap();
    run(&["add", "-A"]);
    run(&["-c", "user.name=Alice", "-c", "user.email=alice@example.com", "commit", "--quiet", "-m", subject]);
    run(&["push", "--quiet", "origin", "HEAD:master"]);
    run(&["rev-parse", "HEAD"])
  }
}

/// `2026-10-14T09:30:00Z` of `minutes` before now: the robots of the fixtures start within the 7 days `bd` lists, so a
/// fixed day would drop out of the list a week later
pub fn minutes_ago(minutes: u32) -> String {
  format_utc_timestamp(unix_now().saturating_sub(minutes * 60))
}

/// A robot of the user on the virtual branch `name`, at `revision`, started three hours ago
pub fn robot(id: &str, status: &str, name: &str, revision: &str) -> Value {
  let mut robot = robot_json(id, status, &format!("refs/heads/test-user/virtual/{name}"), &minutes_ago(180));
  robot["sourceRevision"] = json!(revision);
  robot
}

/// The rules of master as GitHub answers them (`GET rules/branches/master`); `fake_github` answers none
pub const MASTER_RULES: &str = "/repos/acme/monorepo/rules/branches/master";

/// The rules of a branch whose pull request rule dismisses the approvals on a push of new commits, as GitHub answers them
pub fn dismissing_rules() -> Value {
  json!([{"type": "pull_request", "parameters": {"dismiss_stale_reviews_on_push": true, "required_approving_review_count": 1}}])
}

/// A fake GitHub of the repository `acme/monorepo` and its URL: the token is of `me`, the search lists
/// `pull_requests`, a created pull request is #7 of auth (assigned and reviewed as asked), the `/patronus` comment is
/// accepted at once, master has no rules (`MASTER_RULES`)
pub fn fake_github(space: &Space, pull_requests: &[Value]) -> (Arc<FakeGitHub>, String) {
  let (fake, client) = space.runtime.block_on(start_fake_github());
  list_pull_requests(&fake, pull_requests);
  fake.answer("GET", "/user", FakeAnswer::ok(json!({"login": "me"})));
  // master has no rules: a push keeps the approvals
  fake.answer("GET", MASTER_RULES, FakeAnswer::ok(json!([])));
  fake.answer(
    "POST",
    "/repos/acme/monorepo/pulls",
    FakeAnswer::status(
      201,
      json!({"number": 7, "html_url": "https://github.com/acme/monorepo/pull/7", "title": "Add auth", "head": {"ref": AUTH, "sha": "1111"}, "base": {"ref": "master"}}),
    ),
  );
  fake.answer(
    "POST",
    "/repos/acme/monorepo/issues/7/assignees",
    FakeAnswer::status(201, json!({"assignees": [{"login": "me"}]})),
  );
  fake.answer(
    "POST",
    "/repos/acme/monorepo/pulls/7/requested_reviewers",
    FakeAnswer::status(201, json!({"requested_reviewers": [{"login": "alice-gh"}]})),
  );
  let comment = json!({"id": 4402, "html_url": "https://github.com/acme/monorepo/pull/7#issuecomment-4402", "created_at": "2026-10-14T09:30:00Z"});
  fake.answer("POST", "/repos/acme/monorepo/issues/7/comments", FakeAnswer::status(201, comment.clone()));
  let mut accepted = comment;
  accepted["reactions"] = json!({"+1": 1, "-1": 0});
  fake.answer("GET", "/repos/acme/monorepo/issues/comments/4402", FakeAnswer::ok(accepted));
  (fake, client.web_url().to_string())
}

/// The search of the user's pull requests answers `pull_requests`
pub fn list_pull_requests(fake: &FakeGitHub, pull_requests: &[Value]) {
  fake.answer("POST", "/graphql", search_answer(pull_requests));
}

/// The open pull request #7 of auth into master, approved by `approvers`
pub fn pull_request_json(approvers: &[&str]) -> Value {
  let reviews: Vec<Value> = approvers.iter().map(|login| json!({"author": {"login": login}, "state": "APPROVED"})).collect();
  json!({"number": 7, "title": "Add auth", "url": "https://github.com/acme/monorepo/pull/7", "state": "OPEN", "isDraft": false,
    "headRefName": AUTH, "headRefOid": "1111", "baseRefName": "master", "headRepositoryOwner": {"login": "acme"},
    "isCrossRepository": false, "updatedAt": "2026-10-14T09:30:00Z", "latestReviews": {"nodes": reviews}})
}

/// The branches are reviewed on GitHub (`acme/monorepo`), through the fake GitHub at `github_url`, asked with the token
/// `t`; the robot of the comment on pull request #7 is listed
pub fn reviewed_on_github(space: &Space, github_url: &str) -> Environment {
  git(&space.repo, &["config", "branchdeck.githubRepository", "acme/monorepo"]);
  let mut started = robot("from-comment", "RUNNING", "other", "0000");
  started["githubPrUrl"] = json!("https://github.com/acme/monorepo/pull/7");
  space.set_robots(vec![started]);
  Environment {
    github_api_url: github_url.to_string(),
    github_tokens: TokenSources::fixed("t"),
    ..space.environment()
  }
}

/// The answer of a Patronus that evaluates pull request #7: IJPL Code Platform has `approved` and Ilya Korennoy the
/// review state `state`
pub fn evaluated_review(approved: bool, state: &str) -> Value {
  json!({
    "approvalRequired": !approved,
    "reviewerGroups": [{"groupName": "IJPL Code Platform", "paths": ["auth.txt"], "requiredApprovals": 1, "collectedApprovals": u32::from(approved), "approved": approved, "selfApprovalAllowed": false,
      "reviewers": [{"spaceUsername": "ikorennoy", "fullName": "Ilya Korennoy", "githubUsername": "ikorennoy", "isAvailable": true, "isSuggested": true, "approvalState": state}]}],
  })
}

/// The stack branch of the stack of auth (`{prefix}/stack/auth`)
pub const STACK: &str = "test-user/stack/auth";

/// auth with login and session built on it: a stack that forks, whose stack branch has the commits of all three
pub fn forking_stack() -> Space {
  let space = Space::new(&["(auth) Add auth", "(auth/login) Add login", "(auth/session) Add session"]);
  space.repo.set_config("branchdeck.stacks", "true").unwrap();
  space
}

/// A robot of the whole stack of auth, on its stack branch at `revision`, started three hours ago
pub fn stack_robot(id: &str, status: &str, revision: &str) -> Value {
  let mut robot = robot_json(id, status, &format!("refs/heads/{STACK}"), &minutes_ago(180));
  robot["sourceRevision"] = json!(revision);
  robot
}

/// A repository whose `origin` is a local bare repository, with the commits of `subjects`
pub fn repo_with_remote(subjects: &[&str]) -> (TestRepo, TempDir) {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let remote = tempfile::tempdir().unwrap();
  git(&repo, &["init", "--bare", "-q", remote.path().to_str().unwrap()]);
  disable_background_maintenance(remote.path());
  git(&repo, &["remote", "set-url", "origin", remote.path().to_str().unwrap()]);
  commit_subjects(&repo, subjects);
  (repo, remote)
}

/// `push_from_elsewhere` onto the remote branch `branch`
pub fn push_from_elsewhere_to(remote: &TempDir, branch: &str, subject: &str) -> String {
  let elsewhere = tempfile::tempdir().unwrap();
  let run = |args: &[&str]| {
    let status = git_command().args(args).current_dir(elsewhere.path()).output().unwrap();
    assert!(status.status.success(), "{args:?}: {}", String::from_utf8_lossy(&status.stderr));
  };
  run(&["clone", "-q", "--branch", branch, remote.path().to_str().unwrap(), "."]);
  disable_background_maintenance(elsewhere.path());
  let (file, content) = their_change(subject);
  std::fs::write(elsewhere.path().join(&file), content).unwrap();
  run(&["add", &file]);
  run(&["-c", "user.name=Other", "-c", "user.email=other@example.com", "commit", "-q", "-m", subject]);
  run(&["push", "-q", "origin", branch]);
  remote_tip(remote, branch)
}

/// The baseline moves on by a commit of the target and the user's branch is rebased onto it: the next sync rebuilds the
/// branches on it, so a remote branch pushed before has their commits by content, under other hashes
pub fn rebuild_on_a_newer_baseline(repo: &TestRepo) {
  let baseline = git(repo, &["rev-parse", "refs/remotes/origin/master"]);
  let upstream = git(repo, &["commit-tree", &format!("{baseline}^{{tree}}"), "-p", &baseline, "-m", "Upstream change"]);
  git(repo, &["update-ref", "refs/remotes/origin/master", &upstream]);
  git(repo, &["rebase", "-q", &upstream]);
}

/// The subjects of `land_auth_by_content`: one branch, two commits
pub const LANDED_SUBJECTS: [&str; 2] = ["(auth) Add auth", "(auth) Add login"];

/// After the push of `LANDED_SUBJECTS`, a Safe Push landed both commits of auth into the baseline, as the robot of a
/// whole stack does: `Add auth` rebuilt on a target that changed its lines (another patch; the author, the author time and
/// the subject kept), `Add login` as it was; Update Project put the branch onto it, and auth got one new commit. Returns
/// the baseline commit of `Add auth`
pub fn land_auth_by_content(repo: &TestRepo) -> String {
  git(repo, &["reset", "-q", "--hard", "refs/remotes/origin/master"]);
  write(repo, "f0.txt", "(auth) Add auth, rebuilt on a changed target");
  let landed = commit_all(repo, "Add auth", 1_700_000_000);
  write(repo, "f1.txt", LANDED_SUBJECTS[1]);
  commit_all(repo, "Add login", 1_700_000_100);
  git(repo, &["update-ref", "refs/remotes/origin/master", "HEAD"]);
  write(repo, "f2.txt", "(auth) Add session");
  commit_all(repo, "(auth) Add session", 1_700_000_300);
  landed
}

/// auth is synced and has the open pull request #7 of `me`, which nobody approved; IJPL Code Platform has to approve
/// it, and Patronus suggests Ilya Korennoy (`ikorennoy`) with the review state `state`
pub fn needs_code_owner(state: &str) -> (Space, Arc<FakeGitHub>, Environment) {
  let space = Space::new(&["(auth) Add auth"]);
  space.fake.review_required.store(true, Ordering::SeqCst);
  *space.fake.pull_request_review.lock().unwrap() = Some(evaluated_review(false, state));
  let (github, url) = fake_github(&space, &[own_pull_request()]);
  let environment = reviewed_on_github(&space, &url);
  let (exit_code, output) = run_json(&environment, &["--prefix", PREFIX, "sync"]);
  assert_eq!(exit_code, 0, "{output}");
  (space, github, environment)
}

pub fn search_answer(pull_requests: &[Value]) -> FakeAnswer {
  FakeAnswer::ok(json!({"data": {"repository": {"viewerPermission": "WRITE"}, "search": {"nodes": pull_requests}}}))
}

/// The file and the content of the commit `push_from_elsewhere` makes for `subject`
pub fn their_change(subject: &str) -> (String, String) {
  (format!("{}.txt", subject.to_lowercase().replace(' ', "-")), format!("{subject}\n"))
}

pub fn remote_tip(remote: &TempDir, branch: &str) -> String {
  let output = git_command()
    .args(["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")])
    .current_dir(remote.path())
    .output()
    .unwrap();
  String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// The open pull request #7 of auth, by `me`
pub fn own_pull_request() -> Value {
  let mut pull_request = pull_request_json(&[]);
  pull_request["author"] = json!({"login": "me"});
  pull_request
}

/// `bd safe-push` with `args`
pub fn safe_push(environment: &Environment, args: &[&str]) -> (i32, Value) {
  let mut all_args = vec!["--prefix", PREFIX, "safe-push"];
  all_args.extend_from_slice(args);
  run_json(environment, &all_args)
}

impl Space {
  /// `bd robot` with `args`
  pub fn robot(&self, args: &[&str]) -> (i32, Value) {
    let mut all_args = vec!["--prefix", PREFIX, "robot"];
    all_args.extend_from_slice(args);
    run_json(&self.environment(), &all_args)
  }

  /// `bd robot` with `args`, which succeeds with an output of its schema
  pub fn robot_ok(&self, args: &[&str]) -> Value {
    let (exit_code, output) = self.robot(args);
    assert_eq!(exit_code, 0, "{output}");
    assert_schema(self, &output);
    output
  }

  pub fn sync(&self) {
    let (exit_code, output) = run_json(&self.environment(), &["--prefix", PREFIX, "sync"]);
    assert_eq!(exit_code, 0, "{output}");
  }

  /// A robot of the list and, as `GET /robots/{id}` answers for it, with the Large Scale Change profile, rebased onto
  /// `bbb222` and built as `ccc111`
  pub fn add_robot(&self, id: &str, status: &str, name: &str, revision: &str) {
    self.add_listed_robot(id, robot(id, status, name, revision));
  }

  /// `add_robot` of the robot `listed` as the list has it
  pub fn add_listed_robot(&self, id: &str, listed: Value) {
    let status = listed["status"].as_str().unwrap_or_default().to_string();
    let mut details = listed.clone();
    details["options"] = json!({"patronus.merge.profile.id": "lsc", "patronus.merge.profile.name": "Large Scale Change"});
    details["testReferenceRevision"] = json!("ccc111");
    details["targetRevision"] = json!("bbb222");
    if status != "RUNNING" {
      details["finishDateTime"] = json!(minutes_ago(120));
    }
    self.fake.details.lock().unwrap().insert(id.to_string(), details);
    let mut robots = self.fake.robots.lock().unwrap().take().unwrap_or_default();
    robots.push(listed);
    *self.fake.robots.lock().unwrap() = Some(robots);
  }
}

/// The output matches the schema of `bd robot`
pub fn assert_schema(space: &Space, output: &Value) {
  let (_, schema) = run_json(&environment(space.repo.path()), &["schema", "--all"]);
  assert_covered(&schema["outputs"]["robot"], &schema["outputs"]["robot"], output, "robot");
}
