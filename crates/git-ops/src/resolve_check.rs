//! The checks of a resolved file in a conflict round of `bd update` (docs/design/git-operations.md, "Resolve
//! session"). The merge result that bd wrote (`edit`, with zdiff3 markers) is plain text with conflict blocks; the
//! resolution may replace each block with anything, but must keep every byte outside the blocks, because there the
//! merge already holds every change of both sides. Lines of a side that the resolution dropped are reported, not
//! refused: the agent shows them to the user. Everything works on bytes; only the reported lines are decoded.

/// Why a resolution is refused
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckFailure {
  /// The resolution changed a line outside the conflict blocks
  OutsideConflict,
  /// A conflict marker is left where a block was
  UnresolvedMarkers,
}

/// The lines of each side that a resolution no longer has, for the report
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DroppedLines {
  /// Lines that the new parent (`onto`: the upstream and the commits replayed so far) added in a block
  pub upstream: Vec<String>,
  /// Lines that the replayed commit added in a block
  pub commit: Vec<String>,
}

/// At most this many dropped lines are reported per side and file
pub const MAX_REPORTED_LINES: usize = 20;

/// A conflict block of zdiff3 output: the lines of each side, without the marker lines
#[derive(Debug, Default)]
struct Block<'a> {
  ours: Vec<&'a [u8]>,
  base: Vec<&'a [u8]>,
  theirs: Vec<&'a [u8]>,
}

/// Which marker a line is: `<<<<<<<`, `|||||||`, `=======` or `>>>>>>>`, followed by a space or the line end
fn marker_of(line: &[u8]) -> Option<u8> {
  let first = *line.first()?;
  if !matches!(first, b'<' | b'|' | b'=' | b'>') || line.len() < 7 || line[..7].iter().any(|&b| b != first) {
    return None;
  }
  match line.get(7) {
    None | Some(b'\n' | b'\r') => Some(first),
    Some(b' ') if first != b'=' => Some(first),
    _ => None,
  }
}

/// The number of conflict blocks left in `content`: its `<<<<<<<` lines
pub fn count_markers(content: &[u8]) -> usize {
  content.split_inclusive(|&b| b == b'\n').filter(|line| marker_of(line) == Some(b'<')).count()
}

/// Whether `content` has any conflict marker line
pub fn has_markers(content: &[u8]) -> bool {
  content.split_inclusive(|&b| b == b'\n').any(|line| marker_of(line).is_some())
}

/// Lines with their line ends
type Lines<'a> = Vec<&'a [u8]>;

/// The plain runs of `merged` (one more than the blocks) and its blocks; `None` if the markers don't nest as zdiff3
/// writes them
fn split_blocks(merged: &[u8]) -> Option<(Vec<Lines<'_>>, Vec<Block<'_>>)> {
  #[derive(PartialEq)]
  enum Part {
    Plain,
    Ours,
    Base,
    Theirs,
  }
  let mut plains = vec![Vec::new()];
  let mut blocks: Vec<Block<'_>> = Vec::new();
  let mut part = Part::Plain;
  for line in merged.split_inclusive(|&b| b == b'\n') {
    match (marker_of(line), &part) {
      (Some(b'<'), Part::Plain) => {
        blocks.push(Block::default());
        part = Part::Ours;
      }
      (Some(b'|'), Part::Ours) => part = Part::Base,
      (Some(b'='), Part::Ours | Part::Base) => part = Part::Theirs,
      (Some(b'>'), Part::Theirs) => {
        plains.push(Vec::new());
        part = Part::Plain;
      }
      (Some(_), _) => return None,
      (None, Part::Plain) => plains.last_mut()?.push(line),
      (None, Part::Ours) => blocks.last_mut()?.ours.push(line),
      (None, Part::Base) => blocks.last_mut()?.base.push(line),
      (None, Part::Theirs) => blocks.last_mut()?.theirs.push(line),
    }
  }
  (part == Part::Plain).then_some((plains, blocks))
}

/// The first position at or after `from` where `needle` (lines) starts in `haystack`
fn find_lines(haystack: &[&[u8]], needle: &[&[u8]], from: usize) -> Option<usize> {
  if needle.is_empty() {
    return Some(from);
  }
  (from..=haystack.len().checked_sub(needle.len())?).find(|&start| haystack[start..start + needle.len()] == *needle)
}

/// Check the resolution `resolved` of the merge result `merged` (the `edit` file as bd wrote it): outside the conflict
/// blocks it must be byte for byte the same, and no marker may be left where a block was. Returns the lines each side
/// added in a block that the resolution dropped
///
/// # Errors
/// `CheckFailure::OutsideConflict` when the plain runs of `merged` are not found in order, `UnresolvedMarkers` when a
/// replaced region still has a marker line
pub fn check_resolution(merged: &[u8], resolved: &[u8]) -> Result<DroppedLines, CheckFailure> {
  let Some((plains, blocks)) = split_blocks(merged) else {
    // bd's own copy always nests; anything else is checked as a whole
    return if merged == resolved {
      Err(CheckFailure::UnresolvedMarkers)
    } else {
      Err(CheckFailure::OutsideConflict)
    };
  };
  let lines: Vec<&[u8]> = resolved.split_inclusive(|&b| b == b'\n').collect();
  let (first, last) = (&plains[0], &plains[plains.len() - 1]);
  if blocks.is_empty() {
    return if lines == *first {
      Ok(DroppedLines::default())
    } else {
      Err(CheckFailure::OutsideConflict)
    };
  }
  if !lines.starts_with(first) {
    return Err(CheckFailure::OutsideConflict);
  }
  // the regions that replace the blocks, between the plain runs matched in order (leftmost, which leaves the most room
  // for the later runs)
  let mut position = first.len();
  let mut replaced: Vec<&[u8]> = Vec::new();
  for plain in &plains[1..plains.len() - 1] {
    let start = find_lines(&lines, plain, position).ok_or(CheckFailure::OutsideConflict)?;
    replaced.extend_from_slice(&lines[position..start]);
    position = start + plain.len();
  }
  let suffix_start = lines
    .len()
    .checked_sub(last.len())
    .filter(|&start| start >= position && lines[start..] == **last)
    .ok_or(CheckFailure::OutsideConflict)?;
  replaced.extend_from_slice(&lines[position..suffix_start]);
  if replaced.iter().any(|line| marker_of(line).is_some()) {
    return Err(CheckFailure::UnresolvedMarkers);
  }

  let kept: std::collections::HashSet<&[u8]> = replaced.iter().map(|line| trim(line)).collect();
  let mut dropped = DroppedLines::default();
  for block in &blocks {
    let base: std::collections::HashSet<&[u8]> = block.base.iter().map(|line| trim(line)).collect();
    let added = |side: &[&[u8]], into: &mut Vec<String>| {
      for line in side.iter().map(|line| trim(line)) {
        if !line.is_empty() && !base.contains(line) && !kept.contains(line) && into.len() < MAX_REPORTED_LINES {
          into.push(String::from_utf8_lossy(line).into_owned());
        }
      }
    };
    added(&block.ours, &mut dropped.upstream);
    added(&block.theirs, &mut dropped.commit);
  }
  Ok(dropped)
}

/// A line without its trailing whitespace and line end, as the dropped-line report compares lines
fn trim(line: &[u8]) -> &[u8] {
  let end = line.iter().rposition(|b| !b.is_ascii_whitespace()).map_or(0, |i| i + 1);
  &line[..end]
}

#[cfg(test)]
#[path = "resolve_check_test.rs"]
mod resolve_check_test;
