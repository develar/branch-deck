//! The resolve by words on real merges of git: what qualifies, the result, and the tree, checked against the tree git
//! writes for the same files.

use crate::merge_tree_output::{MergeTreeOutput, parse_merge_tree_output};
use crate::word_resolve::{NotResolved, WordResolveOutcome, resolve_by_words};
use line_diff::model::DiffBudget;
use line_diff::policy::WhitespacePolicy;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, git_command};
use text_diff::compose::compose_merge;
use text_diff::merge_model::merge_texts;

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

/// A conflict that only a resolve ignoring whitespace, the line ends too, would take stays the conflict of git: such a
/// resolve joins lines into one that no side has (`b\nb` and `a b\n\n` of `b` into `ab\nb`, `a b` and `b\nx y z\n}` of
/// `b\n` into `a bx y z\n}`), or merges a final line end removed beside a removed last line; the resolve by words takes
/// the default policy alone (`line_diff::merge_resolve_util::try_resolve_by_default`)
#[test]
fn a_conflict_that_only_ignoring_whitespace_resolves_stays_the_conflict_of_git() {
  for (base, ours, theirs) in [("b", "b\nb", "a b\n\n"), ("b\n", "a b", "b\nx y z\n}"), ("}\n", "}", "")] {
    let repo = TestRepo::new();
    let (_, outcome) = resolve(&repo, &[("f.txt", base), ("kept.txt", "kept\n")], &[("f.txt", ours)], &[("f.txt", theirs)]);
    assert_eq!(reason(&outcome), Some(NotResolved::Unresolved), "{base:?}, ours {ours:?}, theirs {theirs:?}");
  }
}

/// Two insertions at the end of a file, which git conflicts on, stay the conflict of git: the merge pairs the empty line
/// after the final line end of the base only with its like, so the blank line of one side and the line of the other are
/// one conflict, never two insertions on two sides of that line (`line_diff::last_line`)
#[test]
fn two_insertions_at_the_end_of_a_file_stay_the_conflict_of_git() {
  for (base, ours, theirs) in [
    ("x\n", "x\n\n", "x\ny\n"),
    ("x\n", "x\ny\n", "x\n\n"),
    ("x\r\n", "x\r\n\r\n", "x\r\ny\r\n"),
    ("", "\n", "x\n"),
  ] {
    let repo = TestRepo::new();
    let (_, outcome) = resolve(&repo, &[("end.txt", base), ("kept.txt", "kept\n")], &[("end.txt", ours)], &[("end.txt", theirs)]);
    assert_eq!(reason(&outcome), Some(NotResolved::Unresolved), "{base:?}, ours {ours:?}, theirs {theirs:?}");
  }
}

/// The ends of three texts merge as `git merge-file` merges them: a conflict where git sees one, else git's result,
/// with LF and CRLF, a side without a final line end and an empty base
#[test]
fn the_end_of_a_file_merges_as_git_merge_file_merges_it() {
  // (ours, base, theirs), the order of `git merge-file`
  let cases = [
    ("x\n\n", "x\n", "x\ny\n"),
    ("x\ny\n", "x\n", "x\n\n"),
    ("x\r\n\r\n", "x\r\n", "x\r\ny\r\n"),
    ("x\n", "x\n\n", "x\n\nz\n"),
    ("x\n\nz\n", "x\n\n", "x\n"),
    ("x", "x\n", "x\ny\n"),
    ("x\n\n", "x", "x\ny"),
    ("a\nb", "a\nb\n", "a\nB\n"),
    ("a\nb\n\n", "a\nb\n", "a\nb"),
    ("\n", "", "x\n"),
    ("a\n", "", "b\n"),
    ("A\nb\nc\n", "a\nb\nc\n", "a\nb\nc\n\nd\n"),
    ("A\nb\n", "a\nb\n", "a\nb\n\n"),
    ("x\n\n", "x\n", "x\n\n"),
    ("A\r\nb\r\n", "a\r\nb\r\n", "a\r\nb\r\n\r\n"),
    // a line without an end pairs only with an equal line without an end
    ("a", "}\na", "a\n"),
    ("b\n}", "\nb", "b"),
    ("x y z\nx y z\n}", "}", "}\na\na c\nx y z\na c\nb"),
    ("a\nb\nc", "a\nb\nc\n", "A\nb\nc\n"),
    ("x", "x", "x\ny"),
  ];
  let directory = tempfile::tempdir().unwrap();
  for (ours, base, theirs) in cases {
    let git = git_merge_file(directory.path(), ours, base, theirs);
    let merge = merge_texts(ours, base, theirs, WhitespacePolicy::Default, &DiffBudget::NONE);
    let case = format!("ours {ours:?}, base {base:?}, theirs {theirs:?}");
    assert_eq!(merge.conflicts > 0, git.status.code() != Some(0), "{case}");
    if git.status.success() {
      let composed = compose_merge(ours.as_bytes(), base.as_bytes(), theirs.as_bytes(), &merge);
      assert_eq!(
        (String::from_utf8(composed.bytes).unwrap(), composed.exact),
        (String::from_utf8(git.stdout).unwrap(), true),
        "{case}"
      );
    }
  }
}

/// `git merge-file -p` of the three texts in `directory`: its exit code is the number of conflicts
fn git_merge_file(directory: &std::path::Path, ours: &str, base: &str, theirs: &str) -> std::process::Output {
  for (name, text) in [("ours", ours), ("base", base), ("theirs", theirs)] {
    std::fs::write(directory.join(name), text).unwrap();
  }
  git_command().current_dir(directory).args(["merge-file", "-p", "ours", "base", "theirs"]).output().unwrap()
}

/// A side merged again with the result of a resolve by words can conflict, and git conflicts on it too: the words
/// resolve a line added at the top on one side and the blank line and the last line removed on the other, a conflict
/// of git, and the second side merged again with that result is one conflict for git as for the engine. Why
/// `apply_test::an_exact_result_has_the_lines_of_a_merge` of text-diff does not ask a merge again to resolve
#[test]
fn a_side_merged_again_with_the_result_conflicts_in_git_too() {
  let directory = tempfile::tempdir().unwrap();
  let (ours, base, theirs) = ("\n", "\n\na", "a\n\na");
  assert_eq!(git_merge_file(directory.path(), ours, base, theirs).status.code(), Some(1));
  let merge = merge_texts(ours, base, theirs, WhitespacePolicy::Default, &DiffBudget::NONE);
  let composed = compose_merge(ours.as_bytes(), base.as_bytes(), theirs.as_bytes(), &merge);
  assert_eq!((merge.unresolved, String::from_utf8(composed.bytes).unwrap(), composed.exact), (0, "a\n".to_string(), true));

  let again = git_merge_file(directory.path(), ours, base, "a\n");
  assert_eq!(
    (again.status.code(), String::from_utf8(again.stdout).unwrap()),
    (Some(1), "<<<<<<< ours\n\n=======\na\n>>>>>>> theirs\n".to_string())
  );
  assert_eq!(merge_texts(ours, base, "a\n", WhitespacePolicy::Default, &DiffBudget::NONE).unresolved, 1);
}
