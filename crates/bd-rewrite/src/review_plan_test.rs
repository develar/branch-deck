use crate::review_plan_command::REVIEW_PLAN_SCHEMA_VERSION;
use bd_cli::fixture_test::{assert_covered, commit_all, commit_subjects, git, repo_with_baseline, run_in, write};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// After a mark, a commit added on top is the only commit since the review, its range the scope; after a second mark,
/// an amend of that commit is `rewritten` with the lines of each version, and the scope is the diff from the reviewed
/// tip, the base of the branch the same
#[test_log::test]
fn prints_the_commits_since_the_review() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  commit_subjects(&repo, &["(auth) Add login"]);
  assert_eq!(run_in(&repo, &["sync"]).0, 0);
  let base = git(&repo, &["rev-parse", "origin/master"]);
  let reviewed = git(&repo, &["rev-parse", "refs/heads/test-user/virtual/auth"]);
  let (exit_code, output) = run_in(&repo, &["mark-reviewed", "auth"]);
  assert_eq!(exit_code, 0, "{output}");

  write(&repo, "logout.txt", "logout\n");
  commit_all(&repo, "(auth) Add logout", 1_700_000_200);
  assert_eq!(run_in(&repo, &["sync"]).0, 0);
  let tip = git(&repo, &["rev-parse", "refs/heads/test-user/virtual/auth"]);
  let (exit_code, output) = run_in(&repo, &["--full-ids", "review-plan", "auth", "--since-reviewed"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["schema"], &output["branch"], &output["title"], &output["base"], &output["head"]),
    (
      &json!(REVIEW_PLAN_SCHEMA_VERSION),
      &json!("test-user/virtual/auth"),
      &json!("auth"),
      &json!(base),
      &json!(tip)
    )
  );
  assert_eq!(
    output["interdiff"],
    json!({
      "reviewed_base": base,
      "reviewed_tip": reviewed,
      "commits": [{ "id": tip, "reviewed_id": null, "subject": "Add logout", "state": "added" }],
      "scope": { "base": reviewed, "head": tip, "carried": false },
    })
  );

  assert_eq!(run_in(&repo, &["mark-reviewed", "auth"]).0, 0);
  write(&repo, "logout.txt", "logout everywhere\n");
  git(&repo, &["add", "-A"]);
  repo.commit_with(&["--amend", "--no-edit"], &[("GIT_COMMITTER_DATE", "1700000300 +0000")]);
  assert_eq!(run_in(&repo, &["sync"]).0, 0);
  let amended = git(&repo, &["rev-parse", "refs/heads/test-user/virtual/auth"]);
  let (exit_code, output) = run_in(&repo, &["--full-ids", "review-plan", "auth", "--since-reviewed"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["interdiff"]["commits"], &output["interdiff"]["scope"]),
    (
      &json!([{
        "id": amended,
        "reviewed_id": tip,
        "subject": "Add logout",
        "state": "rewritten",
        "new_only": [{ "path": "logout.txt", "lines": ["+logout everywhere"] }],
        "reviewed_only": [{ "path": "logout.txt", "lines": ["+logout"] }],
      }]),
      &json!({ "base": tip, "head": amended, "carried": true })
    )
  );
  let files: Vec<&str> = output["files"].as_array().unwrap().iter().map(|file| file["path"].as_str().unwrap()).collect();
  assert_eq!(files, ["logout.txt"]);

  // a body written since the third mark, the lines kept: `reworded`, with the blocks its body added, and no file read
  assert_eq!(run_in(&repo, &["mark-reviewed", "auth"]).0, 0);
  repo.commit_with(
    &["--amend", "-m", "(auth) Add logout\n\nThe logout clears the session of the user."],
    &[("GIT_COMMITTER_DATE", "1700000400 +0000")],
  );
  assert_eq!(run_in(&repo, &["sync"]).0, 0);
  let reworded = git(&repo, &["rev-parse", "refs/heads/test-user/virtual/auth"]);
  let (exit_code, output) = run_in(&repo, &["--full-ids", "review-plan", "auth", "--since-reviewed"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["interdiff"]["commits"], &output["interdiff"]["scope"], &output["files"]),
    (
      &json!([{
        "id": reworded,
        "reviewed_id": amended,
        "subject": "Add logout",
        "state": "reworded",
        "message_change": {
          "subjectBefore": null,
          "hunks": [{
            "start": 0,
            "added": { "blocks": [{ "type": "paragraph", "lines": ["The logout clears the session of the user."] }], "code": [] },
            "removed": { "blocks": [], "code": [] },
          }],
        },
      }]),
      &json!({ "base": reworded, "head": reworded, "carried": false }),
      &json!([]),
    )
  );
}

