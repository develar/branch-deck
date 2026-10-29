use bd_cli::fixture_test::assert_covered;
use bd_cli::fixture_test::{
  PREFIX, assert_dry_run, assert_error, assert_refused_during, assert_undo_restores, environment, file_commits, git, hold_the_lock, repo_on_baseline, repo_with_commits, run_in,
  run_json, start_cherry_pick, write,
};
use git_ops::revs::short_id;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_utils::git_test_utils::TestRepo;
use test_utils::history::HistoryCommit;

/// `(auth) Add a`, `(ui) Add b`, a fixup of each, and an orphan fixup; returns the repository and the ids, oldest first
fn repo_with_fixups() -> (TestRepo, Vec<String>) {
  let (repo, ids) = repo_on_baseline(
    &[("README.md", "readme\n")],
    &file_commits(&[
      ("(auth) Add a", "a.txt", "a\n"),
      ("(ui) Add b", "b.txt", "b\n"),
      ("fixup! (auth) Add a", "a.txt", "a\nfixed\n"),
      ("fixup! (ui) Add b", "b.txt", "b\nfixed\n"),
      ("fixup! Nothing like it", "c.txt", "c\n"),
    ]),
    ".",
  );
  (repo, ids)
}

fn subjects(repo: &TestRepo) -> String {
  git(repo, &["log", "--format=%s", "origin/master..HEAD"])
}

