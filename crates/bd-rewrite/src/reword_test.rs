use bd_cli::fixture_test::assert_covered;
use bd_cli::fixture_test::{
  PREFIX, assert_dry_run, assert_error, assert_refused_during, assert_undo_restores, commit_all, commit_subjects, environment, git, hold_the_lock, repo_with_baseline, run_in,
  run_json, start_cherry_pick, write,
};
use git_ops::revs::short_id;
use serde_json::json;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use test_utils::git_test_utils::TestRepo;

/// Three commits of `feat` ahead of the baseline, each with its own file `f<index>.txt`; the middle one has a body.
/// Returns the repository and the ids, oldest first
fn repo_with_three() -> (TestRepo, Vec<String>) {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let ids = commit_subjects(
    &repo,
    &[
      "(feat) Add the first part",
      "(feat) Add the second part with a subject that is far longer than the rule of the repository allows\n\nThe body.",
      "(feat) Add the third part",
    ],
  );
  (repo, ids)
}

fn subjects(repo: &TestRepo) -> String {
  git(repo, &["log", "--format=%s", "origin/master..HEAD"])
}

/// The trees of the commits ahead of the baseline, newest first
fn trees(repo: &TestRepo) -> String {
  git(repo, &["log", "--format=%T", "origin/master..HEAD"])
}

fn mapping_note(repo: &TestRepo, commit: &str) -> String {
  git(repo, &["notes", "show", commit])
}

#[test_log::test]
fn reword_subject_changes_only_that_commit() {
  let (repo, ids) = repo_with_three();
  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  let note_of_the_third = mapping_note(&repo, &ids[2]);
  let head = repo.head();
  let trees_before = trees(&repo);
  write(&repo, "f0.txt", "dirty\n");
  write(&repo, "staged.txt", "staged\n");
  git(&repo, &["add", "staged.txt"]);
  let status_before = git(&repo, &["status", "--porcelain=v1"]);

  let (exit_code, output) = run_in(
    &repo,
    &["reword", short_id(&ids[1]), "--subject", "(feat) Add the second part", "--expect-head", short_id(&head)],
  );
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(subjects(&repo), "(feat) Add the third part\n(feat) Add the second part\n(feat) Add the first part");
  assert_eq!(git(&repo, &["log", "-1", "--format=%B", "HEAD~1"]), "(feat) Add the second part\n\nThe body.");
  assert_eq!(trees(&repo), trees_before, "every tree stays");
  assert_eq!(git(&repo, &["rev-parse", "HEAD~2"]), ids[0], "the older commit stays");
  assert_ne!(repo.head(), head, "the later commit is recreated");
  assert_eq!(mapping_note(&repo, "HEAD"), note_of_the_third, "the note of the later commit follows it");
  assert_eq!(git(&repo, &["status", "--porcelain=v1"]), status_before, "the index and the files stay");

  assert_eq!((&output["operation"], &output["rewritten"]), (&json!("reword"), &json!(1)));
  let new_middle = git(&repo, &["rev-parse", "HEAD~1"]);
  assert_eq!(
    output["changes"],
    json!([{
      "commit": short_id(&ids[1]),
      "new_id": short_id(&new_middle),
      "old_subject": "(feat) Add the second part with a subject that is far longer than the rule of the repository allows",
      "new_subject": "(feat) Add the second part",
    }])
  );
  let (_, schema) = run_json(&environment(repo.path()), &["schema", "--all"]);
  assert_covered(&schema["outputs"]["rewrite"], &schema["outputs"]["rewrite"], &output, "rewrite");
  assert_undo_restores(&repo, &output, &head);
  assert!(!mapping_note(&repo, &ids[1]).is_empty(), "the old commits keep their notes for the undo");
}

/// The environment variable that makes `bd_reword_in_a_child` run: `<repo>|<commit>`
const CHILD_ENV: &str = "BD_REWORD_CHILD";

