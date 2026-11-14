//! The marks of the files of a commit through the service (`FileDiffs::marks`): the `commit_marks` template of the E2E
//! tests. The marks of texts read with the bindings of `code-symbols` are its tests (`marks_bindings_test.rs`,
//! `marks_bindings_renames_test.rs`, `marks_bindings_subject_test.rs`).

use crate::fixture_test::unhurried;
use crate::model::{DiffContent, DiffSource};
use code_symbols::marks_text::{MarkedDiff, show};
use git_executor::git_command_executor::GitCommandExecutor;
use line_diff::policy::WhitespacePolicy::Default as Exact;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;

/// The `commit_marks` template of the E2E tests (`test_utils::repo_template`), through the service: each case is a
/// block of its own, and the marks this side decides are there. The layout the later tests of the views rely on
#[test]
fn the_marks_template_has_each_case_in_a_block_of_its_own() {
  let directory = tempfile::tempdir().unwrap();
  test_utils::repo_template::templates::commit_marks().build(directory.path()).unwrap();
  let git = GitCommandExecutor::new();
  let path = directory.path().to_str().unwrap();
  let head = git.execute_command(&["rev-parse", "HEAD"], path).unwrap();
  let files = unhurried()
    .read(&git, path, &ObjectCache::disabled(), &DiffSource::Commit { id: head }, Exact.into())
    .unwrap();
  let shown: Vec<(&str, String)> = files
    .iter()
    .map(|file| match &file.content {
      DiffContent::Text(diff) => (
        file.path.as_str(),
        show(&MarkedDiff {
          diff: diff.line_diff(),
          marks: diff.marks.clone().into(),
        }),
      ),
      other => panic!("{}: {other:?}", file.path),
    })
    .collect();
  let expected = [
    (
      "src/Ledger.kt",
      // `accounts` moved in, the wrapped call, `describe` moved in, the three places of `total`, `accounts` moved
      // out, the comment, `isEmpty`, `describe` moved out, `reportTitle` moved out. `accounts` moved with the blank
      // line that was before it and is after it now; `describe` moved with the one after it, was indented and has an
      // edited line. The blocks the two left have no start. The three of the local `total`, which is `sum` now, are
      // each a run of a renamed name, quiet: nothing but the name changed between its places, and no block of them
      // has a start
      "blocks [5..5/5..10, 6..7/11..14, 11..11/18..25, 13..14/27..28, 16..17/30..31, 19..20/33..34, 21..26/35..35, 27..28/36..37, 32..33/41..42, 45..52/54..54, 53..58/55..55] \
       runs [6..7/11..14 formatting quiet, 13..14/27..28 renamed quiet, 16..17/30..31 renamed quiet, 19..20/33..34 renamed quiet, 27..28/36..37 comments] \
       moves [22..26->5..9 blanks 1+0, 45..52->18..25 indent +2 edited 1] \
       renames [total->sum local declared [0] at 13:8 16:8 19:11 / 27:8 30:8 33:11] \
       starts [0, 1, 5, 2, 1, 18, 7, 1, 36, 8, 1, 41, 10, 0, 53] \
       moved out [54..58->1:17]"
        .to_string(),
    ),
    // the two calls of `openStore`, which the file does not declare and `Store.kt` does: a rename by the other file,
    // each call a run of it, a label; `reportTitle` moved in from `Ledger.kt`, a label too: every block is read
    (
      "src/Report.kt",
      "blocks [4..5/4..5, 10..11/10..11, 16..16/16..21] runs [4..5/4..5 renamed, 10..11/10..11 renamed] starts [0, 1, 4, 1, 1, 10, 2, 1, 16] \
       moved in [17..21->0:54] used [openStore->connectStore declared [] at 4:16 10:16 / 4:16 10:16 in 2:18 Unavailable]"
        .to_string(),
    ),
    // the declaration of `openStore`, which is `connectStore` now: a rename with one place, read; no index was asked
    (
      "src/Store.kt",
      "blocks [18..19/18..19] runs [] renames [openStore->connectStore declared [0] at 18:4 / 18:4] starts [0, 1, 18] declared [0 Unavailable]".to_string(),
    ),
  ];
  assert_eq!(shown, expected);
}
