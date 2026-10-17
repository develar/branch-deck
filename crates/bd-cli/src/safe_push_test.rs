use crate::context::Environment;
use crate::fixture_test::{ACP_HOST, PREFIX, assert_error, commit_all, commit_dangling_reference, environment, git, run_json, write};
use crate::push_test::repo_with_remote;
use crate::schema_test::assert_covered;
use github::fake_github::{FakeAnswer, FakeGitHub, start_fake as start_fake_github};
use github::token::TokenSources;
use patronus::fake_patronus::{FakePatronus, robot_json, start_fake};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tempfile::TempDir;
use test_utils::fake_executable::fake_executable;
use test_utils::git_test_utils::{TestRepo, git_command};
use tokio::runtime::Runtime;

pub(crate) const AUTH: &str = "test-user/virtual/auth";

/// A repository whose `origin` is on Space (`ssh://git@git.jetbrains.team/ij/ultimate.git`), served by a local bare
/// repository through a fake `ssh`, and a Patronus that answers for it
pub(crate) struct Space {
  pub(crate) repo: TestRepo,
  remote: TempDir,
  pub(crate) home: TempDir,
  pub(crate) fake: Arc<FakePatronus>,
  patronus_url: String,
  /// Serves the fake Patronus (and a fake GitHub) while the test runs
  pub(crate) runtime: Runtime,
}

impl Space {
  pub(crate) fn new(subjects: &[&str]) -> Self {
    let (repo, remote) = repo_with_remote(subjects);
    let home = tempfile::tempdir().unwrap();
    // git runs `<ssh> [options] git@git.jetbrains.team "git-receive-pack '/ij/ultimate.git'"`: serve the bare repository
    let ssh = fake_executable(
      home.path(),
      "ssh",
      &format!(
        "for last; do :; done\ncase \"$last\" in\n  *receive-pack*) exec git receive-pack '{0}' ;;\n  *) exec git upload-pack '{0}' ;;\nesac",
        remote.path().display()
      ),
    );
    git(&repo, &["config", "core.sshCommand", ssh.to_str().unwrap()]);
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

  pub(crate) fn environment(&self) -> Environment {
    Environment {
      home: Some(self.home.path().to_path_buf()),
      patronus_url: self.patronus_url.clone(),
      ..environment(self.repo.path())
    }
  }

  fn run(&self, args: &[&str]) -> (i32, Value) {
    safe_push(&self.environment(), args)
  }

  fn remote_tip(&self, branch: &str) -> String {
    let output = git_command()
      .args(["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")])
      .current_dir(self.remote.path())
      .output()
      .unwrap();
    String::from_utf8(output.stdout).unwrap().trim().to_string()
  }

  pub(crate) fn local_tip(&self, branch: &str) -> String {
    git(&self.repo, &["rev-parse", &format!("refs/heads/{branch}")])
  }

  pub(crate) fn set_robots(&self, robots: Vec<Value>) {
    *self.fake.robots.lock().unwrap() = Some(robots);
  }

  fn last_start(&self) -> Value {
    self.fake.last_start.lock().unwrap().clone()
  }
}

/// `bd safe-push` with `args`
fn safe_push(environment: &Environment, args: &[&str]) -> (i32, Value) {
  let mut all_args = vec!["--prefix", PREFIX, "safe-push"];
  all_args.extend_from_slice(args);
  run_json(environment, &all_args)
}

/// A robot of the user on the virtual branch `name`, at `revision`
pub(crate) fn robot(id: &str, status: &str, name: &str, revision: &str) -> Value {
  let mut robot = robot_json(id, status, &format!("refs/heads/test-user/virtual/{name}"), "2026-10-14T09:30:00Z");
  robot["sourceRevision"] = json!(revision);
  robot
}

#[test_log::test]
fn check_names_the_profiles_and_the_review_and_starts_nothing() {
  let space = Space::new(&["(auth) Add auth"]);
  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  let (_, schema) = run_json(&environment(space.repo.path()), &["schema"]);
  assert_covered(&schema["outputs"]["safe_push"], &schema["outputs"]["safe_push"], &output, "safe_push");
  assert_eq!(output["mode"], json!("safe_push"));
  assert_eq!(output["target"], json!("master"));
  // the emergency profile is never offered; no robot of the branch yet, so no default
  assert_eq!(
    output["profiles"],
    json!([{"id": "default", "name": "Default"}, {"id": "lsc", "name": "Large Scale Change", "description": "All the tests"}])
  );
  assert_eq!(output["profile"], Value::Null);
  assert_eq!(
    output["review"],
    json!({"required": false, "skipped": false, "counts_in": "space_merge_request", "merge_request": null, "groups": []})
  );
  assert_eq!((output["push_needed"].clone(), output["robot"].clone()), (json!(true), Value::Null));
  assert_eq!(space.fake.start_calls.load(Ordering::SeqCst), 0);
  assert_eq!(space.remote_tip(AUTH), "");
}

