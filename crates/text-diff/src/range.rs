// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/util/Range.kt` of IntelliJ Platform
// (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors).

//! A pair of half-open intervals, one in each of the two compared sequences.

/// `[start1, end1)` of the first sequence and `[start2, end2)` of the second
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Range {
  pub start1: usize,
  pub end1: usize,
  pub start2: usize,
  pub end2: usize,
}

impl Range {
  pub const fn new(start1: usize, end1: usize, start2: usize, end2: usize) -> Self {
    debug_assert!(start1 <= end1 && start2 <= end2);
    Self { start1, end1, start2, end2 }
  }

  pub const fn is_empty(&self) -> bool {
    self.start1 == self.end1 && self.start2 == self.end2
  }
}
