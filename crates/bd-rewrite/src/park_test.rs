use bd_cli::fixture_test::assert_covered;
use bd_cli::fixture_test::{assert_error, commit_subjects, repo_with_baseline, run_in};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn status_branch(output: &Value, name: &str) -> Value {
  output["branches"]
    .as_array()
    .unwrap()
    .iter()
    .find(|branch| branch["name"] == json!(name))
    .cloned()
    .unwrap_or_else(|| panic!("no {name} in {output}"))
}

#[test_log::test]
fn park_and_unpark_print_the_note_and_status_names_the_parked_branches() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  commit_subjects(&repo, &["(auth) Add auth", "(auth/login) Add login", "(docs) Add docs"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let head = repo.head();

  let (exit_code, output) = run_in(&repo, &["park", "auth", "--note", "waits for the fix of IJPL-1"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["branch"], &output["parked"], &output["note"], &output["changed"]),
    (&json!("auth"), &json!(true), &json!("waits for the fix of IJPL-1"), &json!(true))
  );
  let (_, schema) = run_in(&repo, &["schema"]);
  assert_covered(&schema["outputs"]["park"], &schema["outputs"]["park"], &output, "park");
  assert_eq!(repo.head(), head);

  let (exit_code, output) = run_in(&repo, &["status"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(status_branch(&output, "auth")["parked"], json!({"note": "waits for the fix of IJPL-1"}));
  // login carries the commits of auth
  assert_eq!(status_branch(&output, "login")["parked"], json!({"note": "waits for the fix of IJPL-1", "with": "auth"}));
  assert_eq!(status_branch(&output, "docs").get("parked"), None);
  let (_, brief) = run_in(&repo, &["status", "--brief"]);
  assert_eq!(status_branch(&brief, "login")["parked"]["with"], json!("auth"));

  let (exit_code, output) = run_in(&repo, &["park", "ghost"]);
  assert_error(&output, exit_code, (2, "unknown_branch"));
  let (exit_code, output) = run_in(&repo, &["unpark", "docs"]);
  assert_error(&output, exit_code, (2, "not_parked"));

  let (exit_code, output) = run_in(&repo, &["unpark", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["parked"], &output["previous_note"], output.get("note")),
    (&json!(false), &json!("waits for the fix of IJPL-1"), None)
  );
  let (_, output) = run_in(&repo, &["status"]);
  assert_eq!(status_branch(&output, "login").get("parked"), None);
}
