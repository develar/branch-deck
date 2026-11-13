//! The result of a merge on bytes: the resolves by words, the line ends, and property tests that a change of one side
//! gives that side.

use crate::compose::{ComposedMerge, compose_merge};
use crate::merge_model::{MergeResolution, merge_texts};
use crate::test_support::{proptest_config, text_of_lines};
use line_diff::model::DiffBudget;
use line_diff::policy::WhitespacePolicy;
use pretty_assertions::assert_eq;
use proptest::prelude::Strategy;
use proptest::proptest;

/// The result of the merge of three texts, and whether it is exact
fn composed(left: &str, base: &str, right: &str) -> (String, bool) {
  let merge = merge_texts(left, base, right, WhitespacePolicy::Default, &DiffBudget::NONE);
  let ComposedMerge { bytes, exact } = compose_merge(left.as_bytes(), base.as_bytes(), right.as_bytes(), &merge);
  (String::from_utf8(bytes).unwrap(), exact)
}

#[test]
fn a_conflict_that_its_words_resolve_takes_both_changes() {
  assert_eq!(
    composed("fun f(A, b, c)\nx\n", "fun f(a, b, c)\nx\n", "fun f(a, b, C)\nx\n"),
    ("fun f(A, b, C)\nx\n".to_string(), true)
  );
  // without a final newline, the resolved line stays the last one without an end
  assert_eq!(composed("A b c", "a b c", "a b C"), ("A b C".to_string(), true));
}

#[test]
fn the_changes_of_both_sides_apply_to_the_base() {
  let base = "a\nb\nc\nd\ne\n";
  assert_eq!(composed("A\nb\nc\nd\ne\n", base, "a\nb\nc\nd\nE\n"), ("A\nb\nc\nd\nE\n".to_string(), true));
  // one side adds the final newline, the other changes a line
  assert_eq!(composed("a\nb", "a\nb", "A\nb"), ("A\nb".to_string(), true));
  assert_eq!(composed("a\nb\n", "a\nb", "A\nb"), ("A\nb\n".to_string(), true));
}

/// A resolve by words that leaves nothing of the texts is the empty text, exact: no line is before it whose end it
/// would decide; with a line before it, the end of that line is not known. The resolve under the default policy gives no
/// such resolve for these texts (one side removed the final line end, the other the last line: the conflict stays
/// git's, `an_exact_result_has_the_lines_of_a_merge`), so the conflict takes it here as a resolve would give it
#[test]
fn a_resolve_that_leaves_nothing_is_the_empty_text() {
  let composed_with_nothing = |left: &str, base: &str, right: &str| {
    let mut merge = merge_texts(left, base, right, WhitespacePolicy::Default, &DiffBudget::NONE);
    assert_eq!(merge.unresolved, 1, "the conflict of git");
    let block = merge.blocks.last_mut().unwrap();
    (block.resolution, block.resolved_text) = (Some(MergeResolution::Text), Some(String::new()));
    let ComposedMerge { bytes, exact } = compose_merge(left.as_bytes(), base.as_bytes(), right.as_bytes(), &merge);
    (String::from_utf8(bytes).unwrap(), exact)
  };
  assert_eq!(composed_with_nothing("}", "}\n", ""), (String::new(), true));
  assert_eq!(composed_with_nothing("x\n}", "x\n}\n", "x\n").1, false);
}

#[test]
fn a_conflict_that_does_not_resolve_keeps_its_base_and_is_not_exact() {
  assert_eq!(composed("x\nA\n", "x\na\n", "x\nB\n"), ("x\na\n".to_string(), false));
}

#[test]
fn the_lines_keep_the_ends_of_the_texts() {
  let crlf = |text: &str| text.replace('\n', "\r\n");
  let (left, base, right) = (crlf("fun f(A, b, c)\nx\ny\n"), crlf("fun f(a, b, c)\nx\ny\n"), crlf("fun f(a, b, C)\nx\nY\n"));
  assert_eq!(composed(&left, &base, &right), (crlf("fun f(A, b, C)\nx\nY\n"), true));
  let cr = |text: &str| text.replace('\n', "\r");
  assert_eq!(composed(&cr("A b c\nx\n"), &cr("a b c\nx\n"), &cr("a b C\nx\n")), (cr("A b C\nx\n"), true));
  // the merge takes a line that one side ends otherwise as equal: the result keeps the base's end and is not exact
  assert_eq!(composed("A\nx\r\n", "a\nx\n", "a\nx\n"), ("A\nx\n".to_string(), false));
  // a resolve by words over lines that end in more than one way keeps its base and is not exact either
  let base = "a b\r\nc d\n";
  assert_eq!(composed("A b\r\nC d\n", base, "a b\r\nc D\n"), (base.to_string(), false));
}

/// A side merged again with the result of its merge can conflict, as in git: the words resolve the conflict of a line
/// added at the top on one side and the blank line and the last line removed on the other, and the second side merged
/// again with that result is one conflict, which `git merge-file` sees in both merges
/// (`word_resolve_test::a_side_merged_again_with_the_result_conflicts_in_git_too` of git-ops). Of the class of the shrunk
/// case of `an_exact_result_has_the_lines_of_a_merge` (its seed), which asked a merge again to resolve
#[test]
fn a_side_merged_again_with_the_result_can_conflict_as_in_git() {
  assert_eq!(composed("\n", "\n\na", "a\n\na"), ("a\n".to_string(), true));
  let again = merge_texts("\n", "\n\na", "a\n", WhitespacePolicy::Default, &DiffBudget::NONE);
  assert_eq!((again.conflicts, again.unresolved), (1, 1));
}

fn merge_text() -> impl Strategy<Value = String> {
  text_of_lines(vec!["a", "b", "a b", "a c", "x y z", "", "}"], vec!["\n", "\n", "\r\n"], 8)
}

proptest! {
  #![proptest_config(proptest_config(400))]

  #[test]
  fn a_change_of_one_side_gives_that_side(base in merge_text(), changed in merge_text()) {
    for (left, right) in [(base.as_str(), changed.as_str()), (changed.as_str(), base.as_str())] {
      let (result, exact) = composed(left, &base, right);
      if exact {
        assert_eq!(result, changed);
      }
      if !changed.contains('\r') && !base.contains('\r') {
        assert!(exact, "{left:?} {base:?} {right:?}");
      }
    }
    // the same change on both sides
    let (result, exact) = composed(&changed, &base, &changed);
    assert!(exact || base.contains('\r') || changed.contains('\r'));
    if exact {
      assert_eq!(result, changed);
    }
  }

  #[test]
  fn an_exact_result_has_the_lines_of_a_merge(left in merge_text(), base in merge_text(), right in merge_text()) {
    let merge = merge_texts(&left, &base, &right, WhitespacePolicy::Default, &DiffBudget::NONE);
    let composed = compose_merge(left.as_bytes(), base.as_bytes(), right.as_bytes(), &merge);
    assert_eq!(composed.exact && merge.unresolved > 0, false);
    // no side merged again with the result needs to give the result, or to resolve: the side and the result each
    // align with the base on their own, so the same change can stand in other blocks (a repeated line matched
    // elsewhere, a change joined with its neighbour, a final line end dropped beside it), and `git merge-file`
    // conflicts on such a merge too (`a_side_merged_again_with_the_result_can_conflict_as_in_git`)
    if merge.unresolved == 0 && !left.contains('\r') && !base.contains('\r') && !right.contains('\r') {
      assert!(composed.exact, "{left:?} {base:?} {right:?}");
    }
  }
}
