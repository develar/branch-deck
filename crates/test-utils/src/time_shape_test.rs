use crate::time_shape::{contains_shape, is_shape, starts_with_shape};

/// Each duration the lines of the tools write stands for a time of the pattern, and nothing else does
#[test]
fn a_time_of_the_pattern_is_any_duration_of_the_lines_and_nothing_else() {
  for duration in ["0 s", "119 s", "4 min", "2 h", "0.0s", "3.1s", "9m02s", "1h05m09s"] {
    assert!(is_shape(&format!("waited {duration} for it"), "waited <time> for it"), "{duration}");
  }
  for text in ["waited s for it", "waited 3 cores for it", "waited 4m for it", "waited 3.s for it", "waited 3.1s, for it"] {
    assert!(!is_shape(text, "waited <time> for it"), "{text}");
  }
  assert!(is_shape("exited with 3 after 3.2s on the host", "exited with 3 after <time> on the host"));
  assert!(
    !is_shape("exited with 4 after 3.2s on the host", "exited with 3 after <time> on the host"),
    "the rest is exact"
  );
  assert!(!is_shape("after 3.2s on the host, again", "after <time> on the host"), "the whole text");
  assert!(starts_with_shape("after 3.2s on the host, again", "after <time> on the host"));
  assert!(contains_shape("> run 1 waits 1 s for a measurement: run 2", "waits <time> for a measurement"));
  assert!(!contains_shape("> run 1 waits for a measurement", "waits <time> for a measurement"));
  assert!(is_shape("no time", "no time"), "a pattern without a time is the text");
}
