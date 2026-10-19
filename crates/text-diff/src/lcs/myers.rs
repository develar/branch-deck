// Ported and modified from `community/platform/util/diff/src/com/intellij/util/diff/MyersLCS.kt` of IntelliJ Platform
// (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors). The bit sets are owned and handed back
// by `into_changes`, instead of shared references.

//! The longest common subsequence of two integer sequences, by E. W. Myers, "An O(ND) Difference Algorithm and Its
//! Variations" (1986): O(ND) time, O(N) memory.

use crate::lcs::diff::{DELTA_THRESHOLD_SIZE, FilesTooBig};
use crate::lcs::window::Window;
use fixedbitset::FixedBitSet;

/// Marks the elements of `first[start1..start1 + count1]` and `second[start2..start2 + count2]` that are not in their
/// longest common subsequence
pub struct MyersLcs<'a> {
  window: Window<'a>,
  changes1: FixedBitSet,
  changes2: FixedBitSet,
  v_forward: Vec<isize>,
  v_backward: Vec<isize>,
}

impl<'a> MyersLcs<'a> {
  pub fn new(first: &'a [u32], second: &'a [u32]) -> Self {
    let window = Window::whole(first, second);
    Self::with_changes(window, window.empty_changes())
  }

  /// Writes into the given sets, which may already hold the changes of other windows
  pub fn with_changes(window: Window<'a>, [mut changes1, mut changes2]: [FixedBitSet; 2]) -> Self {
    changes1.set_range(window.start1..window.start1 + window.count1, true);
    changes2.set_range(window.start2..window.start2 + window.count2, true);

    let total_sequence_length = window.count1 + window.count2;
    Self {
      window,
      changes1,
      changes2,
      v_forward: vec![0; total_sequence_length + 1],
      v_backward: vec![0; total_sequence_length + 1],
    }
  }

  /// Runs O(ND) Myers algorithm where D is bound by A + B * sqrt(N).
  ///
  /// Under certain assumptions about the distribution of the elements of the sequences the expected running time of
  /// the Myers algorithm is O(N + D^2). Thus under given constraints it reduces to O(N).
  pub fn execute_linear(&mut self) {
    let threshold = 20000 + 10 * (self.window.count1 + self.window.count2).isqrt();
    // without the exception the threshold cannot fail
    let _ = self.execute_until(threshold, false);
  }

  pub fn execute(&mut self) {
    let _ = self.execute_until(self.window.count1 + self.window.count2, false);
  }

  /// Gives up when the edit distance is above the threshold
  pub fn execute_with_threshold(&mut self) -> Result<(), FilesTooBig> {
    let threshold = (20000 + 10 * (self.window.count1 + self.window.count2).isqrt()).max(DELTA_THRESHOLD_SIZE);
    self.execute_until(threshold, true)
  }

  /// The changed elements of the first and the second sequence
  pub fn into_changes(self) -> [FixedBitSet; 2] {
    [self.changes1, self.changes2]
  }

  pub const fn changes(&self) -> [&FixedBitSet; 2] {
    [&self.changes1, &self.changes2]
  }

  fn execute_until(&mut self, threshold: usize, throw_exception: bool) -> Result<(), FilesTooBig> {
    if self.window.count1 == 0 || self.window.count2 == 0 {
      return Ok(());
    }
    self.execute_range(
      0,
      signed(self.window.count1),
      0,
      signed(self.window.count2),
      signed(threshold.min(self.window.count1 + self.window.count2)),
      throw_exception,
    )
  }

