use bd_cli::fixture_test::{PREFIX, commit_all, commit_files, git, repo_with_baseline, run_in, write};
use bd_cli::fixture_test::{assert_covered, assert_dry_run, assert_error, assert_refused_during, assert_undo_restores, environment, hold_the_lock, run_json, start_cherry_pick};
use git_ops::revs::short_id;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use test_utils::git_test_utils::TestRepo;

/// The case of the report that asked for `bd squash`: `(lenses) Add lenses` and two commits that only change what it
/// added, a commit of `ui` between them and one after them, and a branch `docs` after them; returns the repository and
/// the ids, oldest first
fn repo_with_lenses() -> (TestRepo, Vec<String>) {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let ids = commit_files(
    &repo,
    &[
      ("(lenses) Add lenses\n\nThe lenses of the duplicates", "lenses.txt", "lenses\n"),
      ("(ui) Add b", "b.txt", "b\n"),
      ("(lenses) Rename the lens", "lenses.txt", "lens\n"),
      ("(lenses) Fix the lens", "lenses.txt", "lens fixed\n"),
      ("(ui) Add c", "c.txt", "c\n"),
      ("(docs) Add d", "d.txt", "d\n"),
    ],
  );
  (repo, ids)
}

fn subjects(repo: &TestRepo) -> String {
  git(repo, &["log", "--format=%s", "origin/master..HEAD"])
}

