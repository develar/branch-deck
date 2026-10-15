use crate::zoom::{min_window_width, stored_zoom};

#[test]
fn the_stored_zoom_is_the_app_settings_zoom() {
  assert!((stored_zoom(r##"{"appSettings":{"primaryColor":"#000","zoom":1.25}}"##) - 1.25).abs() < f64::EPSILON);
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
