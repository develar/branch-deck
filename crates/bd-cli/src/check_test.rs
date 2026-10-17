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

/// The branch `name` of a `bd check` output
fn branch<'a>(output: &'a Value, name: &str) -> &'a Value {
  output["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!(name)).unwrap()
}

/// `commit_dangling_reference`, whose commit that changes the use changes another file too, so only the change of the
/// one file can move: `(the commit that moves the constant, the commit that changes the use)`
fn dangling_reference_in_two_files(repo: &TestRepo) -> (String, String) {
  let (removed_by, _) = commit_dangling_reference(repo);
  write(repo, "plugins/air/tests/flow/Other.kt", "package flow\n");
  git(repo, &["add", "-A"]);
  git(repo, &["commit", "-q", "--amend", "--no-edit"]);
  (removed_by, repo.head())
}

/// A repository with stacked branches on
fn stacked_repo() -> TestRepo {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  repo
}

/// Write `content` to `path` and commit it
fn commit_file(repo: &TestRepo, subject: &str, path: &str, content: &str, time: i64) -> String {
  write(repo, path, content);
  commit_all(repo, subject, time)
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
fn fix_finds_the_regroup_after_which_the_plan_applies_and_writes_nothing() {
  let (repo, ids) = repo();
  let before = state(&repo);
  // the plan sends the change of shared away from the commit that adds the file
  let (exit_code, output) = check(&repo, &json!({ "branches": { "b": [ids[1]] } }), &[]);
  assert_eq!((exit_code, output.get("fix")), (1, None), "{output}");
  let file = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(file.path(), json!({ "branches": { "b": [ids[1]] } }).to_string()).unwrap();
  let (exit_code, output) = run_in(&repo, &["check", "--fix", "--plan", file.path().to_str().unwrap()]);
  assert_eq!(exit_code, 1, "the plan itself still conflicts: {output}");
  let fix = &output["fix"];
  assert_eq!(fix["search"], "complete", "{output}");
  let component = &fix["components"][0];
  assert_eq!(component["fixed"], json!(["b"]), "{output}");
  // the whole plan: the user's move and the fix on top of it
  let plan = &component["plan"];
  let (exit_code, checked) = check(&repo, plan, &[]);
  assert_eq!(exit_code, 0, "the fix applies: {plan} {checked}");
  assert_eq!(state(&repo), before, "a check with --fix writes objects only");
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
        "via": "syntax_tree",
        "fix": { "kind": "move_commit", "branches": { "fake-agent": [short(&fixed_by)] } },
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

/// The commit that changes the use changes another file too: the fix moves the change of the one file, and the command
/// as printed makes every branch build
#[test_log::test]
fn a_mutual_dangling_reference_is_fixed_by_the_move_hunk_it_names() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let (removed_by, fixed_by) = dangling_reference_in_two_files(&repo);

  let (exit_code, output) = run_in(&repo, &["check"]);
  assert_eq!(exit_code, 1, "{output}");
  let fake_agent = branch(&output, "fake-agent");
  let command = format!("bd move-hunk {} {} {ACP_HOST}", short(&fixed_by), short(&removed_by));
  assert_eq!(fake_agent["dangling_references"][0]["fix"], json!({ "kind": "move_hunk", "command": command }));

  let args: Vec<&str> = command.strip_prefix("bd ").unwrap().split(' ').collect();
  let (exit_code, moved) = run_in(&repo, &args);
  assert_eq!(exit_code, 0, "{moved}");
  let (exit_code, output) = run_in(&repo, &["check"]);
  assert_eq!((exit_code, &output["ok"]), (0, &json!(true)), "{output}");
}

/// A later commit of `lane-document` changes the use again: the move of the hunk would leave it without the lines it
/// changes, so `lane-document` would conflict. The trial build sees it, and no fix is offered (the incident of
/// 2026-10-17, where the printed `bd move-hunk` would have broken a branch)
#[test_log::test]
fn a_move_hunk_fix_that_breaks_another_branch_is_not_offered() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let (removed_by, fixed_by) = dangling_reference_in_two_files(&repo);
  let host = std::fs::read_to_string(repo.path().join(ACP_HOST)).unwrap();
  write(&repo, ACP_HOST, host.replace("CONTROL_URL_ENV to url", "CONTROL_URL_ENV to url, \"mode\" to \"lane\""));
  commit_all(&repo, "(lane-document) IJAI-3878 Pass the lane mode", 1_700_000_300);
  let before = state(&repo);

  let (exit_code, output) = run_in(&repo, &["check"]);
  assert_eq!(exit_code, 1, "{output}");
  let fake_agent = branch(&output, "fake-agent");
  assert_eq!(fake_agent["result"], json!("dangling_reference"), "{output}");
  assert_eq!(fake_agent["dangling_references"][0]["mutual"], json!(true));
  assert_eq!(fake_agent["dangling_references"][0].get("fix"), None, "{output}");
  assert_eq!(branch(&output, "lane-document")["result"], json!("ok"));
  assert_eq!(state(&repo), before, "the trial moves no ref and writes no note");

  // what the trial saw: after the move, lane-document conflicts
  let (exit_code, moved) = run_in(&repo, &["move-hunk", &fixed_by, &removed_by, ACP_HOST]);
  assert_eq!(exit_code, 0, "{moved}");
  let (_, output) = run_in(&repo, &["check"]);
  assert_eq!(branch(&output, "lane-document")["result"], json!("conflict"), "{output}");
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
      "via": "syntax_tree",
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

/// A parent that needs a commit of its own child: `bd check` names the case and the fixes that build, each one a plan
/// for `bd group`
#[test_log::test]
fn a_parent_that_needs_its_child_is_an_inverted_stack() {
  let repo = stacked_repo();
  let child_commit = commit_file(&repo, "(a/b) Add shared", "shared.txt", "one\n", 1_700_000_100);
  let parent_commit = commit_file(&repo, "(a) Change shared", "shared.txt", "two\n", 1_700_000_200);
  commit_file(&repo, "(a) Add own", "own.txt", "own\n", 1_700_000_300);

  let (exit_code, output) = run_in(&repo, &["check"]);
  assert_eq!(exit_code, 1, "{output}");
  let a = branch(&output, "a");
  assert_eq!(a["conflict"]["missing_commits"], json!([short(&child_commit)]));
  assert_eq!(
    a["inverted_stack"],
    json!({
      "branch": "a",
      "child": "b",
      "fixes": [
        { "kind": "move_to_child", "branches": { "a/b": [short(&parent_commit)] } },
        { "kind": "swap", "branches": { "b/a": [] }, "unstack": ["b"] },
      ],
    })
  );
  let (_, schema) = run_in(&repo, &["schema"]);
  crate::schema_test::assert_covered(&schema["outputs"]["check"], &schema["outputs"]["check"], &output, "check");

  // each fix, applied as printed, builds
  let head = repo.head();
  for fix in a["inverted_stack"]["fixes"].as_array().unwrap() {
    git(&repo, &["reset", "-q", "--hard", &head]);
    let mut plan = fix.clone();
    plan.as_object_mut().unwrap().remove("kind");
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), plan.to_string()).unwrap();
    let (exit_code, grouped) = run_in(&repo, &["group", "--plan", file.path().to_str().unwrap()]);
    assert_eq!(exit_code, 0, "{fix}: {grouped}");
    let (exit_code, output) = run_in(&repo, &["check"]);
    assert_eq!((exit_code, &output["ok"]), (0, &json!(true)), "{fix}: {output}");
  }
}

