//! A table of GitHub Markdown in a block of a body (`table_at`, `CommitBodyBlock::Table`):
//! - a header row, then a delimiter row of as many cells, each dashes with a colon at either end or none (`---`,
//!   `:--`, `--:`, `:-:`: the alignment of its column), then the rows of the body up to the first line that is no row;
//! - a row is a line with a pipe that is not escaped, indented by at most three spaces, that starts no list item, heading
//!   or quote; the pipes at its edges are optional, `\|` is a pipe inside a cell, and its cells are trimmed;
//! - a row of the body as wide as the header: a short one filled with empty cells, the cells past the header dropped;
//! - lines of pipes without the delimiter row under their first one are no table: each stays a line of prose.

use crate::commit_body::{is_code_indented, is_heading, list_item};
use domain_types::commit_body::{ColumnAlign, CommitBodyBlock};

/// The table that starts at `lines[0]` and the count of its lines; `None` when no table starts there
pub(crate) fn table_at(lines: &[&str]) -> Option<(CommitBodyBlock, usize)> {
  let align = header_align(lines)?;
  let header = cells(lines[0]);
  let rows: Vec<Vec<String>> = lines[2..]
    .iter()
    .take_while(|line| is_row(line))
    .map(|line| {
      let mut row = cells(line);
      row.resize(header.len(), String::new());
      row
    })
    .collect();
  let taken = 2 + rows.len();
  Some((CommitBodyBlock::Table { header, align, rows }, taken))
}

/// Whether a table starts at `lines[0]`: its header row and its delimiter row, nothing built
pub(crate) fn starts_table(lines: &[&str]) -> bool {
  header_align(lines).is_some()
}

/// The alignment of the columns of the table whose header row is `lines[0]`, by its delimiter row `lines[1]` of as many
/// cells
fn header_align(lines: &[&str]) -> Option<Vec<ColumnAlign>> {
  let [header, delimiter, ..] = lines else { return None };
  if !is_row(header) || !has_pipe(delimiter) || is_code_indented(delimiter) {
    return None;
  }
  let align: Vec<ColumnAlign> = raw_cells(delimiter).into_iter().map(column_align).collect::<Option<_>>()?;
  (align.len() == raw_cells(header).len()).then_some(align)
}

/// A line of a table: a pipe not escaped, indented as no code, and no list item, heading or quote
fn is_row(line: &str) -> bool {
  let trimmed = line.trim();
  has_pipe(line) && !is_code_indented(line) && !trimmed.starts_with('>') && !is_heading(trimmed) && list_item(line).is_none()
}

/// Whether `line` has a pipe that no backslash escapes
fn has_pipe(line: &str) -> bool {
  pipes(line).next().is_some()
}

/// The places of the pipes of `line` that no backslash escapes
fn pipes(line: &str) -> impl Iterator<Item = usize> + '_ {
  let mut escaped = false;
  line.char_indices().filter_map(move |(at, c)| {
    let pipe = c == '|' && !escaped;
    escaped = c == '\\' && !escaped;
    pipe.then_some(at)
  })
}

/// The cells of a row as written, trimmed: between its pipes, those at its edges aside
fn raw_cells(line: &str) -> Vec<&str> {
  let row = line.trim();
  let mut cuts: Vec<usize> = pipes(row).collect();
  let start = if cuts.first() == Some(&0) {
    cuts.remove(0);
    1
  } else {
    0
  };
  let end = if cuts.last().is_some_and(|&last| last + 1 == row.len()) && start < row.len() {
    cuts.pop();
    row.len() - 1
  } else {
    row.len()
  };
  std::iter::once(start)
    .chain(cuts.iter().map(|cut| cut + 1))
    .zip(cuts.iter().copied().chain(std::iter::once(end)))
    .map(|(from, to)| row.get(from..to).unwrap_or_default().trim())
    .collect()
}

/// The cells of a row, each `\|` a pipe
fn cells(line: &str) -> Vec<String> {
  raw_cells(line).into_iter().map(|cell| cell.replace("\\|", "|")).collect()
}

/// The alignment of a delimiter cell: dashes, a colon at either end or none; `None` for any other cell
fn column_align(cell: &str) -> Option<ColumnAlign> {
  let left = cell.starts_with(':');
  let without_left = cell.strip_prefix(':').unwrap_or(cell);
  let right = without_left.ends_with(':');
  let dashes = without_left.strip_suffix(':').unwrap_or(without_left);
  if dashes.is_empty() || !dashes.chars().all(|c| c == '-') {
    return None;
  }
  Some(match (left, right) {
    (true, true) => ColumnAlign::Center,
    (true, false) => ColumnAlign::Left,
    (false, true) => ColumnAlign::Right,
    (false, false) => ColumnAlign::None,
  })
}
