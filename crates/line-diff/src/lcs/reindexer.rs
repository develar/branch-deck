// Ported and modified from `community/platform/util/diff/src/com/intellij/util/diff/Reindexer.kt` of IntelliJ Platform
// (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors).

//! Discards the elements that only one side has before the LCS runs, and maps its changes back to the full sequences.

use crate::lcs::diff::LcsBuilder;
use fixedbitset::FixedBitSet;

#[derive(Default)]
pub struct Reindexer {
  old_indices: [Vec<usize>; 2],
  original_lengths: [usize; 2],
  discarded_lengths: [usize; 2],
}

impl Reindexer {
  /// Both sequences without the elements the other one does not contain
  pub fn discard_unique(&mut self, ints1: &[u32], ints2: &[u32]) -> [Vec<u32>; 2] {
    let discarded1 = self.discard(ints2, ints1, 0);
    let discarded2 = self.discard(&discarded1, ints2, 1);
    [discarded1, discarded2]
  }

  /// Maps each index to itself, for a test without `discard_unique`
  #[cfg(test)]
  pub(crate) fn id_init(&mut self, length1: usize, length2: usize) {
    self.original_lengths = [length1, length2];
    self.discarded_lengths = [length1, length2];
    self.old_indices = [(0..length1).collect(), (0..length2).collect()];
  }

  /// The index in the full sequence `array` of the element at `index` of its discarded sequence
  #[cfg(test)]
  pub(crate) fn restore_index(&self, index: usize, array: usize) -> usize {
    self.old_indices[array][index]
  }

  fn discard(&mut self, needed: &[u32], to_discard: &[u32], array_index: usize) -> Vec<u32> {
    self.original_lengths[array_index] = to_discard.len();
    let mut sorted = needed.to_vec();
    sorted.sort_unstable();
    let mut discarded = Vec::with_capacity(to_discard.len());
    let mut old_indices = Vec::with_capacity(to_discard.len());
    for (i, &value) in to_discard.iter().enumerate() {
      if sorted.binary_search(&value).is_ok() {
        discarded.push(value);
        old_indices.push(i);
      }
    }
    self.old_indices[array_index] = old_indices;
    self.discarded_lengths[array_index] = discarded.len();
    discarded
  }

  /// Reports the changes of the discarded sequences, `discarded_changes`, as changes of the full ones
  pub fn reindex(&self, discarded_changes: &[FixedBitSet; 2], builder: &mut impl LcsBuilder) {
    let [original_length1, original_length2] = self.original_lengths;
    let [discarded_length1, discarded_length2] = self.discarded_lengths;
    let mapped;
    let (changes1, changes2) = if discarded_length1 == original_length1 && discarded_length2 == original_length2 {
      (&discarded_changes[0], &discarded_changes[1])
    } else {
      let mut changes1 = FixedBitSet::with_capacity(original_length1);
      let mut changes2 = FixedBitSet::with_capacity(original_length2);
      let [old_indices1, old_indices2] = &self.old_indices;
      let mut x = 0;
      let mut y = 0;
      while x < discarded_length1 || y < discarded_length2 {
        if (x < discarded_length1 && y < discarded_length2) && !discarded_changes[0].contains(x) && !discarded_changes[1].contains(y) {
          x = increment(old_indices1, x, &mut changes1, original_length1);
          y = increment(old_indices2, y, &mut changes2, original_length2);
        } else if discarded_changes[0].contains(x) {
          changes1.insert(old_indices1[x]);
          x = increment(old_indices1, x, &mut changes1, original_length1);
        } else if discarded_changes[1].contains(y) {
          changes2.insert(old_indices2[y]);
          y = increment(old_indices2, y, &mut changes2, original_length2);
        }
      }
      changes1.set_range(0..if discarded_length1 == 0 { original_length1 } else { old_indices1[0] }, true);
      changes2.set_range(0..if discarded_length2 == 0 { original_length2 } else { old_indices2[0] }, true);
      mapped = [changes1, changes2];
      (&mapped[0], &mapped[1])
    };

    let mut x = 0;
    let mut y = 0;
    while x < original_length1 && y < original_length2 {
      let start_x = x;
      while x < original_length1 && y < original_length2 && !changes1.contains(x) && !changes2.contains(y) {
        x += 1;
        y += 1;
      }
      if x > start_x {
        builder.add_equal(x - start_x);
      }
      let mut dx = 0;
      let mut dy = 0;
      while x < original_length1 && changes1.contains(x) {
        dx += 1;
        x += 1;
      }
      while y < original_length2 && changes2.contains(y) {
        dy += 1;
        y += 1;
      }
      if dx != 0 || dy != 0 {
        builder.add_change(dx, dy);
      }
    }
    if x != original_length1 || y != original_length2 {
      builder.add_change(original_length1 - x, original_length2 - y);
    }
  }
}

/// Marks the discarded elements after `indexes[i]` as changed and returns the next index
fn increment(indexes: &[usize], i: usize, set: &mut FixedBitSet, length: usize) -> usize {
  if i + 1 < indexes.len() {
    set.set_range(indexes[i] + 1..indexes[i + 1], true);
  } else {
    set.set_range(indexes[i] + 1..length, true);
  }
  i + 1
}
