use crate::fixture_test::{assert_error, environment, git, repo_with_baseline, run_in, run_json};
use crate::schema_test::assert_covered;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use test_utils::git_test_utils::{TestRepo, TestWorktree};

fn read(repo: &TestRepo, file: &str) -> String {
  std::fs::read_to_string(repo.path().join(file)).unwrap()
}

fn subjects(output: &Value, field: &str) -> Vec<String> {
  output[field]
    .as_array()
    .unwrap()
    .iter()
    .map(|commit| commit["subject"].as_str().unwrap().to_string())
    .collect()
}

#[test_log::test]
fn a_pick_lands_the_worktree_commits_with_their_groups_and_the_undo_moves_both_back() {
  let repo = repo_with_baseline(&[("a.txt", "a\n")]);
  let lane = TestWorktree::detached(&repo);
  let lane_head = lane.commit("(lane) Add b", "b.txt", "b\n");
  lane.commit("ABC-1 Fix c", "c.txt", "c\n");
  lane.commit("setup vm: one more script", "d.txt", "d\n");
  let own = repo.create_commit("(own) Other", "own.txt", "own\n");

  let (exit_code, output) = run_in(&repo, &["--full-ids", "pick", "--from", &lane.path, "--follow"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["state"], &output["previous_head"], &output["dry_run"]),
    (&json!("picked"), &json!(own), &json!(false))
  );
  let groups: Vec<(&Value, &Value)> = output["picked"]
    .as_array()
    .unwrap()
    .iter()
    .map(|commit| (&commit["group"], &commit["grouped_by"]))
    .collect();
  assert_eq!(
    groups,
    [(&json!("lane"), &json!("prefix")), (&json!("ABC-1"), &json!("issue")), (&Value::Null, &json!("none"))]
  );
  let new_head = repo.head();
  assert_eq!(output["new_head"], json!(new_head));
  assert_eq!(
    (&output["followed"]["state"], &output["followed"]["previous_head"], &output["followed"]["worktree"]),
    (&json!("moved"), &json!(lane.git(&["rev-parse", "HEAD@{1}"])), &json!(lane.path))
  );
  assert_eq!(lane.git(&["rev-parse", "HEAD"]), new_head);
  let lane_previous = lane.git(&["rev-parse", "HEAD@{1}"]);
  assert_eq!(output["undo_token"], json!(format!("pick:{own}..{new_head}+follow:lane:{lane_previous}..{new_head}")));
  assert_eq!(read(&repo, "d.txt"), "d\n");
  assert_eq!(git(&repo, &["log", "-g", "-n1", "--format=%gs", "refs/heads/master"]), "branch-deck: pick");

  let token = output["undo_token"].as_str().unwrap();
  let (exit_code, undo) = run_in(&repo, &["--full-ids", "undo", token]);
  assert_eq!(exit_code, 0, "{undo}");
  assert_eq!(
    (&undo["head"], &undo["undone"], &undo["follow_undone"], &undo["already_undone"]),
    (&json!(own), &json!(new_head), &json!(true), &json!(false))
  );
  assert_eq!(repo.head(), own);
  assert_eq!(lane.git(&["rev-parse", "HEAD"]), lane_previous);
  assert!(!repo.path().join("b.txt").exists());
  // the commits are the worktree's again: the same pick lands them again
  let (exit_code, again) = run_in(&repo, &["undo", token]);
  assert_eq!((exit_code, &again["already_undone"]), (0, &json!(true)), "{again}");
  let (exit_code, dry_run) = run_in(&repo, &["pick", "--from", &lane.path, "--dry-run"]);
  assert_eq!((exit_code, subjects(&dry_run, "picked").len()), (0, 3), "{dry_run}");
  assert_eq!(lane.git(&["rev-list", "--count", &format!("{lane_head}^..HEAD")]), "3");
}

