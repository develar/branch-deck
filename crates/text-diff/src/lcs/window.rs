//! The parts of two integer sequences that an LCS algorithm compares. Branch Deck's own: the Kotlin classes take
//! these six values as constructor parameters each.

use fixedbitset::FixedBitSet;

/// `first[start1..start1 + count1]` and `second[start2..start2 + count2]`
#[derive(Clone, Copy)]
pub struct Window<'a> {
  pub first: &'a [u32],
  pub second: &'a [u32],
  pub start1: usize,
  pub count1: usize,
  pub start2: usize,
  pub count2: usize,
}

impl<'a> Window<'a> {
  pub const fn new(first: &'a [u32], second: &'a [u32], start1: usize, count1: usize, start2: usize, count2: usize) -> Self {
    Self {
      first,
      second,
      start1,
      count1,
      start2,
      count2,
    }
  }

  /// Both sequences whole
  pub const fn whole(first: &'a [u32], second: &'a [u32]) -> Self {
    Self::new(first, second, 0, first.len(), 0, second.len())
  }

  /// Two empty change sets, with room for each sequence
  pub fn empty_changes(&self) -> [FixedBitSet; 2] {
    [FixedBitSet::with_capacity(self.first.len()), FixedBitSet::with_capacity(self.second.len())]
  }
}
