use crate::fixture_test::{ACP_HOST, commit_all, commit_dangling_reference, git, repo_with_baseline, run_in, with_header, write};
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

/// A plan file with the byte order mark that Windows PowerShell 5.1 writes
#[test_log::test]
fn a_plan_with_a_byte_order_mark_is_read() {
  let (repo, ids) = repo();
  let file = tempfile::NamedTempFile::new().unwrap();
  let plan = json!({ "branches": { "a": [ids[0], ids[1]], "c": [ids[2]] } });
  std::fs::write(file.path(), format!("\u{feff}{plan}")).unwrap();
  let (exit_code, output) = run_in(&repo, &["check", "--plan", file.path().to_str().unwrap()]);
  assert_eq!((exit_code, &output["ok"]), (0, &json!(true)), "{output}");
}

/// The Safe Push failure of 2026-10-16: git merges `fake-agent`, which moved a constant, but a file of its tree still
/// uses the old one; the commit that changes the use is in `lane-document`
#[test_log::test]
fn a_branch_that_removes_a_used_declaration_is_dangling_reference() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let (removed_by, fixed_by) = commit_dangling_reference(&repo);
  let before = state(&repo);

  let (exit_code, output) = run_in(&repo, &["check"]);
  assert_eq!(exit_code, 1, "{output}");
  assert_eq!(output["ok"], json!(false));
  let fake_agent = output["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!("fake-agent")).unwrap();
  assert_eq!(
    fake_agent,
    &json!({
      "name": "fake-agent",
      "built_on": null,
      "commit_count": 1,
      "result": "dangling_reference",
      "dangling_references": [{
        "identifier": "CONTROL_URL_ENV",
        "file": ACP_HOST,
        "line": 7,
        "removed_by": short(&removed_by),
        "fixed_by": short(&fixed_by),
        "mutual": true,
      }],
    })
  );
  assert_eq!(state(&repo), before, "a check changes no ref, note or HEAD");
  let (_, schema) = run_in(&repo, &["schema"]);
  crate::schema_test::assert_covered(&schema["outputs"]["check"], &schema["outputs"]["check"], &output, "check");

  // the fix in the same branch: ok
  let plan = json!({ "branches": { "fake-agent": [removed_by, fixed_by] } });
  let (exit_code, output) = check(&repo, &plan, &[]);
  assert_eq!(exit_code, 0, "{output}");
}

/// A use that no commit here changes: only possible, a warning, exit 0
#[test_log::test]
fn a_possible_dangling_reference_is_a_warning_and_exit_0() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  write(&repo, "src/util/Util.kt", "package util\n\nfun controlUrl(): String = \"\"\n");
  write(&repo, "src/flow/Start.kt", "package flow\n\nfun start() = 1\n");
  let base = commit_all(&repo, "Add util", 1_700_000_050);
  git(&repo, &["update-ref", "refs/remotes/origin/master", &base]);
  let removed_by = {
    write(&repo, "src/util/Util.kt", "package util\n");
    commit_all(&repo, "(cleanup) Drop controlUrl", 1_700_000_100)
  };
  // another branch adds a use: HEAD has it too, and nothing removes it
  write(&repo, "src/flow/Start.kt", "package flow\n\nfun start() = util.controlUrl()\n");
  commit_all(&repo, "(flow) Use controlUrl", 1_700_000_200);
  // HEAD is broken too, so no dangling reference: the split adds no failure
  let (exit_code, output) = run_in(&repo, &["check"]);
  assert_eq!((exit_code, output.get("warnings")), (0, None), "{output}");

  // a use on the baseline after the base: Patronus merges onto it
  git(&repo, &["checkout", "-q", "-b", "upstream", &base]);
  write(&repo, "src/flow/Late.kt", "package flow\n\nfun late() = util.controlUrl()\n");
  let upstream = commit_all(&repo, "Use it late", 1_700_000_300);
  git(&repo, &["checkout", "-q", "master"]);
  git(&repo, &["update-ref", "refs/remotes/origin/master", &upstream]);
  let (exit_code, output) = run_in(&repo, &["check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    output["warnings"],
    json!([{
      "kind": "possible_dangling_reference",
      "branch": "cleanup",
      "identifier": "controlUrl",
      "file": "src/flow/Late.kt",
      "line": 3,
      "removed_by": short(&removed_by),
      "on_baseline": true,
    }])
  );
}

#[test_log::test]
fn check_build_runs_the_command_on_each_branch_tree() {
  let (repo, _) = repo();
  // each tree has its own files: `(a)` has shared.txt, `(c)` doesn't
  let (exit_code, output) = run_in(&repo, &["check", "--build", "test -f shared.txt && echo \"$BD_BRANCH\""]);
  assert_eq!(exit_code, 1, "{output}");
  let results: Vec<(&Value, &Value, &Value)> = output["branches"]
    .as_array()
    .unwrap()
    .iter()
    .map(|branch| (&branch["name"], &branch["result"], &branch["build"]["exit_code"]))
    .collect();
  assert_eq!(results, [(&json!("a"), &json!("ok"), &json!(0)), (&json!("c"), &json!("build_failed"), &json!(1))]);
  assert_eq!(output["ok"], json!(false));

  // --build-paths extracts only those paths, and the output of a failure is kept
  let (exit_code, output) = run_in(&repo, &["check", "--build", "ls; exit 3", "--build-paths", "other.txt"]);
  assert_eq!(exit_code, 1, "{output}");
  let c = output["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!("c")).unwrap();
  assert_eq!(c["build"], json!({ "exit_code": 3, "output": "other.txt" }));
}
