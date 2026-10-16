use crate::context::Environment;
use crate::fixture_test::{PREFIX, assert_error, environment, git, run_json};
use crate::push_test::repo_with_remote;
use crate::schema_test::assert_covered;
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

const AUTH: &str = "test-user/virtual/auth";

/// A repository whose `origin` is on Space (`ssh://git@git.jetbrains.team/ij/ultimate.git`), served by a local bare
/// repository through a fake `ssh`, and a Patronus that answers for it
struct Space {
  repo: TestRepo,
  remote: TempDir,
  home: TempDir,
  fake: Arc<FakePatronus>,
  patronus_url: String,
  /// Serves the fake Patronus while the test runs
  _runtime: Runtime,
}

impl Space {
  fn new(subjects: &[&str]) -> Self {
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
      _runtime: runtime,
    }
  }

  fn environment(&self) -> Environment {
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

  fn local_tip(&self, branch: &str) -> String {
    git(&self.repo, &["rev-parse", &format!("refs/heads/{branch}")])
  }

  fn set_robots(&self, robots: Vec<Value>) {
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
fn robot(id: &str, status: &str, name: &str, revision: &str) -> Value {
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
  assert!(message.contains("bd safe-push auth --create-merge-request"), "{message}");
  assert_eq!(output["error"]["review"]["groups"][0]["reviewers"][0]["username"], json!("alice"));
  assert_eq!((space.fake.start_calls.load(Ordering::SeqCst), space.remote_tip(AUTH)), (0, String::new()));

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
fn a_merge_request_collects_the_approvals_and_the_start_links_it() {
  let space = Space::new(&["(auth) Add auth"]);
  space.fake.review_required.store(true, Ordering::SeqCst);

  let (exit_code, output) = space.run(&["auth", "--create-merge-request"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["merge_request"], json!({"key": "IJ-CR-2", "url": "https://jetbrains.team/p/ij/reviews/2"}));
  assert_eq!(output["robot"], Value::Null);
  assert_eq!(space.fake.start_calls.load(Ordering::SeqCst), 0);
  assert_eq!(space.remote_tip(AUTH), space.local_tip(AUTH));

  // not approved yet: the error names the merge request
  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (4, "approvals_missing"));
  assert!(output["error"]["message"].as_str().unwrap().contains("Waiting for approval in IJ-CR-2"), "{output}");

  space.fake.review_approved.store(true, Ordering::SeqCst);
  let (exit_code, output) = space.run(&["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["review"]["merge_request"]["key"], json!("IJ-CR-2"));
  assert_eq!(space.last_start()["spaceReview"], json!({"projectKey": "IJ", "reviewKey": "IJ-CR-2"}));
  // nothing new to push
  assert_eq!(output["pushed"][0]["status"], json!("up_to_date"));
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

#[test_log::test]
fn a_robot_on_the_last_commit_refuses_and_one_on_an_older_commit_is_canceled_when_told() {
  let space = Space::new(&["(auth) Add auth"]);
  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  let tip = space.local_tip(AUTH);

  space.set_robots(vec![robot("running", "RUNNING", "auth", &tip)]);
  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  assert_eq!(output["error"]["message"], json!("A Safe Push of the last commit is running"));

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

#[test_log::test]
fn a_stacked_branch_leads_root_first_or_lands_its_parents_when_told() {
  let space = Space::new(&["(auth) Add auth", "(auth/login) Add login"]);
  space.repo.set_config("branchdeck.stacks", "true").unwrap();

  let (exit_code, output) = space.run(&["login"]);
  assert_error(&output, exit_code, (4, "stack_not_landed"));
  assert!(output["error"]["message"].as_str().unwrap().contains("Safe Push auth first"), "{output}");

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

#[test_log::test]
fn a_branch_with_an_open_pull_request_starts_from_a_comment() {
  let space = Space::new(&["(auth) Add auth"]);
  git(&space.repo, &["config", "branchdeck.githubRepository", "acme/monorepo"]);
  let gh_dir = tempfile::tempdir().unwrap();
  let gh = fake_executable(
    gh_dir.path(),
    "gh",
    &format!(
      r#"printf '%s\n' "$@" >> '{args}'
case "$1 $2 $3" in
  "pr list "*) echo '[{{"number": 7, "title": "Add auth", "url": "https://github.com/acme/monorepo/pull/7", "state": "OPEN", "isDraft": false, "headRefName": "{AUTH}", "headRefOid": "1111", "baseRefName": "master", "headRepositoryOwner": {{"login": "acme"}}, "isCrossRepository": false, "updatedAt": "2026-10-14T09:30:00Z"}}]' ;;
  "api --method POST") echo '{{"id": 4402, "html_url": "https://github.com/acme/monorepo/pull/7#issuecomment-4402", "created_at": "2026-10-14T09:30:00Z"}}' ;;
  *) echo '{{"id": 4402, "html_url": "https://github.com/acme/monorepo/pull/7#issuecomment-4402", "created_at": "2026-10-14T09:30:00Z", "reactions": {{"+1": 1, "-1": 0}}}}' ;;
esac"#,
      args = gh_dir.path().join("args").display()
    ),
  );
  let mut started = robot("from-comment", "RUNNING", "other", "0000");
  started["githubPrUrl"] = json!("https://github.com/acme/monorepo/pull/7");
  space.set_robots(vec![started]);
  let environment = Environment {
    gh: Some(gh),
    ..space.environment()
  };

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
  let args = std::fs::read_to_string(gh_dir.path().join("args")).unwrap();
  assert!(args.contains("body=/patronus merge --keep-source-branch"), "{args}");
}
