use bd_cli::fixture_test::assert_covered;
#[cfg(unix)]
use bd_cli::fixture_test::{LockSpanGit, subcommand};
use bd_cli::fixture_test::{assert_refused_during, commit_all, git, repo_with_baseline, run_in, start_cherry_pick, sync_statuses, with_header, write};
use git_ops::revs::short_id;
use history_rewrite::repository_lock::{DEFAULT_LOCK_TIMEOUT, lock_repository};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use test_utils::git_test_utils::TestRepo;

/// Commits with these subjects ahead of `origin/master`; returns their ids
fn repo_with(subjects: &[&str]) -> (TestRepo, Vec<String>) {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let ids = subjects
    .iter()
    .enumerate()
    .map(|(index, subject)| {
      write(&repo, &format!("file{index}.txt"), *subject);
      commit_all(&repo, subject, 1_700_000_000 + i64::try_from(index).unwrap() * 100)
    })
    .collect();
  (repo, ids)
}

/// The commits ahead of the baseline, oldest first
fn ahead(repo: &TestRepo) -> Vec<String> {
  git(repo, &["rev-list", "--reverse", "origin/master..HEAD"]).lines().map(str::to_string).collect()
}

fn subjects(repo: &TestRepo) -> Vec<String> {
  git(repo, &["log", "--reverse", "--format=%s", "origin/master..HEAD"]).lines().map(str::to_string).collect()
}

/// Run `bd group` with `plan` written to a file
fn group(repo: &TestRepo, plan: &Value, extra: &[&str]) -> (i32, Value) {
  let file = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(file.path(), plan.to_string()).unwrap();
  let mut args = vec!["group", "--plan", file.path().to_str().unwrap()];
  args.extend_from_slice(extra);
  run_in(repo, &args)
}

fn assert_error(output: &Value, exit_code: i32, expected_exit_code: i32, code: &str) {
  assert_eq!((exit_code, &output["error"]["code"]), (expected_exit_code, &json!(code)), "{output}");
}

/// Without `--plan` there is no plan: stdin is never read
#[test_log::test]
fn group_without_a_plan_is_refused() {
  let (repo, _) = repo_with(&["Add login"]);
  let (exit_code, output) = run_in(&repo, &["group"]);
  assert_error(&output, exit_code, 2, "invalid_arguments");
}

#[test_log::test]
fn group_applies_a_plan() {
  let (repo, ids) = repo_with(&["Add login", "Fix crash", "Tidy up"]);
  let head = repo.head();
  let (exit_code, output) = group(
    &repo,
    &json!({ "branches": { "auth": [short_id(&ids[0]), short_id(&ids[1])] } }),
    &["--expect-head", short_id(&head)],
  );
  assert_eq!(exit_code, 0, "{output}");

  let new_ids = ahead(&repo);
  assert_eq!(
    output,
    with_header(
      &repo,
      json!({
      "operation": "group",
      "dry_run": false,
      "previous_head": short_id(&head),
      "new_head": short_id(&new_ids[2]),
      "rewritten": 2,
      "undo_token": format!("{head}..{}", new_ids[2]),
      "groups": { "auth": [short_id(&new_ids[0]), short_id(&new_ids[1])] },
      "unassigned": [short_id(&new_ids[2])],
      })
    )
  );
  assert_eq!(subjects(&repo), ["(auth) Add login", "(auth) Fix crash", "Tidy up"]);
}

