use crate::fixture_test::{assert_covered, commit_all, environment, repo_with_commits, run_in, run_json, write};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use test_utils::history::HistoryCommit;

#[test_log::test]
fn schema_lists_every_output() {
  let directory = tempfile::tempdir().unwrap();
  let (exit_code, output) = run_json(&environment(directory.path()), &["schema", "--all"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["schema"], json!(2));
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
      "land",
      "lease_adopt",
      "lease_clean",
      "lease_get",
      "lease_pick",
      "lease_reclaim",
      "lease_return",
      "lease_status",
      "mark_reviewed",
      "move_hunk",
      "operation_forget",
      "park",
      "pick",
      "push",
      "reapply",
      "request_review",
      "review_plan",
      "rewrite",
      "robot",
      "robot_stats",
      "safe_push",
      "squash",
      "status",
      "status_brief",
      "suggest",
      "sync",
      "unapply",
      "undo",
      "update",
      "update_abort",
      "update_status",
      "version",
      "workspace"
    ]
  );
  for (name, schema) in outputs {
    // the experimental plan of a review has a version of its own
    let version = if name == "review_plan" { 7 } else { 2 };
    assert_eq!(schema["properties"]["schema"]["const"], json!(version), "{name}");
    assert_eq!(schema["properties"]["operation_id"]["type"], json!("string"), "{name}");
    for field in ["schema", "operation_id"] {
      assert!(schema["required"].as_array().unwrap().contains(&json!(field)), "{name}: {field}");
    }
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
    "stack_conflict",
    "head_moved",
    "uncommitted_overlap",
    "unresolved_markers",
    "session_stale",
    "landed",
    "git_error",
    "internal",
    "canceled",
  ] {
    assert!(codes.contains(code), "{code} in {codes:?}");
  }
}

/// The real outputs use only properties the schema declares
#[test_log::test]
fn outputs_match_schema() {
  let (repo, _) = repo_with_commits(
    &[("README.md", "readme\n")],
    &[
      HistoryCommit::new("(feature) Add a", &[("a.txt", "a\n")]).at(1_700_000_100),
      HistoryCommit::new("fixup! (feature) Add a", &[("a.txt", "a\nb\n")]).at(1_700_000_200),
      HistoryCommit::new("PROJ-1 Add b", &[("b.txt", "b\n")]).at(1_700_000_300),
    ],
  );

  let (_, schema) = run_json(&environment(repo.path()), &["schema", "--all"]);
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
  let (_, workspace) = run_in(&repo, &["workspace"]);
  assert_covered(&schema["outputs"]["workspace"], &schema["outputs"]["workspace"], &workspace, "workspace");
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
  let (_, check) = run_in(&repo, &["check", "--fix"]);
  assert_eq!(check.get("error"), None, "{check}");
  assert!(check.get("fix").is_some(), "{check}");
  assert_covered(&schema["outputs"]["check"], &schema["outputs"]["check"], &check, "check");
  let (_, sync) = run_in(&repo, &["sync"]);
  assert_eq!(sync.get("error"), None, "{sync}");
  assert_covered(&schema["outputs"]["sync"], &schema["outputs"]["sync"], &sync, "sync");
  // the second mark names the pair it replaced
  for _ in 0..2 {
    let (_, marked) = run_in(&repo, &["mark-reviewed", "PROJ-1"]);
    assert_eq!(marked.get("error"), None, "{marked}");
    assert_covered(&schema["outputs"]["mark_reviewed"], &schema["outputs"]["mark_reviewed"], &marked, "mark_reviewed");
  }
  let (_, plan) = run_in(&repo, &["review-plan", "PROJ-1"]);
  assert_ne!(plan["interdiff"]["commits"], json!([]), "{plan}");
  assert_covered(&schema["outputs"]["review_plan"], &schema["outputs"]["review_plan"], &plan, "review_plan");
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

  // a stack that forks: its stack branch in the sync and the status
  write(&repo, "d.txt", "d\n");
  commit_all(&repo, "(PROJ-1/side) Add d", 1_700_000_500);
  write(&repo, "e.txt", "e\n");
  commit_all(&repo, "(PROJ-1/other) Add e", 1_700_000_600);
  let (_, sync) = run_in(&repo, &["sync"]);
  assert_eq!(sync["stacks"][0]["state"], json!("built"), "{sync}");
  assert_covered(&schema["outputs"]["sync"], &schema["outputs"]["sync"], &sync, "sync with a stack branch");
  let (_, status) = run_in(&repo, &["status"]);
  assert_eq!(status["stacks"][0]["state"], json!("built"), "{status}");
  assert_covered(&schema["outputs"]["status"], &schema["outputs"]["status"], &status, "status with a stack branch");
  let (_, brief) = run_in(&repo, &["status", "--brief"]);
  assert_covered(
    &schema["outputs"]["status_brief"],
    &schema["outputs"]["status_brief"],
    &brief,
    "status_brief with a stack branch",
  );

  let (_, error) = run_in(&repo, &["status", "--branch", "missing"]);
  assert_covered(&schema["outputs"]["error"], &schema["outputs"]["error"], &error, "error");
}

