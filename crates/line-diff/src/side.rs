// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/util/Side.kt` of IntelliJ Platform
// (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors): only what the comparison uses.

//! One of the two compared sides.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
  Left,
  Right,
}

impl Side {
  pub const fn from_left(is_left: bool) -> Self {
    if is_left { Self::Left } else { Self::Right }
  }

  #[must_use]
  pub const fn other(self) -> Self {
    match self {
      Self::Left => Self::Right,
      Self::Right => Self::Left,
    }
  }

  /// `left` for the left side, `right` for the right one
  pub fn select<T>(self, left: T, right: T) -> T {
    match self {
      Self::Left => left,
      Self::Right => right,
    }
  }
}