#[test_log::test]
fn fold_keeps_the_tree_and_the_undo_restores_the_head() {
  let (repo, ids) = repo_with_fixups();
  let head = repo.head();
  let tree = git(&repo, &["rev-parse", "HEAD^{tree}"]);
  write(&repo, "a.txt", "dirty\n");

  let (exit_code, output) = run_in(&repo, &["fold", "--expect-head", short_id(&head)]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(subjects(&repo), "fixup! Nothing like it\n(ui) Add b\n(auth) Add a");
  assert_eq!(git(&repo, &["rev-parse", "HEAD^{tree}"]), tree);
  assert_eq!(std::fs::read_to_string(repo.path().join("a.txt")).unwrap(), "dirty\n");
  assert_eq!(output["previous_head"], json!(short_id(&head)));
  let folded = output["folded"].as_array().unwrap();
  assert_eq!(folded.len(), 2, "{output}");
  assert_eq!((&folded[0]["commit"], &folded[0]["fixups"]), (&json!(short_id(&ids[0])), &json!([short_id(&ids[2])])));
  assert_eq!(output["orphans"], json!([short_id(&ids[4])]));

  assert_undo_restores(&repo, &output, &head);
}

#[test_log::test]
fn fold_of_one_branch_and_a_dry_run() {
  let (repo, ids) = repo_with_fixups();
  let head = repo.head();

  let (exit_code, output) = run_in(&repo, &["fold", "--branch", "ui", "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_dry_run(&output, &head);
  assert_eq!(
    output["folded"],
    json!([{ "commit": short_id(&ids[1]), "subject": "(ui) Add b", "fixups": [short_id(&ids[3])] }])
  );
  assert_eq!(repo.head(), head);

  let (exit_code, output) = run_in(&repo, &["fold", "--branch", "ui"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(subjects(&repo), "fixup! Nothing like it\nfixup! (auth) Add a\n(ui) Add b\n(auth) Add a");

  // a branch that doesn't exist is no "nothing to fold"
  let (exit_code, output) = run_in(&repo, &["fold", "--branch", "uii"]);
  assert_eq!((exit_code, &output["error"]["code"]), (2, &json!("unknown_branch")), "{output}");

  // nothing left to fold in ui: no rewrite, no token
  let (exit_code, output) = run_in(&repo, &["fold", "--branch", "ui"]);
  assert_eq!((exit_code, &output["folded"], output.get("undo_token")), (0, &json!([]), None), "{output}");
}

#[test_log::test]
fn fold_is_refused_on_another_head_or_while_the_lock_is_held() {
  let (repo, ids) = repo_with_fixups();
  let head = repo.head();
  let (exit_code, output) = run_in(&repo, &["fold", "--expect-head", short_id(&ids[1])]);
  assert_eq!((exit_code, &output["error"]["code"]), (3, &json!("head_moved")), "{output}");

  let held = hold_the_lock(&repo);
  let (exit_code, output) = run_in(&repo, &["fold"]);
  assert_eq!((exit_code, &output["error"]["code"]), (3, &json!("locked")), "{output}");
  drop(held);
  assert_eq!(repo.head(), head);

  let (_, schema) = run_json(&environment(repo.path()), &["schema", "--all"]);
  let (_, output) = run_in(&repo, &["fold"]);
  assert_covered(&schema["outputs"]["fold"], &schema["outputs"]["fold"], &output, "fold");
}

#[test_log::test]
fn fold_and_its_dry_run_are_refused_during_a_cherry_pick() {
  let (repo, _) = repo_with_fixups();
  start_cherry_pick(&repo, repo.path());
  assert_refused_during(&repo, "a cherry-pick", |extra| run_in(&repo, &[&["fold"][..], extra].concat()));
}

#[test_log::test]
fn a_cherry_pick_in_another_worktree_does_not_refuse_the_fold() {
  let (repo, _) = repo_with_fixups();
  let linked = tempfile::tempdir().unwrap();
  let linked_path = linked.path().join("wt");
  git(&repo, &["worktree", "add", "-q", "-b", "other", linked_path.to_str().unwrap()]);
  start_cherry_pick(&repo, &linked_path);
  // the state files are per worktree: the linked one is in the middle of a cherry-pick, this one is not
  let (exit_code, output) = run_json(&environment(&linked_path), &["--prefix", PREFIX, "fold", "--dry-run"]);
  assert_error(&output, exit_code, (4, "operation_in_progress"));

  let (exit_code, output) = run_in(&repo, &["fold"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(subjects(&repo), "fixup! Nothing like it\n(ui) Add b\n(auth) Add a");
}

/// A fixup that changes the line a commit between it and its target changed too folds nothing: `squash_conflict` with
/// the commits it names, never a `git_error`
#[test_log::test]
fn a_fold_whose_fixup_conflicts_is_refused_as_a_squash_conflict() {
  let (repo, [target, _, fixup]) = repo_with_commits(
    &[("README.md", "readme\n")],
    &[
      HistoryCommit::new("(auth) Add a", &[("a.txt", "1\n")]).at(1_700_000_000),
      HistoryCommit::new("(ui) Change a", &[("a.txt", "2\n")]).at(1_700_000_100),
      HistoryCommit::new("fixup! (auth) Add a", &[("a.txt", "3\n")]).at(1_700_000_200),
    ],
  );
  let head = repo.head();

  let (exit_code, output) = run_in(&repo, &["fold"]);
  assert_error(&output, exit_code, (4, "squash_conflict"));
  assert_eq!(output["error"]["commits"], json!([fixup, target]), "{output}");
  assert_eq!(output["error"]["paths"], json!(["a.txt"]), "{output}");
  assert_eq!(repo.head(), head);
}

/// Another process that moves HEAD after the plan of `bd fold`, before its core reads the commits ahead of the baseline
/// (the second listing of the remotes), or between two reads of the core (its range and its history): `bd fold` folds
/// the fixups of HEAD as its core finds it, keeps the commit of the other process and prints what the core folded,
/// from that HEAD, whose undo goes back to it; HEAD without the fixups (another process folded them) is nothing to
/// fold. With `--expect-head` the moved HEAD is the answer (`head_moved`)
#[cfg(unix)]
#[test_log::test]
fn a_head_moved_after_the_plan_of_the_fold_is_folded_as_its_core_finds_it() {
  use test_utils::wrapped_git::WrappedGit;
  let commit = "echo g1 > g1.txt && git add -- g1.txt && git commit -q -m '(gamma) G1'";
  let after_the_plan = WrappedGit::move_head_before("remote", 2, commit);
  let between_the_reads = WrappedGit::move_head_before("log --first-parent", 1, commit);
  for (arm, under_the_lock) in [(&after_the_plan, false), (&between_the_reads, true)] {
    let (repo, ids) = repo_with_fixups();
    let wrapped = bd_cli::fixture_test::LockSpanGit::new(&repo, arm);
    let (exit_code, output) = wrapped.run(&["fold"]);
    assert_eq!(exit_code, 0, "{output}");
    assert!(wrapped.dir().join("moved").exists(), "HEAD moved");
    assert_eq!(subjects(&repo), "(gamma) G1\nfixup! Nothing like it\n(ui) Add b\n(auth) Add a");
    let moved = git(&repo, &["rev-parse", "HEAD@{1}"]);
    assert_eq!(git(&repo, &["log", "-1", "--format=%s", &moved]), "(gamma) G1");
    assert_eq!(output["previous_head"], json!(short_id(&moved)));
    let new_id = |rev: &str| json!(short_id(&git(&repo, &["rev-parse", rev])));
    assert_eq!(
      output["folded"],
      json!([
        { "commit": short_id(&ids[0]), "subject": "(auth) Add a", "new_id": new_id("HEAD~3"), "fixups": [short_id(&ids[2])] },
        { "commit": short_id(&ids[1]), "subject": "(ui) Add b", "new_id": new_id("HEAD~2"), "fixups": [short_id(&ids[3])] },
      ])
    );
    assert_eq!(output["orphans"], json!([short_id(&ids[4])]));
    let read_under_the_lock = wrapped.under_lock().iter().any(|args| bd_cli::fixture_test::subcommand(args) == "log");
    assert_eq!(read_under_the_lock, under_the_lock, "the range read once more under the lock: {:#?}", wrapped.commands());
    assert_undo_restores(&repo, &output, &moved);
  }

  let (repo, _) = repo_with_fixups();
  let wrapped = bd_cli::fixture_test::LockSpanGit::new(&repo, &WrappedGit::move_head_before("remote", 2, "git reset -q --hard HEAD~3"));
  let (exit_code, output) = wrapped.run(&["fold"]);
  assert!(wrapped.dir().join("moved").exists(), "HEAD moved");
  let head = short_id(&repo.head()).to_string();
  let printed = (&output["previous_head"], &output["new_head"], &output["folded"], output.get("undo_token"));
  assert_eq!((exit_code, printed), (0, (&json!(head), &json!(head), &json!([]), None)), "{output}");

  let (repo, _) = repo_with_fixups();
  let head = repo.head();
  let wrapped = bd_cli::fixture_test::LockSpanGit::new(&repo, &after_the_plan);
  let (exit_code, output) = wrapped.run(&["fold", "--expect-head", short_id(&head)]);
  assert_error(&output, exit_code, (3, "head_moved"));
  assert_eq!(git(&repo, &["log", "-1", "--format=%s"]), "(gamma) G1");
  assert_eq!(git(&repo, &["rev-parse", "HEAD~1"]), head, "nothing folded");
}
