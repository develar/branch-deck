//! RFC 3339 timestamps of the services Branch Deck reads (GitHub, Patronus), as Unix seconds, and the UTC days of the
//! archive refs (`<prefix>/archived/YYYY-MM-DD/<name>`)

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::Offset;

const SECONDS_PER_DAY: i64 = 86_400;

/// Unix seconds of an RFC 3339 timestamp: `2026-10-14T09:30:00Z`, `2026-10-14T09:30:00.123456Z`,
/// `2026-10-14T11:30:00.5+02:00` (fractional seconds are dropped); None for another format, an invalid date or time,
/// or a time before 1970
pub fn parse_rfc3339_timestamp(value: &str) -> Option<u32> {
  u32::try_from(value.parse::<Timestamp>().ok()?.as_second()).ok()
}

/// The current time in Unix seconds; 0 if the clock is before 1970 or after 2106
pub fn unix_now() -> u32 {
  u32::try_from(Timestamp::now().as_second()).unwrap_or(0)
}

/// `2026-10-14T09:30:00Z` of Unix seconds
pub fn format_utc_timestamp(unix_seconds: u32) -> String {
  Timestamp::from_second(i64::from(unix_seconds)).map_or_else(|_| String::new(), |timestamp| timestamp.to_string())
}

/// `2026-10-14`, the UTC day of Unix seconds
pub fn format_utc_date(unix_seconds: i64) -> String {
  Timestamp::from_second(unix_seconds).map_or_else(|_| String::new(), |timestamp| Offset::UTC.to_datetime(timestamp).date().to_string())
}

/// `2026-10-14`, today in UTC
pub fn utc_today() -> String {
  format_utc_date(Timestamp::now().as_second())
}

/// Unix seconds of the start of the UTC day `YYYY-MM-DD`; None for another format or an invalid date
pub fn parse_utc_date(date: &str) -> Option<i64> {
  if date.len() != 10 {
    return None;
  }
  let date = date.parse::<Date>().ok()?;
  Some(Offset::UTC.to_timestamp(date.to_datetime(jiff::civil::Time::midnight())).ok()?.as_second())
}

/// Unix seconds `days` days before now
pub fn days_ago(days: i64) -> i64 {
  Timestamp::now().as_second().saturating_sub(days.saturating_mul(SECONDS_PER_DAY))
}

#[cfg(test)]
mod tests {
  use super::{format_utc_date, format_utc_timestamp, parse_rfc3339_timestamp, parse_utc_date};

  #[test]
  fn parses_utc_and_offset_timestamps() {
    assert_eq!(parse_rfc3339_timestamp("2026-10-14T09:30:00Z"), Some(1_791_970_200));
    assert_eq!(parse_rfc3339_timestamp("2026-10-14T09:30:00.123456Z"), Some(1_791_970_200));
    assert_eq!(parse_rfc3339_timestamp("2026-10-14T11:30:00.5+02:00"), Some(1_791_970_200));
    assert_eq!(parse_rfc3339_timestamp("2026-10-14T04:00:00-05:30"), Some(1_791_970_200));
    assert_eq!(parse_rfc3339_timestamp("2026-10-14t09:30:00z"), Some(1_791_970_200));
    assert_eq!(parse_rfc3339_timestamp("2026-10-14 09:30:00"), None);
    assert_eq!(parse_rfc3339_timestamp("1969-12-31T23:59:59Z"), None);
  }

  #[test]
  fn rejects_invalid_dates_and_times() {
    assert_eq!(parse_rfc3339_timestamp("2026-13-14T09:30:00Z"), None);
    assert_eq!(parse_rfc3339_timestamp("2026-02-30T09:30:00Z"), None);
    assert_eq!(parse_rfc3339_timestamp("2026-10-14T25:30:00Z"), None);
    assert_eq!(parse_rfc3339_timestamp("2026-1-5T1:2:3Z"), None);
    assert_eq!(parse_rfc3339_timestamp(""), None);
  }

  #[test]
  fn formats_what_it_parses() {
    assert_eq!(format_utc_timestamp(1_791_970_200), "2026-10-14T09:30:00Z");
    assert_eq!(format_utc_timestamp(0), "1970-01-01T00:00:00Z");
    for seconds in [951_782_400, 1_709_164_800, 4_102_444_799] {
      assert_eq!(parse_rfc3339_timestamp(&format_utc_timestamp(seconds)), Some(seconds));
    }
  }

  #[test]
  fn utc_days() {
    assert_eq!(format_utc_date(1_791_970_200), "2026-10-14");
    assert_eq!(format_utc_date(0), "1970-01-01");
    assert_eq!(parse_utc_date("2026-10-14"), Some(1_791_936_000));
    assert_eq!(parse_utc_date("1970-01-01"), Some(0));
    assert_eq!(parse_utc_date("2026-10-32"), None);
    assert_eq!(parse_utc_date("2026-1-5"), None);
    assert_eq!(parse_utc_date("20261014"), None);
    assert_eq!(parse_utc_date("latest"), None);
  }
}