#[test_log::test]
fn missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway() {
  let space = Space::new(&["(auth) Add auth"]);
  space.fake.review_required.store(true, Ordering::SeqCst);

  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (4, "approvals_missing"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains("Needs review by Air Plugin Release Team (suggested: Alice)"), "{message}");
  // without a GitHub repository nothing can collect them
  assert!(message.contains("git config branchdeck.githubRepository"), "{message}");
  assert!(message.contains("bd safe-push auth --create-pull-request"), "{message}");
  assert_eq!(output["error"]["review"]["groups"][0]["reviewers"][0]["username"], json!("alice"));
  assert_eq!((space.fake.start_calls.load(Ordering::SeqCst), space.remote_tip(AUTH)), (0, String::new()));

  let (exit_code, output) = space.run(&["auth", "--create-pull-request"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  // --check reports it instead of failing
  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["refusals"][0]["code"], json!("approvals_missing"));

  let (exit_code, output) = space.run(&["auth", "--without-approvals"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["started_via"], json!("api"));
  assert_eq!(output["robot"]["id"], json!("started"));
  assert_eq!(space.remote_tip(AUTH), space.local_tip(AUTH));
  let start = space.last_start();
  assert_eq!(start["sourceBranch"], json!(format!("refs/heads/{AUTH}")));
  assert_eq!(start["mode"], json!("REBASE_AUTOSQUASH"));
  assert_eq!(start.get("spaceReview"), None, "{start}");
}

#[test_log::test]
fn a_dry_run_takes_a_profile_by_name_and_checks_no_review() {
  let space = Space::new(&["(auth) Add auth"]);
  space.fake.review_required.store(true, Ordering::SeqCst);

  let (exit_code, output) = space.run(&["auth", "--dry-run", "--profile", "large scale change"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["mode"].clone(), output["review"].clone()), (json!("dry_run"), Value::Null));
  assert_eq!(output["profile"]["id"], json!("lsc"));
  let start = space.last_start();
  assert_eq!((start["mode"].clone(), start["mergeProfileId"].clone()), (json!("DRY_RUN"), json!("lsc")));
  assert_eq!(space.fake.check_calls.load(Ordering::SeqCst), 0);

  let (exit_code, output) = space.run(&["auth", "--dry-run", "--profile", "nightly"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  assert!(output["error"]["message"].as_str().unwrap().contains("lsc (Large Scale Change)"), "{output}");
}

#[test_log::test]
fn the_profile_of_the_last_robot_is_the_default_and_none_drops_it() {
  let space = Space::new(&["(auth) Add auth"]);
  // its details name the Large Scale Change profile
  space.set_robots(vec![robot("older", "FAILURE", "auth", "0000")]);

  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["profile"]["id"], json!("lsc"));

  let (exit_code, output) = space.run(&["auth", "--profile", "none"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["profile"], Value::Null);
  assert_eq!(space.last_start()["mergeProfileId"], Value::Null);
}

/// A Safe Push of auth runs on `revision`: `bd safe-push auth` refuses to start another one on the same commits
fn assert_refused_for_a_robot_on(space: &Space, revision: &str) {
  space.set_robots(vec![robot("running", "RUNNING", "auth", revision)]);
  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  assert_eq!(output["error"]["message"], json!("A Safe Push of the last commit is running"));
}

#[test_log::test]
fn a_robot_on_the_last_commit_refuses_and_one_on_an_older_commit_is_canceled_when_told() {
  let space = Space::new(&["(auth) Add auth"]);
  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  let tip = space.local_tip(AUTH);

  assert_refused_for_a_robot_on(&space, &tip);

  // a fix since: the push would fail the robot of the older commit
  space.set_robots(vec![robot("running", "RUNNING", "auth", "0000")]);
  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  assert!(output["error"]["message"].as_str().unwrap().contains("--cancel-running"), "{output}");
  assert_eq!(space.remote_tip(AUTH), "");

  let (exit_code, output) = space.run(&["auth", "--cancel-running"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["canceled"]["id"], json!("running"));
  assert_eq!(*space.fake.canceled.lock().unwrap(), vec!["running".to_string()]);
  assert_eq!(output["robot"]["id"], json!("started"));
}

/// Pushes auth (`bd push`), then moves the baseline on and the user's branch onto it, so the next sync rebuilds auth on
/// it: the remote branch has its commits by content, under other hashes. Returns the pushed commit.
fn push_and_rebuild_on_a_newer_baseline(space: &Space, environment: &Environment) -> String {
  let (exit_code, output) = run_json(environment, &["--prefix", PREFIX, "push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let pushed = space.remote_tip(AUTH);
  let baseline = git(&space.repo, &["rev-parse", "refs/remotes/origin/master"]);
  let upstream = git(&space.repo, &["commit-tree", &format!("{baseline}^{{tree}}"), "-p", &baseline, "-m", "Upstream change"]);
  git(&space.repo, &["update-ref", "refs/remotes/origin/master", &upstream]);
  git(&space.repo, &["rebase", "-q", &upstream]);

  let (exit_code, output) = safe_push(environment, &["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["push_needed"], json!(false));
  assert_ne!(space.local_tip(AUTH), pushed, "the sync rebuilt auth on the newer baseline");
  pushed
}

#[test_log::test]
fn a_branch_only_rebuilt_on_a_newer_baseline_starts_from_its_remote_branch_without_a_push() {
  let space = Space::new(&["(auth) Add auth"]);
  let pushed = push_and_rebuild_on_a_newer_baseline(&space, &space.environment());

  // a robot on the remote branch runs on the same commits: a start would repeat it
  assert_refused_for_a_robot_on(&space, &pushed);

  space.set_robots(vec![robot("older", "FAILURE", "auth", &pushed)]);
  let (exit_code, output) = space.run(&["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["push_needed"].clone(), output.get("pushed")), (json!(false), None));
  assert_eq!(output["robot"]["id"], json!("started"));
  assert_eq!(space.remote_tip(AUTH), pushed, "nothing was pushed");
  assert_eq!(space.last_start()["sourceBranch"], json!(format!("refs/heads/{AUTH}")));
}

#[test_log::test]
fn a_stacked_branch_leads_root_first_or_lands_its_parents_when_told() {
  let space = Space::new(&["(auth) Add auth", "(auth/login) Add login"]);
  space.repo.set_config("branchdeck.stacks", "true").unwrap();

  let (exit_code, output) = space.run(&["login"]);
  assert_error(&output, exit_code, (4, "stack_not_landed"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains("Safe Push auth first"), "{output}");
  // its commit changes a file of its own: it builds alone
  assert!(message.contains("login builds alone on the baseline: `bd unstack login` makes it a root"), "{output}");

  let (exit_code, output) = space.run(&["login", "--land-stack"]);
  assert_eq!(exit_code, 0, "{output}");
  let pushed: Vec<&Value> = output["pushed"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(pushed, [&json!("auth"), &json!("login")]);
  assert_eq!(space.last_start()["sourceBranch"], json!("refs/heads/test-user/virtual/login"));

  // a dry run checks the parents' commits too, and lands nothing: no --land-stack
  let (exit_code, output) = space.run(&["login", "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(space.last_start()["mode"], json!("DRY_RUN"));
}

#[test_log::test]
fn a_stacked_branch_that_needs_a_commit_of_its_parent_names_it() {
  let space = Space::new(&["(auth) Add auth", "(auth/login) Add login"]);
  space.repo.set_config("branchdeck.stacks", "true").unwrap();
  // a second commit of login changes the file the commit of auth adds
  write(&space.repo, "f0.txt", "(auth) Add auth, for login");
  commit_all(&space.repo, "(auth/login) Use auth", 1_700_000_500);
  let auth = git(&space.repo, &["rev-parse", "HEAD~2"])[..8].to_string();

  let (exit_code, output) = space.run(&["login", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  let refusal = output["refusals"]
    .as_array()
    .unwrap()
    .iter()
    .find(|refusal| refusal["code"] == json!("stack_not_landed"))
    .unwrap();
  assert!(
    refusal["message"]
      .as_str()
      .unwrap()
      .ends_with(&format!("login needs {auth} (auth): move it into the branch, then `bd unstack login`")),
    "{output}"
  );
}

#[test_log::test]
fn without_a_token_or_space_nothing_runs() {
  let space = Space::new(&["(auth) Add auth"]);
  let environment = Environment {
    home: Some(Path::new(space.repo.path()).join("no-home")),
    ..space.environment()
  };
  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_error(&output, exit_code, (4, "not_logged_in"));
  assert!(output["error"]["message"].as_str().unwrap().contains("./safePush.cmd"), "{output}");

  git(&space.repo, &["remote", "set-url", "origin", space.remote.path().to_str().unwrap()]);
  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (4, "not_on_space"));
  assert_eq!(space.fake.list_calls.load(Ordering::SeqCst), 0);
}

/// A fake GitHub of the repository `acme/monorepo` and its URL: the token is of `me`, the search lists
/// `pull_requests`, a created pull request is #7 of auth (assigned and reviewed as asked), the `/patronus` comment is
/// accepted at once
fn fake_github(space: &Space, pull_requests: &[Value]) -> (Arc<FakeGitHub>, String) {
  let (fake, client) = space.runtime.block_on(start_fake_github());
  list_pull_requests(&fake, pull_requests);
  fake.answer("GET", "/user", FakeAnswer::ok(json!({"login": "me"})));
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

/// auth needs the code-owner review, which counts in its GitHub pull request (`fake_github`, `reviewed_on_github`)
fn needs_review_on_github() -> (Space, Arc<FakeGitHub>, Environment) {
  let space = Space::new(&["(auth) Add auth"]);
  space.fake.review_required.store(true, Ordering::SeqCst);
  let (github, url) = fake_github(&space, &[]);
  let environment = reviewed_on_github(&space, &url);
  (space, github, environment)
}

/// `bd safe-push auth --create-pull-request`, which succeeds
fn create_pull_request(environment: &Environment) -> Value {
  let (exit_code, output) = safe_push(environment, &["auth", "--create-pull-request"]);
  assert_eq!(exit_code, 0, "{output}");
  output
}

/// The search of the user's pull requests answers `pull_requests`
fn list_pull_requests(fake: &FakeGitHub, pull_requests: &[Value]) {
  fake.answer("POST", "/graphql", search_answer(pull_requests));
}

fn search_answer(pull_requests: &[Value]) -> FakeAnswer {
  FakeAnswer::ok(json!({"data": {"repository": {"viewerPermission": "WRITE"}, "search": {"nodes": pull_requests}}}))
}

/// The open pull request #7 of auth into master, approved by `approvers`
fn pull_request_json(approvers: &[&str]) -> Value {
  let reviews: Vec<Value> = approvers.iter().map(|login| json!({"author": {"login": login}, "state": "APPROVED"})).collect();
  json!({"number": 7, "title": "Add auth", "url": "https://github.com/acme/monorepo/pull/7", "state": "OPEN", "isDraft": false,
    "headRefName": AUTH, "headRefOid": "1111", "baseRefName": "master", "headRepositoryOwner": {"login": "acme"},
    "isCrossRepository": false, "updatedAt": "2026-10-14T09:30:00Z", "latestReviews": {"nodes": reviews}})
}

/// The branches are reviewed on GitHub (`acme/monorepo`), through the fake GitHub at `github_url`, asked with the token
/// `t`; the robot of the comment on pull request #7 is listed
fn reviewed_on_github(space: &Space, github_url: &str) -> Environment {
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

#[test_log::test]
fn a_branch_with_an_open_pull_request_starts_from_a_comment() {
  let space = Space::new(&["(auth) Add auth"]);
  let (github, url) = fake_github(&space, &[pull_request_json(&[])]);
  let environment = reviewed_on_github(&space, &url);

  // a comment can't choose a profile
  let (exit_code, output) = safe_push(&environment, &["auth", "--profile", "lsc"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["started_via"], json!("pull_request"));
  assert_eq!(output["review"]["counts_in"], json!("pull_request"));
  assert_eq!(
    output["pull_request"],
    json!({"number": 7, "url": "https://github.com/acme/monorepo/pull/7", "comment_url": "https://github.com/acme/monorepo/pull/7#issuecomment-4402", "reply": "accepted"})
  );
  assert_eq!(output["robot"]["id"], json!("from-comment"));
  assert_eq!(space.fake.start_calls.load(Ordering::SeqCst), 0);
  // branches reviewed on GitHub: no Space merge request is looked up
  assert_eq!(space.fake.merge_request_calls.load(Ordering::SeqCst), 0);
  assert_eq!(
    github.requests_to("POST", "/repos/acme/monorepo/issues/7/comments")[0].json(),
    json!({"body": "/patronus merge --keep-source-branch"})
  );
}

#[test_log::test]
fn the_approvals_count_in_the_pull_request_it_opens_and_the_safe_push_starts_from_it() {
  let (space, github, environment) = needs_review_on_github();

  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_error(&output, exit_code, (4, "approvals_missing"));
  assert!(output["error"]["message"].as_str().unwrap().contains("bd safe-push auth --create-pull-request"), "{output}");

  let output = create_pull_request(&environment);
  assert_eq!(
    output["pull_request"],
    json!({"number": 7, "url": "https://github.com/acme/monorepo/pull/7", "created": true, "comment_url": null, "reply": null, "assignees": ["me"]})
  );
  assert_eq!((output["robot"].clone(), space.fake.start_calls.load(Ordering::SeqCst)), (Value::Null, 0));
  assert_eq!(space.remote_tip(AUTH), space.local_tip(AUTH));
  // filled as `gh pr create --fill`: the one commit's subject
  assert_eq!(
    github.requests_to("POST", "/repos/acme/monorepo/pulls")[0].json(),
    json!({"title": "Add auth", "body": "", "head": AUTH, "base": "master"})
  );

  // not approved yet: the error names the pull request
  list_pull_requests(&github, &[pull_request_json(&[])]);
  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_error(&output, exit_code, (4, "approvals_missing"));
  assert!(output["error"]["message"].as_str().unwrap().contains("pull request #7"), "{output}");

  // the robot of the pull request counts its approvals, which the check before it can't see
  list_pull_requests(&github, &[pull_request_json(&["alice"])]);
  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["review"]["approved_on_github"], json!(["alice"]));
  assert_eq!(output["started_via"], json!("pull_request"));
}

#[test_log::test]
fn a_pull_request_without_a_github_token_is_not_logged_in() {
  let space = Space::new(&["(auth) Add auth"]);
  let (github, url) = fake_github(&space, &[]);
  let environment = Environment {
    github_tokens: TokenSources::default(),
    ..reviewed_on_github(&space, &url)
  };
  let (exit_code, output) = safe_push(&environment, &["auth", "--create-pull-request"]);
  assert_error(&output, exit_code, (4, "not_logged_in"));
  assert!(github.requests().is_empty());
}

#[test_log::test]
fn a_pull_request_that_already_exists_is_found_instead() {
  let space = Space::new(&["(auth) Add auth", "(auth) Test auth"]);
  let (github, url) = fake_github(&space, &[]);
  let environment = reviewed_on_github(&space, &url);
  github.answer(
    "POST",
    "/repos/acme/monorepo/pulls",
    FakeAnswer::status(
      422,
      json!({"message": "Validation Failed", "errors": [{"resource": "PullRequest", "code": "custom", "message": format!("A pull request already exists for acme:{AUTH}.")}]}),
    ),
  );
  // GitHub lists it once the search sees it, e.g. after the plan of this run
  github.answer_in_turn("POST", "/graphql", vec![search_answer(&[]), search_answer(&[pull_request_json(&[])])]);
  let output = create_pull_request(&environment);
  assert_eq!(
    output["pull_request"],
    json!({"number": 7, "url": "https://github.com/acme/monorepo/pull/7", "comment_url": null, "reply": null,
      "warnings": ["pull request #7 already existed: it was left as it is, without the assignee and the reviewers"]})
  );
  assert!(github.requests_to("POST", "/repos/acme/monorepo/issues/7/assignees").is_empty());
  // filled as `gh pr create --fill` from two commits: the branch name and their subjects
  assert_eq!(
    github.requests_to("POST", "/repos/acme/monorepo/pulls")[0].json(),
    json!({"title": "auth", "body": "- **Add auth**\n- **Test auth**", "head": AUTH, "base": "master"})
  );
}

#[test_log::test]
fn the_pull_request_of_a_branch_only_rebuilt_on_a_newer_baseline_is_opened_without_a_push() {
  let (space, github, environment) = needs_review_on_github();
  let pushed = push_and_rebuild_on_a_newer_baseline(&space, &environment);
  let output = create_pull_request(&environment);
  assert_eq!((output["pull_request"]["created"].clone(), output.get("pushed")), (json!(true), None));
  assert_eq!(space.remote_tip(AUTH), pushed, "nothing was pushed");
  assert_eq!(
    github.requests_to("POST", "/repos/acme/monorepo/pulls")[0].json(),
    json!({"title": "Add auth", "body": "", "head": AUTH, "base": "master"})
  );
}

#[test_log::test]
fn the_pull_request_it_opens_asks_the_suggested_code_owner_by_the_github_login_patronus_names() {
  let (_space, github, environment) = needs_review_on_github();
  let (exit_code, output) = safe_push(&environment, &["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  let login = output["review"]["groups"][0]["reviewers"][0]["github_login"].clone();
  assert_eq!(login, json!("alice-gh"));

  let (exit_code, output) = safe_push(&environment, &["auth", "--create-pull-request", "--reviewer", "alice-gh", "--no-assign"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["pull_request"]["reviewers"], json!(["alice-gh"]));
  assert_eq!(output["pull_request"].get("assignees"), None);
  assert_eq!(
    github.requests_to("POST", "/repos/acme/monorepo/pulls/7/requested_reviewers")[0].json(),
    json!({"reviewers": ["alice-gh"]})
  );
  assert!(github.requests_to("POST", "/repos/acme/monorepo/issues/7/assignees").is_empty());
}

/// The Safe Push failure of 2026-10-16 is refused before anything is pushed: the build would fail on the use that
/// another branch changes
#[test_log::test]
fn a_dangling_reference_refuses_the_safe_push_unless_allowed() {
  let space = Space::new(&[]);
  let (removed_by, fixed_by) = commit_dangling_reference(&space.repo);

  let (exit_code, output) = space.run(&["fake-agent"]);
  assert_error(&output, exit_code, (4, "dangling_reference"));
  assert_eq!(
    output["error"]["message"],
    json!(format!(
      "fake-agent removes CONTROL_URL_ENV in {removed}, and {ACP_HOST}:7 still uses it: {fixed} changes that use in another branch, so the build fails. To fix it, move {fixed} into fake-agent: it changes only {ACP_HOST}; --allow-dangling starts anyway",
      removed = &removed_by[..8],
      fixed = &fixed_by[..8],
    ))
  );
  assert_eq!(space.remote_tip("test-user/virtual/fake-agent"), "", "nothing was pushed");

  let codes = |output: &Value| -> Vec<Value> {
    output["refusals"]
      .as_array()
      .map_or_default(|refusals| refusals.iter().map(|refusal| refusal["code"].clone()).collect())
  };
  let (_, output) = space.run(&["fake-agent", "--check"]);
  assert!(codes(&output).contains(&json!("dangling_reference")), "{output}");
  let (_, output) = space.run(&["fake-agent", "--check", "--allow-dangling"]);
  assert!(!codes(&output).contains(&json!("dangling_reference")), "{output}");
}

#[test_log::test]
fn push_refuses_a_branch_whose_safe_push_runs_and_restarts_it_when_told() {
  let space = Space::new(&["(auth) Add auth", "(auth/login) Add login"]);
  space.repo.set_config("branchdeck.stacks", "true").unwrap();
  let push = |args: &[&str]| {
    let mut all_args = vec!["--prefix", PREFIX, "push"];
    all_args.extend_from_slice(args);
    run_json(&space.environment(), &all_args)
  };

  space.set_robots(vec![robot("running", "RUNNING", "auth", "0000")]);
  let (exit_code, output) = push(&["auth"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  assert!(output["error"]["message"].as_str().unwrap().contains("--restart-running"), "{output}");
  assert_eq!(space.remote_tip(AUTH), "");

  // login is built on auth: its push starts above it, auth stays as the robot took it
  let (exit_code, output) = push(&["login"]);
  assert_error(&output, exit_code, (4, "push_refused"));
  assert!(output["error"]["message"].as_str().unwrap().contains("its parent auth is not on the remote"), "{output}");

  let (exit_code, output) = push(&["auth", "--restart-running"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(*space.fake.canceled.lock().unwrap(), vec!["running".to_string()]);
  assert_eq!(space.fake.restart_calls.load(Ordering::SeqCst), 1);
  assert_eq!(output["restarted"].as_array().map(Vec::len), Some(1), "{output}");
  assert_eq!(space.remote_tip(AUTH), space.local_tip(AUTH));

  // a canceling or a dry-run robot holds nothing
  space.set_robots(vec![robot("canceling", "CANCELING", "login", "0000")]);
  let (exit_code, output) = push(&["login"]);
  assert_eq!(exit_code, 0, "{output}");
}
