use crate::fixture_test::{commit_all, git, repo_with_baseline, run_in, with_header, write};
use git_ops::rewrite_plan::short_id;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sync_core::repository_lock::{DEFAULT_LOCK_TIMEOUT, lock_repository};
use test_utils::git_test_utils::TestRepo;

fn short(id: &str) -> String {
  short_id(id).to_string()
}

/// Commits with these subjects ahead of `origin/master`; returns their ids
fn repo_with(subjects: &[&str]) -> (TestRepo, Vec<String>) {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let ids = subjects
    .iter()
    .enumerate()
    .map(|(index, subject)| {
      write(&repo, &format!("file{index}.txt"), *subject);
      commit_all(&repo, subject, 1_700_000_000 + i64::try_from(index).unwrap() * 100)
    })
    .collect();
  (repo, ids)
}

/// The commits ahead of the baseline, oldest first
fn ahead(repo: &TestRepo) -> Vec<String> {
  git(repo, &["rev-list", "--reverse", "origin/master..HEAD"]).lines().map(str::to_string).collect()
}

fn subjects(repo: &TestRepo) -> Vec<String> {
  git(repo, &["log", "--reverse", "--format=%s", "origin/master..HEAD"]).lines().map(str::to_string).collect()
}

/// Run `bd group` with `plan` written to a file
fn group(repo: &TestRepo, plan: &Value, extra: &[&str]) -> (i32, Value) {
  let file = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(file.path(), plan.to_string()).unwrap();
  let mut args = vec!["group", "--plan", file.path().to_str().unwrap()];
  args.extend_from_slice(extra);
  run_in(repo, &args)
}

fn assert_error(output: &Value, exit_code: i32, expected_exit_code: i32, code: &str) {
  assert_eq!((exit_code, &output["error"]["code"]), (expected_exit_code, &json!(code)), "{output}");
}

#[test_log::test]
fn group_applies_a_plan() {
  let (repo, ids) = repo_with(&["Add login", "Fix crash", "Tidy up"]);
  let head = repo.head();
  let (exit_code, output) = group(
    &repo,
    &json!({ "branches": { "auth": [short(&ids[0]), short(&ids[1])] } }),
    &["--expect-head", &short(&head)],
  );
  assert_eq!(exit_code, 0, "{output}");

  let new_ids = ahead(&repo);
  assert_eq!(
    output,
    with_header(
      &repo,
      json!({
      "operation": "group",
      "dry_run": false,
      "previous_head": short(&head),
      "new_head": short(&new_ids[2]),
      "rewritten": 2,
      "undo_token": format!("{head}..{}", new_ids[2]),
      "groups": { "auth": [short(&new_ids[0]), short(&new_ids[1])] },
      "unassigned": [short(&new_ids[2])],
      })
    )
  );
  assert_eq!(subjects(&repo), ["(auth) Add login", "(auth) Fix crash", "Tidy up"]);
}

