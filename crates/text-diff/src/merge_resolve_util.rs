// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/comparison/MergeResolveUtil.kt` of
// IntelliJ Platform (Apache License 2.0, Copyright 2000-2026 JetBrains s.r.o. and contributors). A `Vec<u16>` replaces
// the `MergingCharSequence` of the inserted parts.

//! The "magic" resolve of a conflict: a merged text when the changes of the two sides do not overlap at the word level.

use crate::by_word;
use crate::cancellation::{Deadline, DiffTooBig};
use crate::comparison_util::is_equal_texts;
use crate::fragments::DiffFragment;
use crate::merge_conflict_type::{MergeConflictKind, MergeConflictType};
use crate::merge_range::{MergeRange, ThreeSide};
use crate::merge_range_util::{compare_word_merge_contents, get_word_merge_type};
use crate::policy::ComparisonPolicy;
use crate::side::Side;

/// The merged text of a conflict by its word blocks, or `None`: under the default policy first, then ignoring
/// whitespace. A word block that both sides changed differently fails the resolve.
pub fn try_resolve(left_text: &[u16], base_text: &[u16], right_text: &[u16], deadline: &Deadline) -> Option<Vec<u16>> {
  let resolve = || -> Result<Option<Vec<u16>>, DiffTooBig> {
    if let Some(resolved) = SimpleHelper::new(left_text, base_text, right_text).execute(ComparisonPolicy::Default, deadline)? {
      return Ok(Some(resolved));
    }
    SimpleHelper::new(left_text, base_text, right_text).execute(ComparisonPolicy::IgnoreWhitespaces, deadline)
  };
  resolve().ok().flatten()
}

/// The merged text of a conflict, more eager than `try_resolve`, or `None`.
///
/// Here we assume that resolve results are explicitly verified by the user and can be safely undone. Thus we trade
/// higher chances of an incorrect resolve for higher chances of a correct one.
///
/// "A-X-B" and "B-X-A" conflicts must produce equal results. So insertion-insertion conflicts can't be resolved (if
/// the inserted fragments differ): the right order of the inserted chunks is unknown, and sorting them alphabetically
/// or by length makes no sense. Deleted-inserted conflicts are resolved by applying both of them. Deleted-deleted
/// conflicts are resolved by merging the deleted intervals. Modifications are an insertion and a deletion.
pub fn try_greedy_resolve(left_text: &[u16], base_text: &[u16], right_text: &[u16], deadline: &Deadline) -> Option<Vec<u16>> {
  let resolve = || -> Result<Option<Vec<u16>>, DiffTooBig> {
    if let Some(resolved) = GreedyHelper::new(left_text, base_text, right_text).execute(ComparisonPolicy::Default, deadline)? {
      return Ok(Some(resolved));
    }
    GreedyHelper::new(left_text, base_text, right_text).execute(ComparisonPolicy::IgnoreWhitespaces, deadline)
  };
  resolve().ok().flatten()
}

struct SimpleHelper<'a> {
  texts: [&'a [u16]; 3],
  new_content: Vec<u16>,
  last1: usize,
  last2: usize,
  last3: usize,
}

impl<'a> SimpleHelper<'a> {
  const fn new(left_text: &'a [u16], base_text: &'a [u16], right_text: &'a [u16]) -> Self {
    Self {
      texts: [left_text, base_text, right_text],
      new_content: Vec::new(),
      last1: 0,
      last2: 0,
      last3: 0,
    }
  }

  fn execute(mut self, policy: ComparisonPolicy, deadline: &Deadline) -> Result<Option<Vec<u16>>, DiffTooBig> {
    let [left_text, base_text, right_text] = self.texts;
    let changes = by_word::compare_three(left_text, base_text, right_text, policy, deadline)?;

    for fragment in changes {
      let base_range = self.next_merge_range(fragment.start1, fragment.start2, fragment.start3);
      self.append_base(base_range);

      let conflict_range = self.next_merge_range(fragment.end1, fragment.end2, fragment.end3);
      if !self.append_conflict(conflict_range, policy) {
        return Ok(None);
      }
    }

    let trailing_range = self.next_merge_range(left_text.len(), base_text.len(), right_text.len());
    self.append_base(trailing_range);

    Ok(Some(self.new_content))
  }

  const fn next_merge_range(&mut self, end1: usize, end2: usize, end3: usize) -> MergeRange {
    let range = MergeRange::new(self.last1, end1, self.last2, end2, self.last3, end3);
    self.last1 = end1;
    self.last2 = end2;
    self.last3 = end3;
    range
  }

  fn append_base(&mut self, range: MergeRange) {
    if range.is_empty() {
      return;
    }

    let policy = ComparisonPolicy::Default;
    if self.is_unchanged_range(&range, policy) {
      self.append(&range, ThreeSide::Base);
    } else {
      let conflict = self.conflict_type(&range, policy);
      if conflict.is_change(Side::Left) {
        self.append(&range, ThreeSide::Left);
      } else if conflict.is_change(Side::Right) {
        self.append(&range, ThreeSide::Right);
      } else {
        self.append(&range, ThreeSide::Base);
      }
    }
  }