/// A branch never marked and a range have no interdiff; `--since-reviewed` or `--base` of a range, and a range rev that
/// names no commit, are `invalid_arguments`
#[test_log::test]
fn a_branch_never_marked_and_a_range_have_no_interdiff() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  commit_subjects(&repo, &["(auth) Add login"]);
  assert_eq!(run_in(&repo, &["sync"]).0, 0);
  let (exit_code, output) = run_in(&repo, &["review-plan", "auth", "--since-reviewed"]);
  assert_eq!((exit_code, &output["interdiff"]), (0, &json!(null)), "{output}");

  let head = git(&repo, &["rev-parse", "HEAD"]);
  let (exit_code, output) = run_in(&repo, &["--full-ids", "review-plan", "origin/master..HEAD"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["branch"], &output["head"], &output["interdiff"]), (&json!(null), &json!(head), &json!(null)));

  for args in [
    &["review-plan", "origin/master..HEAD", "--since-reviewed"][..],
    &["review-plan", "origin/master..HEAD", "--base=master"],
    &["review-plan", "nope..HEAD"],
  ] {
    let (exit_code, output) = run_in(&repo, args);
    assert_eq!((exit_code, &output["error"]["code"]), (2, &json!("invalid_arguments")), "{args:?}: {output}");
  }
}

