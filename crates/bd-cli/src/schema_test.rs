use crate::fixture_test::{commit_all, environment, repo_with_baseline, run_in, run_json, write};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// The object or array schema behind a `$ref` into `$defs` or the non-null alternative of an `anyOf` (an `Option`)
fn resolve<'a>(root: &'a Value, schema: &'a Value) -> &'a Value {
  if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
    return resolve(root, &root["$defs"][reference.trim_start_matches("#/$defs/")]);
  }
  if let Some(alternatives) = schema.get("anyOf").and_then(Value::as_array)
    && let Some(alternative) = alternatives.iter().find(|alternative| alternative["type"] != json!("null"))
  {
    return resolve(root, alternative);
  }
  schema
}

/// Every key of `value` is a property of `schema` (or a key of a map), and every key the schema requires is in `value`,
/// recursively through objects and arrays
pub(crate) fn assert_covered(root: &Value, schema: &Value, value: &Value, path: &str) {
  let schema = resolve(root, schema);
  match value {
    Value::Object(object) => {
      // an output leaves an empty optional field out, so the schema must not require it
      for key in schema["required"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        assert!(object.contains_key(key), "{path}.{key} is required by the schema but missing");
      }
      for (key, child) in object {
        // a map declares its values with `additionalProperties`
        let property = schema["properties"].get(key).unwrap_or(&schema["additionalProperties"]);
        assert!(!property.is_null(), "{path}.{key} is not in the schema {schema}");
        assert_covered(root, property, child, &format!("{path}.{key}"));
      }
    }
    Value::Array(items) => {
      for (index, item) in items.iter().enumerate() {
        assert_covered(root, &schema["items"], item, &format!("{path}[{index}]"));
      }
    }
    _ => {}
  }
}

