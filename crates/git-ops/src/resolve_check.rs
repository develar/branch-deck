//! The checks of a resolved file in a conflict round of `bd update` (docs/design/git-operations.md, "Resolve
//! session"). The merge result that bd wrote (`edit`, with zdiff3 markers) is plain text with conflict blocks; the
//! resolution may replace each block with anything, but must keep every byte outside the blocks, because there the
//! merge already holds every change of both sides. A block is recognised only by the labels bd passed to `merge-tree`
//! (the new parent, `<commit>^` and the commit), at any marker length (`conflict-marker-size`), so a line such as a
//! Markdown `=======` outside the blocks is text. Lines of a side's change that the resolution dropped are reported,
//! not refused: the agent shows them to the user. Everything works on bytes; only the reported lines are decoded.

use std::collections::HashSet;

/// Why a resolution is refused
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckFailure {
  /// The resolution changed a line outside the conflict blocks
  OutsideConflict,
  /// Only the line ends outside the blocks changed (CRLF and LF, or the newline at the end of the file)
  LineEnds,
  /// A conflict marker is left where a block was
  UnresolvedMarkers,
}

/// The labels after the markers of one round: what bd passed to `merge-tree --merge-base <base> <onto> <commit>`
#[derive(Debug, Clone, Copy)]
pub struct Labels<'a> {
  /// The new parent
  pub onto: &'a str,
  /// `<commit>^`
  pub base: &'a str,
  pub commit: &'a str,
}

/// At most this many dropped lines are reported per side and file
pub const MAX_REPORTED_LINES: usize = 20;
/// The shortest marker git writes; `conflict-marker-size` makes it longer
const MIN_MARKER: usize = 7;

/// The line without its line end (`\n` or `\r\n`)
fn content_of(line: &[u8]) -> &[u8] {
  let line = line.strip_suffix(b"\n").unwrap_or(line);
  line.strip_suffix(b"\r").unwrap_or(line)
}

/// The length of the marker run of `marker` that starts `line` when the line is that run, a space and `label`; with a
/// rename, merge-ort adds `:<path>` (as it is, not quoted) after the label
fn labelled_marker(line: &[u8], marker: u8, label: &str) -> Option<usize> {
  let line = content_of(line);
  let run = line.iter().take_while(|&&b| b == marker).count();
  let rest = line[run..].strip_prefix(b" ")?.strip_prefix(label.as_bytes())?;
  (run >= MIN_MARKER && (rest.is_empty() || rest.starts_with(b":"))).then_some(run)
}

/// Whether `line` is exactly `size` `=` characters
fn is_separator(line: &[u8], size: usize) -> bool {
  let line = content_of(line);
  line.len() == size && line.iter().all(|&b| b == b'=')
}

/// Whether `line` is a marker line of one of the labels: what a resolution must not leave
fn is_labelled_marker(line: &[u8], labels: Labels<'_>) -> bool {
  labelled_marker(line, b'<', labels.onto).is_some() || labelled_marker(line, b'|', labels.base).is_some() || labelled_marker(line, b'>', labels.commit).is_some()
}

/// The number of conflict blocks left in `content`: its `<` marker lines with the new parent's label
pub fn count_markers(content: &[u8], labels: Labels<'_>) -> usize {
  content
    .split_inclusive(|&b| b == b'\n')
    .filter(|line| labelled_marker(line, b'<', labels.onto).is_some())
    .count()
}

/// Whether `content` has a marker line of one of the labels
pub fn has_markers(content: &[u8], labels: Labels<'_>) -> bool {
  content.split_inclusive(|&b| b == b'\n').any(|line| is_labelled_marker(line, labels))
}

/// Lines with their line ends
type Lines<'a> = Vec<&'a [u8]>;

