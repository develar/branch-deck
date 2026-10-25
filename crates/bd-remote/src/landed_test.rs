use bd_cli::fixture_test::assert_covered;
use bd_cli::fixture_test::{PREFIX, assert_error, commit_all, git, run_json, write};
use bd_cli::remote_fixture_test::{STACK, Space, forking_stack, minutes_ago, robot};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// `auth` and `login` built on it on Space, synced: the Safe Push of login merged on login as the sync built it
fn merged_login() -> Space {
  let space = Space::new(&["(auth) Add auth", "(auth/login) Add login"]);
  space.repo.set_config("branchdeck.stacks", "true").unwrap();
  sync(&space);
  space.set_robots(vec![merged(&space, "merged", "login", &minutes_ago(90))]);
  space
}

fn sync(space: &Space) {
  let (exit_code, output) = bd(space, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
}

/// `bd --prefix test-user <args>` in the repository of `space`
pub(crate) fn bd(space: &Space, args: &[&str]) -> (i32, Value) {
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

/// `bd status` of `space`, every field of it in `bd schema`
fn covered_status(space: &Space) -> Value {
  let (exit_code, status) = bd(space, &["status"]);
  assert_eq!(exit_code, 0, "{status}");
  let (_, schema) = run_json(&space.environment(), &["schema", "--all"]);
  assert_covered(&schema["outputs"]["status"], &schema["outputs"]["status"], &status, "status");
  status
}

#[test_log::test]
fn status_names_the_branches_a_merged_safe_push_landed_and_what_is_left_of_them() {
  let space = merged_login();
  let status = covered_status(&space);
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
  let (_, schema) = run_json(&space.environment(), &["schema", "--all"]);
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

/// The names of the objects of the array `key` of `output`
fn names<'a>(output: &'a Value, key: &str) -> Vec<&'a Value> {
  output[key].as_array().map_or_else(Vec::new, |items| items.iter().map(|item| &item["name"]).collect())
}

#[test_log::test]
fn push_leaves_out_a_named_branch_a_merged_safe_push_landed_whole_and_pushes_the_others() {
  // auth and login built on it, docs apart: the Safe Push of login merged, docs has work
  let space = Space::new(&["(auth) Add auth", "(auth/login) Add login", "(docs) Add docs"]);
  space.repo.set_config("branchdeck.stacks", "true").unwrap();
  sync(&space);
  space.set_robots(vec![merged(&space, "merged", "login", &minutes_ago(90))]);
  let (_, status) = bd(&space, &["status"]);
  let url = landed_by(&status, "login")["robot"]["url"].as_str().unwrap().to_string();

  // login is left out with who landed it, docs goes
  let (exit_code, output) = bd(&space, &["push", "login", "docs"]);
  assert_eq!(exit_code, 0, "{output}");
  let (_, schema) = run_json(&space.environment(), &["schema", "--all"]);
  assert_covered(&schema["outputs"]["push"], &schema["outputs"]["push"], &output, "push");
  assert_eq!(names(&output, "branches"), [&json!("docs")], "{output}");
  assert_eq!(names(&output, "skipped"), [&json!("login")], "{output}");
  let skipped = &output["skipped"][0];
  assert_eq!(
    (&skipped["code"], &skipped["message"]),
    (
      &json!("landed"),
      &json!(format!("login merged into master with its Safe Push ({url}): nothing of it is left to push"))
    )
  );
  assert_eq!(
    (&skipped["landed_by"]["branch"], &skipped["landed_by"]["commits"]),
    (&json!("login"), &json!([short(&space, "HEAD~1")]))
  );
  assert_eq!(space.remote_tip("test-user/virtual/docs"), space.local_tip("test-user/virtual/docs"));
  assert_eq!(space.remote_tip("test-user/virtual/login"), "");

  // with --with-children the same: auth and login are left out, docs goes
  let (exit_code, output) = bd(&space, &["push", "auth", "docs", "--with-children", "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(names(&output, "branches"), [&json!("docs")], "{output}");
  assert_eq!(names(&output, "skipped"), [&json!("auth"), &json!("login")], "{output}");
  assert_eq!(
    output["skipped"][0]["message"],
    json!(format!("auth landed in master with the Safe Push of login ({url}): nothing of it is left to push"))
  );

  // nothing asked for is left: refused, nothing pushed
  let (exit_code, output) = bd(&space, &["push", "login", "auth"]);
  assert_error(&output, exit_code, (4, "landed"));
  assert_eq!(
    output["error"]["message"],
    json!(format!(
      "Nothing was pushed: auth landed in master with the Safe Push of login ({url}): nothing of it is left to push; login merged into master with its Safe Push ({url}): nothing of it is left to push"
    ))
  );
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
  space.set_robots(vec![merged(&space, "login-robot", "login", &minutes_ago(90))]);
  let (exit_code, output) = bd(&space, &["safe-push", "auth", "--whole-stack", "--check"]);
  assert_eq!((exit_code, output["lands"].clone(), output.get("refusals")), (0, json!(["session"]), None), "{output}");

  // session landed with its own Safe Push too: nothing of the whole stack is left
  space.set_robots(vec![
    merged(&space, "login-robot", "login", &minutes_ago(90)),
    merged(&space, "session-robot", "session", &minutes_ago(30)),
  ]);
  let (exit_code, output) = bd(&space, &["safe-push", "auth", "--whole-stack"]);
  assert_error(&output, exit_code, (4, "landed"));
  assert_eq!(
    output["error"]["message"],
    json!("Nothing was pushed or started: every branch of the whole auth stack landed in master with the Safe Pushes of login and session: nothing of it is left to land")
  );
}

/// `--with-children` pushes what is left of a stack, as the app's Push with Children does: the branches merged Safe
/// Pushes landed whole stay as they are, and so does the stack branch once every branch of it landed
#[test_log::test]
fn push_with_children_leaves_out_what_merged_safe_pushes_landed_and_refuses_once_nothing_else_is_left() {
  let space = forking_stack();
  sync(&space);
  // the stack branch is on the remote (its pull request put it there); a new commit of session rebuilds it
  git(&space.repo, &["push", "-q", "origin", &format!("refs/heads/{STACK}:refs/heads/{STACK}")]);
  let pushed_before = space.remote_tip(STACK);
  write(&space.repo, "session-more.txt", "more\n");
  commit_all(&space.repo, "(auth/session) More session", 1_700_000_900);
  sync(&space);
  assert_ne!(space.local_tip(STACK), pushed_before);

  // the Safe Pushes of login and session merged: every branch of the stack landed, nothing of it is left to push
  space.set_robots(vec![
    merged(&space, "login-robot", "login", &minutes_ago(90)),
    merged(&space, "session-robot", "session", &minutes_ago(30)),
  ]);
  let (_, status) = bd(&space, &["status"]);
  let url = landed_by(&status, "auth")["robot"]["url"].as_str().unwrap().to_string();
  let (exit_code, output) = bd(&space, &["push", "auth", "--with-children"]);
  assert_error(&output, exit_code, (4, "landed"));
  assert_eq!(
    output["error"]["message"],
    json!(format!(
      "Nothing was pushed: auth landed in master with the Safe Push of login ({url}): nothing of it is left to push"
    ))
  );
  assert_eq!(space.remote_tip(STACK), pushed_before);

  // a commit of session added since is what is left: it goes, with the stack branch; auth and login stay as they are
  write(&space.repo, "session-check.txt", "check\n");
  commit_all(&space.repo, "(auth/session) Check the session", 1_700_001_000);
  let (exit_code, output) = bd(&space, &["push", "auth", "--with-children"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(names(&output, "branches"), [&json!("session")], "{output}");
  assert_eq!(names(&output, "skipped"), [&json!("auth"), &json!("login")], "{output}");
  assert_eq!((&output["stacks"][0]["name"], output.get("stack_warnings")), (&json!("auth"), None), "{output}");
  assert_eq!(space.remote_tip(STACK), space.local_tip(STACK));
}

/// `branch` of `bd status` by name
fn status_branch<'a>(status: &'a Value, name: &str) -> &'a Value {
  status["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!(name)).unwrap()
}

/// On Space `bd status` says what the Safe Push of each branch needs of a branch outside what it lands, as
/// `bd safe-push` would refuse it: `flow` uses a name `util` adds, `ui` is built on `flow`; `util` needs nothing
#[test_log::test]
fn status_names_what_the_safe_push_of_a_branch_needs_of_a_branch_outside_it() {
  let space = Space::new(&[]);
  space.repo.set_config("branchdeck.stacks", "true").unwrap();
  write(&space.repo, "src/a/Util.kt", "package a\n\nfun controlUrl(): String = \"\"\n");
  let declares = commit_all(&space.repo, "(util) Add controlUrl", 1_700_000_100)[..8].to_string();
  write(&space.repo, "src/b/Use.kt", "package b\n\nimport a.controlUrl\n\nfun start() = controlUrl()\n");
  commit_all(&space.repo, "(flow) Start with controlUrl", 1_700_000_200);
  write(&space.repo, "src/c/Ui.kt", "package c\n\nfun show() = 1\n");
  commit_all(&space.repo, "(flow/ui) Show the start", 1_700_000_300);
  sync(&space);

  let status = covered_status(&space);
  let need = json!([{
    "code": "missing_declaration",
    "identifier": "controlUrl",
    "at": "src/b/Use.kt:3",
    "needs": declares,
    "needs_branch": "util",
    "fix": "bd restack flow util",
  }]);
  assert_eq!(status_branch(&status, "flow")["landing_needs"], need);
  assert_eq!(status_branch(&status, "ui")["landing_needs"], need);
  assert_eq!(status_branch(&status, "util")["landing_needs"], json!([]));
}
