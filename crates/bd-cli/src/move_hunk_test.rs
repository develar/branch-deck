use crate::fixture_test::{
  ACP_AGENT, ACP_HOST, ACP_PROTOCOL, acp_host, assert_error, assert_undo_restores, commit_all, commit_dangling_reference, environment, git, numbered_lines, repo_with_baseline,
  run_in, run_json, sync_statuses, write,
};
use crate::schema_test::assert_covered;
use git_ops::revs::short_id;
use serde_json::json;
use sync_core::repository_lock::lock_repository;
use test_utils::git_test_utils::TestRepo;

/// The incident of `commit_dangling_reference`, with an unrelated commit of `docs` between the two and a second file in
/// the fix, so that the fix's commit keeps a change: commit A of `fake-agent` moves `CONTROL_URL_ENV` into
/// AcpControlProtocol.java, commit B of `lane-document` changes its use in AirFlowLaneHost.kt, which belongs to A: built
/// alone, `fake-agent` doesn't compile. Returns the repository and the ids of A, the docs commit and B
fn incident() -> (TestRepo, [String; 3]) {
  let agent = |body: &str| format!("package acp;\n\npublic final class AcpScenarioAgent {{\n{body}}}\n");
  let repo = repo_with_baseline(&[
    (ACP_AGENT, &agent("  public static final String CONTROL_URL_ENV = \"AIR_CONTROL_URL\";\n")),
    (ACP_HOST, &acp_host("AcpScenarioAgent")),
  ]);
  write(&repo, ACP_AGENT, agent(""));
  write(
    &repo,
    ACP_PROTOCOL,
    "package acp;\n\npublic final class AcpControlProtocol {\n  public static final String CONTROL_URL_ENV = \"AIR_CONTROL_URL\";\n}\n",
  );
  let a = commit_all(&repo, "(fake-agent) IJAI-3880 Move the control constants", 1_700_000_100);
  write(&repo, "NOTES.md", "notes\n");
  let docs = commit_all(&repo, "(docs) Add notes", 1_700_000_200);
  write(&repo, ACP_HOST, acp_host("AcpControlProtocol"));
  write(&repo, "flow.txt", "flow\n");
  let b = commit_all(&repo, "(lane-document) IJAI-3878 Use the control protocol", 1_700_000_300);
  (repo, [a, docs, b])
}

fn virtual_branch(repo: &TestRepo, name: &str) -> String {
  git(repo, &["rev-parse", &format!("refs/heads/test-user/virtual/{name}")])
}

fn files_of(repo: &TestRepo, commit: &str) -> String {
  git(repo, &["diff-tree", "--no-commit-id", "--name-only", "-r", commit])
}

#[test_log::test]
fn the_fix_moves_into_the_commit_it_belongs_to_and_the_sync_rebuilds_the_branch() {
  let (repo, [a, docs, b]) = incident();
  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  let docs_branch = virtual_branch(&repo, "docs");
  let head = repo.head();
  let tree = git(&repo, &["rev-parse", "HEAD^{tree}"]);
  write(&repo, "flow.txt", "dirty\n");

  let (exit_code, output) = run_in(&repo, &["move-hunk", &b, &a, ACP_HOST, "--expect-head", short_id(&head), "--sync"]);

  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(git(&repo, &["rev-parse", "HEAD^{tree}"]), tree, "HEAD keeps its tree");
  assert_eq!(std::fs::read_to_string(repo.path().join("flow.txt")).unwrap(), "dirty\n");
  assert_eq!(
    (&output["previous_head"], &output["moved"]["path"], &output["moved"]["from"], &output["moved"]["to"]),
    (&json!(short_id(&head)), &json!(ACP_HOST), &json!(short_id(&b)), &json!(short_id(&a)))
  );
  assert_eq!(output["moved"]["hunks"].as_array().map(Vec::len), Some(1), "the import and the use: one hunk");
  assert_eq!(output["rewritten"], json!(2), "A and B; the docs commit keeps its diff");
  assert_eq!(output["commits"].as_array().unwrap().len(), 3);
  // the sync rebuilt `fake-agent` with the fix and kept `docs`, whose mapping note followed it
  let agent = virtual_branch(&repo, "fake-agent");
  assert_eq!(files_of(&repo, &agent), [ACP_PROTOCOL, ACP_AGENT, ACP_HOST].join("\n"));
  assert_eq!(git(&repo, &["show", &format!("{agent}:{ACP_HOST}")]), acp_host("AcpControlProtocol").trim_end());
  assert_eq!(files_of(&repo, &virtual_branch(&repo, "lane-document")), "flow.txt");
  assert_eq!(virtual_branch(&repo, "docs"), docs_branch, "the docs commit is reused");
  let statuses = sync_statuses(&output);
  assert!(statuses.contains(&(&json!("docs"), &json!("unchanged"))), "{statuses:?}");

  let (_, schema) = run_json(&environment(repo.path()), &["schema"]);
  assert_covered(&schema["outputs"]["move_hunk"], &schema["outputs"]["move_hunk"], &output, "move_hunk");

  assert_eq!(output["commits"][1]["from"], json!(short_id(&docs)));
  assert_undo_restores(&repo, &output, &head);
}

