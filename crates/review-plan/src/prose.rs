//! The prose a change rewrites, as a reader of the brief reads it (openspec/specs/branch/review-tour/spec.md, "Brief"): whole
//! paragraphs, never the changed lines of a hard-wrapped one; a paragraph changed in a few words once, with what the
//! commits wrote and removed in it by word (`text_diff::by_word`); a link of Markdown as its text with its target; of a
//! paragraph of several sentences the sentences that hold its changes; a row of a table one paragraph under the cells
//! of its header. The front matter of a file, a thematic break and a line of metadata (`refs: flow:x, spec:y`) are no
//! prose. No git: the texts are in the memory of the plan.

use crate::units::UnitBlock;
use domain_types::commit_body::CodeSpan;
use serde::Serialize;
use std::ops::Range;
use std::time::Duration;
use sync_utils::code_spans::code_spans;
use text_diff::by_word;
use text_diff::cancellation::Deadline;
use text_diff::lcs::diff::build_changes;
use text_diff::policy::ComparisonPolicy;

/// A paragraph of prose the commits add, remove or change
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProseParagraph {
  /// Its text after the commits, its lines joined, without the marker of its item, a link as its text; of a removed
  /// paragraph its text before them; of a row of a table the cells of its header, a line break, then its own cells,
  /// each ` | ` apart
  pub text: String,
  /// Whether this paragraph is prose, the contents of a fenced code block or a row of a table
  pub form: ProseForm,
  /// Its spans of code (`sync_utils::code_spans`), the grammar the page sets code by; of a row those of each cell
  pub code: Vec<CodeSpan>,
  /// Its links of Markdown (`[text](target)`), each the range of its text in `text`
  pub links: Vec<ProseLink>,
  pub change: ProseChange,
  /// Of a paragraph of several sentences changed in a few of them, the range of `text` of the sentences that hold its
  /// changes, which the page shows until the reader opens the paragraph whole; `None` shows it whole
  pub excerpt: Option<TextRange>,
}

/// The form of a paragraph of the brief
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProseForm {
  /// Markdown prose, with inline code spans
  Prose,
  /// The lines inside a fenced code block
  Code { language: Option<String> },
  /// A row of a table of Markdown (a header row, a delimiter row, then its rows): the ranges of `text` the cells of
  /// its header take, then those of its own cells, as many as its header has; the header row itself has no header.
  /// Its changes are those of its own cells
  Row { header: Vec<TextRange>, cells: Vec<TextRange> },
}

/// A paragraph parsed from Markdown, before its word changes are computed
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParagraphText {
  pub text: String,
  pub form: ProseForm,
}

fn flush_prose(paragraphs: &mut Vec<ParagraphText>, current: &mut String) {
  if !current.is_empty() {
    paragraphs.push(ParagraphText {
      text: std::mem::take(current),
      form: ProseForm::Prose,
    });
  }
}

/// What the commits did to a paragraph
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProseChange {
  /// A paragraph the commits add
  Added,
  /// A paragraph the commits remove
  Removed,
  /// A paragraph the commits change in places, the rest of its words kept: the ranges of `text` they wrote and the
  /// words they removed, each at the offset of `text` it stood before
  Changed {
    inserted: Vec<TextRange>,
    deleted: Vec<ProseDeletion>,
    /// The inserted ranges that are part of a repeated replacement in the brief
    substitutions: Vec<SubstitutionMark>,
  },
  /// A paragraph changed only by the repeated replacements of the brief
  Substituted { substitutions: Vec<SubstitutionMark> },
}

/// A repeated replacement at its new word in `ProseParagraph::text`, referring to `ReviewBrief::substitutions`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SubstitutionMark {
  pub start: u32,
  pub end: u32,
  pub substitution: u32,
}

/// Words the commits removed from a changed paragraph
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProseDeletion {
  /// The offset of `ProseParagraph::text` they stood before
  pub at: u32,
  pub text: String,
}

/// A range of a text: UTF-16 code units, the end excluded, as the spans of code count
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct TextRange {
  pub start: u32,
  pub end: u32,
}

/// A slice of `text` at a range measured in UTF-16 code units
pub(crate) fn utf16_slice(text: &str, range: TextRange) -> Option<&str> {
  fn byte_offset(text: &str, offset: u32) -> Option<usize> {
    if offset == 0 {
      return Some(0);
    }
    let mut units = 0;
    for (byte, character) in text.char_indices() {
      units += u32::try_from(character.len_utf16()).ok()?;
      if units == offset {
        return Some(byte + character.len_utf8());
      }
      if units > offset {
        return None;
      }
    }
    (units == offset).then_some(text.len())
  }

  text.get(byte_offset(text, range.start)?..byte_offset(text, range.end)?)
}

