// Ported and modified from IntelliJ Platform (Apache License 2.0, Copyright 2000-2024 JetBrains s.r.o. and
// contributors): `doCompare` of `community/platform/diff-impl/src/com/intellij/diff/tools/util/text/TwosideTextDiffProviderBase.java`
// and `compareRange` of `SimpleTextDiffProvider.java`, without a custom `DiffComputer`.

//! The comparison of a two-side diff view: the blocks of changed lines and the changes inside them, under the
//! whitespace option and the highlight option of the view.

use crate::cancellation::{Deadline, DiffTooBig};
use crate::fragments::LineFragment;
use crate::manager::{compare_lines_inner_in_range, compare_lines_inner_with, do_squash, process_adjoining, process_blocks, trimmed_run};
use crate::policy::{HighlightPolicy, WhitespacePolicy};
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
/// `ignored`. Branch Deck's own: IntelliJ drops them (`ComparisonManagerImpl.processBlocks` with `trim`), but the two
/// sides of a block with other line counts still need their alignment. A block that changed only in what the policy
/// ignores (an empty list of inner changes) is no ignored block: IntelliJ keeps it as a change, drawn pale
/// (`SimpleDiffChangeUi.createHighlighter`, `withIgnored`), counted and navigated.
pub fn compare_for_view(text1: &Utf16Text, text2: &Utf16Text, policy: WhitespacePolicy, highlight: HighlightPolicy, deadline: &Deadline) -> Result<Vec<ViewFragment>, DiffTooBig> {
  deadline.check_canceled()?;
  let fragments = compare_lines_inner_with(text1, text2, policy.comparison_policy(), highlight.fragments_policy(), deadline)?;
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