/// Neither fix builds both: the parent's next commit needs the one that would move into the child, and the child needs
/// a commit of the parent. The case is named with the reason instead of an empty list alone
#[test_log::test]
fn an_inverted_stack_that_no_fix_builds_says_why() {
  let repo = stacked_repo();
  commit_file(&repo, "(a/b) Add shared", "shared.txt", "one\n", 1_700_000_100);
  commit_file(&repo, "(a) Add own", "own.txt", "own\n", 1_700_000_150);
  commit_file(&repo, "(a) Change shared", "shared.txt", "two\n", 1_700_000_200);
  commit_file(&repo, "(a) Change shared again", "shared.txt", "three\n", 1_700_000_250);
  commit_file(&repo, "(a/b) Change own", "own.txt", "own 2\n", 1_700_000_300);

  let (exit_code, output) = run_in(&repo, &["check"]);
  assert_eq!(exit_code, 1, "{output}");
  assert_eq!(
    branch(&output, "a")["inverted_stack"],
    json!({
      "branch": "a",
      "child": "b",
      "fixes": [],
      "reason": "neither moving its commits into the child nor stacking it on the child builds both: `bd check --fix` searches the regroups",
    }),
    "{output}"
  );
}

/// The incident as it was: `lane-document` stacked on `fake-agent`, whose build needs the child's change of the use.
/// The change needs the class the parent adds, so the parent can't stack on the child: no swap is offered
#[test_log::test]
fn a_mutual_dangling_reference_of_a_stack_is_an_inverted_stack_without_a_swap() {
  let repo = stacked_repo();
  let (_, fixed_by) = commit_dangling_reference(&repo);
  let plan = json!({ "branches": { "fake-agent/lane-document": [fixed_by] } });
  let (exit_code, output) = check(&repo, &plan, &[]);
  assert_eq!(exit_code, 1, "{output}");
  let fake_agent = branch(&output, "fake-agent");
  assert_eq!(fake_agent["result"], json!("dangling_reference"));
  assert_eq!(fake_agent["inverted_stack"]["child"], json!("lane-document"));

  assert!(
    !fake_agent["inverted_stack"]["fixes"]
      .as_array()
      .unwrap()
      .iter()
      .map(|fix| &fix["kind"])
      .any(|kind| kind == &json!("swap")),
    "{output}"
  );
}

/// `flow` imports a function that `util` adds: git builds both, and the check names the name `flow` misses; the exit
/// code stays, the names are no compiler. With the symbols on report only it says nothing
#[test_log::test]
fn a_branch_that_uses_a_name_another_branch_adds_is_a_missing_declaration() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let declares = commit_file(
    &repo,
    "(util) Add controlUrl",
    "src/a/Util.kt",
    "package a\n\nfun controlUrl(): String = \"\"\n",
    1_700_000_100,
  );
  let uses = commit_file(
    &repo,
    "(flow) Start with controlUrl",
    "src/b/Use.kt",
    "package b\n\nimport a.controlUrl\n\nfun start() = controlUrl()\n",
    1_700_000_200,
  );

  let (exit_code, output) = run_in(&repo, &["check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    output["missing_declarations"],
    json!([{
      "branch": "flow",
      "identifier": "controlUrl",
      "file": "src/b/Use.kt",
      "line": 3,
      "used_by": short(&uses),
      "declared_by": short(&declares),
      "declared_in": "util",
      "evidence": "import",
    }])
  );
  let (_, schema) = run_in(&repo, &["schema"]);
  crate::schema_test::assert_covered(&schema["outputs"]["check"], &schema["outputs"]["check"], &output, "check");

  git(&repo, &["config", "branchdeck.symbols", "report"]);
  let (exit_code, output) = run_in(&repo, &["check"]);
  assert_eq!((exit_code, output.get("missing_declarations")), (0, None), "{output}");
}