/// A link of Markdown in a paragraph: the range of its text and its target, a path of the repository resolved against
/// the file of the paragraph (its `#anchor` dropped) or a URL as written
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProseLink {
  pub start: u32,
  pub end: u32,
  pub target: String,
}

/// An excerpt is shown only when it leaves out at least this share of the paragraph, as `(numerator, denominator)`
const EXCERPT_MAX_SHARE: (usize, usize) = (3, 4);
/// The word diff of one pair of paragraphs gets this long, then the pair is a removal and an addition
const WORD_DIFF_BUDGET: Duration = Duration::from_millis(50);
/// What stands between two cells of a row in the text of its paragraph
const CELL_SEPARATOR: &str = " | ";

/// The lines of the front matter of a file of Markdown: a file whose first line is `---` has YAML up to the next line
/// `---`, both of them its own; 0 for a file without one (a `---` never closed is a thematic break)
pub(crate) fn front_matter_end(lines: &[&str]) -> usize {
  if lines.first().map(|line| line.trim_end()) != Some("---") {
    return 0;
  }
  lines.iter().skip(1).position(|line| line.trim_end() == "---").map_or(0, |at| at + 2)
}

/// Whether a line ends a paragraph by itself and is no prose: a blank line, a heading, a fence of code, a thematic
/// break, a line of metadata
fn is_break(line: &str) -> bool {
  let trimmed = line.trim();
  trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("```") || is_thematic_break(trimmed) || is_metadata(trimmed)
}

/// Whether a trimmed line is a thematic break of Markdown: three or more of one of `-`, `*` and `_`, spaces between
/// them allowed
fn is_thematic_break(trimmed: &str) -> bool {
  let mut marks = trimmed.chars().filter(|c| !c.is_whitespace());
  let Some(mark @ ('-' | '*' | '_')) = marks.next() else {
    return false;
  };
  let mut count = 1;
  for other in marks {
    if other != mark {
      return false;
    }
    count += 1;
  }
  count >= 3
}

/// Whether a trimmed line is a line of metadata: one key (`refs:`) and after it only references (`flow:x`) and paths
/// (`../../src/a.kt`), comma-separated (`refs: flow:x, spec:y, flag:z`); a key alone is a label of prose
fn is_metadata(trimmed: &str) -> bool {
  let Some((key, rest)) = trimmed.split_once(':') else {
    return false;
  };
  if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') || !rest.starts_with(char::is_whitespace) {
    return false;
  }
  let mut tokens = rest.split(|c: char| c == ',' || c.is_whitespace()).filter(|token| !token.is_empty()).peekable();
  tokens.peek().is_some() && tokens.all(|token| is_reference(token.trim_matches('`')))
}

/// Whether a token is a reference of metadata: `kind:name` (`spec:landing`, a URL) or a path (`a/b`, `file.kt`)
fn is_reference(token: &str) -> bool {
  let reference = token.split_once(':').is_some_and(|(kind, name)| !kind.is_empty() && !name.is_empty());
  let path = token.contains('/')
    || token
      .rsplit_once('.')
      .is_some_and(|(stem, extension)| !stem.is_empty() && extension.starts_with(|c: char| c.is_ascii_alphabetic()) && extension.chars().all(|c| c.is_ascii_alphanumeric()));
  reference || path
}

/// The cells of a row of a table of Markdown: its line split at each `|` but an escaped one (`\|`, read as `|`), the
/// pipes at its ends dropped, each cell trimmed
fn table_cells(line: &str) -> Vec<String> {
  let trimmed = line.trim();
  let trimmed = trimmed.strip_prefix('|').unwrap_or(trimmed);
  let trimmed = if trimmed.ends_with('|') && !trimmed.ends_with("\\|") {
    &trimmed[..trimmed.len() - 1]
  } else {
    trimmed
  };
  let mut cells = Vec::new();
  let mut cell = String::new();
  let mut chars = trimmed.chars().peekable();
  while let Some(character) = chars.next() {
    if character == '\\' && chars.peek() == Some(&'|') {
      cell.push('|');
      chars.next();
    } else if character == '|' {
      cells.push(cell.trim().to_string());
      cell.clear();
    } else {
      cell.push(character);
    }
  }
  cells.push(cell.trim().to_string());
  cells
}

