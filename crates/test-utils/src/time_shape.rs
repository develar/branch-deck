//! A text that holds a measured time, compared by its shape: a test never asserts the number of a time the code measured
//! (AGENTS.md, Verification: no wall-clock limit an assertion depends on), only that a duration stands where the
//! pattern names [`TIME`]. A duration is one that the lines of the tools write: `0 s`, `45 s`, `4 min`, `2 h` (a wait,
//! `lease_pool::host_runs`), or `3.1s`, `9m02s`, `1h05m09s` (a step, `host_client::phases::duration_text`)

/// The place of one duration in a pattern
pub const TIME: &str = "<time>";

/// Whether `text` is `pattern`, each [`TIME`] of the pattern being one duration of `text`
#[must_use]
pub fn is_shape(text: &str, pattern: &str) -> bool {
  shape_end(text, pattern) == Some(text.len())
}

/// Whether `text` starts with `pattern`, each [`TIME`] of the pattern being one duration of `text`
#[must_use]
pub fn starts_with_shape(text: &str, pattern: &str) -> bool {
  shape_end(text, pattern).is_some()
}

/// Whether `text` holds `pattern` somewhere, each [`TIME`] of the pattern being one duration of `text`
#[must_use]
pub fn contains_shape(text: &str, pattern: &str) -> bool {
  text.char_indices().any(|(at, _)| shape_end(&text[at..], pattern).is_some())
}

/// The end in `text` of `pattern` matched from the start of `text`
fn shape_end(text: &str, pattern: &str) -> Option<usize> {
  let mut pieces = pattern.split(TIME);
  let first = pieces.next().unwrap_or_default();
  if !text.starts_with(first) {
    return None;
  }
  let mut at = first.len();
  for piece in pieces {
    at += duration_len(&text[at..])?;
    if !text[at..].starts_with(piece) {
      return None;
    }
    at += piece.len();
  }
  Some(at)
}

/// The length of the duration that `text` starts with
fn duration_len(text: &str) -> Option<usize> {
  let number = number_len(text)?;
  for unit in [" s", " min", " h"] {
    if text[number..].starts_with(unit) {
      return Some(number + unit.len());
    }
  }
  // the compact form: units of hours and minutes, then seconds
  let mut at = 0;
  loop {
    let number = number_len(&text[at..])?;
    match text.as_bytes().get(at + number) {
      Some(b's') => return Some(at + number + 1),
      Some(b'h' | b'm') => at += number + 1,
      _ => return None,
    }
  }
}

/// The length of the number that `text` starts with: digits, then maybe a point and digits
fn number_len(text: &str) -> Option<usize> {
  let digits = |from: usize| text.as_bytes()[from..].iter().take_while(|byte| byte.is_ascii_digit()).count();
  let whole = digits(0);
  if whole == 0 {
    return None;
  }
  if text.as_bytes().get(whole) == Some(&b'.') {
    let tenths = digits(whole + 1);
    if tenths > 0 {
      return Some(whole + 1 + tenths);
    }
  }
  Some(whole)
}
