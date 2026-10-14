use crate::fixture_test::{PREFIX, commit_all, environment, git, repo_with_baseline, run_in, run_json, write};
use git_ops::rewrite_plan::short_id;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use test_utils::git_test_utils::TestRepo;

/// Two prefixed commits, an issue-key commit and an unassigned one, at fixed times
fn grouped_repo() -> (TestRepo, Vec<String>) {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let mut ids = Vec::new();
  write(&repo, "auth.rs", "fn login() {}\n");
  ids.push(commit_all(&repo, "(auth) Add login", 1_700_000_100));
  write(&repo, "crash.rs", "fix\n");
  ids.push(commit_all(&repo, "ABC-12 Fix crash", 1_700_000_200));
  write(&repo, "misc.txt", "misc\n");
  ids.push(commit_all(&repo, "Tidy up", 1_700_000_300));
  write(&repo, "auth.rs", "fn login() {}\nfn logout() {}\n");
  ids.push(commit_all(&repo, "(auth) Add logout", 1_700_000_400));
  (repo, ids)
}

fn short(id: &str) -> String {
  short_id(id).to_string()
}

#[test_log::test]
fn status_shape() {
  let (repo, ids) = grouped_repo();
  let (exit_code, output) = run_in(&repo, &["status"]);
  assert_eq!(exit_code, 0, "{output}");

  let repository = std::fs::canonicalize(repo.path()).unwrap();
  assert_eq!(
    output,
    json!({
      "schema": 1,
      "repository": repository.to_str().unwrap(),
      "prefix": PREFIX,
      "head": { "id": short(&ids[3]), "branch": "master" },
      "baseline": "origin/master",
      "branch_count": 2,
      "unassigned_count": 1,
      "branches": [
        {
          "name": "auth",
          "ref": "refs/heads/test-user/virtual/auth",
          "latest_time": 1_700_000_400,
          "commit_count": 2,
          "tip": null,
          "commits": [
            { "id": short(&ids[3]), "subject": "Add logout" },
            { "id": short(&ids[0]), "subject": "Add login" },
          ],
        },
        {
          "name": "ABC-12",
          "ref": "refs/heads/test-user/virtual/ABC-12",
          "issue_key": "ABC-12",
          "latest_time": 1_700_000_200,
          "commit_count": 1,
          "tip": null,
          "commits": [{ "id": short(&ids[1]), "subject": "ABC-12 Fix crash" }],
        },
      ],
      "unassigned": [{ "id": short(&ids[2]), "subject": "Tidy up" }],
    })
  );
}

#[test_log::test]
fn status_filters_and_pages() {
  let (repo, ids) = grouped_repo();
  git(&repo, &["update-ref", "refs/heads/test-user/virtual/auth", &ids[0]]);

  let (exit_code, output) = run_in(&repo, &["status", "--branch", "auth", "--limit", "1", "--offset", "1"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output.get("unassigned"), None);
  let branches = output["branches"].as_array().unwrap();
  assert_eq!(branches.len(), 1);
  assert_eq!(branches[0]["tip"], json!(short(&ids[0])));
  assert_eq!(branches[0]["commit_count"], json!(2));
  assert_eq!(branches[0]["commits"], json!([{ "id": short(&ids[0]), "subject": "Add login" }]));

  let (exit_code, output) = run_in(&repo, &["status", "--unassigned"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output.get("branches"), None);
  assert_eq!(output["unassigned"], json!([{ "id": short(&ids[2]), "subject": "Tidy up" }]));

  let (exit_code, output) = run_in(&repo, &["status", "--limit", "1", "--offset", "1"]);
  assert_eq!(exit_code, 0, "{output}");
  let names: Vec<&Value> = output["branches"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(names, [&json!("ABC-12")]);

  let (exit_code, output) = run_in(&repo, &["status", "--branch", "nope"]);
  assert_eq!(exit_code, 2);
  assert_eq!(output["error"]["code"], json!("unknown_branch"));
}

#[test_log::test]
fn full_ids() {
  let (repo, ids) = grouped_repo();
  let (_, output) = run_in(&repo, &["status", "--full-ids", "--unassigned"]);
  assert_eq!(output["head"]["id"], json!(ids[3]));
  assert_eq!(output["unassigned"][0]["id"], json!(ids[2]));
}

#[test_log::test]
fn prefix_from_app_settings() {
  let (repo, _) = grouped_repo();
  let settings = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(settings.path(), r#"{"appSettings": {"globalUserBranchPrefix": "from-app"}}"#).unwrap();
  let mut environment = environment(repo.path());
  environment.settings_file = Some(settings.path().to_path_buf());

  let (exit_code, output) = run_json(&environment, &["status", "--unassigned"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["prefix"], json!("from-app"));

  // --prefix wins over the app setting
  let (_, output) = run_json(&environment, &["--prefix", "explicit", "status", "--unassigned"]);
  assert_eq!(output["prefix"], json!("explicit"));
}

#[test_log::test]
fn prefix_from_git_config() {
  let (repo, _) = grouped_repo();
  repo.set_config("branchdeck.branchPrefix", "from-git").unwrap();
  let (exit_code, output) = run_json(&environment(repo.path()), &["status", "--unassigned"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["prefix"], json!("from-git"));
}

fn assert_error(output: &Value, code: &str) {
  assert_eq!(output["schema"], json!(1));
  assert_eq!(output["error"]["code"], json!(code), "{output}");
  assert!(output["error"]["message"].as_str().is_some_and(|message| !message.is_empty()));
  assert_eq!(output["error"]["commits"], json!([]));
  assert_eq!(output["error"]["retryable"], json!(false));
}

#[test_log::test]
fn empty_prefix_is_an_error() {
  let (repo, _) = grouped_repo();
  let (exit_code, output) = run_json(&environment(repo.path()), &["--prefix", "", "status"]);
  assert_eq!(exit_code, 2);
  assert_error(&output, "no_branch_prefix");

  // a repository-level empty value overrides any global one
  repo.set_config("branchdeck.branchPrefix", "").unwrap();
  let (exit_code, output) = run_json(&environment(repo.path()), &["status"]);
  assert_eq!(exit_code, 2);
  assert_error(&output, "no_branch_prefix");
}

#[test_log::test]
fn not_a_repository() {
  let directory = tempfile::tempdir().unwrap();
  let (exit_code, output) = run_json(&environment(directory.path()), &["--prefix", PREFIX, "status"]);
  assert_eq!(exit_code, 2);
  assert_error(&output, "not_a_repository");

  let missing = directory.path().join("missing");
  let (exit_code, output) = run_json(&environment(directory.path()), &["-C", missing.to_str().unwrap(), "--prefix", PREFIX, "commits"]);
  assert_eq!(exit_code, 2);
  assert_error(&output, "not_a_repository");
}

#[test_log::test]
fn invalid_arguments() {
  let directory = tempfile::tempdir().unwrap();
  let (exit_code, output) = run_json(&environment(directory.path()), &["status", "--no-such-flag"]);
  assert_eq!(exit_code, 2);
  assert_error(&output, "invalid_arguments");
}

#[test_log::test]
fn subdirectory_resolves_to_top_level() {
  let (repo, _) = grouped_repo();
  write(&repo, "nested/dir/file.txt", "x\n");
  let (exit_code, output) = run_json(&environment(&repo.path().join("nested")), &["-C", "dir", "--prefix", PREFIX, "status", "--unassigned"]);
  assert_eq!(exit_code, 0, "{output}");
  let repository = std::fs::canonicalize(repo.path()).unwrap();
  assert_eq!(output["repository"], json!(repository.to_str().unwrap()));
}