/// Whether a line is the delimiter row of a table (`| --- | :---: |`): every cell of it dashes, a colon at either end
/// allowed
fn is_delimiter_row(line: &str) -> bool {
  line.contains('-')
    && table_cells(line).iter().all(|cell| {
      let dashes = cell.strip_prefix(':').unwrap_or(cell);
      let dashes = dashes.strip_suffix(':').unwrap_or(dashes);
      !dashes.is_empty() && dashes.chars().all(|c| c == '-')
    })
}

/// The cells of the header of the table line `index` of `lines` starts: a line with a `|`, then a delimiter row of as
/// many cells; `None` for a line that starts none (a line of pipes alone is prose)
fn table_header(lines: &[&str], index: usize) -> Option<Vec<String>> {
  let line = lines[index];
  if !line.contains('|') || is_break(line) || !lines.get(index + 1).is_some_and(|next| is_delimiter_row(next)) {
    return None;
  }
  let header = table_cells(line);
  (header.len() == table_cells(lines[index + 1]).len()).then_some(header)
}

/// The text of a row of a table and the ranges the cells of its header and its own take in it: the cells of the header,
/// a line break, then its own, each `CELL_SEPARATOR` apart; the header row itself (`header` empty) its cells alone
fn row_text<S: AsRef<str>>(header: &[S], cells: &[S]) -> (String, Vec<TextRange>, Vec<TextRange>) {
  fn push<S: AsRef<str>>(cells: &[S], text: &mut String) -> Vec<TextRange> {
    let mut ranges = Vec::with_capacity(cells.len());
    for (index, cell) in cells.iter().enumerate() {
      if index > 0 {
        text.push_str(CELL_SEPARATOR);
      }
      let start = utf16(text);
      text.push_str(cell.as_ref());
      ranges.push(TextRange { start, end: utf16(text) });
    }
    ranges
  }

  let mut text = String::new();
  let header_ranges = push(header, &mut text);
  if !header.is_empty() {
    text.push('\n');
  }
  let cell_ranges = push(cells, &mut text);
  (text, header_ranges, cell_ranges)
}

/// A row of a table under `header` as a paragraph, as many cells as its header has (GFM: the missing ones empty, the
/// rest dropped); the header row itself under no header
fn row_paragraph(header: &[String], mut cells: Vec<String>) -> ParagraphText {
  if !header.is_empty() {
    cells.resize(header.len(), String::new());
  }
  let (text, header, cells) = row_text(header, &cells);
  ParagraphText {
    text,
    form: ProseForm::Row { header, cells },
  }
}

/// The part of a paragraph its changes are read in, and its offset in its text: of a row its own cells, its header
/// aside; of another paragraph its whole text
fn compared<'a>(text: &'a str, form: &ProseForm) -> (u32, &'a str) {
  let ProseForm::Row { cells, .. } = form else {
    return (0, text);
  };
  let end = utf16(text);
  let start = cells.first().map_or(end, |cell| cell.start);
  (start, utf16_slice(text, TextRange { start, end }).unwrap_or_default())
}

/// The text of an item of a list without its marker (`- `, `* `, `+ `, `1. `), `None` for a line that starts none
fn item_text(trimmed: &str) -> Option<&str> {
  ["- ", "* ", "+ "].iter().find_map(|marker| trimmed.strip_prefix(marker)).or_else(|| {
    let digits = trimmed.find(|c: char| !c.is_ascii_digit()).unwrap_or(0);
    (digits > 0).then(|| trimmed[digits..].strip_prefix(". ")).flatten()
  })
}

/// Whether line `index` of `lines` starts a paragraph (or is a break): the first line, a line after a break, a break,
/// the start of an item
fn starts_paragraph(lines: &[&str], index: usize) -> bool {
  index == 0 || index >= lines.len() || is_break(lines[index]) || is_break(lines[index - 1]) || item_text(lines[index].trim()).is_some()
}

/// The lines above `range` the paragraph its first line is in reaches: of an empty range, the paragraph it stands inside
fn reach_up(lines: &[&str], range: &Range<usize>) -> usize {
  let start = range.start;
  let inside = if range.is_empty() {
    start > 0 && start < lines.len() && !starts_paragraph(lines, start)
  } else {
    start < lines.len() && !is_break(lines[start])
  };
  if !inside {
    return 0;
  }
  let mut at = start;
  while at > 0 && !starts_paragraph(lines, at) {
    at -= 1;
  }
  start - at
}

