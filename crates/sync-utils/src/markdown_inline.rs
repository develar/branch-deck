//! The inline Markdown of a line of a body as a reader reads it (`inline_text`): the commit preview, the review and the
//! description of a pull request, which GitHub keeps as Markdown (`remote_facts::review_requests`):
//! - an image (`![alt](url)`, also the `<img … alt="…">` GitHub's editor writes for a pasted screenshot) reads as its
//!   alt text, a link (`[text](url)`) as its text: the page links no URL;
//! - strong emphasis (`**text**`, `__text__`) reads as its text without the marks: the opening marks before a word and
//!   after a space, a punctuation mark or the start, the closing ones after a word and before a space, a punctuation
//!   mark or the end; `__name__` around one identifier is a name (Python's `__init__`, which `code_spans` sets as
//!   code), and marks around a path that starts or ends with `/` a glob (`**/generated/**`): both stay as written;
//! - a code span (backticks) and an escaped mark (`\*`, `\[`) stay as written, nothing inside them is read.
//!
//! The body of a pull request Branch Deck writes keeps them as the author wrote them (`commit_body::markdown_blocks`),
//! so GitHub shows the link, the image and the emphasis.

use std::borrow::Cow;

/// `line` as a reader reads it: its images, links and strong emphasis by their text (the module's header)
pub(crate) fn inline_text(line: &str) -> Cow<'_, str> {
  if !has_marks(line) {
    return Cow::Borrowed(line);
  }
  let mut read = String::with_capacity(line.len());
  read_into(line, &mut read);
  Cow::Owned(read)
}

/// Whether `line` may hold a mark `inline_text` reads: most lines hold none and are read without a copy
fn has_marks(line: &str) -> bool {
  line.contains("**") || line.contains("__") || line.contains("](") || line.as_bytes().windows(4).any(|four| four.eq_ignore_ascii_case(b"<img"))
}

fn read_into(line: &str, read: &mut String) {
  let mut at = 0;
  while let Some(next) = line[at..].chars().next() {
    match mark_at(line, at) {
      Some((Text::Read(text), len)) => {
        read_into(text, read);
        at += len;
      }
      Some((Text::Kept(text), len)) => {
        read.push_str(text);
        at += len;
      }
      None => {
        read.push(next);
        at += next.len_utf8();
      }
    }
  }
}

