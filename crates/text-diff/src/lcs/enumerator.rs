// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/util/Enumerator.kt` of IntelliJ Platform
// (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors).

//! Numbers equal objects with equal integers, so the LCS algorithms compare integers.

use rustc_hash::{FxBuildHasher, FxHashMap};
use std::hash::Hash;

/// Gives each distinct object a number, 1 for the first one seen
pub struct Enumerator<'a, T> {
  numbers: FxHashMap<&'a T, u32>,
  next_number: u32,
}

impl<'a, T: Eq + Hash> Enumerator<'a, T> {
  pub fn new(expected_capacity: usize) -> Self {
    Self {
      numbers: FxHashMap::with_capacity_and_hasher(expected_capacity, FxBuildHasher),
      next_number: 1,
    }
  }

  /// The numbers of `objects` without the first `start_shift` and the last `end_cut` of them
  pub fn enumerate(&mut self, objects: &'a [T], start_shift: usize, end_cut: usize) -> Vec<u32> {
    objects[start_shift..objects.len() - end_cut].iter().map(|object| self.enumerate_one(object)).collect()
  }

  fn enumerate_one(&mut self, object: &'a T) -> u32 {
    *self.numbers.entry(object).or_insert_with(|| {
      let number = self.next_number;
      self.next_number += 1;
      number
    })
  }
}