/// The lines below `range` the paragraph its last line is in reaches: of an empty range, the paragraph it stands inside
fn reach_down(lines: &[&str], range: &Range<usize>) -> usize {
  let end = range.end;
  let inside = if range.is_empty() {
    end > 0 && end < lines.len() && !starts_paragraph(lines, end)
  } else {
    end > 0 && end <= lines.len() && !is_break(lines[end - 1])
  };
  if !inside {
    return 0;
  }
  let mut at = end;
  while at < lines.len() && !starts_paragraph(lines, at) {
    at += 1;
  }
  at - end
}

/// Extends a change within a fenced code block through its delimiters, so the paragraph parser sees the code form
fn include_fenced_block(lines: &[&str], range: &mut Range<usize>) {
  let mut opening = None;
  for (index, line) in lines.iter().enumerate().take(range.start) {
    if line.trim().starts_with("```") {
      opening = if opening.is_some() { None } else { Some(index) };
    }
  }
  let Some(opening) = opening else {
    return;
  };
  let closing = lines
    .iter()
    .enumerate()
    .skip(opening + 1)
    .find_map(|(index, line)| line.trim().starts_with("```").then_some(index));
  range.start = range.start.min(opening);
  range.end = range.end.max(closing.map_or(lines.len(), |closing| closing + 1));
}

/// The changed blocks of a file of prose widened to whole paragraphs on both sides, the blocks whose paragraphs meet
/// joined into one: each its lines before and after the commits and the indexes of its blocks. The lines around a block are
/// the same on both sides, so a paragraph one side reaches into widens the other side by as many lines
pub(crate) fn paragraph_spans(old: &[&str], new: &[&str], blocks: &[UnitBlock]) -> Vec<(Range<usize>, Range<usize>, Vec<u32>)> {
  let mut spans: Vec<(Range<usize>, Range<usize>, Vec<u32>)> = Vec::new();
  for block in blocks {
    let mut old_range = block.old_start as usize..block.old_end as usize;
    let mut new_range = block.new_start as usize..block.new_end as usize;
    include_fenced_block(old, &mut old_range);
    include_fenced_block(new, &mut new_range);
    let mut indexes = vec![block.index];
    loop {
      // a paragraph that reaches the span before: one span, the lines between them the same on both sides
      if let Some((last_old, _, _)) = spans.last()
        && old_range.start <= last_old.end
        && let Some((last_old, last_new, last_indexes)) = spans.pop()
      {
        old_range.start = last_old.start;
        new_range.start = last_new.start;
        indexes.splice(0..0, last_indexes);
      }
      let up = reach_up(old, &old_range).max(reach_up(new, &new_range)).min(old_range.start).min(new_range.start);
      let down = reach_down(old, &old_range)
        .max(reach_down(new, &new_range))
        .min(old.len().saturating_sub(old_range.end))
        .min(new.len().saturating_sub(new_range.end));
      if up == 0 && down == 0 {
        break;
      }
      old_range = old_range.start - up..old_range.end + down;
      new_range = new_range.start - up..new_range.end + down;
    }
    spans.push((old_range, new_range, indexes));
  }
  spans
}

/// The paragraphs of `lines`: a break and the start of an item of a list end one; each prose paragraph's lines are
/// joined by a space, code fence contents keep their line breaks, a table is its header row and each of its rows
/// under it (`ProseForm::Row`); the `[@test]` lines apart, as the paths they name
pub(crate) fn paragraphs(lines: &[&str]) -> (Vec<ParagraphText>, Vec<String>) {
  let (mut paragraphs, mut proven_by) = (Vec::new(), Vec::new());
  let mut current = String::new();
  let mut code_lines: Vec<&str> = Vec::new();
  let mut code_language = None;
  let mut in_code = false;
  // the cells of the header of the table the lines are in: its rows go on up to a break or a line without a `|`
  let mut table: Option<Vec<String>> = None;
  let mut index = 0;
  while index < lines.len() {
    let line = lines[index];
    index += 1;
    let trimmed = line.trim();
    if let Some(header) = &table {
      if !in_code && !is_break(line) && trimmed.contains('|') && !trimmed.starts_with("[@test]") {
        paragraphs.push(row_paragraph(header, table_cells(line)));
        continue;
      }
      table = None;
    }
    if !in_code && let Some(header) = table_header(lines, index - 1) {
      flush_prose(&mut paragraphs, &mut current);
      paragraphs.push(row_paragraph(&[], header.clone()));
      table = Some(header);
      // the delimiter row
      index += 1;
      continue;
    }
    if in_code {
      if trimmed.starts_with("```") {
        paragraphs.push(ParagraphText {
          text: code_lines.join("\n"),
          form: ProseForm::Code { language: code_language.take() },
        });
        code_lines.clear();
        in_code = false;
      } else {
        code_lines.push(line);
      }
      continue;
    }
    if let Some(fence) = trimmed.strip_prefix("```") {
      flush_prose(&mut paragraphs, &mut current);
      let language = fence.trim();
      code_language = (!language.is_empty()).then(|| language.to_string());
      in_code = true;
      continue;
    }
    if let Some(test) = trimmed.strip_prefix("[@test]") {
      if let Some(path) = test.split_whitespace().next() {
        proven_by.push(path.trim_start_matches("../").to_string());
      }
      continue;
    }
    let item = item_text(trimmed);
    if is_break(line) || item.is_some() {
      flush_prose(&mut paragraphs, &mut current);
    }
    if is_break(line) {
      continue;
    }
    if !current.is_empty() {
      current.push(' ');
    }
    current.push_str(item.unwrap_or(trimmed));
  }
  flush_prose(&mut paragraphs, &mut current);
  if in_code {
    paragraphs.push(ParagraphText {
      text: code_lines.join("\n"),
      form: ProseForm::Code { language: code_language },
    });
  }
  (paragraphs, proven_by)
}

