use bd_cli::fixture_test::{assert_covered, commit_all, commit_subjects, git, repo_with_baseline, run_in, write};
use pretty_assertions::assert_eq;
use serde_json::json;

/// After a mark, a commit added on top is the only commit since the review, its range the scope; after a second mark,
/// an amend of that commit is `rewritten` with the lines of each version, and no plain range
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
    (&output["schema"], &output["branch"], &output["base"], &output["head"]),
    (&json!(3), &json!("test-user/virtual/auth"), &json!(base), &json!(tip))
  );
  assert_eq!(
    output["interdiff"],
    json!({
      "reviewed_base": base,
      "reviewed_tip": reviewed,
      "commits": [{ "id": tip, "reviewed_id": null, "subject": "Add logout", "state": "added" }],
      "scope": { "base": reviewed, "head": tip },
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
      &json!(null)
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
/// before and after, the facts per file, and why there are no usages (accio missing here, or without an index of the
/// test repository)
#[test_log::test]
fn prints_the_api_and_the_files_of_a_range() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  write(&repo, "src/a/Api.kt", "package a\n\nfun open(path: String): Int = 1\n");
  write(&repo, "src/test/a/ApiTest.kt", "package a\n\nclass ApiTest\n");
  commit_all(&repo, "Add api", 1_700_000_100);
  let base = git(&repo, &["rev-parse", "HEAD"]);
  write(&repo, "src/a/Api.kt", "package a\n\nfun open(path: String, mode: Int): Int = mode\n");
  write(&repo, "src/a/Session.kt", "package a\n\ninternal fun session(): Int = 1\n");
  commit_all(&repo, "Change api", 1_700_000_200);

  let (exit_code, output) = run_in(&repo, &["review-plan", &format!("{base}..HEAD")]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["api"], &output["files"]),
    (
      &json!([{
        "path": "src/a/Api.kt",
        "line": 3,
        "name": "open",
        "kind": "changed",
        "changes": ["parameters"],
        "before": "fun open(path:String) : Int",
        "after": "fun open(path:String, mode:Int) : Int",
        "usages": null,
      }]),
      &json!([
        { "path": "src/a/Api.kt", "old_path": null, "mechanical": null, "uses": 0, "used_by": 0, "new_links": 0, "breaks": 0, "untested": true, "repeated_edit": null },
        { "path": "src/a/Session.kt", "old_path": null, "mechanical": null, "uses": 0, "used_by": 0, "new_links": 0, "breaks": 0, "untested": true, "repeated_edit": null },
      ])
    )
  );
  assert!(output["index_note"].as_str().is_some_and(|note| note.starts_with("accio ")), "{output}");
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
    json!([{ "from": "src/a/User.kt", "to": "src/b/Store.kt", "symbols": [{ "name": "Store", "line": 3 }], "state": "added", "cross_module": false }])
  );
}