  /// LCS( old[oldStart, oldEnd), new[newStart, newEnd) )
  #[expect(clippy::many_single_char_names, reason = "the names of the paper and of MyersLCS.kt")]
  fn execute_range(&mut self, old_start: isize, old_end: isize, new_start: isize, new_end: isize, difference_estimate: isize, throw_exception: bool) -> Result<(), FilesTooBig> {
    debug_assert!(old_start <= old_end && new_start <= new_end);
    if old_start < old_end && new_start < new_end {
      let old_length = old_end - old_start;
      let new_length = new_end - new_start;
      self.v_forward[index(new_length + 1)] = 0;
      self.v_backward[index(new_length + 1)] = 0;
      let half_d = (difference_estimate + 1) / 2;
      let mut xx: isize = -1;
      let mut kk: isize = -1;
      let mut td: isize = -1;

      'outer: for d in 0..=half_d {
        let l = new_length + (-d).max(-new_length + ((d ^ new_length) & 1));
        let r = new_length + d.min(old_length - ((d ^ old_length) & 1));
        let mut k = l;
        while k <= r {
          let mut x = if k == l || k != r && self.forward(k - 1) < self.forward(k + 1) {
            self.forward(k + 1)
          } else {
            self.forward(k - 1) + 1
          };
          let y = x - k + new_length;
          x += self.common_subsequence_length_forward(old_start + x, new_start + y, (old_end - old_start - x).min(new_end - new_start - y));
          self.v_forward[index(k)] = x;
          k += 2;
        }

        if (old_length - new_length) % 2 != 0 {
          let mut k = l;
          while k <= r {
            if old_length - (d - 1) <= k && k <= old_length + (d - 1) && self.forward(k) + self.backward(new_length + old_length - k) >= old_length {
              xx = self.forward(k);
              kk = k;
              td = 2 * d - 1;
              break 'outer;
            }
            k += 2;
          }
        }

        let mut k = l;
        while k <= r {
          let mut x = if k == l || k != r && self.backward(k - 1) < self.backward(k + 1) {
            self.backward(k + 1)
          } else {
            self.backward(k - 1) + 1
          };
          let y = x - k + new_length;
          x += self.common_subsequence_length_backward(old_end - 1 - x, new_end - 1 - y, (old_end - old_start - x).min(new_end - new_start - y));
          self.v_backward[index(k)] = x;
          k += 2;
        }

        if (old_length - new_length) % 2 == 0 {
          let mut k = l;
          while k <= r {
            if old_length - d <= k && k <= old_length + d && self.forward(old_length + new_length - k) + self.backward(k) >= old_length {
              xx = old_length - self.backward(k);
              kk = old_length + new_length - k;
              td = 2 * d;
              break 'outer;
            }
            k += 2;
          }
        }
      }

      if td > 1 {
        let yy = xx - kk + new_length;
        let old_diff = (td + 1) / 2;
        if 0 < xx && 0 < yy {
          self.execute_range(old_start, old_start + xx, new_start, new_start + yy, old_diff, throw_exception)?;
        }
        if old_start + xx < old_end && new_start + yy < new_end {
          self.execute_range(old_start + xx, old_end, new_start + yy, new_end, td - old_diff, throw_exception)?;
        }
      } else if td >= 0 {
        let mut x = old_start;
        let mut y = new_start;
        while x < old_end && y < new_end {
          let common_length = self.common_subsequence_length_forward(x, y, (old_end - x).min(new_end - y));
          if common_length > 0 {
            self.add_unchanged(x, y, common_length);
            x += common_length;
            y += common_length;
          } else if old_end - old_start > new_end - new_start {
            x += 1;
          } else {
            y += 1;
          }
        }
      } else if throw_exception {
        // the difference is more than the given estimate
        return Err(FilesTooBig);
      }
    }
    Ok(())
  }

  fn forward(&self, k: isize) -> isize {
    self.v_forward[index(k)]
  }

  fn backward(&self, k: isize) -> isize {
    self.v_backward[index(k)]
  }

  fn add_unchanged(&mut self, start1: isize, start2: isize, count: isize) {
    let (start1, start2, count) = (index(start1), index(start2), index(count));
    self.changes1.set_range(self.window.start1 + start1..self.window.start1 + start1 + count, false);
    self.changes2.set_range(self.window.start2 + start2..self.window.start2 + start2 + count, false);
  }

  fn common_subsequence_length_forward(&self, old_index: isize, new_index: isize, max_length: isize) -> isize {
    let mut x = old_index;
    let mut y = new_index;

    let max_length = max_length.min((signed(self.window.count1) - old_index).min(signed(self.window.count2) - new_index));
    while x - old_index < max_length && self.window.first[self.window.start1 + index(x)] == self.window.second[self.window.start2 + index(y)] {
      x += 1;
      y += 1;
    }
    x - old_index
  }

  fn common_subsequence_length_backward(&self, old_index: isize, new_index: isize, max_length: isize) -> isize {
    let mut x = old_index;
    let mut y = new_index;

    let max_length = max_length.min(old_index.min(new_index) + 1);
    while old_index - x < max_length && self.window.first[self.window.start1 + index(x)] == self.window.second[self.window.start2 + index(y)] {
      x -= 1;
      y -= 1;
    }
    old_index - x
  }
}

/// A diagonal or an offset of the algorithm as an index, never negative where it indexes
#[expect(clippy::cast_sign_loss, reason = "the algorithm indexes only with values it proved non-negative")]
const fn index(value: isize) -> usize {
  debug_assert!(value >= 0);
  value as usize
}

#[expect(clippy::cast_possible_wrap, reason = "a sequence is shorter than isize::MAX")]
const fn signed(value: usize) -> isize {
  value as isize
}