/// The paragraphs `new` has that `old` lacks, in their order: an equal paragraph is no change (a paragraph wrapped
/// anew says the same, a row under a header changed alike), a removed paragraph and the first similar added one of its
/// form at its place one changed paragraph by word, the rest added or removed.
/// `path` is the file of the paragraphs, the base of the paths their links name
pub(crate) fn prose_changes(old: &[ParagraphText], new: &[ParagraphText], path: &str) -> Vec<ProseParagraph> {
  let directory = path.rsplit_once('/').map_or("", |(directory, _)| directory);
  let prepare = |paragraph: &ParagraphText| match &paragraph.form {
    ProseForm::Prose => {
      let (text, links) = strip_links(&paragraph.text, directory);
      (text, links, ProseForm::Prose)
    }
    ProseForm::Code { .. } => (paragraph.text.clone(), Vec::new(), paragraph.form.clone()),
    ProseForm::Row { header, cells } => strip_row_links(&paragraph.text, header, cells, directory),
  };
  let old: Vec<(String, Vec<ProseLink>, ProseForm)> = old.iter().map(&prepare).collect();
  let new: Vec<(String, Vec<ProseLink>, ProseForm)> = new.iter().map(&prepare).collect();
  let key = |(text, _, form): &(String, Vec<ProseLink>, ProseForm)| match form {
    ProseForm::Prose => format!("\0prose\0{text}"),
    ProseForm::Code { .. } => format!("\0code\0{text}"),
    ProseForm::Row { .. } => format!("\0row\0{}", compared(text, form).1),
  };
  let old_keys: Vec<String> = old.iter().map(key).collect();
  let new_keys: Vec<String> = new.iter().map(key).collect();
  let old_texts: Vec<&str> = old_keys.iter().map(String::as_str).collect();
  let new_texts: Vec<&str> = new_keys.iter().map(String::as_str).collect();
  let changes = build_changes(&old_texts, &new_texts).unwrap_or_else(|_| {
    vec![text_diff::lcs::diff::Change {
      line0: 0,
      line1: 0,
      deleted: old.len(),
      inserted: new.len(),
    }]
  });
  let mut result = Vec::new();
  for change in changes {
    let removed = &old[change.line0..change.line0 + change.deleted];
    let added = &new[change.line1..change.line1 + change.inserted];
    // each removed paragraph pairs with the first similar added one after the last pair: the added ones it skips are
    // new, a removed one without a pair is gone
    let mut next = 0;
    for (before, before_links, before_form) in removed {
      let pair = (next..added.len()).find_map(|at| {
        let (after, _, after_form) = &added[at];
        if !same_form_kind(before_form, after_form) {
          return None;
        }
        let (offset, after_part) = compared(after, after_form);
        word_change(compared(before, before_form).1, after_part).map(|(inserted, deleted)| (at, shifted(inserted, deleted, offset)))
      });
      let Some((at, (inserted, deleted))) = pair else {
        result.push(paragraph(before, before_form.clone(), before_links, ProseChange::Removed, None));
        continue;
      };
      for (text, links, form) in &added[next..at] {
        result.push(paragraph(text, form.clone(), links, ProseChange::Added, None));
      }
      let (after, links, after_form) = &added[at];
      let excerpt = match after_form {
        ProseForm::Prose => excerpt(after, &inserted, &deleted),
        ProseForm::Code { .. } => code_excerpt(after, &inserted, &deleted),
        ProseForm::Row { .. } => None,
      };
      result.push(paragraph(
        after,
        after_form.clone(),
        links,
        ProseChange::Changed {
          inserted,
          deleted,
          substitutions: Vec::new(),
        },
        excerpt,
      ));
      next = at + 1;
    }
    for (text, links, form) in &added[next..] {
      result.push(paragraph(text, form.clone(), links, ProseChange::Added, None));
    }
  }
  result
}

