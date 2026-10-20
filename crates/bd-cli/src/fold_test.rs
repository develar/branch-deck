use crate::fixture_test::{
  PREFIX, assert_dry_run, assert_error, assert_refused_during, assert_undo_restores, commit_all, environment, git, hold_the_lock, repo_with_baseline, run_in, run_json,
  start_cherry_pick, write,
};
use crate::schema_test::assert_covered;
use git_ops::revs::short_id;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_utils::git_test_utils::TestRepo;

/// `(auth) Add a`, `(ui) Add b`, a fixup of each, and an orphan fixup; returns the repository and the ids, oldest first
fn repo_with_fixups() -> (TestRepo, Vec<String>) {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let mut ids = Vec::new();
  for (index, (subject, file, content)) in [
    ("(auth) Add a", "a.txt", "a\n"),
    ("(ui) Add b", "b.txt", "b\n"),
    ("fixup! (auth) Add a", "a.txt", "a\nfixed\n"),
    ("fixup! (ui) Add b", "b.txt", "b\nfixed\n"),
    ("fixup! Nothing like it", "c.txt", "c\n"),
  ]
  .into_iter()
  .enumerate()
  {
    write(&repo, file, content);
    ids.push(commit_all(&repo, subject, 1_700_000_000 + i64::try_from(index).unwrap() * 100));
  }
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

  let (_, schema) = run_json(&environment(repo.path()), &["schema"]);
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