/// The tips of the virtual branches after `bd sync`, by name
fn synced_tips(repo: &TestRepo) -> Value {
  let (exit_code, output) = run_in(repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  let virtual_refs = format!("refs/heads/{PREFIX}/virtual/");
  let tips = git(repo, &["for-each-ref", "--format=%(refname) %(objectname)", &virtual_refs]);
  let tips: serde_json::Map<String, Value> = tips
    .lines()
    .filter_map(|line| line.split_once(' '))
    .filter_map(|(name, tip)| Some((name.strip_prefix(&virtual_refs)?.to_string(), json!(tip))))
    .collect();
  assert!(!tips.is_empty(), "no virtual branches");
  Value::Object(tips)
}

#[test_log::test]
fn squash_folds_the_commits_into_the_oldest_keeps_the_message_and_the_copies_after_them() {
  let (repo, ids) = repo_with_lenses();
  let before = synced_tips(&repo);
  let head = repo.head();
  let tree = git(&repo, &["rev-parse", "HEAD^{tree}"]);
  write(&repo, "b.txt", "dirty\n");

  let (exit_code, output) = run_in(
    &repo,
    &["squash", short_id(&ids[0]), short_id(&ids[3]), short_id(&ids[2]), "--expect-head", short_id(&head)],
  );
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(subjects(&repo), "(docs) Add d\n(ui) Add c\n(ui) Add b\n(lenses) Add lenses");
  assert_eq!(git(&repo, &["log", "-1", "--format=%B", "HEAD~3"]), "(lenses) Add lenses\n\nThe lenses of the duplicates");
  assert_eq!(git(&repo, &["show", "HEAD~3:lenses.txt"]), "lens fixed");
  assert_eq!(git(&repo, &["rev-parse", "HEAD^{tree}"]), tree);
  assert_eq!(std::fs::read_to_string(repo.path().join("b.txt")).unwrap(), "dirty\n");
  assert_eq!(
    (&output["into"], &output["squashed"], &output["subject"], &output["branch"]),
    (
      &json!(short_id(&ids[0])),
      &json!([short_id(&ids[2]), short_id(&ids[3])]),
      &json!("(lenses) Add lenses"),
      &json!("lenses")
    ),
  );
  // ui has a commit between the squashed ones: its files change, so its branch is built anew
  assert_eq!(output["rebuilt_branches"], json!(["lenses", "ui"]));

  // docs has only commits after the squashed ones: they keep their mapping notes, so its copy stays as it was; lenses
  // and ui are built anew (ui with the same files)
  let after = synced_tips(&repo);
  assert_eq!(after["docs"], before["docs"]);
  assert_ne!(after["lenses"], before["lenses"]);
  assert_ne!(after["ui"], before["ui"]);
  let tree_of = |tips: &Value| git(&repo, &["rev-parse", &format!("{}^{{tree}}", tips["ui"].as_str().unwrap())]);
  assert_eq!(tree_of(&after), tree_of(&before));

  assert_undo_restores(&repo, &output, &head);
}

#[test_log::test]
fn squash_dry_run_builds_and_drops_the_squash_and_a_message_is_the_users() {
  let (repo, ids) = repo_with_lenses();
  let head = repo.head();
  let refs = git(&repo, &["for-each-ref"]);

  let (exit_code, output) = run_in(&repo, &["squash", short_id(&ids[0]), short_id(&ids[2]), "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_dry_run(&output, &head);
  assert_eq!((output.get("new_id"), &output["squashed"]), (None, &json!([short_id(&ids[2])])), "{output}");
  let (_, schema) = run_json(&environment(repo.path()), &["schema", "--all"]);
  assert_covered(&schema["outputs"]["squash"], &schema["outputs"]["squash"], &output, "squash");
  assert_eq!((repo.head(), git(&repo, &["for-each-ref"])), (head, refs));

  let (exit_code, output) = run_in(&repo, &["squash", short_id(&ids[0]), short_id(&ids[2]), "--subject", "(lenses) Add the lens"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(git(&repo, &["log", "-1", "--format=%B", "HEAD~4"]), "(lenses) Add the lens\n\nThe lenses of the duplicates");
  let (exit_code, output) = run_in(&repo, &["squash", short_id(&ids[0]), "--message", "x"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
}

#[test_log::test]
fn squash_refuses_what_is_no_squash_of_one_branch() {
  let (repo, ids) = repo_with_lenses();
  let head = repo.head();
  let base = git(&repo, &["rev-parse", "origin/master"]);
  for (args, expected) in [
    // into is not the oldest
    (vec![short_id(&ids[2]), short_id(&ids[0])], (2, "invalid_arguments")),
    // a commit of another branch
    (vec![short_id(&ids[0]), short_id(&ids[1])], (2, "invalid_arguments")),
    // a commit of the baseline
    (vec![short_id(&base), short_id(&ids[2])], (2, "not_ahead_of_baseline")),
    // a message that moves the commit to another branch
    (vec![short_id(&ids[0]), short_id(&ids[2]), "--subject", "(ui) Add lenses"], (2, "invalid_arguments")),
  ] {
    let (exit_code, output) = run_in(&repo, &[&["squash"][..], &args].concat());
    assert_error(&output, exit_code, expected);
  }
  assert_eq!(repo.head(), head);
}

#[test_log::test]
fn squash_is_refused_by_a_conflict_or_an_empty_result_another_head_or_the_lock() {
  let (repo, ids) = repo_with_lenses();
  write(&repo, "lenses.txt", "lens again\n");
  let conflicting = commit_all(&repo, "(ui) Touch the lens", 1_700_001_000);
  write(&repo, "lenses.txt", "lens last\n");
  let last = commit_all(&repo, "(lenses) Last lens", 1_700_002_000);
  let head = repo.head();

  // the commit between them changes the line the squashed commit changes
  let (exit_code, output) = run_in(&repo, &["squash", short_id(&ids[0]), short_id(&last), "--dry-run"]);
  assert_error(&output, exit_code, (4, "squash_conflict"));
  assert!(!output["error"]["commits"].as_array().unwrap().is_empty(), "{output}");
  let (exit_code, output) = run_in(&repo, &["squash", short_id(&ids[0]), short_id(&last)]);
  assert_error(&output, exit_code, (4, "squash_conflict"));
  let _ = conflicting;

  let (exit_code, output) = run_in(&repo, &["squash", short_id(&ids[0]), short_id(&ids[2]), "--expect-head", short_id(&ids[4])]);
  assert_error(&output, exit_code, (3, "head_moved"));
  let held = hold_the_lock(&repo);
  let (exit_code, output) = run_in(&repo, &["squash", short_id(&ids[0]), short_id(&ids[2])]);
  assert_error(&output, exit_code, (3, "locked"));
  drop(held);
  assert_eq!(repo.head(), head);
}

#[test_log::test]
fn squash_refuses_a_result_without_changes() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  write(&repo, "x.txt", "x\n");
  let added = commit_all(&repo, "(x) Add x", 1_700_000_000);
  std::fs::remove_file(repo.path().join("x.txt")).unwrap();
  let removed = commit_all(&repo, "(x) Remove x", 1_700_000_100);

  let (exit_code, output) = run_in(&repo, &["squash", short_id(&added), short_id(&removed)]);
  assert_error(&output, exit_code, (4, "squash_empty"));
}

#[test_log::test]
fn squash_and_its_dry_run_are_refused_during_a_cherry_pick() {
  let (repo, ids) = repo_with_lenses();
  start_cherry_pick(&repo, repo.path());
  assert_refused_during(&repo, "a cherry-pick", |extra| {
    run_in(&repo, &[&["squash", short_id(&ids[0]), short_id(&ids[2])][..], extra].concat())
  });
}
