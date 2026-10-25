use crate::size::{format_size, parse_size};

/// A size is a number and a binary unit, as `df -h` counts
#[test]
fn a_size_is_a_number_and_a_binary_unit() {
  for (text, bytes) in [
    ("0", 0),
    ("512", 512),
    ("200G", 200 << 30),
    ("200GiB", 200 << 30),
    ("50M", 50 << 20),
    ("1.5G", u64::MAX),
    ("3T", 3 << 40),
    ("1P", 1 << 50),
    ("10KB", 10 << 10),
  ] {
    assert_eq!(parse_size(text).unwrap_or(u64::MAX), bytes, "{text}");
  }
  for text in ["", "G", "200X", "-1G"] {
    assert!(parse_size(text).is_err(), "{text}");
  }
}

/// A message names a size in the largest binary unit it fills
#[test]
fn a_message_names_a_size_in_the_unit_it_fills() {
  for (bytes, text) in [(0, "0 B"), (1023, "1023 B"), (1024, "1.0 KiB"), (13_230_867, "12.6 MiB"), (200 << 30, "200.0 GiB")] {
    assert_eq!(format_size(bytes), text, "{bytes}");
  }
}
