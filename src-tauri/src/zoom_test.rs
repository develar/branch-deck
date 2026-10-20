use crate::zoom::{min_window_width, stored_zoom, sub_window_min_size};

#[test]
fn the_stored_zoom_is_the_app_settings_zoom() {
  assert!((stored_zoom(r#"{"appSettings":{"autoSyncOnFocus":false,"zoom":1.25}}"#) - 1.25).abs() < f64::EPSILON);
}

#[test]
fn without_a_valid_stored_zoom_the_zoom_is_100_percent() {
  for store in [
    "{}",
    "",
    "not json",
    r#"{"appSettings":{}}"#,
    r#"{"appSettings":{"zoom":"big"}}"#,
    r#"{"appSettings":{"zoom":0}}"#,
    r#"{"appSettings":{"zoom":-2}}"#,
  ] {
    assert!((stored_zoom(store) - 1.0).abs() < f64::EPSILON, "{store}");
  }
}

#[test]
fn a_zoom_above_125_percent_widens_the_minimum_window() {
  assert!((min_window_width(1.0) - 960.0).abs() < f64::EPSILON);
  assert!((min_window_width(1.25) - 960.0).abs() < f64::EPSILON);
  assert!((min_window_width(1.5) - 1152.0).abs() < f64::EPSILON);
}

#[test]
fn a_zoom_above_125_percent_widens_the_minimum_sub_window() {
  assert_size(sub_window_min_size("branch-review", 1.0), (800.0, 500.0));
  assert_size(sub_window_min_size("commit-review", 1.25), (800.0, 500.0));
  assert_size(sub_window_min_size("branch-review", 1.5), (960.0, 500.0));
}

#[test]
fn the_settings_window_is_smaller_and_widens_above_107_percent() {
  assert_size(sub_window_min_size("settings", 1.0), (600.0, 420.0));
  assert_size(sub_window_min_size("settings", 1.25), (700.0, 420.0));
  assert_size(sub_window_min_size("settings", 1.5), (840.0, 420.0));
  assert_size(sub_window_min_size("activity", 1.0), (440.0, 400.0));
  assert_size(sub_window_min_size("activity", 1.25), (500.0, 400.0));
}

fn assert_size(actual: (f64, f64), expected: (f64, f64)) {
  assert!(
    (actual.0 - expected.0).abs() < f64::EPSILON && (actual.1 - expected.1).abs() < f64::EPSILON,
    "{actual:?} != {expected:?}"
  );
}
