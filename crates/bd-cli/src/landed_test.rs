use crate::fixture_test::{PREFIX, assert_error, commit_all, git, run_json, write};
use crate::safe_push_test::{Space, forking_stack, robot};
use crate::schema_test::assert_covered;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// `auth` and `login` built on it on Space, synced: the Safe Push of login merged on login as the sync built it
fn merged_login() -> Space {
  let space = Space::new(&["(auth) Add auth", "(auth/login) Add login"]);
  space.repo.set_config("branchdeck.stacks", "true").unwrap();
  sync(&space);
  space.set_robots(vec![merged(&space, "merged", "login", "2026-10-14T11:00:00Z")]);
  space
}

fn sync(space: &Space) {
  let (exit_code, output) = bd(space, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
}

fn bd(space: &Space, args: &[&str]) -> (i32, Value) {
  let mut all_args = vec!["--prefix", PREFIX];
  all_args.extend_from_slice(args);
  run_json(&space.environment(), &all_args)
}

/// `landed_by` of `name` in `bd status`
fn landed_by<'a>(status: &'a Value, name: &str) -> &'a Value {
  let branch = status["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!(name)).unwrap();
  &branch["landed_by"]
}

/// The short id of `rev`, as `bd` prints it
fn short(space: &Space, rev: &str) -> String {
  git(&space.repo, &["rev-parse", rev])[..8].to_string()
}

#[test_log::test]
fn status_names_the_branches_a_merged_safe_push_landed_and_what_is_left_of_them() {
  let space = merged_login();
  let (exit_code, status) = bd(&space, &["status"]);
  assert_eq!(exit_code, 0, "{status}");
  let (_, schema) = run_json(&space.environment(), &["schema"]);
  assert_covered(&schema["outputs"]["status"], &schema["outputs"]["status"], &status, "status");
  let (auth, login) = (short(&space, "HEAD~1"), short(&space, "HEAD"));
  // login is the robot's own, auth it is built on: all of both landed
  assert_eq!(landed_by(&status, "auth")["branch"], json!("login"));
  assert_eq!(landed_by(&status, "auth")["commits"], json!([auth]));
  assert_eq!(landed_by(&status, "auth").get("after"), None);
  assert_eq!(landed_by(&status, "login")["commits"], json!([login]));
  assert_eq!(landed_by(&status, "login")["robot"]["state"], json!("success"));

  // a new commit of auth rebuilds login on it: by content, the new commit is the only work left of auth
  write(&space.repo, "token.txt", "token\n");
  let check = commit_all(&space.repo, "(auth) Check the token", 1_700_000_900)[..8].to_string();
  sync(&space);
  let (_, status) = bd(&space, &["status"]);
  assert_eq!(landed_by(&status, "auth")["after"], json!([check]));
  assert_eq!(landed_by(&status, "auth")["commits"], json!([auth]));
  assert_eq!(landed_by(&status, "login").get("after"), None);

  // the brief view says it compactly
  let (_, brief) = bd(&space, &["status", "--brief"]);
  assert_covered(&schema["outputs"]["status_brief"], &schema["outputs"]["status_brief"], &brief, "status_brief");
  let mut briefs: Vec<(String, Value)> = brief["branches"]
    .as_array()
    .unwrap()
    .iter()
    .map(|branch| (branch["name"].as_str().unwrap().to_string(), branch["landed_by"].clone()))
    .collect();
  briefs.sort_by(|a, b| a.0.cmp(&b.0));
  assert_eq!(
    briefs,
    [
      ("auth".to_string(), json!({"branch": "login", "after": 1})),
      ("login".to_string(), json!({"branch": "login", "after": 0})),
    ]
  );
}

#[test_log::test]
fn push_and_safe_push_refuse_a_branch_a_merged_safe_push_landed_whole() {
  let space = merged_login();
  let (_, status) = bd(&space, &["status"]);
  let url = landed_by(&status, "login")["robot"]["url"].as_str().unwrap().to_string();
  for (args, message) in [
    (
      vec!["push", "login"],
      format!("Nothing was pushed: login merged into master with its Safe Push ({url}): nothing of it is left to push"),
    ),
    (
      vec!["push", "auth"],
      format!("Nothing was pushed: auth landed in master with the Safe Push of login ({url}): nothing of it is left to push"),
    ),
    (
      vec!["safe-push", "login"],
      format!("Nothing was pushed or started: login merged into master with its Safe Push ({url}): nothing of it is left to land"),
    ),
  ] {
    let (exit_code, output) = bd(&space, &args);
    assert_error(&output, exit_code, (4, "landed"));
    assert_eq!(output["error"]["message"], json!(message), "{args:?}");
  }
  let (exit_code, check) = bd(&space, &["safe-push", "login", "--check"]);
  assert_eq!((exit_code, check["refusals"][0]["code"].clone()), (0, json!("landed")), "{check}");

  // a new commit of auth is work left: auth pushes and lands it, login stays landed
  write(&space.repo, "token.txt", "token\n");
  commit_all(&space.repo, "(auth) Check the token", 1_700_000_900);
  let (exit_code, output) = bd(&space, &["push", "auth", "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  let (exit_code, check) = bd(&space, &["safe-push", "auth", "--check"]);
  assert_eq!((exit_code, check.get("refusals"), check["lands"].clone()), (0, None, json!(["auth"])), "{check}");
  let (exit_code, output) = bd(&space, &["safe-push", "login", "--check"]);
  assert_eq!((exit_code, output["refusals"][0]["code"].clone()), (0, json!("landed")), "{output}");
}

/// A merged robot of `name` at the tip of its virtual branch, finished at `finished`
fn merged(space: &Space, id: &str, name: &str, finished: &str) -> Value {
  let mut merged = robot(id, "SUCCESS", name, &space.local_tip(&format!("test-user/virtual/{name}")));
  merged["finishDateTime"] = json!(finished);
  merged
}

/// The whole stack lands only what pieces left of it, and nothing once every branch of it landed
#[test_log::test]
fn the_whole_stack_lands_what_is_left_of_it_and_refuses_once_every_branch_of_it_landed() {
  let space = forking_stack();
  sync(&space);
  // the Safe Push of login landed auth and login: session is left
  space.set_robots(vec![merged(&space, "login-robot", "login", "2026-10-14T11:00:00Z")]);
  let (exit_code, output) = bd(&space, &["safe-push", "auth", "--whole-stack", "--check"]);
  assert_eq!((exit_code, output["lands"].clone(), output.get("refusals")), (0, json!(["session"]), None), "{output}");

  // session landed with its own Safe Push too: nothing of the whole stack is left
  space.set_robots(vec![
    merged(&space, "login-robot", "login", "2026-10-14T11:00:00Z"),
    merged(&space, "session-robot", "session", "2026-10-14T12:00:00Z"),
  ]);
  let (exit_code, output) = bd(&space, &["safe-push", "auth", "--whole-stack"]);
  assert_error(&output, exit_code, (4, "landed"));
  assert_eq!(
    output["error"]["message"],
    json!("Nothing was pushed or started: every branch of the whole auth stack landed in master with the Safe Pushes of login and session: nothing of it is left to land")
  );
}
