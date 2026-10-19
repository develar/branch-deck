//! The resolve by words on real merges of git: what qualifies, the result, and the tree, checked against the tree git
//! writes for the same files.

use crate::merge_tree_output::{MergeTreeOutput, parse_merge_tree_output};
use crate::word_resolve::{NotResolved, WordResolveOutcome, resolve_by_words};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, git_command};

/// The merge of theirs onto ours with the base, as `merge-tree --write-tree -z` gives it; it must conflict
fn merge_tree(repo: &TestRepo, base: &str, ours: &str, theirs: &str) -> MergeTreeOutput {
  let output = git_command()
    .args(["-C", repo.path_str(), "merge-tree", "--write-tree", "-z", "--merge-base", base, ours, theirs])
    .output()
    .unwrap();
  assert_eq!(output.status.code(), Some(1), "the merge must conflict");
  parse_merge_tree_output(&output.stdout).unwrap()
}

/// The resolve of `merge`; a resolved one must cost one check-attr and one cat-file for all the files
fn resolve_merge(repo: &TestRepo, merge: &MergeTreeOutput) -> WordResolveOutcome {
  let git = repo.git_executor().with_process_stats("resolve");
  let outcome = resolve_by_words(&git, repo.path_str(), merge).unwrap();
  if matches!(outcome, WordResolveOutcome::Resolved(_)) {
    assert_eq!(git.process_stats().unwrap().counts(), vec![("cat-file".to_string(), 1), ("check-attr".to_string(), 1)]);
  }
  outcome
}

/// A base commit with `base`, ours (the target) with `ours` and theirs (the commit picked) with `theirs`, each a list
/// of (path, content); the tree of their merge and its resolve
fn resolve(repo: &TestRepo, base: &[(&str, &str)], ours: &[(&str, &str)], theirs: &[(&str, &str)]) -> (String, WordResolveOutcome) {
  let base_commit = repo.create_commit_with_files("base", base);
  let ours_commit = repo.create_commit_with_files("ours", ours);
  repo.reset_hard(&base_commit).unwrap();
  let theirs_commit = repo.create_commit_with_files("theirs", theirs);
  let merge = merge_tree(repo, &base_commit, &ours_commit, &theirs_commit);
  let outcome = resolve_merge(repo, &merge);
  (merge.tree, outcome)
}

/// Commits of the working tree after `edit` on the base, for ours and for theirs: (base, ours, theirs)
fn commits_by_edits(repo: &TestRepo, base: &[(&str, &str)], ours: impl Fn(&TestRepo), theirs: impl Fn(&TestRepo)) -> (String, String, String) {
  let base_commit = repo.create_commit_with_files("base", base);
  ours(repo);
  repo.git(&["commit", "-qam", "ours"]);
  let ours_commit = repo.head();
  repo.reset_hard(&base_commit).unwrap();
  theirs(repo);
  repo.git(&["commit", "-qam", "theirs"]);
  (base_commit, ours_commit, repo.head())
}

