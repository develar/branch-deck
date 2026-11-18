//! A count of CPU cores in tenths (openspec/specs/cli/host/spec.md, "serve: Heavy runs share the cores of the host by
//! their measure"): what a run of brd-host used on average or at its peak, and what a command is expected to use. It
//! reads and writes as a number with one decimal (`7.5`), and stays an integer inside, so the tables that hold it keep
//! `Eq`

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::time::Duration;

/// Cores in tenths: `Cores(75)` is 7.5 cores
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[must_use]
pub struct Cores(pub u32);

impl Cores {
  /// Whole cores
  pub const fn whole(cores: u32) -> Self {
    Self(cores.saturating_mul(10))
  }

  /// The mean cores of `cpu` time over `wall` time, to the nearest tenth; zero for no wall time
  pub fn of(cpu: Duration, wall: Duration) -> Self {
    if wall.is_zero() {
      return Self(0);
    }
    let tenths = (cpu.as_nanos() * 10 + wall.as_nanos() / 2) / wall.as_nanos();
    Self(u32::try_from(tenths).unwrap_or(u32::MAX))
  }

  pub const fn saturating_add(self, other: Self) -> Self {
    Self(self.0.saturating_add(other.0))
  }
}

impl std::iter::Sum for Cores {
  fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
    iter.fold(Self(0), Self::saturating_add)
  }
}

/// `7.5`, `12.0`
impl fmt::Display for Cores {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}.{}", self.0 / 10, self.0 % 10)
  }
}

impl Serialize for Cores {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_f64(f64::from(self.0) / 10.0)
  }
}

impl<'de> Deserialize<'de> for Cores {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let cores = f64::deserialize(deserializer)?;
    if !cores.is_finite() || cores < 0.0 {
      return Err(serde::de::Error::custom(format!("{cores} is no count of cores")));
    }
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a finite count, not negative, clamped below")]
    let tenths = (cores * 10.0).round().min(f64::from(u32::MAX)) as u32;
    Ok(Self(tenths))
  }
}

#[cfg(test)]
#[path = "cores_test.rs"]
mod cores_test;