/// The plain runs of `merged` (one more than the blocks): a block starts with `<` and the new parent's
/// label, has the base section (`|` and `<commit>^`), the separator of the same length, and ends with `>` and the
/// commit's label. Anything else is text. `None` for a block that never ends
fn split_blocks<'a>(merged: &'a [u8], labels: Labels<'_>) -> Option<Vec<Lines<'a>>> {
  enum Part {
    Plain,
    Ours(usize),
    Base(usize),
    Theirs(usize),
  }
  let mut plains = vec![Vec::new()];
  let mut part = Part::Plain;
  for line in merged.split_inclusive(|&b| b == b'\n') {
    part = match part {
      Part::Plain => {
        if let Some(size) = labelled_marker(line, b'<', labels.onto) {
          Part::Ours(size)
        } else {
          plains.last_mut()?.push(line);
          Part::Plain
        }
      }
      Part::Ours(size) if labelled_marker(line, b'|', labels.base) == Some(size) => Part::Base(size),
      Part::Ours(size) => Part::Ours(size),
      Part::Base(size) if is_separator(line, size) => Part::Theirs(size),
      Part::Base(size) => Part::Base(size),
      Part::Theirs(size) if labelled_marker(line, b'>', labels.commit) == Some(size) => {
        plains.push(Vec::new());
        Part::Plain
      }
      Part::Theirs(size) => Part::Theirs(size),
    };
  }
  matches!(part, Part::Plain).then_some(plains)
}

/// The number of conflict blocks of bd's labels in the merge result `merged`; 0 when it has none that bd can read
pub fn block_count(merged: &[u8], labels: Labels<'_>) -> usize {
  split_blocks(merged, labels).map_or(0, |plains| plains.len() - 1)
}

/// The first position at or after `from` where `needle` (lines) starts in `haystack`
fn find_lines(haystack: &[&[u8]], needle: &[&[u8]], from: usize) -> Option<usize> {
  if needle.is_empty() {
    return Some(from);
  }
  (from..=haystack.len().checked_sub(needle.len())?).find(|&start| haystack[start..start + needle.len()] == *needle)
}

/// The regions of `resolved` that replace the blocks of `merged`: the plain runs must be found in order (leftmost, which
/// leaves the most room for the later runs), the first at the start and the last at the end
fn replaced_regions<'a>(plains: &[Lines<'_>], resolved: &'a [u8]) -> Option<Lines<'a>> {
  let lines: Vec<&[u8]> = resolved.split_inclusive(|&b| b == b'\n').collect();
  let (first, last) = (&plains[0], &plains[plains.len() - 1]);
  if !lines.starts_with(first) {
    return None;
  }
  let mut position = first.len();
  let mut replaced: Vec<&[u8]> = Vec::new();
  for plain in &plains[1..plains.len() - 1] {
    let start = find_lines(&lines, plain, position)?;
    replaced.extend_from_slice(&lines[position..start]);
    position = start + plain.len();
  }
  let suffix_start = lines.len().checked_sub(last.len()).filter(|&start| start >= position && lines[start..] == **last)?;
  replaced.extend_from_slice(&lines[position..suffix_start]);
  Some(replaced)
}

/// `content` with LF line ends and without a newline at the end
fn normalized_line_ends(content: &[u8]) -> Vec<u8> {
  let mut normalized = Vec::with_capacity(content.len());
  let mut bytes = content.iter().peekable();
  while let Some(&b) = bytes.next() {
    if b == b'\r' && bytes.peek() == Some(&&b'\n') {
      continue;
    }
    normalized.push(b);
  }
  if normalized.last() == Some(&b'\n') {
    normalized.pop();
  }
  normalized
}