const fn same_form_kind(left: &ProseForm, right: &ProseForm) -> bool {
  matches!(
    (left, right),
    (ProseForm::Prose, ProseForm::Prose) | (ProseForm::Code { .. }, ProseForm::Code { .. }) | (ProseForm::Row { .. }, ProseForm::Row { .. })
  )
}

/// The changes of the part of a paragraph `compared` read, moved to their places in its text
fn shifted(mut inserted: Vec<TextRange>, mut deleted: Vec<ProseDeletion>, offset: u32) -> (Vec<TextRange>, Vec<ProseDeletion>) {
  for range in &mut inserted {
    range.start += offset;
    range.end += offset;
  }
  for deletion in &mut deleted {
    deletion.at += offset;
  }
  (inserted, deleted)
}

/// A row of a table with the links of each of its cells as their text (`strip_links`), the ranges of its cells moved
/// to match
fn strip_row_links(text: &str, header: &[TextRange], cells: &[TextRange], directory: &str) -> (String, Vec<ProseLink>, ProseForm) {
  let strip =
    |ranges: &[TextRange]| -> Vec<(String, Vec<ProseLink>)> { ranges.iter().map(|range| strip_links(utf16_slice(text, *range).unwrap_or_default(), directory)).collect() };
  let (header, cells) = (strip(header), strip(cells));
  fn texts(cells: &[(String, Vec<ProseLink>)]) -> Vec<&str> {
    cells.iter().map(|(text, _)| text.as_str()).collect()
  }
  let (text, header_ranges, cell_ranges) = row_text(&texts(&header), &texts(&cells));
  let links = header
    .iter()
    .zip(&header_ranges)
    .chain(cells.iter().zip(&cell_ranges))
    .flat_map(|((_, links), range)| {
      links.iter().map(move |link| ProseLink {
        start: link.start + range.start,
        end: link.end + range.start,
        target: link.target.clone(),
      })
    })
    .collect();
  (
    text,
    links,
    ProseForm::Row {
      header: header_ranges,
      cells: cell_ranges,
    },
  )
}

/// Moves repeated replacement pairs out of the word diff and marks their new words
pub(crate) fn apply_substitutions(paragraphs: &mut [ProseParagraph], table: &[crate::brief::Substitution]) {
  for paragraph in paragraphs {
    let updated = match &mut paragraph.change {
      ProseChange::Changed { inserted, deleted, substitutions } => {
        let mut used_deletions = vec![false; deleted.len()];
        let mut kept_insertions = Vec::new();
        for inserted_range in inserted.iter() {
          let same_position: Vec<usize> = deleted
            .iter()
            .enumerate()
            .filter_map(|(index, deletion)| (deletion.at == inserted_range.start).then_some(index))
            .collect();
          let same_inserted_position = inserted.iter().filter(|range| range.start == inserted_range.start).count();
          let substitution = if same_position.len() == 1 && same_inserted_position == 1 {
            let deletion_index = same_position[0];
            let deletion = &deleted[deletion_index];
            utf16_slice(&paragraph.text, *inserted_range)
              .and_then(|to| table.iter().position(|substitution| substitution.from == deletion.text && substitution.to == to))
              .map(|index| (deletion_index, index))
          } else {
            None
          };
          if let Some((deletion_index, substitution)) = substitution {
            let Ok(substitution) = u32::try_from(substitution) else {
              kept_insertions.push(*inserted_range);
              continue;
            };
            used_deletions[deletion_index] = true;
            substitutions.push(SubstitutionMark {
              start: inserted_range.start,
              end: inserted_range.end,
              substitution,
            });
          } else {
            kept_insertions.push(*inserted_range);
          }
        }
        *inserted = kept_insertions;
        *deleted = std::mem::take(deleted)
          .into_iter()
          .enumerate()
          .filter_map(|(index, deletion)| (!used_deletions[index]).then_some(deletion))
          .collect();
        (inserted.is_empty() && deleted.is_empty() && !substitutions.is_empty()).then(|| ProseChange::Substituted {
          substitutions: std::mem::take(substitutions),
        })
      }
      _ => None,
    };
    if let Some(change) = updated {
      paragraph.change = change;
    }
  }
}

