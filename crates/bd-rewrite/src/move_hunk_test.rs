use bd_cli::fixture_test::assert_covered;
use bd_cli::fixture_test::{
  ACP_AGENT, ACP_HOST, ACP_PROTOCOL, acp_host, assert_error, assert_undo_restores, commit_all, commit_dangling_reference, environment, git, hold_the_lock, numbered_lines,
  repo_with_baseline, run_in, run_json, sync_statuses, write,
};
use git_ops::revs::short_id;
use serde_json::json;
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
  assert_eq!(output["moved"]["blocks"].as_array().map(Vec::len), Some(2), "the import and the use: two blocks");
  assert_eq!(output["changed"], json!(2), "A and B; the docs commit keeps its diff");
  assert_eq!(output["commits"].as_array().unwrap().len(), 3);
  // the sync rebuilt `fake-agent` with the fix and kept `docs`, whose mapping note followed it
  let agent = virtual_branch(&repo, "fake-agent");
  assert_eq!(files_of(&repo, &agent), [ACP_PROTOCOL, ACP_AGENT, ACP_HOST].join("\n"));
  assert_eq!(git(&repo, &["show", &format!("{agent}:{ACP_HOST}")]), acp_host("AcpControlProtocol").trim_end());
  assert_eq!(files_of(&repo, &virtual_branch(&repo, "lane-document")), "flow.txt");
  assert_eq!(virtual_branch(&repo, "docs"), docs_branch, "the docs commit is reused");
  let statuses = sync_statuses(&output);
  assert!(statuses.contains(&(&json!("docs"), &json!("unchanged"))), "{statuses:?}");

  let (_, schema) = run_json(&environment(repo.path()), &["schema", "--all"]);
  assert_covered(&schema["outputs"]["move_hunk"], &schema["outputs"]["move_hunk"], &output, "move_hunk");

  assert_eq!(output["commits"][1]["commit"], json!(short_id(&docs)));
  assert_undo_restores(&repo, &output, &head);
}

/// The blocks of `path` in `commit`, as `bd commits <commit> --patch` prints them
/// `bd move-hunk` reads and computes before the repository lock (the commits ahead, the blocks of the file, the history,
/// the merges, the commits with their notes); under it only the witness of HEAD (`for-each-ref`) and the move of the
/// branch (C11 of docs/design/rust-domain-authority.md)
#[cfg(unix)]
#[test_log::test]
fn a_move_hunk_locks_only_the_witness_and_the_write() {
  let (repo, [a, _, b]) = incident();
  let wrapped = bd_cli::fixture_test::LockSpanGit::new(&repo, "");
  let (exit_code, output) = wrapped.run(&["move-hunk", &b, &a, ACP_HOST]);
  assert_eq!(exit_code, 0, "{output}");
  let mut under: Vec<String> = wrapped.under_lock().iter().map(|args| bd_cli::fixture_test::subcommand(args).to_string()).collect();
  under.sort_unstable();
  under.dedup();
  assert_eq!(under, ["for-each-ref", "update-ref"], "{:#?}", wrapped.commands());
  wrapped.assert_before_the_lock("merge-tree");
  wrapped.assert_before_the_lock("fast-import");
}

/// Another process that commits between the commits `bd move-hunk` lists and the history it reads (HEAD moved between
/// two reads before the lock) makes the move compute once more under the lock, on that HEAD: it moves the change and
/// keeps the commit of the other process; with `--expect-head` the moved HEAD is the answer (`head_moved`)
#[cfg(unix)]
#[test_log::test]
fn a_head_moved_between_two_reads_of_a_move_computes_it_once_more_under_the_lock() {
  let arm = test_utils::wrapped_git::WrappedGit::commit_once_before("log --first-parent --boundary", "g1.txt", "(gamma) G1");
  let (repo, [a, _, b]) = incident();
  let wrapped = bd_cli::fixture_test::LockSpanGit::new(&repo, &arm);
  let (exit_code, output) = wrapped.run(&["move-hunk", &b, &a, ACP_HOST]);
  assert_eq!(exit_code, 0, "{output}");
  assert!(wrapped.dir().join("committed").exists(), "HEAD moved");
  assert_eq!(git(&repo, &["log", "-1", "--format=%s"]), "(gamma) G1");
  assert_eq!(output["moved"]["to"], json!(short_id(&a)));
  let under = wrapped.under_lock();
  let under: Vec<&str> = under.iter().map(|args| bd_cli::fixture_test::subcommand(args)).collect();
  assert!(
    under.contains(&"log") && under.contains(&"update-ref"),
    "the history read once more under the lock: {:#?}",
    wrapped.commands()
  );

  let (repo, [a, _, b]) = incident();
  let head = repo.head();
  let wrapped = bd_cli::fixture_test::LockSpanGit::new(&repo, &arm);
  let (exit_code, output) = wrapped.run(&["move-hunk", &b, &a, ACP_HOST, "--expect-head", short_id(&head)]);
  assert_error(&output, exit_code, (3, "head_moved"));
  assert_eq!(git(&repo, &["log", "-1", "--format=%s"]), "(gamma) G1");
}