/// A range that changes the parameters of a public Kotlin function and adds an untested file: `api` with the forms
/// before and after and the `verdict` over its usages, the facts per file, the steps of its story with their coverage and its brief, the story of its
/// commit with the code names its body resolves and the one it does not, and why there are no usages (accio missing
/// here, or without an index of the test repository)
#[test_log::test]
fn prints_the_api_and_the_files_of_a_range() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  write(&repo, "src/a/Api.kt", "package a\n\nfun open(path: String): Int = 1\n");
  write(&repo, "src/test/a/ApiTest.kt", "package a\n\nclass ApiTest\n");
  commit_all(&repo, "Add api", 1_700_000_100);
  let base = git(&repo, &["rev-parse", "HEAD"]);
  write(&repo, "src/a/Api.kt", "package a\n\nfun open(path: String, mode: Int): Int = mode\n");
  write(&repo, "src/a/Session.kt", "package a\n\ninternal fun session(): Int = 1\n");
  commit_all(
    &repo,
    "Change api\n\nThe `open()` of Api.kt takes a mode, and Session.kt opens a session; `Nowhere` changes nothing.\n\nCo-Authored-By: Jane <jane@example.com>",
    1_700_000_200,
  );

  let (exit_code, output) = run_in(&repo, &["review-plan", &format!("{base}..HEAD")]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["schema"], json!(REVIEW_PLAN_SCHEMA_VERSION), "{output}");
  assert_eq!(
    (&output["api"], &output["files"]),
    (
      &json!([{
        "path": "src/a/Api.kt",
        "line": 3,
        "name": "open",
        "kind": "changed",
        "widened": false,
        "changes": ["parameters"],
        "before": "fun open(path: String): Int",
        "after": "fun open(path: String, mode: Int): Int",
        "usages": null,
      }]),
      &json!([
        { "path": "src/a/Api.kt", "old_path": null, "mechanical": null, "generated": null, "uses": 0, "used_by": 0, "new_links": 0, "breaks": 0, "breaks_complete": false, "untested": true, "repeated_edit": null, "repeated_edit_only": false, "module": 0, "folder": "" },
        { "path": "src/a/Session.kt", "old_path": null, "mechanical": null, "generated": null, "uses": 0, "used_by": 0, "new_links": 0, "breaks": 0, "breaks_complete": true, "untested": true, "repeated_edit": null, "repeated_edit_only": false, "module": 0, "folder": "" },
      ])
    )
  );
  // the uses of the changed `open` are unknown, so is whether one left outside breaks
  assert_eq!(output["verdict"], json!("unknown"), "{output}");
  // the units: the change of the form of `open`, then the new file, each with its block and lines from 1
  let units: Vec<(&Value, &Value, &Value, &Value)> = output["units"]
    .as_array()
    .unwrap()
    .iter()
    .map(|unit| (&unit["kind"], &unit["name"], &unit["slices"][0]["path"], &unit["slices"][0]["blocks"]))
    .collect();
  assert_eq!(
    units,
    [
      (
        &json!("shape"),
        &json!("open"),
        &json!("src/a/Api.kt"),
        &json!([{ "index": 0, "old": [3, 3], "new": [3, 3] }])
      ),
      (&json!("newFile"), &json!("Session.kt"), &json!("src/a/Session.kt"), &json!([])),
    ],
    "{output}"
  );
  // `open` is API, the new file no declaration; neither is a test
  let flags: Vec<(&Value, &Value)> = output["units"].as_array().unwrap().iter().map(|unit| (&unit["api"], &unit["test"])).collect();
  assert_eq!(flags, [(&json!(true), &json!(false)), (&json!(false), &json!(false))], "{output}");
  // the story of one commit is its concerns, every block in one stop; no issue, no prose of a spec or a doc
  assert_eq!(
    (&output["steps"]["source"], &output["steps"]["coverage"], &output["brief"]),
    (
      &json!({ "kind": "concerns", "reason": "oneCommit" }),
      &json!({ "blocks": 2, "inStops": 2, "mechanical": 0, "missing": 0, "twice": 0 }),
      &json!({ "issue": null, "substitutions": [], "contract": [] })
    ),
    "{output}"
  );
  // the story of the commit once, its id as short as the others: the names its body resolves, and in backticks the
  // one it does not (`target: null`); its blocks before the steps, which name them in no section
  let story = &output["steps"]["commits"];
  assert_eq!(
    (&story[0]["id"], &story[0]["title"], &story[0]["body"]["trailers"], &story[0]["parts"][0]["told"]),
    (
      &output["head"],
      &json!({ "prefix": null, "text": "Change api" }),
      &json!(["Co-Authored-By: Jane <jane@example.com>"]),
      &json!({ "kind": "overview" })
    ),
    "{output}"
  );
  let mentions: Vec<(&Value, &Value, &Value)> = story[0]["mentions"]
    .as_array()
    .unwrap()
    .iter()
    .map(|mention| (&mention["text"], &mention["target"]["kind"], &mention["target"]["path"]))
    .collect();
  assert_eq!(
    mentions,
    [
      (&json!("open"), &json!("unit"), &json!("src/a/Api.kt")),
      (&json!("Api.kt"), &json!("file"), &json!("src/a/Api.kt")),
      (&json!("Session.kt"), &json!("file"), &json!("src/a/Session.kt")),
      (&json!("Nowhere"), &Value::Null, &Value::Null),
    ],
    "{output}"
  );
  assert_eq!(story[0]["mentions"][3]["target"], Value::Null, "{output}");
  // no accio on the machine, or accio's reason under `--no-build`: the repository of the test has no index
  assert!(
    matches!(output["index_note"].as_str(), Some("accio is not installed" | "no index of this repository")),
    "{output}"
  );
  let (_, schema) = run_in(&repo, &["schema", "--all"]);
  assert_covered(&schema["outputs"]["review_plan"], &schema["outputs"]["review_plan"], &output, "review_plan");
}

/// The files of a range in dependency order: the file the other imports first, the test right after its subject, a file
/// of no known language in its place in the tree, a deleted one too
#[test_log::test]
fn prints_the_files_in_dependency_order() {
  let repo = repo_with_baseline(&[("README.md", "readme\n"), ("old.txt", "old\n")]);
  let base = git(&repo, &["rev-parse", "HEAD"]);
  write(&repo, "src/a/User.kt", "package a\n\nimport b.Store\n\nclass User(val store: Store)\n");
  write(&repo, "src/b/Store.kt", "package b\n\nclass Store\n");
  write(&repo, "src/test/b/StoreTest.kt", "package b\n\nclass StoreTest(val store: Store)\n");
  write(&repo, "README.md", "readme, more\n");
  git(&repo, &["rm", "--quiet", "old.txt"]);
  commit_all(&repo, "Add store", 1_700_000_100);

  let (exit_code, output) = run_in(&repo, &["review-plan", &format!("{base}..HEAD")]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    output["order"],
    json!(["src/b/Store.kt", "src/test/b/StoreTest.kt", "src/a/User.kt", "old.txt", "README.md"])
  );
  // the link that orders them, new; the test uses a type of its package, which links only inside a known module
  assert_eq!(
    output["links"],
    json!([{ "from": "src/a/User.kt", "to": "src/b/Store.kt", "symbols": [{ "name": "Store", "line": 3 }], "state": "added", "cross_module": false, "new_between_modules": false }])
  );
}