#[test_log::test]
fn reword_with_a_message_file_from_stdin_replaces_the_whole_message() {
  let (repo, ids) = repo_with_three();
  // stdin only reaches a child process: this test binary running only `bd_reword_in_a_child`
  let mut child = Command::new(std::env::current_exe().unwrap())
    .args(["--exact", "reword_test::bd_reword_in_a_child", "--nocapture"])
    .env(CHILD_ENV, format!("{}|{}", repo.path().display(), ids[2]))
    .stdin(Stdio::piped())
    .spawn()
    .unwrap();
  child.stdin.take().unwrap().write_all(b"(feat) Add the third part\n\nWith a body from stdin.\n").unwrap();
  let status = child.wait().unwrap();
  assert!(status.success(), "{status}");
  assert_eq!(git(&repo, &["log", "-1", "--format=%B"]), "(feat) Add the third part\n\nWith a body from stdin.");
}

/// The child of `reword_with_a_message_file_from_stdin_replaces_the_whole_message`: `bd reword --message-file -` on the
/// stdin of this process; does nothing in a normal run
#[test_log::test]
fn bd_reword_in_a_child() {
  let Ok(spec) = std::env::var(CHILD_ENV) else { return };
  let (repo, commit) = spec.split_once('|').unwrap();
  let (exit_code, output) = run_json(&environment(Path::new(repo)), &["--prefix", PREFIX, "reword", commit, "--message-file", "-"]);
  assert_eq!((exit_code, &output["rewritten"]), (0, &json!(1)), "{output}");
}

#[test_log::test]
fn reword_is_refused_on_another_head_or_while_the_lock_is_held() {
  let (repo, ids) = repo_with_three();
  let head = repo.head();
  let (exit_code, output) = run_in(&repo, &["reword", &ids[1], "--subject", "(feat) Second", "--expect-head", short_id(&ids[1])]);
  assert_error(&output, exit_code, (3, "head_moved"));

  let held = hold_the_lock(&repo);
  let (exit_code, output) = run_in(&repo, &["reword", &ids[1], "--subject", "(feat) Second"]);
  assert_error(&output, exit_code, (3, "locked"));
  drop(held);
  assert_eq!(repo.head(), head, "nothing changed");
}

#[test_log::test]
fn reword_and_its_dry_run_are_refused_during_a_cherry_pick() {
  let (repo, ids) = repo_with_three();
  start_cherry_pick(&repo, repo.path());
  assert_refused_during(&repo, "a cherry-pick", |extra| {
    run_in(&repo, &[&["reword", ids[1].as_str(), "--subject", "(feat) Second"][..], extra].concat())
  });
}