fn patch_blocks(repo: &TestRepo, commit: &str, path: &str) -> Vec<serde_json::Value> {
  let (exit_code, commits) = run_in(repo, &["commits", commit, "--patch"]);
  assert_eq!(exit_code, 0, "{commits}");
  let files = commits["commits"][0]["patch"].as_array().unwrap();
  let file = files.iter().find(|file| file["path"] == path).unwrap_or_else(|| panic!("no {path} in {commits}"));
  file["blocks"].as_array().unwrap().clone()
}

fn block_id(block: &serde_json::Value) -> &str {
  block["id"].as_str().unwrap()
}

#[test_log::test]
fn the_block_id_is_the_one_of_bd_commits_patch() {
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
  let blocks = patch_blocks(&repo, &b, "f.txt");
  assert_eq!(blocks.len(), 3, "{blocks:?}");

  let (exit_code, preview) = run_in(&repo, &["move-hunk", &b, &a, "f.txt", "--block", block_id(&blocks[1]), "--dry-run"]);
  assert_eq!(exit_code, 0, "{preview}");
  assert_eq!(preview["moved"]["blocks"], json!([blocks[1]]));
  assert_eq!(
    (&preview["dry_run"], &preview["new_head"], preview.get("undo_token")),
    (&json!(true), &json!(short_id(&b)), None)
  );
  assert_eq!(repo.head(), b);

  let (exit_code, output) = run_in(&repo, &["move-hunk", &b, &a, "f.txt", "--block", block_id(&blocks[1])]);
  assert_eq!(exit_code, 0, "{output}");
  let new_a = output["commits"][0]["new_id"].as_str().unwrap();
  assert_eq!(
    git(&repo, &["show", &format!("{new_a}:f.txt")]),
    numbered_lines(40).replace("line 20\n", "twenty\n").trim_end()
  );

  let (exit_code, output) = run_in(&repo, &["move-hunk", "HEAD", &a, "f.txt", "--block", block_id(&blocks[0])]);
  assert_error(&output, exit_code, (2, "not_ahead_of_baseline"));
  // the moved block is gone from the newest commit: the error lists the ids the file has now, which did not change
  let head = repo.head();
  let (exit_code, output) = run_in(&repo, &["move-hunk", &head, new_a, "f.txt", "--block", block_id(&blocks[1])]);
  assert_error(&output, exit_code, (2, "block_not_found"));
  assert_eq!(output["error"]["blocks"], json!([block_id(&blocks[0]), block_id(&blocks[2])]));
  let (exit_code, output) = run_in(&repo, &["move-hunk", &head, new_a, "f.txt", "--block", "2"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let added = patch_blocks(&repo, &head, "b.txt");
  let (exit_code, output) = run_in(&repo, &["move-hunk", &head, new_a, "b.txt", "--block", block_id(&added[0]), "--block", block_id(&added[0])]);
  assert_eq!(exit_code, 0, "the whole file, named by its only block: {output}");
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
  assert_error(&output, exit_code, (4, "hunk_conflict"));
  assert_eq!((&output["error"]["commits"], &output["error"]["paths"]), (&json!([short_id(&a)]), &json!(["f.txt"])));
  assert!(
    output["error"]["message"]
      .as_str()
      .unwrap()
      .ends_with("nothing was written. Move fewer blocks with --block, or leave the change where it is"),
    "{output}"
  );
  assert_eq!(repo.head(), b);

  let (exit_code, output) = run_in(&repo, &["move-hunk", &b, &a, "b.txt", "--expect-head", short_id(&a)]);
  assert_error(&output, exit_code, (3, "head_moved"));
  let held = hold_the_lock(&repo);
  let (exit_code, output) = run_in(&repo, &["move-hunk", &b, &a, "b.txt"]);
  assert_error(&output, exit_code, (3, "locked"));
  drop(held);
  assert_eq!(repo.head(), b);

  let (_, schema) = run_json(&environment(repo.path()), &["schema", "--all"]);
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
  let (exit_code, output) = run_in(&repo, &["move-hunk", &fixed_by, &removed_by, ACP_HOST, "--dry-run"]);

  assert_error(&output, exit_code, (2, "invalid_arguments"));
  assert!(output["error"]["message"].as_str().unwrap().contains("squash it instead"), "{output}");
  assert_eq!(repo.head(), fixed_by);
}