/// The tree git writes for `tree` with each (mode, path, content) in place
fn tree_from_git(repo: &TestRepo, tree: &str, files: &[(&str, &str, &[u8])]) -> String {
  let index = tempfile::NamedTempFile::new().unwrap();
  let git = |args: &[&str], input: Option<&[u8]>| -> String {
    let mut command = git_command();
    command.args(["-C", repo.path_str()]).args(args).env("GIT_INDEX_FILE", index.path());
    let output = match input {
      Some(input) => {
        use std::io::Write;
        let mut child = command.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        child.wait_with_output().unwrap()
      }
      None => command.output().unwrap(),
    };
    assert!(output.status.success(), "{args:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
  };
  git(&["read-tree", tree], None);
  for (mode, path, content) in files {
    let blob = git(&["hash-object", "-w", "--stdin"], Some(content));
    git(&["update-index", "--cacheinfo", &format!("{mode},{blob},{path}")], None);
  }
  git(&["write-tree"], None)
}

fn resolved(outcome: WordResolveOutcome) -> crate::word_resolve::WordResolution {
  match outcome {
    WordResolveOutcome::Resolved(resolution) => resolution,
    WordResolveOutcome::NotResolved { path, reason } => panic!("{path}: {reason:?}"),
  }
}

fn reason(outcome: &WordResolveOutcome) -> Option<NotResolved> {
  match outcome {
    WordResolveOutcome::Resolved(_) => None,
    WordResolveOutcome::NotResolved { reason, .. } => Some(*reason),
  }
}

#[test]
fn a_conflict_that_its_words_resolve_gives_the_tree_git_would_write() {
  let repo = TestRepo::new();
  let (tree, outcome) = resolve(
    &repo,
    &[("src/deep/Call.kt", "call(a, b, c)\nend\n"), ("README.md", "readme\n")],
    &[("src/deep/Call.kt", "call(A, b, c)\nend\n")],
    &[("src/deep/Call.kt", "call(a, b, C)\nend\n")],
  );
  let resolution = resolved(outcome);
  assert_eq!(resolution.files.len(), 1);
  let file = &resolution.files[0];
  assert_eq!((file.path.as_str(), file.mode.as_str(), file.conflicts), ("src/deep/Call.kt", "100644", 1));
  assert_eq!(String::from_utf8(file.content.clone()).unwrap(), "call(A, b, C)\nend\n");
  assert_eq!(resolution.conflicts(), 1);
  assert_eq!(resolution.tree, tree_from_git(&repo, &tree, &[("100644", "src/deep/Call.kt", &file.content)]));
}

#[test]
fn two_files_in_two_directories_resolve_together() {
  let repo = TestRepo::new();
  let (tree, outcome) = resolve(
    &repo,
    &[("a/one.txt", "x y z\n"), ("b/c/two.txt", "p q r\n"), ("a/kept.txt", "kept\n")],
    &[("a/one.txt", "X y z\n"), ("b/c/two.txt", "P q r\n")],
    &[("a/one.txt", "x y Z\n"), ("b/c/two.txt", "p q R\n")],
  );
  let resolution = resolved(outcome);
  let contents: Vec<(&str, String)> = resolution
    .files
    .iter()
    .map(|file| (file.path.as_str(), String::from_utf8(file.content.clone()).unwrap()))
    .collect();
  assert_eq!(contents, vec![("a/one.txt", "X y Z\n".to_string()), ("b/c/two.txt", "P q R\n".to_string())]);
  let expected = tree_from_git(&repo, &tree, &[("100644", "a/one.txt", b"X y Z\n"), ("100644", "b/c/two.txt", b"P q R\n")]);
  assert_eq!(resolution.tree, expected);
}

#[test]
fn crlf_lines_keep_their_ends() {
  let repo = TestRepo::new();
  let (tree, outcome) = resolve(&repo, &[("win.txt", "a b c\r\nx\r\n")], &[("win.txt", "A b c\r\nx\r\n")], &[("win.txt", "a b C\r\nx\r\n")]);
  let resolution = resolved(outcome);
  assert_eq!(resolution.files[0].content, b"A b C\r\nx\r\n");
  assert_eq!(resolution.tree, tree_from_git(&repo, &tree, &[("100644", "win.txt", b"A b C\r\nx\r\n")]));
}

#[test]
fn a_file_with_a_conflict_that_its_words_leave_keeps_the_commit_a_conflict() {
  let repo = TestRepo::new();
  let (_, outcome) = resolve(
    &repo,
    &[("one.txt", "a b c\n"), ("two.txt", "same words\n")],
    &[("one.txt", "A b c\n"), ("two.txt", "ours words\n")],
    &[("one.txt", "a b C\n"), ("two.txt", "theirs words\n")],
  );
  assert_eq!(reason(&outcome), Some(NotResolved::Unresolved));
}

#[test]
fn a_modify_delete_beside_a_resolvable_file_keeps_the_commit_a_conflict() {
  let repo = TestRepo::new();
  let (base, ours, theirs) = commits_by_edits(
    &repo,
    &[("one.txt", "a b c\n"), ("gone.txt", "old\n")],
    |repo| {
      std::fs::write(repo.path().join("one.txt"), "A b c\n").unwrap();
      repo.git(&["rm", "-q", "gone.txt"]);
    },
    |repo| {
      std::fs::write(repo.path().join("one.txt"), "a b C\n").unwrap();
      std::fs::write(repo.path().join("gone.txt"), "changed\n").unwrap();
    },
  );
  let outcome = resolve_merge(&repo, &merge_tree(&repo, &base, &ours, &theirs));
  assert_eq!(
    outcome,
    WordResolveOutcome::NotResolved {
      path: "gone.txt".to_string(),
      reason: NotResolved::NotContent
    }
  );
}

#[test]
fn an_add_add_and_a_binary_file_stay_conflicts() {
  let repo = TestRepo::new();
  let (_, outcome) = resolve(&repo, &[("kept.txt", "kept\n")], &[("new.txt", "a b c\n")], &[("new.txt", "a b C\n")]);
  assert_eq!(reason(&outcome), Some(NotResolved::NotContent));

  let repo = TestRepo::new();
  let (_, outcome) = resolve(&repo, &[("data.bin", "a\0b c\n")], &[("data.bin", "A\0b c\n")], &[("data.bin", "a\0b C\n")]);
  assert_eq!(reason(&outcome), Some(NotResolved::NotContent));
}

#[test]
fn a_merge_driver_keeps_the_conflict_of_git() {
  let repo = TestRepo::new();
  std::fs::write(repo.path().join(".gitattributes"), "*.txt merge=custom\n").unwrap();
  let (_, outcome) = resolve(&repo, &[("one.txt", "a b c\n")], &[("one.txt", "A b c\n")], &[("one.txt", "a b C\n")]);
  assert_eq!(reason(&outcome), Some(NotResolved::MergeAttribute));
}

#[test]
fn text_that_is_not_utf8_stays_a_conflict() {
  let repo = TestRepo::new();
  // `é` in Latin-1, which UTF-8 refuses
  let (base, ours, theirs) = commits_by_edits(
    &repo,
    &[("latin.txt", "a b c\n")],
    |repo| std::fs::write(repo.path().join("latin.txt"), b"\xe9 b c\n").unwrap(),
    |repo| std::fs::write(repo.path().join("latin.txt"), b"a b \xe9\n").unwrap(),
  );
  assert_eq!(reason(&resolve_merge(&repo, &merge_tree(&repo, &base, &ours, &theirs))), Some(NotResolved::NotText));
}

#[test]
fn the_mode_that_one_side_changes_is_kept() {
  let repo = TestRepo::new();
  let (base, ours, theirs) = commits_by_edits(
    &repo,
    &[("run.sh", "echo a b c\n")],
    |repo| {
      std::fs::write(repo.path().join("run.sh"), "echo A b c\n").unwrap();
      // `commit -a` takes the mode of the file on disk
      repo.git(&["update-index", "--chmod=+x", "run.sh"]);
      #[cfg(unix)]
      {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(repo.path().join("run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
      }
    },
    |repo| std::fs::write(repo.path().join("run.sh"), "echo a b C\n").unwrap(),
  );
  let merge = merge_tree(&repo, &base, &ours, &theirs);
  let resolution = resolved(resolve_merge(&repo, &merge));
  assert_eq!(resolution.files[0].mode, "100755");
  assert_eq!(resolution.tree, tree_from_git(&repo, &merge.tree, &[("100755", "run.sh", b"echo A b C\n")]));
}