/// The texts of the UI a range adds and changes in a message bundle, each with its values, its line and its module; a
/// `gradle.properties` holds none
#[test_log::test]
fn prints_the_texts_of_the_ui() {
  let bundle = "gui/resources/messages/GuiBundle.properties";
  let repo = repo_with_baseline(&[
    ("gradle.properties", "kotlin.code.style=official\n"),
    ("gui/intellij.gui.iml", "<module/>\n"),
    (bundle, "gui.open=Open\ngui.play=Play\n"),
  ]);
  let base = git(&repo, &["rev-parse", "HEAD"]);
  write(&repo, "gradle.properties", "kotlin.code.style=official\norg.gradle.jvmargs=-Xmx2g\n");
  write(&repo, bundle, "gui.open=Open\ngui.play=Play the clip\ngui.cannot.play=The chat cannot play this file\n");
  commit_all(&repo, "Add the player texts", 1_700_000_100);

  let (exit_code, output) = run_in(&repo, &["review-plan", &format!("{base}..HEAD")]);
  assert_eq!(exit_code, 0, "{output}");
  let module = output["files"]
    .as_array()
    .and_then(|files| files.iter().find(|file| file["path"] == bundle))
    .map(|file| file["module"].clone())
    .unwrap();
  assert_eq!(
    output["texts"],
    json!([
      { "path": bundle, "key": "gui.play", "change": "changed", "before": "Play", "after": "Play the clip", "line": 2, "module": module },
      { "path": bundle, "key": "gui.cannot.play", "change": "added", "before": null, "after": "The chat cannot play this file", "line": 3, "module": module },
    ]),
    "{output}"
  );
  let place = usize::try_from(module.as_u64().unwrap()).unwrap();
  // the modules are accio's (`accio modules`), and the repository has no index: its files are of the root
  assert_eq!(output["modules"][place]["label"], json!("(root)"));
  let (_, schema) = run_in(&repo, &["schema", "--all"]);
  assert_covered(&schema["outputs"]["review_plan"], &schema["outputs"]["review_plan"], &output, "review_plan");
}

/// The units of a private function renamed at its declaration and its calls, which no index of the repository proves:
/// each call is `behaviour` with the rename it changed in alone, not proven; no other unit says one
#[test_log::test]
fn prints_the_renamed_of_a_body_changed_in_a_rename_alone() {
  let ledger = |name: &str| {
    format!(
      "package a\n\nclass Ledger(private val entries: List<Int>) {{\n  fun balance(): Int {{\n    return {name}(entries)\n  }}\n\n  private fun {name}(list: List<Int>): Int {{\n    return list.sum()\n  }}\n}}\n"
    )
  };
  let repo = repo_with_baseline(&[("src/a/Ledger.kt", &ledger("sumOf"))]);
  let base = git(&repo, &["rev-parse", "HEAD"]);
  write(&repo, "src/a/Ledger.kt", ledger("totalOf"));
  commit_all(&repo, "Rename sumOf", 1_700_000_100);

  let (exit_code, output) = run_in(&repo, &["review-plan", &format!("{base}..HEAD")]);
  assert_eq!(exit_code, 0, "{output}");
  let renamed: Vec<(Value, Value)> = output["units"]
    .as_array()
    .unwrap()
    .iter()
    .map(|unit| (unit["name"].clone(), unit.get("renamed").cloned().unwrap_or(Value::Null)))
    .collect();
  assert_eq!(
    renamed,
    [
      (json!("balance"), json!({ "names": [{ "old": "sumOf", "new": "totalOf" }], "wait": "notProven" })),
      (json!("totalOf"), Value::Null),
      (json!("sumOf"), Value::Null),
    ],
    "{output}"
  );
}
