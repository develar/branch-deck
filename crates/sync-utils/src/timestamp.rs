//! RFC 3339 timestamps of the services Branch Deck reads (GitHub, Patronus), as Unix seconds.

/// Unix seconds of an RFC 3339 timestamp: `2026-10-14T09:30:00Z`, `2026-10-14T09:30:00.123456Z`,
/// `2026-10-14T11:30:00.5+02:00` (fractional seconds are dropped); None for another format or a time before 1970
pub fn parse_rfc3339_timestamp(value: &str) -> Option<u32> {
  let (date, time) = value.split_once('T')?;
  let (time, offset_seconds) = split_offset(time)?;
  let mut date_parts = date.splitn(3, '-').map(str::parse::<i64>);
  let (year, month, day) = (date_parts.next()?.ok()?, date_parts.next()?.ok()?, date_parts.next()?.ok()?);
  let time = time.split_once('.').map_or(time, |(whole, _)| whole);
  let mut time_parts = time.splitn(3, ':').map(str::parse::<i64>);
  let (hour, minute, second) = (time_parts.next()?.ok()?, time_parts.next()?.ok()?, time_parts.next()?.ok()?);
  u32::try_from(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second - offset_seconds).ok()
}

/// The time and the offset from UTC in seconds: `Z`, `+02:00`, `-05:30`
fn split_offset(time: &str) -> Option<(&str, i64)> {
  if let Some(time) = time.strip_suffix('Z') {
    return Some((time, 0));
  }
  let sign_index = time.rfind(['+', '-'])?;
  let (time, offset) = time.split_at(sign_index);
  let sign = if offset.starts_with('-') { -1 } else { 1 };
  let (hours, minutes) = offset[1..].split_once(':')?;
  Some((time, sign * (hours.parse::<i64>().ok()? * 3_600 + minutes.parse::<i64>().ok()? * 60)))
}

/// The current time in Unix seconds; 0 if the clock is before 1970 or after 2106
pub fn unix_now() -> u32 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .ok()
    .and_then(|duration| u32::try_from(duration.as_secs()).ok())
    .unwrap_or(0)
}

/// `2026-10-14T09:30:00Z` of Unix seconds
pub fn format_utc_timestamp(unix_seconds: u32) -> String {
  let seconds = i64::from(unix_seconds);
  let (year, month, day) = civil_from_days(seconds.div_euclid(86_400));
  let time = seconds.rem_euclid(86_400);
  format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", time / 3_600, time % 3_600 / 60, time % 60)
}

/// Days since 1970-01-01 of a civil date (Howard Hinnant's algorithm)
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
  let shifted_year = if month <= 2 { year - 1 } else { year };
  let era = shifted_year.div_euclid(400);
  let year_of_era = shifted_year - era * 400;
  let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
  let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
  era * 146_097 + day_of_era - 719_468
}

/// The civil date of days since 1970-01-01, the inverse of `days_from_civil`
fn civil_from_days(days: i64) -> (i64, i64, i64) {
  let days = days + 719_468;
  let era = days.div_euclid(146_097);
  let day_of_era = days - era * 146_097;
  let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
  let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
  let shifted_month = (5 * day_of_year + 2) / 153;
  let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
  let month = if shifted_month < 10 { shifted_month + 3 } else { shifted_month - 9 };
  (year_of_era + era * 400 + i64::from(month <= 2), month, day)
}

#[cfg(test)]
mod tests {
  use super::{format_utc_timestamp, parse_rfc3339_timestamp};

  #[test]
  fn parses_utc_and_offset_timestamps() {
    assert_eq!(parse_rfc3339_timestamp("2026-10-14T09:30:00Z"), Some(1_791_970_200));
    assert_eq!(parse_rfc3339_timestamp("2026-10-14T09:30:00.123456Z"), Some(1_791_970_200));
    assert_eq!(parse_rfc3339_timestamp("2026-10-14T11:30:00.5+02:00"), Some(1_791_970_200));
    assert_eq!(parse_rfc3339_timestamp("2026-10-14T04:00:00-05:30"), Some(1_791_970_200));
    assert_eq!(parse_rfc3339_timestamp("2026-10-14 09:30:00"), None);
    assert_eq!(parse_rfc3339_timestamp("1969-12-31T23:59:59Z"), None);
  }

  #[test]
  fn formats_what_it_parses() {
    assert_eq!(format_utc_timestamp(1_791_970_200), "2026-10-14T09:30:00Z");
    assert_eq!(format_utc_timestamp(0), "1970-01-01T00:00:00Z");
    for seconds in [951_782_400, 1_709_164_800, 4_102_444_799] {
      assert_eq!(parse_rfc3339_timestamp(&format_utc_timestamp(seconds)), Some(seconds));
    }
  }
}
