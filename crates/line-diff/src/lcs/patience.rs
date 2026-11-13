// Ported and modified from `community/platform/util/diff/src/com/intellij/util/diff/PatienceIntLCS.kt` of IntelliJ
// Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors).

//! Patience diff: the elements unique on both sides anchor the match, Myers' algorithm fills the gaps between them.

use crate::lcs::diff::FilesTooBig;
use crate::lcs::myers::MyersLcs;
use crate::lcs::unique_lcs::UniqueLcs;
use crate::lcs::window::Window;
use fixedbitset::FixedBitSet;
use std::mem::take;

/// Marks the elements of two windows of integer sequences that are not in their patience match
pub struct PatienceIntLcs<'a> {
  window: Window<'a>,
  changes1: FixedBitSet,
  changes2: FixedBitSet,
}

impl<'a> PatienceIntLcs<'a> {
  pub fn new(first: &'a [u32], second: &'a [u32]) -> Self {
    let window = Window::whole(first, second);
    Self::with_changes(window, window.empty_changes())
  }

  /// Writes into the given sets
  pub fn with_changes(window: Window<'a>, [changes1, changes2]: [FixedBitSet; 2]) -> Self {
    Self { window, changes1, changes2 }
  }

  /// With `fail_on_small_reduction`, gives up when the unique elements do not cut the problem in half within two steps
  pub fn execute(&mut self, fail_on_small_reduction: bool) -> Result<(), FilesTooBig> {
    let threshold_check_counter = if fail_on_small_reduction { 2 } else { -1 };
    let Window {
      start1, count1, start2, count2, ..
    } = self.window;
    self.execute_range(start1, count1, start2, count2, threshold_check_counter)
  }

  /// The changed elements of the first and the second sequence
  pub fn into_changes(self) -> [FixedBitSet; 2] {
    [self.changes1, self.changes2]
  }

  fn execute_range(&mut self, mut start1: usize, mut count1: usize, mut start2: usize, mut count2: usize, mut threshold_check_counter: i32) -> Result<(), FilesTooBig> {
    if count1 == 0 && count2 == 0 {
      return Ok(());
    }

    if count1 == 0 || count2 == 0 {
      self.add_change(start1, count1, start2, count2);
      return Ok(());
    }

    let start_offset = self.match_forward(start1, count1, start2, count2);
    start1 += start_offset;
    start2 += start_offset;
    count1 -= start_offset;
    count2 -= start_offset;

    let end_offset = self.match_backward(start1, count1, start2, count2);
    count1 -= end_offset;
    count2 -= end_offset;

    if count1 == 0 || count2 == 0 {
      self.add_change(start1, count1, start2, count2);
      return Ok(());
    }

    if threshold_check_counter == 0 {
      self.check_reduction(count1, count2)?;
    }
    threshold_check_counter = (threshold_check_counter - 1).max(-1);

    let window = Window::new(self.window.first, self.window.second, start1, count1, start2, count2);
    let Some([matching1, matching2]) = UniqueLcs::with_window(window).execute() else {
      if threshold_check_counter >= 0 {
        self.check_reduction(count1, count2)?;
      }
      let mut lcs = MyersLcs::with_changes(window, [take(&mut self.changes1), take(&mut self.changes2)]);
      lcs.execute_linear();
      [self.changes1, self.changes2] = lcs.into_changes();
      return Ok(());
    };

    let matched = matching1.len();
    debug_assert!(matched > 0);

    let mut c1 = matching1[0];
    let mut c2 = matching2[0];

    self.execute_range(start1, c1, start2, c2, threshold_check_counter)?;

    for i in 1..matched {
      let s1 = matching1[i - 1] + 1;
      let s2 = matching2[i - 1] + 1;

      c1 = matching1[i] - s1;
      c2 = matching2[i] - s2;

      if c1 > 0 || c2 > 0 {
        self.execute_range(start1 + s1, c1, start2 + s2, c2, threshold_check_counter)?;
      }
    }

    let s1;
    let s2;
    if matching1[matched - 1] == count1 - 1 {
      s1 = count1 - 1;
      c1 = 0;
    } else {
      s1 = matching1[matched - 1] + 1;
      c1 = count1 - s1;
    }
    if matching2[matched - 1] == count2 - 1 {
      s2 = count2 - 1;
      c2 = 0;
    } else {
      s2 = matching2[matched - 1] + 1;
      c2 = count2 - s2;
    }

    self.execute_range(start1 + s1, c1, start2 + s2, c2, threshold_check_counter)
  }

  fn match_forward(&self, start1: usize, count1: usize, start2: usize, count2: usize) -> usize {
    let size = count1.min(count2);
    (0..size).take_while(|&i| self.window.first[start1 + i] == self.window.second[start2 + i]).count()
  }

  fn match_backward(&self, start1: usize, count1: usize, start2: usize, count2: usize) -> usize {
    let size = count1.min(count2);
    (1..=size)
      .take_while(|&i| self.window.first[start1 + count1 - i] == self.window.second[start2 + count2 - i])
      .count()
  }

  fn add_change(&mut self, start1: usize, count1: usize, start2: usize, count2: usize) {
    self.changes1.set_range(start1..start1 + count1, true);
    self.changes2.set_range(start2..start2 + count2, true);
  }

  const fn check_reduction(&self, count1: usize, count2: usize) -> Result<(), FilesTooBig> {
    if count1 * 2 < self.window.count1 || count2 * 2 < self.window.count2 {
      return Ok(());
    }
    Err(FilesTooBig)
  }
}
