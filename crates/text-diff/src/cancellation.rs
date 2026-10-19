// Ported and modified from IntelliJ Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and
// contributors): `community/platform/util/diff/src/com/intellij/diff/comparison/CancellationChecker.kt` and
// `DiffTooBigException.kt`. A time budget replaces the progress indicator.

//! The time budget of a comparison, and the error that ends it early.

use std::fmt::{Display, Formatter};
use std::time::{Duration, Instant};

/// Why a comparison gave no result
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffTooBig {
  /// The texts differ in too many places for an exact answer (`DiffTooBigException`). A caller can still try a
  /// smaller part of the texts.
  TooManyChanges,
  /// The time budget ran out (the `ProcessCanceledException` of a canceled indicator). The whole comparison stops.
  DeadlineExceeded,
}

impl Display for DiffTooBig {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
    formatter.write_str(match self {
      Self::TooManyChanges => "the texts differ in too many places",
      Self::DeadlineExceeded => "the comparison ran out of time",
    })
  }
}

impl std::error::Error for DiffTooBig {}

/// The time by which a comparison must end (the `CancellationChecker` of the source)
#[derive(Clone, Copy, Debug)]
pub struct Deadline {
  end: Option<Instant>,
}

impl Deadline {
  /// No time limit (`CancellationChecker.EMPTY`)
  pub const NONE: Self = Self { end: None };

  /// Ends `budget` from now
  pub fn after(budget: Duration) -> Self {
    Self {
      end: Instant::now().checked_add(budget),
    }
  }

  /// Fails with `DeadlineExceeded` once the time is over
  pub fn check_canceled(&self) -> Result<(), DiffTooBig> {
    match self.end {
      Some(end) if Instant::now() >= end => Err(DiffTooBig::DeadlineExceeded),
      _ => Ok(()),
    }
  }
}