/// What a mark of a line reads as
enum Text<'a> {
  /// Text whose own marks are read in turn: the text of a link, of strong emphasis
  Read(&'a str),
  /// Text as it is: a code span, an escape, the alt text of an `<img>`, marks that open nothing
  Kept(&'a str),
}

/// The mark at `at` of `line` (the module's header): what it reads as and its length; `None` for a character of no mark
fn mark_at(line: &str, at: usize) -> Option<(Text<'_>, usize)> {
  let rest = &line[at..];
  let next = rest.chars().next()?;
  match next {
    '`' => {
      let span = code_span_len(rest);
      Some((Text::Kept(&rest[..span]), span))
    }
    // an escaped mark is no mark: the backslash and what it escapes as written
    '\\' => {
      let len = 1 + rest[1..].chars().next().map_or(0, char::len_utf8);
      Some((Text::Kept(&rest[..len]), len))
    }
    '!' => link_at(&rest[1..]).map(|(alt, len)| (Text::Read(alt), 1 + len)),
    '[' => link_at(rest).map(|(text, len)| (Text::Read(text), len)),
    '<' => img_tag_at(rest).map(|len| (Text::Kept(alt_of(&rest[..len]).unwrap_or_default()), len)),
    '*' | '_' if rest[1..].starts_with(next) => Some(strong_at(line, at, next).map_or_else(
      || {
        let run = rest.len() - rest.trim_start_matches(next).len();
        (Text::Kept(&rest[..run]), run)
      },
      |(inner, end)| (Text::Read(&line[inner]), end - at),
    )),
    _ => None,
  }
}

/// The length of the code span `rest` starts with: its run of backticks up to the next run as long; the run alone when
/// none closes it
pub(crate) fn code_span_len(rest: &str) -> usize {
  let run = rest.len() - rest.trim_start_matches('`').len();
  let mut at = run;
  while let Some(found) = rest[at..].find('`') {
    let start = at + found;
    let len = rest[start..].len() - rest[start..].trim_start_matches('`').len();
    if len == run {
      return start + len;
    }
    at = start + len;
  }
  run
}

/// The text of the link `rest` starts with (`[text](destination)`, the destination with balanced parentheses, a title
/// in it too) and the length of the link; `None` when `rest` starts no link: `[ ]` of a task, `[^1]` of a footnote
fn link_at(rest: &str) -> Option<(&str, usize)> {
  if !rest.starts_with('[') {
    return None;
  }
  let close = matching(rest, '[', ']')?;
  let destination = &rest[close + 1..];
  if !destination.starts_with('(') {
    return None;
  }
  let end = matching(destination, '(', ')')?;
  Some((&rest[1..close], close + 1 + end + 1))
}

/// The place of the `close` that matches the `open` `text` starts with, nested pairs, code spans and escapes skipped
fn matching(text: &str, open: char, close: char) -> Option<usize> {
  let mut depth = 0usize;
  let mut at = 0;
  while let Some(next) = text[at..].chars().next() {
    match next {
      '\\' => {
        at += 1 + text[at + 1..].chars().next().map_or(0, char::len_utf8);
        continue;
      }
      '`' if open == '[' => {
        at += code_span_len(&text[at..]);
        continue;
      }
      _ if next == open => depth += 1,
      _ if next == close => {
        depth -= 1;
        if depth == 0 {
          return Some(at);
        }
      }
      _ => {}
    }
    at += next.len_utf8();
  }
  None
}

/// The length of the `<img …>` tag `text` starts with (any case, up to its `>` outside quotes); `None` for another tag
/// and for one this line does not close
fn img_tag_at(text: &str) -> Option<usize> {
  let name = text.get(..4)?;
  if !name.eq_ignore_ascii_case("<img") || !text[4..].starts_with(|c: char| c.is_whitespace() || c == '/' || c == '>') {
    return None;
  }
  let mut quote = None;
  for (at, c) in text.char_indices().skip(4) {
    match (quote, c) {
      (None, '"' | '\'') => quote = Some(c),
      (Some(open), _) if c == open => quote = None,
      (None, '>') => return Some(at + 1),
      _ => {}
    }
  }
  None
}

/// The value of the `alt` attribute of the tag `tag`, quoted in `"` or `'`
fn alt_of(tag: &str) -> Option<&str> {
  let lower = tag.to_ascii_lowercase();
  let mut from = 0;
  while let Some(found) = lower[from..].find("alt") {
    let start = from + found;
    from = start + 3;
    if !lower[..start].ends_with(char::is_whitespace) {
      continue;
    }
    let rest = tag[from..].trim_start().strip_prefix('=')?.trim_start();
    let quote = rest.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    let value = &rest[1..];
    return value.find(quote).map(|end| &value[..end]);
  }
  None
}

/// The strong emphasis of the pair of `mark` (`*` or `_`) at `at` of `line`: the range of its text and the end of its
/// closing marks (the module's header); `None` when the marks open none, or `__name__` names an identifier
fn strong_at(line: &str, at: usize, mark: char) -> Option<(std::ops::Range<usize>, usize)> {
  let before = line[..at].chars().next_back();
  let inner_start = at + 2;
  let after = line[inner_start..].chars().next()?;
  let opens = before.is_none_or(|c| c.is_whitespace() || (is_punctuation(c) && c != mark && c != '\\')) && !after.is_whitespace() && after != mark;
  if !opens {
    return None;
  }
  let pair = if mark == '*' { "**" } else { "__" };
  let mut from = inner_start;
  while let Some(found) = line[from..].find(pair) {
    let close = from + found;
    // a run of three or more is no closing pair of two; a code span on the way hides its marks
    if let Some(span) = line[from..close].find('`') {
      from += span + code_span_len(&line[from + span..]);
      continue;
    }
    let run_end = close + line[close..].len() - line[close..].trim_start_matches(mark).len();
    if run_end - close != 2 {
      from = run_end;
      continue;
    }
    let last = line[..close].chars().next_back()?;
    let next = line[run_end..].chars().next();
    let closes = !last.is_whitespace() && next.is_none_or(|c| c.is_whitespace() || (is_punctuation(c) && c != mark));
    let inner = &line[inner_start..close];
    if closes && !inner.is_empty() {
      let name = mark == '_' && inner.chars().all(|c| c.is_alphanumeric() || c == '_');
      let glob = inner.starts_with('/') || inner.ends_with('/');
      return (!name && !glob && inner.chars().any(char::is_alphanumeric)).then_some((inner_start..close, run_end));
    }
    from = run_end;
  }
  None
}

fn is_punctuation(c: char) -> bool {
  c.is_ascii_punctuation() || (!c.is_alphanumeric() && !c.is_whitespace())
}
