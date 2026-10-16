//! CI output for a prompt: without terminal colours, cut to a limit (each cut reported, so the prompt can say so).

/// `text` without its ANSI escape sequences (TeamCity keeps the colours of a test's output in its message)
pub fn strip_ansi(text: &str) -> String {
  let mut result = String::with_capacity(text.len());
  let mut chars = text.chars();
  while let Some(c) = chars.next() {
    if c != '\u{1b}' {
      result.push(c);
      continue;
    }
    // CSI: `ESC [`, parameters, then a final byte in `@`..`~`; any other escape drops the next character only
    if chars.next() == Some('[') {
      for c in chars.by_ref() {
        if ('@'..='~').contains(&c) {
          break;
        }
      }
    }
  }
  result
}

/// The first `max_lines` lines of `text`, at most `max_bytes` (cut at a character), and whether anything was cut
pub fn cut_head(text: &str, max_lines: usize, max_bytes: usize) -> (String, bool) {
  let mut end = text.len();
  if let Some((index, _)) = text.match_indices('\n').nth(max_lines.saturating_sub(1)) {
    end = index;
  }
  let end = text.floor_char_boundary(end.min(max_bytes));
  let cut = end < text.trim_end().len();
  (text[..end].trim_end().to_string(), cut)
}

/// The last `max_bytes` of `text` from a line start, and whether anything was cut
pub fn cut_tail(text: &str, max_bytes: usize) -> (String, bool) {
  let text = text.trim_end();
  if text.len() <= max_bytes {
    return (text.to_string(), false);
  }
  let start = text.ceil_char_boundary(text.len() - max_bytes);
  // a whole first line, unless one line is longer than the limit
  let start = if text.as_bytes()[start - 1] == b'\n' {
    start
  } else {
    text[start..].find('\n').map_or(start, |index| start + index + 1)
  };
  (text[start..].to_string(), true)
}
