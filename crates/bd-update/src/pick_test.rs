use bd_cli::fixture_test::assert_covered;
#[cfg(unix)]
use bd_cli::fixture_test::{LockSpanGit, beside_a_held_sync, hold_the_lock, subcommand};
use bd_cli::fixture_test::{assert_error, environment, git, repo_with_baseline, repo_with_commits, run_in, run_json};
use bd_cli::remote_fixture_test::{AUTH, push_from_elsewhere_to, repo_with_remote, their_fix_on_auth};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use test_utils::git_test_utils::{TestRepo, TestWorktree};
use test_utils::history::HistoryCommit;

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
    (&undo["new_head"], &undo["previous_head"], &undo["follow_undone"], &undo["already_undone"]),
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
  assert_eq!(
    again["already_present"],
    json!([{ "commit": lane_head, "subject": "(lane) Add b", "on": repo.head(), "by": "patch_id" }])
  );
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
    next.contains(&format!("rebase --onto {own}")) && next.contains(&format!("pick --from {} --follow", lane.path)) && next.contains("Never `git cherry-pick`"),
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
  let (repo, _) = repo_with_commits(&[("a.txt", "a\n")], &[HistoryCommit::new("(own) Other", &[("own.txt", "own\n")])]);
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
  let (_, schema) = run_json(&environment(repo.path()), &["schema", "--all"]);
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

  // the branch has the commit by its patch: it is present, left out before the replay
  git(&repo, &["cherry-pick", "-x", &lane.git(&["rev-parse", "HEAD"])]);
  let (_, present) = run_in(&repo, &["pick", "--from", &lane.path, "--dry-run"]);
  assert_eq!(
    (&present["state"], &present["already_present"][0]["by"]),
    (&json!("up_to_date"), &json!("patch_id")),
    "{present}"
  );
  assert_covered(pick_schema, pick_schema, &present, "pick of a present commit");
}

/// `output` matches the `pick` output of `bd schema`
fn assert_pick_schema(repo: &TestRepo, output: &Value, what: &str) {
  let (_, schema) = run_json(&environment(repo.path()), &["schema", "--all"]);
  let pick_schema = &schema["outputs"]["pick"];
  assert_covered(pick_schema, pick_schema, output, what);
}

/// Run `bd group` with `plan` written to a file
fn group(repo: &TestRepo, plan: &Value) -> (i32, Value) {
  let file = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(file.path(), plan.to_string()).unwrap();
  run_in(repo, &["group", "--plan", file.path().to_str().unwrap()])
}

