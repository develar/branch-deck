use crate::fixture_test::{commit_all, git, repo_with_baseline, run_in, with_header, write};
use git_ops::rewrite_plan::short_id;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use test_utils::git_test_utils::TestRepo;

fn short(id: &str) -> String {
  short_id(id).to_string()
}

/// `(a)` creates a file and changes it; `(c)` is unrelated
fn repo() -> (TestRepo, Vec<String>) {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let mut ids = Vec::new();
  for (subject, path, content) in [
    ("(a) Add shared", "shared.txt", "one\n"),
    ("(a) Change shared", "shared.txt", "two\n"),
    ("(c) Add other", "other.txt", "x\n"),
  ] {
    write(&repo, path, content);
    ids.push(commit_all(&repo, subject, 1_700_000_000 + i64::try_from(ids.len()).unwrap() * 100));
  }
  (repo, ids)
}

fn check(repo: &TestRepo, plan: &Value, extra: &[&str]) -> (i32, Value) {
  let file = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(file.path(), plan.to_string()).unwrap();
  let mut args = vec!["check", "--plan", file.path().to_str().unwrap()];
  args.splice(0..0, extra.iter().copied());
  run_in(repo, &args)
}

/// Refs, notes and HEAD: a check changes none of them
fn state(repo: &TestRepo) -> (String, String, String) {
  (
    git(repo, &["for-each-ref", "--format=%(refname) %(objectname)"]),
    git(repo, &["notes", "list"]),
    repo.head(),
  )
}

#[test_log::test]
fn a_conflicting_plan_names_the_commit_and_what_it_needs() {
  let (repo, ids) = repo();
  let before = state(&repo);
  let (exit_code, output) = check(&repo, &json!({ "branches": { "b": [ids[1]] } }), &[]);
  assert_eq!(exit_code, 1, "{output}");
  let base = git(&repo, &["rev-parse", "origin/master"]);
  assert_eq!(
    output,
    with_header(
      &repo,
      json!({
        "head": short(&ids[2]),
        "baseline": "origin/master",
        "base_commit": short(&base),
        "rewritten": 1,
        "ok": false,
        "branches": [
          { "name": "a", "built_on": null, "commit_count": 1, "result": "ok" },
          {
            "name": "b",
            "built_on": null,
            "commit_count": 1,
            "result": "conflict",
            "conflict": {
              "commit": short(&ids[1]),
              "subject": "(b) Change shared",
              "files": ["shared.txt"],
              "missing_commits": [short(&ids[0])],
            },
          },
          { "name": "c", "built_on": null, "commit_count": 1, "result": "ok" },
        ],
        "unassigned_count": 0,
      })
    )
  );
  assert_eq!(state(&repo), before, "a check writes objects only");
}

#[test_log::test]
fn stacking_resolves_the_conflict() {
  let (repo, ids) = repo();
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let before = state(&repo);
  let (exit_code, output) = check(&repo, &json!({ "branches": { "a/b": [ids[1]] } }), &[]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["ok"], json!(true));
  let b = output["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!("b")).unwrap();
  assert_eq!((&b["parent"], &b["built_on"], &b["result"]), (&json!("a"), &json!("a"), &json!("ok")));
  assert_eq!(state(&repo), before);
}

#[test_log::test]
fn the_current_grouping_and_invalid_plans() {
  let (repo, ids) = repo();
  // no plan on stdin: the current grouping
  let (exit_code, output) = run_in(&repo, &["check"]);
  assert_eq!((exit_code, &output["ok"], &output["rewritten"]), (0, &json!(true), &json!(0)), "{output}");

  let (exit_code, output) = check(&repo, &json!({ "branches": { "b": ["0123456789"] } }), &[]);
  assert_eq!((exit_code, &output["error"]["code"]), (2, &json!("not_ahead_of_baseline")));
  let (exit_code, output) = check(&repo, &json!({ "branches": { "b": [ids[0]] }, "unassign": [ids[0]] }), &[]);
  assert_eq!((exit_code, &output["error"]["code"]), (2, &json!("invalid_plan")));
}

#[test_log::test]
fn cached_check_equals_uncached() {
  let (repo, ids) = repo();
  let plan = json!({ "branches": { "b": [ids[1]], "d": [ids[2]] } });
  let (_, uncached) = check(&repo, &plan, &["--no-cache"]);
  let (_, cold) = check(&repo, &plan, &[]);
  let (_, warm) = check(&repo, &plan, &[]);
  assert_eq!(cold, uncached);
  assert_eq!(warm, uncached);
}