  fn append_conflict(&mut self, range: MergeRange, policy: ComparisonPolicy) -> bool {
    let conflict = self.conflict_type(&range, policy);
    if conflict.kind == MergeConflictKind::Conflict {
      return false;
    }

    if conflict.is_change(Side::Left) {
      self.append(&range, ThreeSide::Left);
    } else {
      self.append(&range, ThreeSide::Right);
    }
    true
  }

  fn append(&mut self, range: &MergeRange, side: ThreeSide) {
    let text = self.texts[side.index()];
    self.new_content.extend_from_slice(&text[range.start(side)..range.end(side)]);
  }

  fn conflict_type(&self, range: &MergeRange, policy: ComparisonPolicy) -> MergeConflictType {
    get_word_merge_type(range, self.texts, policy)
  }

  fn is_unchanged_range(&self, range: &MergeRange, policy: ComparisonPolicy) -> bool {
    compare_word_merge_contents(range, self.texts, policy, ThreeSide::Base, ThreeSide::Left)
      && compare_word_merge_contents(range, self.texts, policy, ThreeSide::Base, ThreeSide::Right)
  }
}

struct GreedyHelper<'a> {
  left_text: &'a [u16],
  base_text: &'a [u16],
  right_text: &'a [u16],
  new_content: Vec<u16>,
  last_base_offset: usize,
  index1: usize,
  index2: usize,
}

impl<'a> GreedyHelper<'a> {
  const fn new(left_text: &'a [u16], base_text: &'a [u16], right_text: &'a [u16]) -> Self {
    Self {
      left_text,
      base_text,
      right_text,
      new_content: Vec::new(),
      last_base_offset: 0,
      index1: 0,
      index2: 0,
    }
  }

  fn execute(mut self, policy: ComparisonPolicy, deadline: &Deadline) -> Result<Option<Vec<u16>>, DiffTooBig> {
    let fragments1 = by_word::compare(self.base_text, self.left_text, policy, deadline)?;
    let fragments2 = by_word::compare(self.base_text, self.right_text, policy, deadline)?;

    loop {
      let change_start1 = fragments1.get(self.index1).map(|fragment| fragment.start_offset1);
      let change_start2 = fragments2.get(self.index2).map(|fragment| fragment.start_offset1);

      // skip till the next block of changes
      match (change_start1, change_start2) {
        (None, None) => {
          // no more changes left
          self.append_base(self.base_text.len());
          break;
        }
        (Some(start1), Some(start2)) => self.append_base(start1.min(start2)),
        (Some(start), None) | (None, Some(start)) => self.append_base(start),
      }

      // collect the next block of changes that intersect one another
      let mut base_offset_end = self.last_base_offset;
      let mut end1 = self.index1;
      let mut end2 = self.index2;
      loop {
        if let Some(next1) = fragments1.get(end1)
          && next1.start_offset1 <= base_offset_end
        {
          base_offset_end = base_offset_end.max(next1.end_offset1);
          end1 += 1;
          continue;
        }
        if let Some(next2) = fragments2.get(end2)
          && next2.start_offset1 <= base_offset_end
        {
          base_offset_end = base_offset_end.max(next2.end_offset1);
          end2 += 1;
          continue;
        }
        break;
      }
      assert!(self.index1 != end1 || self.index2 != end2);

      let inserted1 = inserted_content(self.left_text, &fragments1[self.index1..end1]);
      let inserted2 = inserted_content(self.right_text, &fragments2[self.index2..end2]);
      self.index1 = end1;
      self.index2 = end2;

      // merge and apply the deletions
      self.last_base_offset = base_offset_end;

      // merge and apply the insertions that do not conflict
      if inserted1.is_empty() && inserted2.is_empty() {
        continue;
      }
      if inserted2.is_empty() {
        self.new_content.extend_from_slice(&inserted1);
        continue;
      }
      if inserted1.is_empty() {
        self.new_content.extend_from_slice(&inserted2);
        continue;
      }
      if is_equal_texts(&inserted1, &inserted2, policy) {
        let inserted = if inserted1.len() <= inserted2.len() { inserted1 } else { inserted2 };
        self.new_content.extend_from_slice(&inserted);
        continue;
      }

      // we faced conflicting insertions: the resolve failed
      return Ok(None);
    }

    Ok(Some(self.new_content))
  }

  fn append_base(&mut self, end_offset: usize) {
    if self.last_base_offset == end_offset {
      return;
    }
    self.new_content.extend_from_slice(&self.base_text[self.last_base_offset..end_offset]);
    self.last_base_offset = end_offset;
  }
}

/// The text that the fragments insert into the side `text`
fn inserted_content(text: &[u16], fragments: &[DiffFragment]) -> Vec<u16> {
  fragments.iter().flat_map(|fragment| &text[fragment.start_offset2..fragment.end_offset2]).copied().collect()
}