fn paragraph(text: &str, form: ProseForm, links: &[ProseLink], change: ProseChange, excerpt: Option<TextRange>) -> ProseParagraph {
  let (code, links) = match &form {
    ProseForm::Prose => (code_spans(text), links.to_vec()),
    ProseForm::Code { .. } => (Vec::new(), Vec::new()),
    // the spans of each cell, so no span runs from one cell into the next
    ProseForm::Row { header, cells } => (
      header
        .iter()
        .chain(cells)
        .flat_map(|range| {
          code_spans(utf16_slice(text, *range).unwrap_or_default()).into_iter().map(move |span| CodeSpan {
            start: span.start + range.start,
            end: span.end + range.start,
            ..span
          })
        })
        .collect(),
      links.to_vec(),
    ),
  };
  ProseParagraph {
    text: text.to_string(),
    form,
    code,
    links,
    change,
    excerpt,
  }
}

fn utf16(text: &str) -> u32 {
  u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}

/// `text` with each link of Markdown outside code (`[text](target)`, not an image) as its text alone, and the links
/// (`ProseLink`), their targets resolved against `directory`
pub(crate) fn strip_links(text: &str, directory: &str) -> (String, Vec<ProseLink>) {
  let mut out = String::with_capacity(text.len());
  let mut links = Vec::new();
  let mut rest = text;
  let mut in_code = false;
  while let Some(at) = rest.find(['`', '[']) {
    let (before, tail) = rest.split_at(at);
    out.push_str(before);
    if let Some(after) = tail.strip_prefix('`') {
      in_code = !in_code;
      out.push('`');
      rest = after;
      continue;
    }
    let image = out.ends_with('!');
    let parsed = (!in_code && !image).then(|| parse_link(tail)).flatten();
    let Some((label, target, length)) = parsed else {
      out.push('[');
      rest = &tail[1..];
      continue;
    };
    let start = utf16(&out);
    out.push_str(label);
    links.push(ProseLink {
      start,
      end: utf16(&out),
      target: resolve_target(target, directory),
    });
    rest = &tail[length..];
  }
  out.push_str(rest);
  (out, links)
}

/// The label, the target and the length of a link of Markdown at the start of `text` (`[label](target)`)
fn parse_link(text: &str) -> Option<(&str, &str, usize)> {
  let close = text.find(']')?;
  let label = &text[1..close];
  if label.is_empty() || label.contains('[') {
    return None;
  }
  let after = text[close + 1..].strip_prefix('(')?;
  let end = after.find(')')?;
  let target = after[..end].trim();
  // a title after the target (`[a](b "title")`) is no part of it
  let target = target.split_whitespace().next()?;
  Some((label, target, close + 2 + end + 1))
}

/// The target of a link: a URL or an anchor as written, a path of the repository resolved against `directory` without
/// its `#anchor`
fn resolve_target(target: &str, directory: &str) -> String {
  if target.contains("://") || target.starts_with("mailto:") || target.starts_with('#') {
    return target.to_string();
  }
  let path = target.split('#').next().unwrap_or(target);
  let mut parts: Vec<&str> = if path.starts_with('/') {
    Vec::new()
  } else {
    directory.split('/').filter(|part| !part.is_empty()).collect()
  };
  for part in path.split('/') {
    match part {
      "" | "." => {}
      ".." => {
        parts.pop();
      }
      part => parts.push(part),
    }
  }
  parts.join("/")
}

/// What the commits wrote and removed in `after` by word, when `before` is similar: at least half of the characters of
/// the shorter side and a third of the longer one kept (a sentence appended to a paragraph keeps it); `None` for
/// paragraphs too different to read as one (or a diff out of its budget)
fn word_change(before: &str, after: &str) -> Option<(Vec<TextRange>, Vec<ProseDeletion>)> {
  let old: Vec<u16> = before.encode_utf16().collect();
  let new: Vec<u16> = after.encode_utf16().collect();
  let fragments = by_word::compare(&old, &new, ComparisonPolicy::Default, &Deadline::after(WORD_DIFF_BUDGET)).ok()?;
  let removed: usize = fragments.iter().map(|fragment| fragment.end_offset1 - fragment.start_offset1).sum();
  let kept = old.len() - removed.min(old.len());
  if kept * 2 < old.len().min(new.len()) || kept * 3 < old.len().max(new.len()) {
    return None;
  }
  let offset = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
  let inserted = fragments
    .iter()
    .filter(|fragment| fragment.end_offset2 > fragment.start_offset2)
    .map(|fragment| TextRange {
      start: offset(fragment.start_offset2),
      end: offset(fragment.end_offset2),
    })
    .collect();
  let deleted = fragments
    .iter()
    .filter(|fragment| fragment.end_offset1 > fragment.start_offset1)
    .map(|fragment| ProseDeletion {
      at: offset(fragment.start_offset2),
      text: String::from_utf16_lossy(&old[fragment.start_offset1..fragment.end_offset1]),
    })
    .collect();
  Some((inserted, deleted))
}

