// Ported and modified from IntelliJ Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and
// contributors): `community/platform/util/diff/src/com/intellij/diff/fragments/DiffFragment.kt`, `DiffFragmentImpl.kt`,
// `LineFragment.kt` and `LineFragmentImpl.kt`. The interfaces and their implementations are one struct each.

//! The results of a comparison: changed ranges of offsets, and changed blocks of lines with the changes inside them.

use crate::range::Range;

/// A changed part of the texts: `[start_offset1, end_offset1)` became `[start_offset2, end_offset2)`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DiffFragment {
  pub start_offset1: usize,
  pub end_offset1: usize,
  pub start_offset2: usize,
  pub end_offset2: usize,
}

impl DiffFragment {
  pub const fn new(start_offset1: usize, end_offset1: usize, start_offset2: usize, end_offset2: usize) -> Self {
    debug_assert!(start_offset1 != end_offset1 || start_offset2 != end_offset2, "DiffFragment should not be empty");
    debug_assert!(start_offset1 <= end_offset1 && start_offset2 <= end_offset2, "DiffFragment is invalid");
    Self {
      start_offset1,
      end_offset1,
      start_offset2,
      end_offset2,
    }
  }
}

/// A changed block of lines.
///
/// The offsets cover whole lines, with the `\n` at the end; the last line of a text can have none.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LineFragment {
  pub start_line1: usize,
  pub end_line1: usize,
  pub start_line2: usize,
  pub end_line2: usize,
  pub start_offset1: usize,
  pub end_offset1: usize,
  pub start_offset2: usize,
  pub end_offset2: usize,
  /// The changes inside the block (by word, for example), with offsets relative to the start of the block. `None`:
  /// no similar parts were found. Empty: the block changed only in what the policy ignores.
  pub inner_fragments: Option<Vec<DiffFragment>>,
}

impl LineFragment {
  pub fn new(lines: Range, offsets: Range, inner_fragments: Option<Vec<DiffFragment>>) -> Self {
    debug_assert!(lines.start1 != lines.end1 || lines.start2 != lines.end2, "LineFragment should not be empty");
    let inner_fragments = drop_whole_changed_fragments(inner_fragments, offsets.end1 - offsets.start1, offsets.end2 - offsets.start2);
    Self {
      start_line1: lines.start1,
      end_line1: lines.end1,
      start_line2: lines.start2,
      end_line2: lines.end2,
      start_offset1: offsets.start1,
      end_offset1: offsets.end1,
      start_offset2: offsets.start2,
      end_offset2: offsets.end2,
      inner_fragments,
    }
  }

  /// The same block with other inner changes
  #[must_use]
  pub fn with_inner(&self, inner_fragments: Option<Vec<DiffFragment>>) -> Self {
    Self::new(self.lines(), self.offsets(), inner_fragments)
  }

  pub const fn lines(&self) -> Range {
    Range::new(self.start_line1, self.end_line1, self.start_line2, self.end_line2)
  }

  pub const fn offsets(&self) -> Range {
    Range::new(self.start_offset1, self.end_offset1, self.start_offset2, self.end_offset2)
  }
}

/// One inner change that covers the whole block says nothing more than the block
fn drop_whole_changed_fragments(fragments: Option<Vec<DiffFragment>>, length1: usize, length2: usize) -> Option<Vec<DiffFragment>> {
  if let Some([fragment]) = fragments.as_deref()
    && fragment.start_offset1 == 0
    && fragment.start_offset2 == 0
    && fragment.end_offset1 == length1
    && fragment.end_offset2 == length2
  {
    return None;
  }
  fragments
}
