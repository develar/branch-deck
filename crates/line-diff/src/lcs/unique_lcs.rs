// Ported and modified from `community/platform/util/diff/src/com/intellij/util/diff/UniqueLCS.kt` of IntelliJ Platform
// (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors).

//! The longest common subsequence of the elements that occur once in each of two windows.

use crate::lcs::window::Window;
use rustc_hash::{FxBuildHasher, FxHashMap};

pub struct UniqueLcs<'a> {
  window: Window<'a>,
}

impl<'a> UniqueLcs<'a> {
  pub const fn new(first: &'a [u32], second: &'a [u32]) -> Self {
    Self::with_window(Window::whole(first, second))
  }

  pub const fn with_window(window: Window<'a>) -> Self {
    Self { window }
  }

  /// The matched pairs as two lists of indices relative to the window starts, or `None` without a unique common element
  pub fn execute(&self) -> Option<[Vec<usize>; 2]> {
    let Window {
      first,
      second,
      start1,
      count1,
      start2,
      count2,
    } = self.window;
    // map: key -> (offset1 + 1), -1 for a key seen twice
    // match: offset1 -> (offset2 + 1)
    let mut map: FxHashMap<u32, isize> = FxHashMap::with_capacity_and_hasher(count1 + count2, FxBuildHasher);
    let mut matches = vec![0_usize; count1];

    for i in 0..count1 {
      let key = first[start1 + i];
      let value = map.get(&key).copied().unwrap_or(0);

      if value == -1 {
        continue;
      }
      map.insert(key, if value == 0 { signed(i + 1) } else { -1 });
    }

    let mut count = 0_usize;
    for i in 0..count2 {
      let key = second[start2 + i];
      let value = map.get(&key).copied().unwrap_or(0);

      if value == 0 || value == -1 {
        continue;
      }
      let offset1 = value.unsigned_abs() - 1;
      if matches[offset1] == 0 {
        matches[offset1] = i + 1;
        count += 1;
      } else {
        matches[offset1] = 0;
        map.insert(key, -1);
        count -= 1;
      }
    }

    if count == 0 {
      return None;
    }

    // largest increasing subsequence on unique elements
    let mut sequence = vec![0_usize; count];
    let mut last_element = vec![0_usize; count];
    let mut predecessor = vec![None; count1];

    let mut length = 0;
    for (i, &matched) in matches.iter().enumerate() {
      if matched == 0 {
        continue;
      }

      let j = binary_search(&sequence, matched, length);
      if j == length || matched < sequence[j] {
        sequence[j] = matched;
        last_element[j] = i;
        predecessor[i] = j.checked_sub(1).map(|previous| last_element[previous]);
        if j == length {
          length += 1;
        }
      }
    }

    let mut result = [vec![0; length], vec![0; length]];

    let mut i = length;
    let mut current = Some(last_element[length - 1]);
    while let Some(index) = current {
      i -= 1;
      result[0][i] = index;
      result[1][i] = matches[index] - 1;
      current = predecessor[index];
    }

    Some(result)
  }
}

/// The insertion point of `value` in `sequence[..length]`; the matched offsets are unique, so it is never found there
fn binary_search(sequence: &[usize], value: usize, length: usize) -> usize {
  match sequence[..length].binary_search(&value) {
    Ok(index) | Err(index) => index,
  }
}

#[expect(clippy::cast_possible_wrap, reason = "a sequence is shorter than isize::MAX")]
const fn signed(value: usize) -> isize {
  value as isize
}
