// Ported and modified from IntelliJ Platform (Apache License 2.0, Copyright 2000-2024 JetBrains s.r.o. and
// contributors): `doCompare` of `community/platform/diff-impl/src/com/intellij/diff/tools/util/text/TwosideTextDiffProviderBase.java`
// and `compareRange` of `SimpleTextDiffProvider.java`, without a custom `DiffComputer`.

//! The comparison of a two-side diff view: the blocks of changed lines and the changes inside them, under the
//! whitespace option and the highlight option of the view.

use crate::cancellation::{Deadline, DiffTooBig};
use crate::fragments::LineFragment;
use crate::line_offsets::LineOffsets;
use crate::manager::{
  compare_lines_in_range, compare_lines_inner_in_range, compare_lines_inner_with, create_inner_fragments, create_line_fragment, do_squash, process_adjoining, process_blocks,
  trimmed_run,
};
use crate::policy::{ComparisonPolicy, HighlightPolicy, InnerFragmentsPolicy, WhitespacePolicy};
use crate::range::Range;
use crate::text::Utf16Text;

/// Compares two texts as a diff view does.
///
/// A block with an empty `inner_fragments` list changed only in what the policy ignores; a view draws it pale.
pub fn compare_lines_inner(
  text1: &Utf16Text,
  text2: &Utf16Text,
  policy: WhitespacePolicy,
  highlight: HighlightPolicy,
  deadline: &Deadline,
) -> Result<Vec<LineFragment>, DiffTooBig> {
  deadline.check_canceled()?;
  let fragments = compare_lines_inner_with(text1, text2, policy.comparison_policy(), highlight.fragments_policy(), deadline)?;
  Ok(post_process(fragments, text1, text2, policy, highlight))
}

/// Compares the given blocks of lines of two texts as a diff view does, one list of blocks for each range
pub fn compare_line_ranges(
  text1: &Utf16Text,
  text2: &Utf16Text,
  lines_ranges: &[Range],
  policy: WhitespacePolicy,
  highlight: HighlightPolicy,
  deadline: &Deadline,
) -> Result<Vec<Vec<LineFragment>>, DiffTooBig> {
  deadline.check_canceled()?;
  lines_ranges
    .iter()
    .map(|range| {
      let fragments = compare_lines_inner_in_range(*range, text1, text2, policy.comparison_policy(), highlight.fragments_policy(), deadline)?;
      Ok(post_process(fragments, text1, text2, policy, highlight))
    })
    .collect()
}

fn post_process(fragments: Vec<LineFragment>, text1: &Utf16Text, text2: &Utf16Text, policy: WhitespacePolicy, highlight: HighlightPolicy) -> Vec<LineFragment> {
  let squash_fragments = highlight.should_squash() && policy.should_squash();
  let trim_fragments = policy.should_trim_chunks();
  process_blocks(fragments, text1.units(), text2.units(), policy.comparison_policy(), squash_fragments, trim_fragments)
}

/// A block of a view: a block of the comparison, or a whitespace-only block that `IgnoreBlankLines` cuts off a run
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewFragment {
  pub fragment: LineFragment,
  /// `IgnoreBlankLines` cut the block off, where IntelliJ drops it: the view draws it as unchanged and aligns its sides
  pub ignored: bool,
}

/// The blocks of a view: as `compare_lines_inner`, but the blocks that `IgnoreBlankLines` cuts off stay, marked
/// `ignored`, and the empty last line of a text pairs with no line but the empty last line of the other
/// (`compare_content_lines`). Branch Deck's own: IntelliJ drops the cut-off blocks (`ComparisonManagerImpl.processBlocks`
/// with `trim`), but the two sides of a block with other line counts still need their alignment. A block that changed
/// only in what the policy ignores (an empty list of inner changes) is no ignored block: IntelliJ keeps it as a change,
/// drawn pale (`SimpleDiffChangeUi.createHighlighter`, `withIgnored`), counted and navigated.
pub fn compare_for_view(text1: &Utf16Text, text2: &Utf16Text, policy: WhitespacePolicy, highlight: HighlightPolicy, deadline: &Deadline) -> Result<Vec<ViewFragment>, DiffTooBig> {
  deadline.check_canceled()?;
  let fragments = compare_content_lines(text1, text2, policy.comparison_policy(), highlight.fragments_policy(), deadline)?;
  let squash = highlight.should_squash() && policy.should_squash();
  if !policy.should_trim_chunks() {
    let fragments = process_blocks(fragments, text1.units(), text2.units(), policy.comparison_policy(), squash, false);
    return Ok(fragments.into_iter().map(|fragment| ViewFragment { fragment, ignored: false }).collect());
  }

  let mut result = Vec::with_capacity(fragments.len());
  process_adjoining(&fragments, |run| {
    let (start, end) = trimmed_run(run, text1.units(), text2.units(), policy.comparison_policy(), true);
    let mut push = |part: &[LineFragment], ignored: bool| {
      if part.is_empty() {
        return;
      }
      if squash {
        result.push(ViewFragment {
          fragment: do_squash(part),
          ignored,
        });
      } else {
        result.extend(part.iter().map(|fragment| ViewFragment {
          fragment: fragment.clone(),
          ignored,
        }));
      }
    };
    push(&run[..start], true);
    push(&run[start..end], false);
    push(&run[end..], true);
  });
  Ok(result)
}

/// Compares two texts by line without the empty last line of each (`content_lines`), then the changed blocks as
/// `fragments_policy` asks. Branch Deck's own: IntelliJ compares every line of a document, the empty line after a final
/// line end too, so that line can pair with a blank line of the other text. `"x\n"` to `"x\n\n"` then adds the new empty
/// last line, which does not count, instead of the blank line, and a file counts a line fewer than git does, which has
/// no such line. Here the two empty last lines pair with each other, and when only one text has one, it is the change of
/// the final line end: a block of its own, or the end of the block that reaches the ends of both compared parts.
fn compare_content_lines(
  text1: &Utf16Text,
  text2: &Utf16Text,
  policy: ComparisonPolicy,
  fragments_policy: InnerFragmentsPolicy,
  deadline: &Deadline,
) -> Result<Vec<LineFragment>, DiffTooBig> {
  let (offsets1, offsets2) = (text1.line_offsets(), text2.line_offsets());
  let (content1, content2) = (content_lines(offsets1), content_lines(offsets2));
  let mut fragments = compare_lines_in_range(Range::new(0, content1, 0, content2), text1, text2, policy, deadline)?;
  let (lines1, lines2) = (offsets1.line_count(), offsets2.line_count());
  if lines1 - content1 != lines2 - content2 {
    match fragments.last_mut() {
      Some(last) if last.end_line1 == content1 && last.end_line2 == content2 => {
        *last = create_line_fragment(Range::new(last.start_line1, lines1, last.start_line2, lines2), offsets1, offsets2);
      }
      _ => fragments.push(create_line_fragment(Range::new(content1, lines1, content2, lines2), offsets1, offsets2)),
    }
  }
  if fragments_policy == InnerFragmentsPolicy::None {
    return Ok(fragments);
  }
  create_inner_fragments(&fragments, text1.units(), text2.units(), policy, fragments_policy, deadline)
}

/// The number of lines of a text without its empty last line: the line after a final line end, or the one line of an
/// empty text
fn content_lines(offsets: &LineOffsets) -> usize {
  let last = offsets.line_count() - 1;
  if offsets.line_start(last) == offsets.line_end(last) { last } else { last + 1 }
}