#[test_log::test]
fn reword_of_a_commit_not_ahead_of_the_baseline_is_refused() {
  let (repo, ids) = repo_with_three();
  let baseline = git(&repo, &["rev-parse", "origin/master"]);
  for commit in [baseline.as_str(), "abcdef1234"] {
    let (exit_code, output) = run_in(&repo, &["reword", commit, "--subject", "Nothing"]);
    assert_error(&output, exit_code, (2, "not_ahead_of_baseline"));
  }
  // a revision is no commit id: it would name another commit after any rewrite
  let (exit_code, output) = run_in(&repo, &["reword", "HEAD~1", "--subject", "(feat) Second"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  // a commit takes exactly one of the texts
  let (exit_code, output) = run_in(&repo, &["reword", &ids[1], "--subject", "A", "--message", "B"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = run_in(&repo, &["reword", &ids[1]]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
}

#[test_log::test]
fn a_reword_plan_rewrites_two_commits_at_once_and_undo_restores_them() {
  let (repo, ids) = repo_with_three();
  run_in(&repo, &["sync"]);
  let notes: Vec<String> = ids.iter().map(|id| mapping_note(&repo, id)).collect();
  let head = repo.head();
  let reflog = git(&repo, &["reflog", "--format=%gs", "master"]).lines().count();
  let plan = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(
    plan.path(),
    json!({ "rewrites": [
      { "commit": short_id(&ids[0]), "subject": "(feat) First" },
      { "commit": short_id(&ids[1]), "message": "(feat) Second\n\nA new body." },
    ]})
    .to_string(),
  )
  .unwrap();

  let (exit_code, output) = run_in(&repo, &["reword", "--plan", plan.path().to_str().unwrap()]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(subjects(&repo), "(feat) Add the third part\n(feat) Second\n(feat) First");
  assert_eq!(output["rewritten"], json!(2));
  assert_eq!(output["changes"][1]["body_changed"], json!(true));
  assert_eq!(git(&repo, &["reflog", "--format=%gs", "master"]).lines().count(), reflog + 1, "one rewrite");

  assert_undo_restores(&repo, &output, &head);
  let restored: Vec<String> = ids.iter().map(|id| mapping_note(&repo, id)).collect();
  assert_eq!(restored, notes, "the old commits still have their notes");

  // an entry with both texts, or with neither, is no plan
  std::fs::write(plan.path(), json!({ "rewrites": [{ "commit": ids[0], "subject": "A", "message": "B" }] }).to_string()).unwrap();
  let (exit_code, output) = run_in(&repo, &["reword", "--plan", plan.path().to_str().unwrap()]);
  assert_error(&output, exit_code, (2, "invalid_plan"));
  std::fs::write(plan.path(), json!({ "rewrites": [] }).to_string()).unwrap();
  let (exit_code, output) = run_in(&repo, &["reword", "--plan", plan.path().to_str().unwrap()]);
  assert_error(&output, exit_code, (2, "invalid_plan"));
}

#[test_log::test]
fn a_new_prefix_moves_the_commit_to_another_branch() {
  let (repo, ids) = repo_with_three();
  let (exit_code, output) = run_in(&repo, &["reword", &ids[1], "--subject", "(other) Add the second part", "--sync"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["changes"][0]["moved"], json!({ "from": "feat", "to": "other" }));
  assert_eq!(output["groups"]["other"], json!([output["changes"][0]["new_id"]]));
  let built: Vec<&str> = output["sync"]["branches"]
    .as_array()
    .unwrap()
    .iter()
    .map(|branch| branch["name"].as_str().unwrap())
    .collect();
  // newest first by the oldest commit: other begins at the second commit, feat at the first
  assert_eq!(built, ["other", "feat"]);

  let (_, status) = run_in(&repo, &["status"]);
  let other = status["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!("other")).unwrap();
  assert_eq!(other["commits"].as_array().unwrap().len(), 1, "{status}");
}

#[test_log::test]
fn a_reword_that_strands_a_fixup_is_refused() {
  let (repo, ids) = repo_with_three();
  write(&repo, "f0.txt", "fixed\n");
  commit_all(&repo, "fixup! (feat) Add the first part", 1_700_000_300);
  let head = repo.head();
  let (exit_code, output) = run_in(&repo, &["reword", &ids[0], "--subject", "(feat) First"]);
  assert_error(&output, exit_code, (2, "invalid_plan"));
  assert!(output["error"]["message"].as_str().unwrap().contains("bd fold"), "{output}");
  assert_eq!(repo.head(), head);
}

#[test_log::test]
fn reword_dry_run_writes_no_ref() {
  let (repo, ids) = repo_with_three();
  let head = repo.head();
  let (exit_code, output) = run_in(&repo, &["reword", &ids[1], "--subject", "(feat) Second", "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_dry_run(&output, &head);
  assert_eq!(output["changes"][0]["new_subject"], json!("(feat) Second"));
  assert_eq!(output["changes"][0].get("new_id"), None);
  assert_eq!(repo.head(), head);
}

#[test_log::test]
fn a_message_equal_after_cleanup_rewrites_nothing() {
  let (repo, ids) = repo_with_three();
  let head = repo.head();
  let (exit_code, output) = run_in(&repo, &["reword", &ids[2], "--message", "(feat) Add the third part  \n\n"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["rewritten"], output.get("undo_token")), (&json!(0), None));
  assert_eq!(repo.head(), head);
}