/// The parent of `name` in `bd status`, and the branch it is built on
fn parent_in_status(repo: &TestRepo, name: &str) -> (Value, Value) {
  let (exit_code, status) = run_in(repo, &["status"]);
  assert_eq!(exit_code, 0, "{status}");
  let branch = status["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!(name)).unwrap();
  (branch["parent"].clone(), branch["effective_base"].clone())
}

#[test_log::test]
fn a_parent_leaf_prefix_on_a_root_group_stacks_the_group_and_says_so() {
  let repo = repo_with_baseline(&[("a.txt", "a\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  repo.create_commit("(root) Add root", "root.txt", "root\n");
  repo.create_commit("(leaf) Add leaf", "leaf.txt", "leaf\n");
  assert_eq!(parent_in_status(&repo, "leaf").0, Value::Null);
  let lane = TestWorktree::detached(&repo);
  lane.commit("(root/leaf) Extend leaf", "leaf.txt", "leaf\nmore\n");

  let (exit_code, dry_run) = run_in(&repo, &["pick", "--from", &lane.path, "--dry-run"]);
  assert_eq!(exit_code, 0, "{dry_run}");
  assert_eq!(dry_run["reparented"], json!({ "leaf": "root" }));
  let (exit_code, output) = run_in(&repo, &["pick", "--from", &lane.path]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["picked"][0]["group"], &output["reparented"]), (&json!("leaf"), &json!({ "leaf": "root" })));
  assert!(output.get("dependencies").is_none(), "{output}");
  assert_pick_schema(&repo, &output, "pick that reparents");

  // the bare `(leaf)` declares no parent: the whole group is a leaf of root now, and the stack builds
  let (exit_code, sync) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{sync}");
  assert_eq!(parent_in_status(&repo, "leaf"), (json!("root"), json!("root")));
  // the same pick again lands nothing and reparents nothing
  let (exit_code, again) = run_in(&repo, &["pick", "--from", &lane.path, "--dry-run"]);
  assert_eq!((exit_code, &again["state"]), (0, &json!("up_to_date")), "{again}");
  assert!(again.get("reparented").is_none(), "{again}");
}

/// Stacks on: `base`, and `hang` and `teamcity` on it; `hang` adds `monitor.rs`
fn stack_of_hang_and_teamcity() -> TestRepo {
  let repo = repo_with_baseline(&[("a.txt", "a\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  repo.create_commit("(base) Add base", "base.txt", "base\n");
  repo.create_commit("(base/hang) Add the monitor", "monitor.rs", "monitor\n");
  repo.create_commit("(base/teamcity) Add the runner", "runner.rs", "runner\n");
  repo
}

#[test_log::test]
fn a_picked_commit_that_needs_another_branch_names_it_with_the_plan_that_moves_it() {
  let repo = stack_of_hang_and_teamcity();
  let lane = TestWorktree::detached(&repo);
  // the prefix names teamcity, the hunk changes the file hang created
  let watch = lane.commit("(base/teamcity) Watch the monitor", "monitor.rs", "monitor\nwatch\n");
  let expected = json!([{
    "commit": watch,
    "subject": "(base/teamcity) Watch the monitor",
    "prefix_branch": "teamcity",
    "depends_on": ["hang"],
    "plan": { "branches": { "hang": [watch] }, "unassign": [] },
  }]);

  let (exit_code, dry_run) = run_in(&repo, &["--full-ids", "pick", "--from", &lane.path, "--dry-run"]);
  assert_eq!(exit_code, 0, "{dry_run}");
  assert_eq!(dry_run["dependencies"], expected);
  let (exit_code, output) = run_in(&repo, &["--full-ids", "pick", "--from", &lane.path]);
  assert_eq!((exit_code, &output["state"]), (0, &json!("picked")), "{output}");
  assert_eq!(output["dependencies"], expected);
  assert_pick_schema(&repo, &output, "pick with a dependency and its plan");
  // as it is, teamcity does not sync
  let (exit_code, sync) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 1, "{sync}");

  let (exit_code, grouped) = group(&repo, &output["dependencies"][0]["plan"]);
  assert_eq!(exit_code, 0, "{grouped}");
  let (exit_code, sync) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{sync}");
  assert_eq!(git(&repo, &["log", "-1", "--format=%s"]), "(hang) Watch the monitor");
}

/// How the picked commits land is a report: its `log` of the commits ahead and the diffs of its dependency check run
/// after the move, outside the lock, so an agent's `bd pick` never makes the app's Update Project or a regroup wait for
/// them. It still names the dependency and the groups, from the head the check froze
#[cfg(unix)]
#[test_log::test]
fn the_landing_of_a_pick_is_read_after_the_lock() {
  let repo = stack_of_hang_and_teamcity();
  let lane = TestWorktree::detached(&repo);
  let watch = lane.commit("(base/teamcity) Watch the monitor", "monitor.rs", "monitor\nwatch\n");
  let wrapped = LockSpanGit::new(&repo, "");

  let (exit_code, output) = wrapped.run(&["--full-ids", "pick", "--from", &lane.path]);
  assert_eq!((exit_code, &output["state"]), (0, &json!("picked")), "{output}");
  assert_eq!(subjects(&output, "picked"), ["(base/teamcity) Watch the monitor"]);
  assert_eq!(
    (&output["dependencies"][0]["commit"], &output["dependencies"][0]["depends_on"]),
    (&json!(watch), &json!(["hang"])),
    "{output}"
  );

  // the commits ahead (`log --reverse --no-merges`) and their diffs (`log --no-walk=unsorted --stdin --raw`)
  let landing_reads = [" log --reverse --no-merges ", " log --no-walk=unsorted --stdin "];
  let commands = wrapped.commands();
  let moved = commands.iter().rposition(|(locked, _)| *locked).expect("the move runs under the lock");
  for read in landing_reads {
    let at = commands
      .iter()
      .position(|(_, args)| args.contains(read))
      .unwrap_or_else(|| panic!("no{read}: {commands:#?}"));
    assert!(at > moved, "{read} after the lock: {commands:#?}");
  }
}

/// A worktree with two commits that change `a.txt` and add `b.txt`, onto a target that moved on: the pick replays
#[cfg(unix)]
fn lane_onto_a_moved_target(repo: &TestRepo) -> TestWorktree {
  let lane = TestWorktree::detached(repo);
  lane.commit("(lane) Change a", "a.txt", "lane\n");
  lane.commit("(lane) Add b", "b.txt", "b\n");
  repo.create_commit("(own) Other", "own.txt", "own\n");
  lane
}

/// The subcommands of `commands` with how often each ran, sorted
#[cfg(unix)]
fn counted(commands: &[String]) -> Vec<(String, usize)> {
  let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
  for args in commands {
    *counts.entry(subcommand(args).to_string()).or_default() += 1;
  }
  counts.into_iter().collect()
}

/// `bd pick --from <lane> --follow` with its replays held, one per round, until `during` returned for that round;
/// `during` gets the number of the round
#[cfg(unix)]
fn pick_held_in_its_replays(repo: &TestRepo, lane: &TestWorktree, rounds: usize, mut during: impl FnMut(usize)) -> (LockSpanGit, i32, Value) {
  let wrapped = LockSpanGit::new(
    repo,
    "*\" replay \"*) if [ -f '{dir}/hold' ]; then rm '{dir}/hold'; touch '{dir}/held'; while [ ! -f '{dir}/go' ]; do sleep 0.01; done; rm '{dir}/go'; fi;;",
  );
  let (hold, held, go) = (wrapped.dir().join("hold"), wrapped.dir().join("held"), wrapped.dir().join("go"));
  std::fs::write(&hold, "").unwrap();
  let (exit_code, output) = std::thread::scope(|scope| {
    let pick = scope.spawn(|| wrapped.run(&["--full-ids", "pick", "--from", &lane.path, "--follow"]));
    for round in 0..rounds {
      while !held.exists() && !pick.is_finished() {
        std::thread::yield_now();
      }
      assert!(held.exists(), "the pick reached its replay of round {round}");
      std::fs::remove_file(&held).unwrap();
      during(round);
      if round + 1 < rounds {
        std::fs::write(&hold, "").unwrap();
      }
      std::fs::write(&go, "").unwrap();
    }
    pick.join().unwrap()
  });
  (wrapped, exit_code, output)
}

/// The check of a pick (the range, the replay, the dry runs of both worktrees on index copies) runs before the lock:
/// under it only the witness of both worktrees, the local changes on the paths the moves write, and the two moves with
/// their marks
#[cfg(unix)]
#[test_log::test]
fn the_check_of_a_pick_runs_before_the_lock() {
  let repo = repo_with_baseline(&[("a.txt", "a\n")]);
  let lane = lane_onto_a_moved_target(&repo);
  let wrapped = LockSpanGit::new(&repo, "");
  let (exit_code, output) = wrapped.run(&["pick", "--from", &lane.path, "--follow"]);
  assert_eq!(
    (exit_code, &output["state"], &output["followed"]["state"]),
    (0, &json!("picked"), &json!("moved")),
    "{output}"
  );

  wrapped.assert_before_the_lock("replay");
  let commands = wrapped.commands();
  let dry_runs = commands.iter().filter(|(locked, args)| !locked && args.contains(" read-tree -m -u -n ")).count();
  assert_eq!(dry_runs, 2, "both dry runs before the lock: {commands:#?}");
  // the marks of interrupted moves, the witness (the target's guards from the directories bd read, HEAD and upstream;
  // the source's guards and HEAD),
  // the local changes on `a.txt`, the one path a move changes that the old tree has (its index entry, its content), and
  // each move: the check of the symref, the transaction with its mark, the files, the mark deleted
  assert_eq!(
    counted(&wrapped.under_lock()),
    [
      ("cat-file".to_string(), 1),
      ("hash-object".to_string(), 1),
      ("read-tree".to_string(), 2),
      ("rev-parse".to_string(), 8),
      ("symbolic-ref".to_string(), 6),
      ("update-index".to_string(), 4),
      ("update-ref".to_string(), 4),
    ],
    "{commands:#?}"
  );
}

/// While a pick checks, the repository lock is free: another writer (the app's Safe Push, a regroup) takes it at once
#[cfg(unix)]
#[test_log::test]
fn another_writer_takes_the_lock_while_a_pick_replays() {
  let repo = repo_with_baseline(&[("a.txt", "a\n")]);
  let lane = lane_onto_a_moved_target(&repo);
  let (_wrapped, exit_code, output) = pick_held_in_its_replays(&repo, &lane, 1, |_| drop(hold_the_lock(&repo)));
  assert_eq!((exit_code, &output["state"]), (0, &json!("picked")), "{output}");
  assert_eq!(read(&repo, "b.txt"), "b\n");
}

/// A pick beside another agent's `bd sync` of the target held in its merges: a sync takes no repository lock, so the
/// pick checks, moves both worktrees and ends while that sync is still held, which then ends too
#[cfg(unix)]
#[test_log::test]
fn a_pick_beside_a_held_sync_does_not_wait_for_it() {
  let (repo, _) = repo_with_commits(
    &[("a.txt", "a\n")],
    &[
      // a commit of no branch below the target's own: its sync merges `own` onto the baseline
      HistoryCommit::new("Tidy up", &[("misc.txt", "misc\n")]),
    ],
  );
  let lane = lane_onto_a_moved_target(&repo);
  beside_a_held_sync(&repo, || {
    let (exit_code, output) = run_in(&repo, &["pick", "--from", &lane.path, "--follow"]);
    assert_eq!((exit_code, &output["state"]), (0, &json!("picked")), "the pick does not wait for the sync: {output}");
  });
  assert_eq!(read(&repo, "b.txt"), "b\n");
}

/// A writer that moves the target while the pick replays (another session's pick, a regroup) is kept: under the lock the
/// witness finds the branch moved, the pick checks again without the lock and lands on the new tip, the worktree too
#[cfg(unix)]
#[test_log::test]
fn a_commit_on_the_target_during_the_replay_of_a_pick_is_kept_and_the_pick_checks_again() {
  let repo = repo_with_baseline(&[("a.txt", "a\n")]);
  let lane = lane_onto_a_moved_target(&repo);
  let (wrapped, exit_code, output) = pick_held_in_its_replays(&repo, &lane, 1, |_| {
    repo.create_commit("(own) Meanwhile", "meanwhile.txt", "meanwhile\n");
  });
  assert_eq!(
    (exit_code, &output["state"], &output["followed"]["state"]),
    (0, &json!("picked"), &json!("moved")),
    "{output}"
  );
  assert_eq!(wrapped.locked_runs("replay"), [false, false], "the check again, without the lock");
  assert_eq!(
    git(&repo, &["log", "--format=%s", "origin/master..HEAD"]),
    "(lane) Add b\n(lane) Change a\n(own) Meanwhile\n(own) Other"
  );
  assert_eq!(lane.git(&["rev-parse", "HEAD"]), repo.head());
  assert_eq!(output["previous_head"], json!(git(&repo, &["rev-parse", "HEAD~2"])));
}

/// A target moved during every check without the lock: the retries are bounded, the last check runs under the lock,
/// where nobody else moves the branch, and the pick lands
#[cfg(unix)]
#[test_log::test]
fn a_target_that_moves_during_every_check_is_checked_under_the_lock_at_last() {
  let repo = repo_with_baseline(&[("a.txt", "a\n")]);
  let lane = lane_onto_a_moved_target(&repo);
  let (wrapped, exit_code, output) = pick_held_in_its_replays(&repo, &lane, 2, |round| {
    repo.create_commit(&format!("(own) Meanwhile {round}"), &format!("meanwhile{round}.txt"), "meanwhile\n");
  });
  assert_eq!((exit_code, &output["state"]), (0, &json!("picked")), "{output}");
  assert_eq!(wrapped.locked_runs("replay"), [false, false, true], "two checks without the lock, then one under it");
  assert_eq!(
    git(&repo, &["log", "--format=%s", "-4"]),
    "(lane) Add b\n(lane) Change a\n(own) Meanwhile 1\n(own) Meanwhile 0"
  );
}

/// A file the pick writes, edited while the pick replays: the local changes are read again under the lock, so the pick
/// is refused with the path, as if the edit had come before it, and nothing moves
#[cfg(unix)]
#[test_log::test]
fn a_file_edited_during_the_replay_of_a_pick_blocks_it() {
  let repo = repo_with_baseline(&[("a.txt", "a\n")]);
  let lane = lane_onto_a_moved_target(&repo);
  let (head, lane_head) = (repo.head(), lane.git(&["rev-parse", "HEAD"]));
  let (wrapped, exit_code, output) = pick_held_in_its_replays(&repo, &lane, 1, |_| std::fs::write(repo.path().join("a.txt"), "draft\n").unwrap());
  assert_error(&output, exit_code, (4, "uncommitted_overlap"));
  assert_eq!(output["error"]["paths"], json!(["a.txt"]));
  assert_eq!(wrapped.locked_runs("replay"), [false], "no check again");
  assert_eq!((repo.head(), lane.git(&["rev-parse", "HEAD"])), (head, lane_head));
  assert_eq!(read(&repo, "a.txt"), "draft\n");
}

#[test_log::test]
fn a_picked_commit_that_needs_its_own_and_another_branch_offers_the_restack() {
  let repo = repo_with_baseline(&[("a.txt", "a\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  repo.create_commit("(server) Add the server", "server.rs", "server\n");
  repo.create_commit("(client) Add the client", "client.rs", "client\n");
  let lane = TestWorktree::detached(&repo);
  lane.write("server.rs", "server\nport\n");
  lane.git(&["add", "--", "server.rs"]);
  lane.commit("(client) Connect to the server port", "client.rs", "client\nport\n");

  let (exit_code, output) = run_in(&repo, &["pick", "--from", &lane.path]);
  assert_eq!(exit_code, 0, "{output}");
  let dependency = &output["dependencies"][0];
  assert_eq!(
    (&dependency["prefix_branch"], &dependency["depends_on"], &dependency["restack"]),
    (&json!("client"), &json!(["server"]), &json!("bd restack client server"))
  );
  assert!(dependency.get("plan").is_none(), "{output}");
  assert_pick_schema(&repo, &output, "pick with a dependency and its restack");
  let (exit_code, restacked) = run_in(&repo, &["restack", "client", "server"]);
  assert_eq!(exit_code, 0, "{restacked}");
  let (exit_code, sync) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{sync}");
}

/// Someone else pushed to the remote branch of auth: `bd pick --remote auth` fetches it and picks their commit onto the
/// checked out branch with the `(auth)` prefix, so it lands in auth; the push then drops nothing, and a pick again finds
/// nothing to pick
#[test_log::test]
fn a_remote_pick_takes_in_the_commit_someone_pushed_and_the_push_then_drops_nothing() {
  let (repo, _remote, theirs) = their_fix_on_auth();

  let (exit_code, output) = run_in(&repo, &["--full-ids", "pick", "--remote", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["state"], json!("picked"));
  let picked = &output["picked"][0];
  assert_eq!(
    (&picked["commit"], &picked["subject"], &picked["group"], &picked["grouped_by"]),
    (&json!(theirs), &json!("(auth) Their fix"), &json!("auth"), &json!("prefix"))
  );
  assert_eq!(git(&repo, &["log", "-1", "--format=%s|%ae"]), "(auth) Their fix|other@example.com");

  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "a push without --force: {output}");
  let (exit_code, output) = run_in(&repo, &["pick", "--remote", "auth"]);
  assert_error(&output, exit_code, (4, "nothing_to_pick"));
}

/// A remote pick and its undo (the core of the app's Pick Remote Commits and of its undo too) fetch, read and check
/// before the repository lock: under it only the witness of the pick (`recheck_pick`) or of the undo (`for-each-ref` of
/// HEAD and the mark of pending files), and the move with the files (C11 of docs/design/rust-domain-authority.md)
#[cfg(unix)]
#[test_log::test]
fn a_remote_pick_and_its_undo_lock_only_the_witness_and_the_move() {
  let (repo, _remote, _) = their_fix_on_auth();
  let wrapped = LockSpanGit::new(&repo, "");
  let locked = |wrapped: &LockSpanGit| {
    let mut under: Vec<String> = wrapped.under_lock().iter().map(|args| subcommand(args).to_string()).collect();
    under.sort_unstable();
    under.dedup();
    under
  };

  let (exit_code, output) = wrapped.run(&["--full-ids", "pick", "--remote", "auth"]);
  assert_eq!((exit_code, &output["state"]), (0, &json!("picked")), "{output}");
  wrapped.assert_before_the_lock("fetch");
  wrapped.assert_before_the_lock("replay");
  assert_eq!(
    locked(&wrapped),
    ["read-tree", "rev-parse", "symbolic-ref", "update-index", "update-ref"],
    "{:#?}",
    wrapped.commands()
  );

  let token = output["undo_token"].as_str().expect("an undo token").to_string();
  let (exit_code, output) = wrapped.run(&["undo", &token]);
  assert_eq!(exit_code, 0, "{output}");
  wrapped.assert_before_the_lock("log");
  assert_eq!(
    locked(&wrapped),
    ["for-each-ref", "read-tree", "symbolic-ref", "update-index", "update-ref"],
    "{:#?}",
    wrapped.commands()
  );
}

/// The remote moved again since the head the user saw: `--expect-remote` picks nothing
#[test_log::test]
fn a_remote_pick_from_another_head_than_the_one_seen_picks_nothing() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let seen = push_from_elsewhere_to(&remote, AUTH, "Their fix");
  push_from_elsewhere_to(&remote, AUTH, "Their second fix");
  let head = repo.head();

  let (exit_code, output) = run_in(&repo, &["pick", "--remote", "auth", "--expect-remote", &seen]);
  assert_error(&output, exit_code, (4, "lease_rejected"));
  assert_eq!(repo.head(), head, "nothing was picked");
}