/// Check the resolution `resolved` of the merge result `merged` (the `edit` file as bd wrote it): outside the conflict
/// blocks it must be byte for byte the same, and no marker may be left where a block was
///
/// # Errors
/// `CheckFailure::OutsideConflict` when the plain runs of `merged` are not found in order (`LineEnds` when only the line
/// ends differ), `UnresolvedMarkers` when a replaced region still has a marker line of bd's labels
pub fn check_resolution(merged: &[u8], resolved: &[u8], labels: Labels<'_>) -> Result<(), CheckFailure> {
  let Some(plains) = split_blocks(merged, labels).filter(|plains| plains.len() > 1) else {
    // a round opens only with blocks; anything else must stay as it is
    return if merged == resolved { Ok(()) } else { Err(CheckFailure::OutsideConflict) };
  };
  let Some(replaced) = replaced_regions(&plains, resolved) else {
    let (merged, resolved) = (normalized_line_ends(merged), normalized_line_ends(resolved));
    let same_text = split_blocks(&merged, labels).is_some_and(|plains| replaced_regions(&plains, &resolved).is_some());
    return Err(if same_text { CheckFailure::LineEnds } else { CheckFailure::OutsideConflict });
  };
  if replaced.iter().any(|line| is_labelled_marker(line, labels)) {
    return Err(CheckFailure::UnresolvedMarkers);
  }
  Ok(())
}

/// A line without its trailing whitespace and line end, as the dropped-line report compares lines
fn trim(line: &[u8]) -> &[u8] {
  let end = line.iter().rposition(|b| !b.is_ascii_whitespace()).map_or(0, |i| i + 1);
  &line[..end]
}

/// The lines of `content` without their trailing whitespace, empty ones left out
fn trimmed_lines(content: &[u8]) -> Vec<&[u8]> {
  content.split(|&b| b == b'\n').map(trim).filter(|line| !line.is_empty()).collect()
}

/// The words of a line: runs of letters, digits and `_`
fn words(line: &[u8]) -> HashSet<&[u8]> {
  line
    .split(|b| !(b.is_ascii_alphanumeric() || *b == b'_' || *b >= 0x80))
    .filter(|word| !word.is_empty())
    .collect()
}

/// The lines that `side` added to `before` (the side's change) that `result` lost, at most `MAX_REPORTED_LINES`: the
/// report of what a resolution of one file dropped of that side. A changed line counts as kept when one line of the
/// result has every word that the side added to its nearest line of `before` (the one with the most words in common),
/// so a lost change of one value (`APP_MODE=test`, a version) is reported, and a rewrite that keeps the change (both
/// sides combined in one line) is not. Lines without a word (a lone `}`) are not reported
pub fn dropped_lines(before: &[u8], side: &[u8], result: &[u8]) -> Vec<String> {
  let before_lines = trimmed_lines(before);
  let before_set: HashSet<&[u8]> = before_lines.iter().copied().collect();
  let before_words: Vec<HashSet<&[u8]>> = before_lines.iter().map(|line| words(line)).collect();
  let result_lines = trimmed_lines(result);
  let result_set: HashSet<&[u8]> = result_lines.iter().copied().collect();
  let result_words: Vec<HashSet<&[u8]>> = result_lines.iter().map(|line| words(line)).collect();
  let mut seen = HashSet::new();
  let mut dropped = Vec::new();
  for line in trimmed_lines(side) {
    if dropped.len() >= MAX_REPORTED_LINES {
      break;
    }
    if before_set.contains(line) || result_set.contains(line) || !seen.insert(line) {
      continue;
    }
    let line_words = words(line);
    if line_words.is_empty() {
      continue;
    }
    let nearest = before_words
      .iter()
      .max_by_key(|before| before.intersection(&line_words).count())
      .filter(|before| !before.is_disjoint(&line_words));
    let added: Vec<&[u8]> = line_words.iter().copied().filter(|word| nearest.is_none_or(|nearest| !nearest.contains(word))).collect();
    let kept = result_words.iter().any(|result| added.iter().all(|word| result.contains(word)));
    if !kept {
      dropped.push(String::from_utf8_lossy(line).into_owned());
    }
  }
  dropped
}

#[cfg(test)]
#[path = "resolve_check_test.rs"]
mod resolve_check_test;