#[test_log::test]
fn the_hunk_number_is_the_one_of_bd_commits_patch() {
  let repo = repo_with_baseline(&[("f.txt", &numbered_lines(40))]);
  write(&repo, "a.txt", "a\n");
  let a = commit_all(&repo, "(a) Add a", 1_700_000_100);
  let changed = numbered_lines(40)
    .replace("line 2\n", "two\n")
    .replace("line 20\n", "twenty\n")
    .replace("line 38\n", "thirty-eight\n");
  write(&repo, "f.txt", &changed);
  write(&repo, "b.txt", "b\n");
  let b = commit_all(&repo, "(b) Add b", 1_700_000_200);

  let (exit_code, commits) = run_in(&repo, &["commits", &b, "--patch"]);
  assert_eq!(exit_code, 0, "{commits}");
  let patch = commits["commits"][0]["patch"].as_str().unwrap();
  let section = &patch[patch.find("diff --git a/f.txt").unwrap()..];
  let headers: Vec<&str> = section.lines().filter(|line| line.starts_with("@@ ")).collect();
  assert_eq!(headers.len(), 3, "{patch}");

  let (exit_code, preview) = run_in(&repo, &["move-hunk", &b, &a, "f.txt", "--hunk", "2", "--dry-run"]);
  assert_eq!(exit_code, 0, "{preview}");
  assert_eq!(
    preview["moved"]["hunks"],
    json!([{ "header": headers[1], "text": " line 17\n line 18\n line 19\n-line 20\n+twenty\n line 21\n line 22\n line 23\n" }])
  );
  assert_eq!(
    (&preview["dry_run"], &preview["new_head"], preview.get("undo_token")),
    (&json!(true), &json!(short_id(&b)), None)
  );
  assert_eq!(repo.head(), b);

  let (exit_code, output) = run_in(&repo, &["move-hunk", &b, &a, "f.txt", "--hunk", "2"]);
  assert_eq!(exit_code, 0, "{output}");
  let new_a = output["commits"][0]["to"].as_str().unwrap();
  assert_eq!(
    git(&repo, &["show", &format!("{new_a}:f.txt")]),
    numbered_lines(40).replace("line 20\n", "twenty\n").trim_end()
  );

  let (exit_code, output) = run_in(&repo, &["move-hunk", "HEAD", &a, "f.txt", "--hunk", "1"]);
  assert_error(&output, exit_code, (2, "not_ahead_of_baseline"));
  let head = repo.head();
  let (exit_code, output) = run_in(&repo, &["move-hunk", &head, new_a, "f.txt", "--hunk", "3"]);
  assert_error(&output, exit_code, (2, "hunk_not_found"));
  let (exit_code, output) = run_in(&repo, &["move-hunk", &head, new_a, "b.txt", "--hunk", "1", "--hunk", "1"]);
  assert_eq!(exit_code, 0, "the whole file, named by its only hunk: {output}");
  let head = repo.head();
  let (exit_code, output) = run_in(&repo, &["move-hunk", &head, &head, "f.txt"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
}

#[test_log::test]
fn a_commit_between_on_the_same_lines_conflicts_and_another_head_or_the_lock_refuse() {
  let repo = repo_with_baseline(&[("f.txt", &numbered_lines(20))]);
  write(&repo, "a.txt", "a\n");
  let a = commit_all(&repo, "(a) Add a", 1_700_000_100);
  write(&repo, "f.txt", numbered_lines(20).replace("line 10\n", "ten\n"));
  commit_all(&repo, "(x) Change line 10", 1_700_000_200);
  write(&repo, "f.txt", numbered_lines(20).replace("line 10\n", "TEN\n"));
  write(&repo, "b.txt", "b\n");
  let b = commit_all(&repo, "(b) Change line 10 again", 1_700_000_300);

  let (exit_code, output) = run_in(&repo, &["move-hunk", &b, &a, "f.txt"]);
  assert_error(&output, exit_code, (1, "hunk_conflict"));
  assert_eq!((&output["error"]["commits"], &output["error"]["paths"]), (&json!([short_id(&a)]), &json!(["f.txt"])));
  assert_eq!(repo.head(), b);

  let (exit_code, output) = run_in(&repo, &["move-hunk", &b, &a, "b.txt", "--expect-head", short_id(&a)]);
  assert_error(&output, exit_code, (3, "head_moved"));
  let held = lock_repository(repo.git_executor(), repo.path().to_str().unwrap(), std::time::Duration::from_millis(100)).unwrap();
  let (exit_code, output) = run_in(&repo, &["move-hunk", &b, &a, "b.txt"]);
  assert_error(&output, exit_code, (3, "locked"));
  drop(held);
  assert_eq!(repo.head(), b);

  let (_, schema) = run_json(&environment(repo.path()), &["schema"]);
  let (exit_code, output) = run_in(&repo, &["move-hunk", &b, &a, "b.txt", "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_covered(&schema["outputs"]["move_hunk"], &schema["outputs"]["move_hunk"], &output, "move_hunk dry run");
  assert_covered(
    &schema["outputs"]["error"],
    &schema["outputs"]["error"],
    &run_in(&repo, &["move-hunk", &b, &a, "f.txt"]).1,
    "error",
  );
}

#[test_log::test]
fn a_fix_that_is_the_whole_commit_is_refused_as_a_squash() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let (removed_by, fixed_by) = commit_dangling_reference(&repo);

  // the commit changes nothing else: moving its change would leave it empty
  let (exit_code, output) = run_in(&repo, &["move-hunk", &fixed_by, &removed_by, ACP_HOST, "--hunk", "1", "--dry-run"]);

  assert_error(&output, exit_code, (2, "invalid_arguments"));
  assert!(output["error"]["message"].as_str().unwrap().contains("squash it instead"), "{output}");
  assert_eq!(repo.head(), fixed_by);
}