#[test_log::test]
fn dry_run_prints_the_changes_and_the_forest() {
  let (repo, ids) = repo_with(&["(auth) Add auth", "Add login", "Tidy up"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let head = repo.head();
  let (exit_code, output) = group(&repo, &json!({ "branches": { "auth/login": [ids[1]] } }), &["--dry-run", "--map"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(repo.head(), head, "a dry run writes nothing");
  assert_eq!(output["dry_run"], json!(true));
  assert_eq!(output.get("undo_token"), None);
  assert_eq!(output["new_head"], json!(short(&head)));
  assert_eq!(output["changes"], json!([{ "id": short(&ids[1]), "from": "Add login", "to": "(auth/login) Add login" }]));
  assert_eq!(
    output["forest"],
    json!([
      { "name": "auth", "has_commits": true },
      { "name": "login", "parent": "auth", "has_commits": true },
    ])
  );
  assert_eq!(output["map"][1], json!({ "from": short(&ids[1]), "to": short(&ids[1]) }));
}

#[test_log::test]
fn group_errors_and_exit_codes() {
  let (repo, ids) = repo_with(&["One", "Two"]);
  let head = repo.head();

  let (exit_code, output) = group(&repo, &json!({ "branches": { "a": ["0123456789"] } }), &[]);
  assert_error(&output, exit_code, 2, "not_ahead_of_baseline");
  assert_eq!(output["error"]["commits"], json!(["0123456789"]));

  let (exit_code, output) = group(&repo, &json!({ "branches": { "a": [ids[0]] }, "unassign": [ids[0]] }), &[]);
  assert_error(&output, exit_code, 2, "invalid_plan");
  assert_eq!(output["error"]["commits"], json!([short(&ids[0])]));

  let (exit_code, output) = group(&repo, &json!({ "branch": {} }), &[]);
  assert_error(&output, exit_code, 2, "invalid_plan");

  let (exit_code, output) = group(&repo, &json!({ "branches": { "a": [ids[0]] } }), &["--expect-head", &short(&ids[0])]);
  assert_error(&output, exit_code, 3, "head_moved");
  assert_eq!(output["error"]["retryable"], json!(true));

  // another process holds the lock: retry later
  let held = lock_repository(repo.git_executor(), repo.path().to_str().unwrap(), DEFAULT_LOCK_TIMEOUT).unwrap();
  let (exit_code, output) = group(&repo, &json!({ "branches": { "a": [ids[0]] } }), &[]);
  assert_error(&output, exit_code, 3, "locked");
  drop(held);

  git(&repo, &["checkout", "--detach", "-q"]);
  let (exit_code, output) = group(&repo, &json!({ "branches": { "a": [ids[0]] } }), &[]);
  assert_error(&output, exit_code, 4, "detached_head");

  assert_eq!(repo.head(), head, "nothing was written");
}

#[test_log::test]
fn rename_restack_unstack_and_undo() {
  let (repo, _) = repo_with(&["(auth) Add auth", "(login) Add login", "(auth/login/oauth) Add oauth"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let initial_head = repo.head();

  let (exit_code, output) = run_in(&repo, &["restack", "login", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["operation"], json!("restack"));
  assert_eq!(subjects(&repo), ["(auth) Add auth", "(auth/login) Add login", "(auth/login/oauth) Add oauth"]);

  let (exit_code, output) = run_in(&repo, &["rename", "auth", "identity"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["rewritten"], json!(3));
  assert_eq!(subjects(&repo), ["(identity) Add auth", "(identity/login) Add login", "(identity/login/oauth) Add oauth"]);
  let renamed_head = repo.head();
  let rename_token = output["undo_token"].as_str().unwrap().to_string();

  let (exit_code, output) = run_in(&repo, &["unstack", "login"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(subjects(&repo)[1], "(login) Add login");
  let unstack_token = output["undo_token"].as_str().unwrap().to_string();

  // undo the last rewrite, then refuse a stale token
  let (exit_code, output) = run_in(&repo, &["undo", &unstack_token]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["head"], json!(short(&renamed_head)));
  assert_eq!(repo.head(), renamed_head);
  let (exit_code, output) = run_in(&repo, &["undo", &unstack_token]);
  assert_error(&output, exit_code, 3, "head_moved");
  let (exit_code, output) = run_in(&repo, &["undo", "not-a-token"]);
  assert_error(&output, exit_code, 2, "invalid_arguments");

  let (exit_code, output) = run_in(&repo, &["undo", &rename_token]);
  assert_eq!(exit_code, 0, "{output}");
  assert_ne!(repo.head(), initial_head, "the restack before the rename stays");
  assert_eq!(subjects(&repo)[0], "(auth) Add auth");

  let (exit_code, output) = run_in(&repo, &["rename", "auth", "a/b"]);
  assert_error(&output, exit_code, 2, "invalid_plan");
}