#[test_log::test]
fn dry_run_prints_the_changes_and_the_forest() {
  let (repo, ids) = repo_with(&["(auth) Add auth", "Add login", "Tidy up"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let head = repo.head();
  let (exit_code, output) = group(&repo, &json!({ "branches": { "auth/login": [ids[1]] } }), &["--dry-run", "--map"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(repo.head(), head, "a dry run writes nothing");
  assert_eq!(output["dry_run"], json!(true));
  assert_eq!(output.get("undo_token"), None);
  assert_eq!(output["new_head"], json!(short_id(&head)));
  // the path stacks login in the stack parents, so its prefix is the bare leaf
  assert_eq!(
    output["changes"],
    json!([{
      "commit": short_id(&ids[1]),
      "old_subject": "Add login",
      "new_subject": "(login) Add login",
      "moved": { "from": null, "to": "login" },
    }])
  );
  assert_eq!(output["stack_parents"], json!({ "login": "auth" }));
  assert_eq!(
    output["forest"],
    json!([
      { "name": "auth", "has_commits": true },
      { "name": "login", "parent": "auth", "parent_source": "store", "has_commits": true },
    ])
  );
  assert_eq!(output["map"][1], json!({ "commit": short_id(&ids[1]), "new_id": short_id(&ids[1]) }));
}

#[test_log::test]
fn group_errors_and_exit_codes() {
  let (repo, ids) = repo_with(&["One", "Two"]);
  let head = repo.head();

  let (exit_code, output) = group(&repo, &json!({ "branches": { "a": ["0123456789"] } }), &[]);
  assert_error(&output, exit_code, 2, "not_ahead_of_baseline");
  assert_eq!(output["error"]["commits"], json!(["0123456789"]));

  let (exit_code, output) = group(&repo, &json!({ "branches": { "a": [ids[0]] }, "unassign": [ids[0]] }), &[]);
  assert_error(&output, exit_code, 2, "invalid_plan");
  assert_eq!(output["error"]["commits"], json!([short_id(&ids[0])]));

  let (exit_code, output) = group(&repo, &json!({ "branch": {} }), &[]);
  assert_error(&output, exit_code, 2, "invalid_plan");

  let (exit_code, output) = group(&repo, &json!({ "branches": { "a": [ids[0]] } }), &["--expect-head", short_id(&ids[0])]);
  assert_error(&output, exit_code, 3, "head_moved");
  assert_eq!(output["error"]["retryable"], json!(true));

  // another process holds the lock: retry later
  let held = lock_repository(repo.git_executor(), repo.path().to_str().unwrap(), DEFAULT_LOCK_TIMEOUT).unwrap();
  let (exit_code, output) = group(&repo, &json!({ "branches": { "a": [ids[0]] } }), &[]);
  assert_error(&output, exit_code, 3, "locked");
  drop(held);

  git(&repo, &["checkout", "--detach", "-q"]);
  let (exit_code, output) = group(&repo, &json!({ "branches": { "a": [ids[0]] } }), &[]);
  assert_error(&output, exit_code, 4, "detached_head");

  assert_eq!(repo.head(), head, "nothing was written");
}

#[test_log::test]
fn group_and_its_dry_run_are_refused_during_a_cherry_pick() {
  let (repo, ids) = repo_with(&["One", "Two"]);
  start_cherry_pick(&repo, repo.path());
  let plan = json!({ "branches": { "a": [ids[0]] } });
  assert_refused_during(&repo, "a cherry-pick", |extra| group(&repo, &plan, extra));
}

#[test_log::test]
fn group_with_allow_append_lists_the_appended_commits() {
  let (repo, ids) = repo_with(&["One", "Two"]);
  let expected = repo.head();
  write(&repo, "three.txt", "three\n");
  commit_all(&repo, "Three", 1_700_000_900);
  let plan = json!({ "branches": { "a": [ids[0]] } });

  let (exit_code, output) = group(&repo, &plan, &["--expect-head", short_id(&expected)]);
  assert_error(&output, exit_code, 3, "head_moved");

  let (exit_code, output) = group(&repo, &plan, &["--expect-head", short_id(&expected), "--allow-append"]);
  assert_eq!(exit_code, 0, "{output}");
  let new_head = git(&repo, &["rev-parse", "HEAD"]);
  assert_eq!(output["appended"], json!([{ "commit": short_id(&new_head), "subject": "Three" }]));
  assert_eq!(subjects(&repo), ["(a) One", "Two", "Three"]);

  // it needs --expect-head
  let (exit_code, output) = group(&repo, &plan, &["--allow-append"]);
  assert_error(&output, exit_code, 2, "invalid_arguments");
}

#[test_log::test]
fn rename_restack_unstack_and_undo() {
  let (repo, _) = repo_with(&["(auth) Add auth", "(login) Add login", "(auth/login/oauth) Add oauth"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let initial_head = repo.head();

  // the stack parents stack login: no rewrite, `(auth/login/oauth)` declares the same parent
  let (exit_code, output) = run_in(&repo, &["restack", "login", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["operation"], json!("restack"));
  assert_eq!(output["rewritten"], json!(0));
  assert_eq!(output["stack_parents"], json!({ "login": "auth" }));
  assert!(output["undo_token"].as_str().unwrap().starts_with("stacks:"), "{output}");
  assert_eq!(repo.head(), initial_head);
  assert_eq!(subjects(&repo), ["(auth) Add auth", "(login) Add login", "(auth/login/oauth) Add oauth"]);

  // a rename renames the prefixes and the stack parents in one transaction
  let (exit_code, output) = run_in(&repo, &["rename", "auth", "identity"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["rewritten"], json!(2));
  assert_eq!(output["stack_parents"], json!({ "login": "identity" }));
  assert_eq!(subjects(&repo), ["(identity) Add auth", "(login) Add login", "(identity/login/oauth) Add oauth"]);
  let renamed_head = repo.head();
  let rename_token = output["undo_token"].as_str().unwrap().to_string();
  assert!(rename_token.contains("+stacks:"), "{rename_token}");

  // an unstack removes the line and the levels above login in a deeper path
  let (exit_code, output) = run_in(&repo, &["unstack", "login"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(subjects(&repo)[1..], ["(login) Add login", "(login/oauth) Add oauth"]);
  assert_eq!(output["stack_parents"], json!({}));
  let unstack_token = output["undo_token"].as_str().unwrap().to_string();

  // undo the last rewrite, then refuse a stale token
  let (exit_code, output) = run_in(&repo, &["undo", &unstack_token]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["new_head"], json!(short_id(&renamed_head)));
  assert!(output["undone_stack_parents"].is_string(), "{output}");
  assert_eq!(repo.head(), renamed_head);
  // the stack parents moved on: the token can never succeed again
  let (exit_code, output) = run_in(&repo, &["undo", &unstack_token]);
  assert_error(&output, exit_code, 4, "undo_stale");
  assert_eq!(output["error"]["retryable"], json!(false));
  // the words say it can't be undone, not that a retry may help
  let message = output["error"]["message"].as_str().unwrap();
  assert!(
    message.starts_with("The stack parents (") && message.ends_with("; it can't be undone any more"),
    "{message}"
  );
  let (exit_code, output) = run_in(&repo, &["undo", "not-a-token"]);
  assert_error(&output, exit_code, 2, "invalid_arguments");

  let (exit_code, output) = run_in(&repo, &["undo", &rename_token]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(repo.head(), initial_head, "the restack before the rename moved no commit");
  assert_eq!(subjects(&repo)[0], "(auth) Add auth");

  let (exit_code, output) = run_in(&repo, &["rename", "auth", "a/b"]);
  assert_error(&output, exit_code, 2, "invalid_plan");
}

#[test_log::test]
fn issue_groups_plan_and_undo() {
  let (repo, ids) = repo_with(&["ABC-1 Add parser", "ABC-2 Parse lists", "Tidy up"]);
  let head = repo.head();
  let plan = json!({ "issue_groups": { "parser": ["ABC-1", "ABC-2"] } });

  let (exit_code, preview) = group(&repo, &plan, &["--dry-run"]);
  assert_eq!(exit_code, 0, "{preview}");
  assert_eq!(
    (&preview["rewritten"], preview.get("undo_token"), &preview["issue_groups"], &preview["groups"]),
    (
      &json!(0),
      None,
      &json!({ "parser": ["ABC-1", "ABC-2"] }),
      &json!({ "parser": [short_id(&ids[0]), short_id(&ids[1])] })
    )
  );

  let (exit_code, output) = group(&repo, &plan, &["--expect-head", short_id(&head)]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["rewritten"], &output["new_head"]), (&json!(0), &json!(short_id(&head))));
  let token = output["undo_token"].as_str().unwrap().to_string();
  assert!(token.starts_with("groups:"), "{token}");
  assert_eq!(repo.head(), head);
  let (_, status) = run_in(&repo, &["status", "--brief"]);
  assert_eq!(status["branches"][0]["name"], json!("parser"));

  // a mixed plan: one rewrite and a change of the issue groups, undone together
  let (exit_code, mixed) = group(&repo, &json!({ "branches": { "chores": [short_id(&ids[2])] }, "ungroup_issues": ["ABC-2"] }), &[]);
  assert_eq!(exit_code, 0, "{mixed}");
  let mixed_token = mixed["undo_token"].as_str().unwrap().to_string();
  assert!(mixed_token.contains("+groups:"), "{mixed_token}");
  assert_eq!(mixed["issue_groups"], json!({ "parser": ["ABC-1"] }));

  // the first token is stale now: the issue groups moved
  let (exit_code, refused) = run_in(&repo, &["undo", &token]);
  assert_error(&refused, exit_code, 4, "undo_stale");
  assert_eq!(refused["error"]["retryable"], json!(false));

  let (exit_code, undone) = run_in(&repo, &["undo", &mixed_token]);
  assert_eq!(exit_code, 0, "{undone}");
  assert_eq!(repo.head(), head);
  assert!(undone["undone_issue_groups"].is_string(), "{undone}");
  let (exit_code, undone) = run_in(&repo, &["undo", &token]);
  assert_eq!(exit_code, 0, "{undone}");
  assert_eq!((&undone["new_head"], undone.get("previous_head")), (&json!(short_id(&head)), None));
  assert_eq!(git(&repo, &["for-each-ref", "refs/branch-deck/issue-groups"]), "");

  let (exit_code, invalid) = group(&repo, &json!({ "issue_groups": { "parser": ["ABC-1"], "lexer": ["ABC-1"] } }), &[]);
  assert_error(&invalid, exit_code, 2, "invalid_plan");

  // a token takes full ids only, and its map commit must be a write of the map
  let (exit_code, refused) = run_in(&repo, &["undo", "groups:HEAD..refs/branch-deck/issue-groups"]);
  assert_error(&refused, exit_code, 2, "invalid_arguments");
  let (_, again) = group(&repo, &plan, &[]);
  let written = again["undo_token"].as_str().unwrap().rsplit("..").next().unwrap().to_string();
  let (exit_code, refused) = run_in(&repo, &["undo", &format!("groups:{head}..{written}")]);
  assert_error(&refused, exit_code, 2, "invalid_arguments");
}

/// The sync of `--sync` runs after the rewrite released the repository lock: its merges and its writes never make the
/// app or another `bd` wait
#[cfg(unix)]
#[test_log::test]
fn the_sync_of_a_rewrite_runs_after_the_lock() {
  // a commit before it, so the sync merges the branch's commit onto the baseline
  let (repo, _) = repo_with(&["Tidy up", "(auth) Add login"]);
  let wrapped = LockSpanGit::new(&repo, "");
  let (exit_code, output) = wrapped.run(&["rename", "auth", "login", "--sync"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(sync_statuses(&output), [(&json!("login"), &json!("created"))]);
  let commands = wrapped.commands();
  let renamed = commands.iter().rposition(|(locked, _)| *locked).expect("the rename runs under the lock");
  let merged = commands.iter().position(|(_, args)| subcommand(args) == "merge-tree").expect("the sync merges");
  assert!(merged > renamed, "the sync after the lock: {commands:#?}");
}

#[test_log::test]
fn group_writes_descriptions_and_syncs() {
  let (repo, ids) = repo_with(&["Add login", "ABC-1 Fix crash"]);
  let head = repo.head();
  let plan = json!({
    "branches": { "auth": [short_id(&ids[0])] },
    "descriptions": {
      "auth": { "title": " Add login ", "body": "The login form.\n" },
      "ABC-1": { "title": "Fix the crash" },
    },
  });
  let (exit_code, output) = group(&repo, &plan, &["--expect-head", short_id(&head), "--sync"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["rewritten"], &output["described"]), (&json!(1), &json!(["ABC-1", "auth"])));
  assert_eq!(output.get("replaced_descriptions"), None, "no text was there before");
  assert!(output["undo_token"].is_string(), "{output}");
  assert_eq!(sync_statuses(&output), [(&json!("ABC-1"), &json!("created")), (&json!("auth"), &json!("created"))]);
  assert_eq!(git(&repo, &["config", "branch.test-user/virtual/auth.description"]), "Add login\n\nThe login form.");
  assert_eq!(git(&repo, &["config", "branch.test-user/virtual/ABC-1.description"]), "Fix the crash");

  // the same plan with the new id: nothing to rewrite or describe, the branches are synced already
  let mut plan = plan;
  plan["branches"]["auth"] = output["groups"]["auth"].clone();
  let (exit_code, again) = group(&repo, &plan, &["--sync"]);
  assert_eq!(exit_code, 0, "{again}");
  assert_eq!((&again["rewritten"], again.get("described"), again.get("undo_token")), (&json!(0), None, None));
  assert!(
    again["sync"]["branches"].as_array().unwrap().iter().all(|branch| branch["state"] == json!("unchanged")),
    "{again}"
  );

  // descriptions only: no rewrite, no --expect-head needed; the replaced text is printed
  let (exit_code, described) = group(&repo, &json!({ "descriptions": { "auth": { "title": "Login" } } }), &[]);
  assert_eq!(exit_code, 0, "{described}");
  assert_eq!(
    (&described["rewritten"], &described["described"], described.get("sync")),
    (&json!(0), &json!(["auth"]), None)
  );
  assert_eq!(described["replaced_descriptions"], json!({ "auth": { "title": "Add login", "body": "The login form." } }));
  assert_eq!(git(&repo, &["config", "branch.test-user/virtual/auth.description"]), "Login");
}

#[test_log::test]
fn a_sync_conflict_after_group_exits_with_1_and_keeps_the_undo_token() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  write(&repo, "shared.txt", "one\n");
  commit_all(&repo, "(a) Add shared", 1_700_000_100);
  write(&repo, "shared.txt", "two\n");
  let change = commit_all(&repo, "Change shared", 1_700_000_200);

  let (exit_code, output) = group(&repo, &json!({ "branches": { "b": [short_id(&change)] } }), &["--sync"]);
  assert_eq!(exit_code, 1, "{output}");
  assert_eq!(output["rewritten"], json!(1));
  let b = output["sync"]["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!("b")).unwrap();
  assert_eq!(b["state"], json!("conflict"));
  let (exit_code, undone) = run_in(&repo, &["undo", output["undo_token"].as_str().unwrap()]);
  assert_eq!(exit_code, 0, "{undone}");
  assert_eq!(subjects(&repo), ["(a) Add shared", "Change shared"]);
}

#[test_log::test]
fn a_failed_description_write_is_the_error_of_the_output_and_keeps_the_undo_token() {
  let (repo, ids) = repo_with(&["Add login", "Tidy up"]);
  // another git process writes the config: `git config` can't lock it, the regroup doesn't need it
  let config_lock = repo.path().join(".git/config.lock");
  std::fs::write(&config_lock, "").unwrap();
  let plan = json!({ "branches": { "auth": [short_id(&ids[0])] }, "descriptions": { "auth": { "title": "Login" } } });
  let (exit_code, output) = group(&repo, &plan, &["--sync"]);
  std::fs::remove_file(&config_lock).unwrap();

  // the error of the step after the rewrite, never the error envelope: the output of the rewrite stays
  assert_eq!((exit_code, &output["after_error"]["code"], output.get("error")), (5, &json!("git_error"), None), "{output}");
  assert_eq!((&output["rewritten"], output.get("described"), output.get("sync")), (&json!(1), None, None), "{output}");
  let (_, schema) = run_in(&repo, &["schema"]);
  assert_covered(&schema["outputs"]["rewrite"], &schema["outputs"]["rewrite"], &output, "rewrite with an error");
  assert_eq!(description_count(&repo), 0);
  let (exit_code, undone) = run_in(&repo, &["undo", output["undo_token"].as_str().unwrap()]);
  assert_eq!(exit_code, 0, "{undone}");
  assert_eq!(subjects(&repo), ["Add login", "Tidy up"]);
}

#[test_log::test]
fn a_stacked_branch_is_described_by_its_leaf() {
  let (repo, ids) = repo_with(&["(auth) Add auth", "Add login"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let branches = json!({ "auth/login": [short_id(&ids[1])] });
  let (exit_code, output) = group(
    &repo,
    &json!({ "branches": branches, "descriptions": { "auth/login": { "title": "Login" } } }),
    &["--dry-run"],
  );
  assert_error(&output, exit_code, 2, "invalid_plan");
  assert!(output["error"]["message"].as_str().unwrap().contains("`login`"), "{output}");
  let (exit_code, output) = group(&repo, &json!({ "branches": branches, "descriptions": { "login": { "title": "Login" } } }), &["--dry-run"]);
  assert_eq!((exit_code, &output["described"]), (0, &json!(["login"])), "{output}");
}

/// The branch descriptions in the git config
fn description_count(repo: &TestRepo) -> usize {
  let (output, _) = repo
    .git_executor()
    .execute_command_with_status(&["config", "--get-regexp", r"^branch\..*\.description$"], repo.path().to_str().unwrap())
    .unwrap();
  output.lines().count()
}

#[test_log::test]
fn descriptions_are_validated_with_the_plan() {
  let (repo, ids) = repo_with(&["Add login", "Tidy up"]);
  let head = repo.head();
  let moved = json!([short_id(&ids[0])]);
  for descriptions in [
    json!({ "auth": { "title": "" } }),
    json!({ "auth": { "title": "Two\nlines" } }),
    json!({ "auth": { "title": "Two\rlines" } }),
    json!({ "auth": { "body": "No title" } }),
    json!({ "auth": { "title": "Login", "draft": true } }),
    // a branch that no commit is in after the plan
    json!({ "login": { "title": "Login" } }),
  ] {
    let plan = json!({ "branches": { "auth": moved }, "descriptions": descriptions });
    let (exit_code, output) = group(&repo, &plan, &[]);
    assert_error(&output, exit_code, 2, "invalid_plan");
    let (exit_code, output) = group(&repo, &plan, &["--dry-run"]);
    assert_error(&output, exit_code, 2, "invalid_plan");
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), plan.to_string()).unwrap();
    let (exit_code, output) = run_in(&repo, &["check", "--plan", file.path().to_str().unwrap()]);
    assert_error(&output, exit_code, 2, "invalid_plan");
  }
  assert_eq!(repo.head(), head, "nothing was written");
  assert_eq!(description_count(&repo), 0);

  // a valid one: a dry run names it and writes nothing; check accepts it
  let plan = json!({ "branches": { "auth": moved }, "descriptions": { "auth": { "title": "Login" } } });
  let (exit_code, output) = group(&repo, &plan, &["--dry-run"]);
  assert_eq!((exit_code, &output["described"]), (0, &json!(["auth"])), "{output}");
  assert_eq!(description_count(&repo), 0);
  let file = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(file.path(), plan.to_string()).unwrap();
  let (exit_code, output) = run_in(&repo, &["check", "--plan", file.path().to_str().unwrap()]);
  assert_eq!(exit_code, 0, "{output}");

  // a misspelt field names `descriptions` among the expected ones
  let (exit_code, output) = group(&repo, &json!({ "description": { "auth": { "title": "Login" } } }), &[]);
  assert_error(&output, exit_code, 2, "invalid_plan");
  assert!(output["error"]["message"].as_str().unwrap().contains("`descriptions`"), "{output}");

  // a dry run can't sync
  let (exit_code, output) = group(&repo, &plan, &["--dry-run", "--sync"]);
  assert_error(&output, exit_code, 2, "invalid_arguments");

  let (exit_code, output) = group(&repo, &plan, &[]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(description_count(&repo), 1);
}

#[test_log::test]
fn stack_parents_plan_and_undo() {
  let (repo, ids) = repo_with(&["IJAI-3992 Add the model", "IJAI-4047 Use the model", "Tidy up"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let head = repo.head();

  // whole issue branches stack without a rewrite: only the stack parents move
  let (exit_code, output) = group(
    &repo,
    &json!({ "branches": { "IJAI-3992/IJAI-4047": [short_id(&ids[1])] } }),
    &["--expect-head", short_id(&head)],
  );
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["rewritten"], &output["stack_parents"]), (&json!(0), &json!({ "IJAI-4047": "IJAI-3992" })));
  let token = output["undo_token"].as_str().unwrap().to_string();
  assert!(token.starts_with("stacks:"), "{token}");
  assert_eq!(repo.head(), head);
  let (_, status) = run_in(&repo, &["status", "--branch", "IJAI-4047"]);
  assert_eq!(
    (&status["branches"][0]["parent"], &status["branches"][0]["parent_source"]),
    (&json!("IJAI-3992"), &json!("store"))
  );

  // a commit without a key gets the bare leaf; `unstack` makes a branch a root again
  let (exit_code, output) = group(&repo, &json!({ "branches": { "IJAI-4047/tidy": [short_id(&ids[2])] }, "unstack": ["IJAI-4047"] }), &[]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(subjects(&repo)[2], "(tidy) Tidy up");
  assert_eq!(output["stack_parents"], json!({ "tidy": "IJAI-4047" }));
  let mixed = output["undo_token"].as_str().unwrap().to_string();
  assert!(mixed.contains("+stacks:"), "{mixed}");

  // a token whose stack parents moved on is `undo_stale`
  let (exit_code, output) = run_in(&repo, &["undo", &token]);
  assert_error(&output, exit_code, 4, "undo_stale");
  for undo in [&mixed, &token] {
    let (exit_code, output) = run_in(&repo, &["undo", undo]);
    assert_eq!(exit_code, 0, "{output}");
    assert!(output["undone_stack_parents"].is_string(), "{output}");
  }
  assert_eq!(repo.head(), head);
  let (_, status) = run_in(&repo, &["status", "--branch", "IJAI-4047"]);
  assert_eq!(status["branches"][0].get("parent"), None);
}