/// Without a name, a few KB that name every command and its outputs; every subcommand either names outputs that exist
/// or has none, and together the commands cover every output but `error`
#[test]
fn schema_without_a_name_is_the_index_of_the_commands() {
  let directory = tempfile::tempdir().unwrap();
  let environment = environment(directory.path());
  let mut out = Vec::new();
  assert_eq!(crate::run_with(["schema"].map(std::ffi::OsString::from), &environment, &mut out), 0);
  assert!(out.len() < 4096, "{} bytes", out.len());
  let index: Value = serde_json::from_slice(&out).unwrap();
  assert_eq!(index["schema"], json!(2));
  assert_eq!(index["commands"]["group"], json!(["rewrite"]));
  assert_eq!(index["commands"]["status"], json!(["status", "status_brief"]));

  let every: BTreeSet<String> = crate::schema::every_output().into_keys().collect();
  let mut covered = BTreeSet::from(["error".to_string()]);
  let mut command = <crate::Cli as clap::CommandFactory>::command();
  command.build();
  for subcommand in command.get_subcommands().map(clap::Command::get_name) {
    if crate::schema::WITHOUT_OUTPUT.contains(&subcommand) {
      assert_eq!(index["commands"].get(subcommand), None, "{subcommand}");
      continue;
    }
    let keys = index["commands"][subcommand]
      .as_array()
      .unwrap_or_else(|| panic!("no outputs for `bd {subcommand}` in COMMAND_OUTPUTS"));
    for key in keys {
      let key = key.as_str().unwrap();
      assert!(every.contains(key), "`bd {subcommand}` names the output {key}, which doesn't exist");
      covered.insert(key.to_string());
    }
  }
  assert_eq!(covered, every, "outputs that no command names");
}

/// A command prints the schemas of its outputs, an output name its own; an unknown name names them all
#[test]
fn schema_of_a_command_is_only_its_outputs() {
  let directory = tempfile::tempdir().unwrap();
  let environment = environment(directory.path());
  let keys = |names: &[&str]| -> Vec<String> {
    let (exit_code, output) = run_json(&environment, &[&["schema"][..], names].concat());
    assert_eq!(exit_code, 0, "{output}");
    output["outputs"].as_object().unwrap().keys().cloned().collect()
  };
  assert_eq!(keys(&["group"]), ["rewrite"]);
  assert_eq!(keys(&["status"]), ["status", "status_brief"]);
  assert_eq!(keys(&["safe-push", "error"]), ["error", "safe_push"]);
  assert_eq!(keys(&["lease_get"]), ["lease_get"]);

  let (exit_code, output) = run_json(&environment, &["schema", "stauts"]);
  assert_eq!(exit_code, 2, "{output}");
  let message = output["error"]["message"].as_str().unwrap();
  assert!(
    message.contains("\"stauts\"") && message.contains("safe-push") && message.contains("status_brief"),
    "{message}"
  );
}