/// The sentences of `text` (UTF-16 ranges): a `.`, `!` or `?` and the whitespace after it end one, the whitespace
/// between two sentences belongs to neither
fn sentences(text: &[u16]) -> Vec<Range<usize>> {
  let is_space = |unit: u16| unit == u16::from(b' ') || unit == u16::from(b'\t');
  let is_end = |unit: u16| unit == u16::from(b'.') || unit == u16::from(b'!') || unit == u16::from(b'?');
  let mut result = Vec::new();
  let mut start = 0;
  let mut at = 0;
  while at < text.len() {
    if is_end(text[at]) && text.get(at + 1).is_some_and(|&next| is_space(next)) {
      result.push(start..at + 1);
      at += 1;
      while at < text.len() && is_space(text[at]) {
        at += 1;
      }
      start = at;
    } else {
      at += 1;
    }
  }
  if start < text.len() {
    result.push(start..text.len());
  }
  result
}

/// Of a paragraph of several sentences, the sentences that hold its changes, when they leave out enough of it
/// (`EXCERPT_MAX_SHARE`)
fn excerpt(text: &str, inserted: &[TextRange], deleted: &[ProseDeletion]) -> Option<TextRange> {
  let units: Vec<u16> = text.encode_utf16().collect();
  let sentences = sentences(&units);
  if sentences.len() < 2 {
    return None;
  }
  // the sentence a position of a change is in: the one it starts in, the last one for the end of the text
  let sentence_of = |at: usize| sentences.iter().position(|sentence| at < sentence.end).unwrap_or_else(|| sentences.len().saturating_sub(1));
  let touched = inserted
    .iter()
    .flat_map(|range| {
      [
        sentence_of(range.start as usize),
        sentence_of((range.end as usize).saturating_sub(1).max(range.start as usize)),
      ]
    })
    .chain(deleted.iter().map(|deletion| sentence_of(deletion.at as usize)));
  let (first, last) = touched.fold((usize::MAX, 0), |(first, last), index| (first.min(index), last.max(index)));
  if first == usize::MAX {
    return None;
  }
  let range = sentences[first].start..sentences[last].end;
  let (numerator, denominator) = EXCERPT_MAX_SHARE;
  (range.len() * denominator <= units.len() * numerator).then(|| TextRange {
    start: u32::try_from(range.start).unwrap_or(u32::MAX),
    end: u32::try_from(range.end).unwrap_or(u32::MAX),
  })
}

/// Of a long fenced block changed on multiple lines, the changed lines and one line of context on either side
fn code_excerpt(text: &str, inserted: &[TextRange], deleted: &[ProseDeletion]) -> Option<TextRange> {
  let lines: Vec<&str> = text.split('\n').collect();
  if lines.len() <= 8 {
    return None;
  }
  let mut starts = Vec::with_capacity(lines.len());
  let mut offset = 0;
  for (index, line) in lines.iter().enumerate() {
    starts.push(offset);
    offset += utf16(line);
    if index + 1 < lines.len() {
      offset += 1;
    }
  }
  let line_of = |at: u32| starts.iter().take_while(|&&start| start <= at).count().saturating_sub(1);
  let mut changed_lines = Vec::new();
  for range in inserted {
    let first = line_of(range.start);
    let last = line_of(range.end.saturating_sub(1).max(range.start));
    changed_lines.extend(first..=last);
  }
  changed_lines.extend(deleted.iter().map(|deletion| line_of(deletion.at)));
  changed_lines.sort_unstable();
  changed_lines.dedup();
  if changed_lines.len() <= 1 {
    return None;
  }
  let first = changed_lines[0].saturating_sub(1);
  let last = (changed_lines[changed_lines.len() - 1] + 1).min(lines.len() - 1);
  let start = starts[first];
  let mut end = starts[last] + utf16(lines[last]);
  if last + 1 < lines.len() {
    end += 1;
  }
  (start > 0 || end < offset).then_some(TextRange { start, end })
}
