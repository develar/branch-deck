use crate::calendar::Calendar;

/// 2026-10-23 10:00 UTC, a Friday
const FRIDAY_10_UTC: u32 = 1_792_749_600;
const HOUR: u32 = 3_600;
const DAY: u32 = 86_400;

#[test]
fn weekday_and_hour_are_local() {
  let berlin = Calendar::named("Europe/Berlin").unwrap();
  // CEST, UTC+2
  assert_eq!(berlin.weekday_hour(FRIDAY_10_UTC), (4, 12));
  assert_eq!(Calendar::utc().weekday_hour(FRIDAY_10_UTC), (4, 10));
  assert_eq!(berlin.name(), "Europe/Berlin");
}

#[test]
fn the_hour_follows_the_daylight_saving_change() {
  let berlin = Calendar::named("Europe/Berlin").unwrap();
  // Monday 2026-10-26 10:00 UTC: CET since the 25th, UTC+1
  let monday = FRIDAY_10_UTC + 3 * DAY;
  assert_eq!(berlin.weekday_hour(monday), (0, 11));
  assert!(berlin.is_workday(monday));
  assert!(!berlin.is_workday(FRIDAY_10_UTC + DAY));
}

#[test]
fn a_week_starts_on_the_local_monday() {
  let berlin = Calendar::named("Europe/Berlin").unwrap();
  // Monday 2026-10-19 00:00 CEST is Sunday 22:00 UTC
  assert_eq!(berlin.week_start(FRIDAY_10_UTC), FRIDAY_10_UTC - 4 * DAY - 12 * HOUR);
  // the week after the change starts at 00:00 CET, 23:00 UTC
  let next_week = FRIDAY_10_UTC + 7 * DAY;
  assert_eq!(berlin.week_start(next_week), FRIDAY_10_UTC + 3 * DAY - 11 * HOUR);
  assert_eq!(Calendar::utc().week_start(FRIDAY_10_UTC), FRIDAY_10_UTC - 4 * DAY - 10 * HOUR);
}
