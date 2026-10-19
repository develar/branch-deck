// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/util/MergeRange.kt` and
// `ThreeSide.kt`, and the merge fragments `fragments/MergeLineFragment*.kt` and `MergeWordFragment*.kt`, of IntelliJ
// Platform (Apache License 2.0, Copyright 2000-2021 JetBrains s.r.o. and contributors). A `MergeRange` stands for the
// merge fragments too: of lines for a line merge, of offsets for a word merge.

//! A triple of half-open intervals, one in each of the three merged sequences, and the three sides.

use std::fmt::{Display, Formatter};

/// One of the three sides of a merge
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThreeSide {
  Left,
  Base,
  Right,
}

impl ThreeSide {
  pub const ALL: [Self; 3] = [Self::Left, Self::Base, Self::Right];

  pub const fn index(self) -> usize {
    match self {
      Self::Left => 0,
      Self::Base => 1,
      Self::Right => 2,
    }
  }

  /// `left`, `base` or `right` for this side
  pub fn select<T>(self, left: T, base: T, right: T) -> T {
    match self {
      Self::Left => left,
      Self::Base => base,
      Self::Right => right,
    }
  }
}

/// `[start1, end1)` of the left sequence, `[start2, end2)` of the base and `[start3, end3)` of the right one
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MergeRange {
  pub start1: usize,
  pub end1: usize,
  pub start2: usize,
  pub end2: usize,
  pub start3: usize,
  pub end3: usize,
}

impl MergeRange {
  pub const fn new(start1: usize, end1: usize, start2: usize, end2: usize, start3: usize, end3: usize) -> Self {
    Self {
      start1,
      end1,
      start2,
      end2,
      start3,
      end3,
    }
  }

  pub const fn is_empty(&self) -> bool {
    self.start1 == self.end1 && self.start2 == self.end2 && self.start3 == self.end3
  }

  /// The start of the interval of `side` (`getStartLine` and `getStartOffset` of the fragments)
  pub const fn start(&self, side: ThreeSide) -> usize {
    match side {
      ThreeSide::Left => self.start1,
      ThreeSide::Base => self.start2,
      ThreeSide::Right => self.start3,
    }
  }

  /// The end of the interval of `side`
  pub const fn end(&self, side: ThreeSide) -> usize {
    match side {
      ThreeSide::Left => self.end1,
      ThreeSide::Base => self.end2,
      ThreeSide::Right => self.end3,
    }
  }
}

impl Display for MergeRange {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
    write!(
      formatter,
      "[{}, {}) - [{}, {}) - [{}, {})",
      self.start1, self.end1, self.start2, self.end2, self.start3, self.end3
    )
  }
}
