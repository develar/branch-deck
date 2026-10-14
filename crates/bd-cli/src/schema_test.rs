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

/// Every key of `value` is a property of `schema`, recursively through objects and arrays
fn assert_covered(root: &Value, schema: &Value, value: &Value, path: &str) {
  let schema = resolve(root, schema);
  match value {
    Value::Object(object) => {
      for (key, child) in object {
        let property = &schema["properties"][key];
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
  assert_eq!(outputs.keys().map(String::as_str).collect::<Vec<_>>(), ["commits", "error", "status"]);
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
  for code in ["invalid_arguments", "not_a_repository", "no_branch_prefix", "conflict", "head_moved", "git_error"] {
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
  let (_, status) = run_in(&repo, &["status"]);
  assert_covered(&schema["outputs"]["status"], &schema["outputs"]["status"], &status, "status");
  let (_, commits) = run_in(&repo, &["commits", "--files", "--patch"]);
  assert_ne!(commits["dependencies"], json!([]));
  assert_covered(&schema["outputs"]["commits"], &schema["outputs"]["commits"], &commits, "commits");
  let (_, error) = run_in(&repo, &["status", "--branch", "missing"]);
  assert_covered(&schema["outputs"]["error"], &schema["outputs"]["error"], &error, "error");
}