#[test_log::test]
fn a_dry_run_writes_no_ref_and_a_pick_again_finds_every_commit_present() {
  let repo = repo_with_baseline(&[("a.txt", "a\n")]);
  let lane = TestWorktree::detached(&repo);
  lane.commit("(lane) Add b", "b.txt", "b\n");
  let own = repo.create_commit("(own) Other", "own.txt", "own\n");
  let refs = git(&repo, &["for-each-ref"]);

  let (exit_code, dry_run) = run_in(&repo, &["pick", "--from", &lane.path, "--follow", "--dry-run"]);
  assert_eq!(exit_code, 0, "{dry_run}");
  assert_eq!(
    (&dry_run["state"], &dry_run["dry_run"], &dry_run["followed"]["state"]),
    (&json!("picked"), &json!(true), &json!("would_move"))
  );
  assert!(dry_run.get("undo_token").is_none(), "{dry_run}");
  assert_eq!((repo.head(), git(&repo, &["for-each-ref"])), (own, refs));

  // landed by hand with a cherry-pick: the commit is present, by the commit with its patch
  let lane_head = lane.git(&["rev-parse", "HEAD"]);
  git(&repo, &["cherry-pick", "-x", &lane_head]);
  let (exit_code, again) = run_in(&repo, &["--full-ids", "pick", "--from", &lane.path]);
  assert_eq!(exit_code, 0, "{again}");
  assert_eq!(again["state"], json!("up_to_date"));
  assert_eq!(again["already_present"], json!([{ "commit": lane_head, "subject": "(lane) Add b", "on": repo.head() }]));
  assert!(again.get("undo_token").is_none(), "{again}");
}

#[test_log::test]
fn a_conflict_changes_nothing_and_says_what_to_do() {
  let repo = repo_with_baseline(&[("a.txt", "a\n")]);
  let lane = TestWorktree::detached(&repo);
  let conflicting = lane.commit("(lane) Change a", "a.txt", "lane\n");
  let own = repo.create_commit("(own) Change a", "a.txt", "own\n");

  let (exit_code, output) = run_in(&repo, &["--full-ids", "pick", "--from", &lane.path, "--follow"]);
  assert_eq!(exit_code, 1, "{output}");
  assert_eq!(
    (&output["state"], &output["conflict"]["commit"], &output["conflict"]["files"]),
    (&json!("conflict"), &json!(conflicting), &json!(["a.txt"]))
  );
  let next = output["conflict"]["next"].as_str().unwrap();
  assert!(
    next.contains(&format!("rebase --onto {own}")) && next.contains(&format!("pick --from {} --follow", lane.path)),
    "{next}"
  );
  assert_eq!(repo.head(), own);
  assert_eq!(lane.git(&["rev-parse", "HEAD"]), conflicting);
}

