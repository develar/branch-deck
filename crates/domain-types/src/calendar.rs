//! The local calendar of the statistics: the weekday and hour of a moment and the start of its week, in a time zone
//! with its daylight saving changes. A value: the host reads the system zone (`jiff::tz::TimeZone::system()`, the app
//! and `bd`), the E2E server and the tests name one, so the rules that use it read no system state.

use jiff::Timestamp;
use jiff::civil::Weekday;
use jiff::tz::TimeZone;

#[derive(Clone, Debug)]
pub struct Calendar {
  zone: TimeZone,
}

impl Calendar {
  pub const fn utc() -> Self {
    Self { zone: TimeZone::UTC }
  }

  pub const fn from_zone(zone: TimeZone) -> Self {
    Self { zone }
  }

  /// An IANA zone, e.g. `Europe/Berlin`; None for a name the bundled database does not know
  pub fn named(name: &str) -> Option<Self> {
    TimeZone::get(name).ok().map(Self::from_zone)
  }

  /// The IANA name of the zone, e.g. `Europe/Berlin`; `UTC` for a zone without one
  pub fn name(&self) -> String {
    self.zone.iana_name().unwrap_or("UTC").to_string()
  }

  /// The local weekday (0 = Monday) and hour (0–23) of Unix seconds
  pub fn weekday_hour(&self, unix_seconds: u32) -> (u8, u8) {
    let local = self.zone.to_datetime(timestamp(unix_seconds));
    (local.weekday().to_monday_zero_offset().cast_unsigned(), local.hour().cast_unsigned())
  }

  /// Monday to Friday, local
  pub fn is_workday(&self, unix_seconds: u32) -> bool {
    !matches!(self.zone.to_datetime(timestamp(unix_seconds)).weekday(), Weekday::Saturday | Weekday::Sunday)
  }

  /// Unix seconds of the local Monday 00:00 of the week of `unix_seconds`
  pub fn week_start(&self, unix_seconds: u32) -> u32 {
    let local = self.zone.to_datetime(timestamp(unix_seconds));
    let monday = local
      .date()
      .checked_sub(jiff::Span::new().days(local.weekday().to_monday_zero_offset()))
      .unwrap_or_else(|_| local.date());
    monday
      .to_zoned(self.zone.clone())
      .ok()
      .and_then(|zoned| u32::try_from(zoned.timestamp().as_second()).ok())
      .unwrap_or(unix_seconds)
  }
}

fn timestamp(unix_seconds: u32) -> Timestamp {
  Timestamp::from_second(i64::from(unix_seconds)).unwrap_or(Timestamp::UNIX_EPOCH)
}

#[cfg(test)]
#[path = "calendar_test.rs"]
mod calendar_test;
