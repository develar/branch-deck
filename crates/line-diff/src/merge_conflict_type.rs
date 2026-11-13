// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/util/MergeConflictType.kt` and
// `MergeConflictResolutionStrategy.kt` of IntelliJ Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o.
// and contributors).

//! What a block of a three-way merge is: an insertion, a deletion, a modification or a conflict, which sides changed,
//! and how it can be resolved.

use crate::merge_range::ThreeSide;
use crate::side::Side;

/// The kind of a merge block (`MergeConflictType.Type`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MergeConflictKind {
  Inserted,
  Deleted,
  Modified,
  Conflict,
}

/// How a block is resolved (`MergeConflictResolutionStrategy`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResolutionStrategy {
  /// Only when the block is no conflict: take the side that changed
  Default,
  /// The changes do not overlap at the word level (`ComparisonMergeUtil::try_resolve_conflict`)
  Text,
  /// The structure of the file resolves the conflict (`LangSpecificMergeConflictResolver`, not ported)
  Semantic,
}

/// The kind of a merge block, the sides that changed it and its resolution
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MergeConflictType {
  pub kind: MergeConflictKind,
  left_change: bool,
  right_change: bool,
  /// `None`: the block cannot be resolved without the user
  pub resolution: Option<ResolutionStrategy>,
}

impl MergeConflictType {
  /// A block the default strategy resolves
  pub const fn new(kind: MergeConflictKind, left_change: bool, right_change: bool) -> Self {
    Self::with_resolution(kind, left_change, right_change, Some(ResolutionStrategy::Default))
  }

  pub const fn with_resolution(kind: MergeConflictKind, left_change: bool, right_change: bool, resolution: Option<ResolutionStrategy>) -> Self {
    Self {
      kind,
      left_change,
      right_change,
      resolution,
    }
  }

  pub const fn can_be_resolved(&self) -> bool {
    self.resolution.is_some()
  }

  /// Whether `side` changed the block
  pub const fn is_change(&self, side: Side) -> bool {
    match side {
      Side::Left => self.left_change,
      Side::Right => self.right_change,
    }
  }

  /// Whether `side` changed the block; the base always counts as changed
  pub const fn is_change_of(&self, side: ThreeSide) -> bool {
    match side {
      ThreeSide::Left => self.left_change,
      ThreeSide::Base => true,
      ThreeSide::Right => self.right_change,
    }
  }
}