#[test_log::test]
fn schema_lists_every_output() {
  let directory = tempfile::tempdir().unwrap();
  let (exit_code, output) = run_json(&environment(directory.path()), &["schema"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["schema"], json!(1));
  let outputs = output["outputs"].as_object().unwrap();
  assert_eq!(
    outputs.keys().map(String::as_str).collect::<Vec<_>>(),
    [
      "check",
      "commits",
      "commits_brief",
      "describe",
      "error",
      "fold",
      "install",
      "move_hunk",
      "push",
      "rewrite",
      "safe_push",
      "status",
      "status_brief",
      "suggest",
      "sync",
      "undo",
      "update",
      "update_abort",
      "update_status",
      "version"
    ]
  );
  for (name, schema) in outputs {
    assert_eq!(schema["properties"]["schema"]["const"], json!(1), "{name}");
    assert!(schema["required"].as_array().unwrap().contains(&json!("schema")), "{name}");
  }

  let codes = &output["outputs"]["error"]["$defs"]["ErrorCode"];
  let codes: BTreeSet<&str> = codes["oneOf"]
    .as_array()
    .map(|variants| variants.iter().filter_map(|variant| variant["const"].as_str()).collect())
    .or_else(|| codes["enum"].as_array().map(|variants| variants.iter().filter_map(Value::as_str).collect()))
    .unwrap();
  for code in [
    "invalid_arguments",
    "not_a_repository",
    "no_branch_prefix",
    "conflict",
    "head_moved",
    "uncommitted_overlap",
    "unresolved_markers",
    "session_stale",
    "git_error",
  ] {
    assert!(codes.contains(code), "{code} in {codes:?}");
  }
}

/// The real outputs use only properties the schema declares
#[test_log::test]
fn outputs_match_schema() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  write(&repo, "a.txt", "a\n");
  commit_all(&repo, "(feature) Add a", 1_700_000_100);
  write(&repo, "a.txt", "a\nb\n");
  commit_all(&repo, "fixup! (feature) Add a", 1_700_000_200);
  write(&repo, "b.txt", "b\n");
  commit_all(&repo, "PROJ-1 Add b", 1_700_000_300);

  let (_, schema) = run_json(&environment(repo.path()), &["schema"]);
  let (_, status) = run_in(&repo, &["status", "--stats"]);
  assert_covered(&schema["outputs"]["status"], &schema["outputs"]["status"], &status, "status");
  let (_, commits) = run_in(&repo, &["commits", "--files", "--patch"]);
  assert_ne!(commits["dependencies"], json!([]));
  assert_covered(&schema["outputs"]["commits"], &schema["outputs"]["commits"], &commits, "commits");
  let (_, brief) = run_in(&repo, &["commits", "--brief"]);
  assert_ne!(brief["units"], json!([]));
  assert_covered(&schema["outputs"]["commits_brief"], &schema["outputs"]["commits_brief"], &brief, "commits_brief");
  let (_, suggest) = run_in(&repo, &["suggest", "--fresh"]);
  assert_eq!(suggest.get("error"), None, "{suggest}");
  assert_ne!(suggest["clusters"], json!([]));
  assert_covered(&schema["outputs"]["suggest"], &schema["outputs"]["suggest"], &suggest, "suggest");
  let (_, version) = run_in(&repo, &["version"]);
  assert_covered(&schema["outputs"]["version"], &schema["outputs"]["version"], &version, "version");
  // a dry run prints every optional part of a rewrite; then a real one and its undo
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let (_, rewrite) = run_in(&repo, &["rename", "feature", "renamed", "--dry-run", "--map"]);
  assert_covered(&schema["outputs"]["rewrite"], &schema["outputs"]["rewrite"], &rewrite, "rewrite");
  let (_, rewrite) = run_in(&repo, &["restack", "feature", "PROJ-1"]);
  assert_eq!(rewrite.get("error"), None, "{rewrite}");
  assert_covered(&schema["outputs"]["rewrite"], &schema["outputs"]["rewrite"], &rewrite, "rewrite");
  let (_, undo) = run_in(&repo, &["undo", rewrite["undo_token"].as_str().unwrap()]);
  assert_covered(&schema["outputs"]["undo"], &schema["outputs"]["undo"], &undo, "undo");

  let (_, describe) = run_in(&repo, &["describe", "PROJ-1", "--title", "Add b", "--body", "Body"]);
  assert_covered(&schema["outputs"]["describe"], &schema["outputs"]["describe"], &describe, "describe");
  let (_, check) = run_in(&repo, &["check"]);
  assert_eq!(check.get("error"), None, "{check}");
  assert_covered(&schema["outputs"]["check"], &schema["outputs"]["check"], &check, "check");
  let (_, sync) = run_in(&repo, &["sync"]);
  assert_eq!(sync.get("error"), None, "{sync}");
  assert_covered(&schema["outputs"]["sync"], &schema["outputs"]["sync"], &sync, "sync");
  let (_, status) = run_in(&repo, &["status", "--stats"]);
  assert_covered(&schema["outputs"]["status"], &schema["outputs"]["status"], &status, "status with stacks");
  let (_, brief) = run_in(&repo, &["status", "--brief"]);
  assert_covered(&schema["outputs"]["status_brief"], &schema["outputs"]["status_brief"], &brief, "status_brief");

  // the issue groups: a rewrite of them only, the status with the group, and the undo
  write(&repo, "c.txt", "c\n");
  commit_all(&repo, "PROJ-2 Add c", 1_700_000_400);
  let plan = tempfile::NamedTempFile::new().unwrap();
  let descriptions = json!({ "proj": { "title": "Proj", "body": "Body" } });
  std::fs::write(
    plan.path(),
    json!({ "issue_groups": { "proj": ["PROJ-1", "PROJ-2"] }, "descriptions": descriptions }).to_string(),
  )
  .unwrap();
  let (_, rewrite) = run_in(&repo, &["group", "--sync", "--plan", plan.path().to_str().unwrap()]);
  assert_eq!(rewrite.get("error"), None, "{rewrite}");
  assert!(rewrite["sync"].is_object() && rewrite["described"].is_array(), "{rewrite}");
  assert_covered(&schema["outputs"]["rewrite"], &schema["outputs"]["rewrite"], &rewrite, "rewrite of issue groups");
  let (_, status) = run_in(&repo, &["status"]);
  assert_covered(&schema["outputs"]["status"], &schema["outputs"]["status"], &status, "status with issue groups");
  let (_, undo) = run_in(&repo, &["undo", rewrite["undo_token"].as_str().unwrap()]);
  assert_covered(&schema["outputs"]["undo"], &schema["outputs"]["undo"], &undo, "undo of issue groups");

  let (_, error) = run_in(&repo, &["status", "--branch", "missing"]);
  assert_covered(&schema["outputs"]["error"], &schema["outputs"]["error"], &error, "error");
}