#[test_log::test]
fn local_changes_in_the_way_and_a_wrong_source_are_refused() {
  let repo = repo_with_baseline(&[("a.txt", "a\n")]);
  let lane = TestWorktree::detached(&repo);
  lane.commit("(lane) Change a", "a.txt", "lane\n");
  repo.create_commit("(own) Other", "own.txt", "own\n");
  std::fs::write(repo.path().join("a.txt"), "draft\n").unwrap();

  let (exit_code, output) = run_in(&repo, &["pick", "--from", &lane.path]);
  assert_error(&output, exit_code, (4, "uncommitted_overlap"));
  assert_eq!(output["error"]["paths"], json!(["a.txt"]));

  let (exit_code, output) = run_in(&repo, &["pick", "--from", "."]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = run_in(&repo, &["pick", "HEAD", "--follow"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = run_in(&repo, &["pick"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
}

#[test_log::test]
fn expect_head_refuses_a_pick_onto_another_head() {
  let repo = repo_with_baseline(&[("a.txt", "a\n")]);
  let lane = TestWorktree::detached(&repo);
  lane.commit("(lane) Add b", "b.txt", "b\n");
  let read = repo.head();
  let moved = repo.create_commit("(own) Meanwhile", "own.txt", "own\n");

  let (exit_code, output) = run_in(&repo, &["pick", "--from", &lane.path, "--expect-head", &read[..12]]);
  assert_error(&output, exit_code, (3, "head_moved"));
  assert_eq!(repo.head(), moved);
  let (exit_code, output) = run_in(&repo, &["pick", "--from", &lane.path, "--expect-head", &moved[..12]]);
  assert_eq!((exit_code, &output["state"]), (0, &json!("picked")), "{output}");
}

#[test_log::test]
fn a_worktree_on_a_newer_baseline_is_refused() {
  let repo = repo_with_baseline(&[("a.txt", "a\n")]);
  repo.create_commit("(own) Other", "own.txt", "own\n");
  let lane = TestWorktree::detached(&repo);
  lane.git(&["checkout", "-q", "--detach", "origin/master"]);
  let upstream = lane.commit("Upstream", "up.txt", "up\n");
  git(&repo, &["update-ref", "refs/remotes/origin/master", &upstream]);
  lane.commit("(lane) Add b", "b.txt", "b\n");

  let (exit_code, output) = run_in(&repo, &["pick", "--from", &lane.path]);
  assert_error(&output, exit_code, (4, "source_on_newer_baseline"));
}

/// A peer's commits on top of HEAD join an open resolve session as more rounds: a pick is such a peer
#[test_log::test]
fn a_pick_during_an_open_update_session_is_carried_by_its_finish() {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("shared.txt", "one\ntwo\nthree\n")]);
  clone.create_commit("(auth) Change line 2", "shared.txt", "one\nmine\nthree\n");
  upstream.create_commit("Change line 2 upstream", "shared.txt", "one\ntheirs\nthree\n");
  let lane = TestWorktree::detached(&clone);
  lane.commit("(lane) Add b", "b.txt", "b\n");

  let (exit_code, round) = run_in(&clone, &["update"]);
  assert_eq!((exit_code, &round["state"]), (1, &json!("conflict")), "{round}");
  let (exit_code, picked) = run_in(&clone, &["pick", "--from", &lane.path, "--follow"]);
  assert_eq!((exit_code, &picked["state"]), (0, &json!("picked")), "{picked}");

  let edit = round["session"]["files"][0]["edit"].as_str().unwrap();
  std::fs::write(edit, "one\ntheirs and mine\nthree\n").unwrap();
  let (exit_code, done) = run_in(&clone, &["update", "--continue"]);
  assert_eq!(exit_code, 0, "{done}");
  assert_eq!(git(&clone, &["log", "--format=%s", "origin/master..HEAD"]), "(lane) Add b\n(auth) Change line 2");
  assert_eq!(read(&clone, "b.txt"), "b\n");
}

#[test_log::test]
fn pick_outputs_match_the_schema() {
  let repo = repo_with_baseline(&[("a.txt", "a\n")]);
  let (_, schema) = run_json(&environment(repo.path()), &["schema"]);
  let pick_schema = &schema["outputs"]["pick"];
  let lane = TestWorktree::detached(&repo);
  lane.commit("(lane) Change a", "a.txt", "lane\n");
  repo.create_commit("(own) Change a", "a.txt", "own\n");
  let (_, conflict) = run_in(&repo, &["pick", "--from", &lane.path, "--follow"]);
  assert_covered(pick_schema, pick_schema, &conflict, "pick conflict");

  git(&repo, &["reset", "-q", "--hard", "HEAD~1"]);
  repo.create_commit("(own) Other", "own.txt", "own\n");
  let (_, picked) = run_in(&repo, &["pick", "--from", &lane.path, "--follow"]);
  assert_eq!(picked["state"], json!("picked"), "{picked}");
  assert_covered(pick_schema, pick_schema, &picked, "pick");
  let (_, undo) = run_in(&repo, &["undo", picked["undo_token"].as_str().unwrap()]);
  assert_covered(&schema["outputs"]["undo"], &schema["outputs"]["undo"], &undo, "undo of a pick");
}
