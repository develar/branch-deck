use bd_cli::fixture_test::assert_covered;
use bd_cli::fixture_test::{assert_error, repo_with_subjects, run_in};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test_log::test]
fn a_branch_declares_what_lands_before_it_and_a_second_need_joins_its_line() {
  let (repo, _) = repo_with_subjects(&[("README.md", "readme\n")], &["(auth) Add auth", "(docs) Add docs", "(api) Add api"]);
  let head = repo.head();

  let (exit_code, output) = run_in(&repo, &["need", "auth", "docs"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["branch"], &output["needs"], &output["previous_needs"], &output["changed"]),
    (&json!("auth"), &json!(["docs"]), &json!([]), &json!(true))
  );
  let (_, schema) = run_in(&repo, &["schema", "--all"]);
  assert_covered(&schema["outputs"]["need"], &schema["outputs"]["need"], &output, "need");
  // no rewrite: only the store moved
  assert_eq!(repo.head(), head);

  // a need of a companion branch joins the same line, and the line stays sorted
  let (exit_code, output) = run_in(&repo, &["need", "auth", "ci-config:test-data-download"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["needs"], &output["changed"]), (&json!(["ci-config:test-data-download", "docs"]), &json!(true)));

  // declaring it again writes nothing
  let (exit_code, output) = run_in(&repo, &["need", "auth", "docs"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["needs"], &output["changed"]), (&json!(["ci-config:test-data-download", "docs"]), &json!(false)));

  let (exit_code, output) = run_in(&repo, &["need", "auth", "--remove", "docs"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["needs"], &output["previous_needs"], &output["changed"]),
    (&json!(["ci-config:test-data-download"]), &json!(["ci-config:test-data-download", "docs"]), &json!(true))
  );

  let (exit_code, output) = run_in(&repo, &["need", "auth", "--remove", "ci-config:test-data-download"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["needs"], &output["changed"]), (&json!([]), &json!(true)));
}

#[test_log::test]
fn a_need_of_an_unknown_branch_a_self_need_and_a_need_that_was_never_declared_are_refused() {
  let (repo, _) = repo_with_subjects(&[("README.md", "readme\n")], &["(auth) Add auth", "(docs) Add docs"]);

  let (exit_code, output) = run_in(&repo, &["need", "ghost", "docs"]);
  assert_error(&output, exit_code, (2, "unknown_branch"));

  let (exit_code, output) = run_in(&repo, &["need", "auth", "auth"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  let (exit_code, output) = run_in(&repo, &["need", "auth", "--remove", "docs"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  // a need of a branch that is nowhere is allowed: the order can be declared before the branch is synced
  let (exit_code, output) = run_in(&repo, &["need", "auth", "ghost"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(&output["needs"], &json!(["ghost"]));
}
