//! The grammar of a commit body (`CommitBody`), one for the commit preview of the page (`commit_body`), the body of a
//! pull request (`remote_facts::pull_request_text`), which writes the blocks as Markdown, and the steps of a review
//! (`review_plan`):
//! - blocks are separated by blank lines; a fence (```` ``` ```` or `~~~`) starts a preformatted block that runs to the
//!   closing fence or the end of the body, blank lines included;
//! - a last block whose lines are all trailers (`Token: value`, the token letters, digits and `-`, an indented line
//!   continuing one; `(cherry picked from commit …)`), at least one of them a token of words joined by `-` or the
//!   cherry-pick note, is the trailers: a one-word `Note: …` alone is prose;
//! - inside a block, a line starting with `- `, `* `, `+ `, `• `, `1. ` or `1) ` (not a rule) starts a list, and every
//!   following line starts an item or continues the last one, until a heading, a quote, a table row, a rule or a label
//!   line right before an item; a change between bullets and numbers starts another list;
//! - a run of lines indented by four spaces or a tab is preformatted, without the common indent;
//! - a heading and a rule are blocks of their own;
//! - the other lines are prose: hard-wrapped lines joined, a line after a hard break, a quote line and a table row a
//!   line of their own;
//! - a label (`LEAD_IN_MAX`, `RUN_IN_MAX`) heads the blocks after it: a paragraph that is only `Label:` followed by a
//!   block, a `Label:` line right before a list item, a `Label:` line that starts its block (or follows a list) right
//!   before a line of prose or code, or the 1–3 words of a paragraph that starts `Label: text`; its kind is by its
//!   words (`VERIFICATION_LABELS`, `LOOK_CLOSELY`); a run-in `Look closely: text` heads its paragraph and the list or
//!   code right under it (no blank line between) alone;
//! - the code of each line of prose (`CommitBody::code`) by the grammar of `code_spans`;
//! - the parts and the fold (`CommitBody::parts`): the lead alone, a labelled section whole, a run of other blocks; the
//!   first part shows, then whole parts while they read in `FOLD_LINES` lines (a line of prose about `LINE_CHARS`
//!   characters), the rest folds; a verification section folds and a Look closely section shows wherever it is, and
//!   neither costs a line.

use crate::code_spans::code_spans;
use domain_types::commit_body::{CommitBody, CommitBodyBlock, CommitBodyPart, CommitBodyPartKind, LabelKind, LineCode};

/// The lines a body shows before its fold
const FOLD_LINES: u32 = 12;
/// The characters of a line of prose as the fold counts them
const LINE_CHARS: usize = 100;
/// The longest label of a paragraph that is only `Label:`, its colon aside: "Gates on the final tree, each exit 0"
const LEAD_IN_MAX: usize = 39;
/// The longest run-in label of a `Label: text` paragraph: "Why", "Left out", "Look closely"
const RUN_IN_MAX: usize = 20;
/// The words a verification section starts with (a leading "The" and the case aside): how the author checked the change
const VERIFICATION_LABELS: [&str; 9] = ["tests", "test", "evidence", "gates", "verified", "verification", "results", "proof", "left out"];
/// The words of the label where the author asks the reviewer to decide
const LOOK_CLOSELY: [&str; 1] = ["look closely"];

/// The body of `message`: the lines after its first one, the subject line the rows show
pub fn message_body(message: &str) -> &str {
  message.split_once('\n').map_or("", |(_, body)| body)
}

/// Whether `message` has a body a reader is shown: a block after its subject line other than its trailers. The rows of a
/// commit carry it (`Commit::has_body`), so the page marks a message worth opening without reading its grammar; one
/// pass over the lines, without the blocks `commit_body` builds
pub fn has_body(message: &str) -> bool {
  match raw_blocks(message_body(message)).as_slice() {
    [] => false,
    [RawBlock::Lines(lines)] => trailers(lines).is_none(),
    _ => true,
  }
}

/// The body of a commit, `body` the message without its subject line
pub fn commit_body(body: &str) -> CommitBody {
  let body = body.replace("\r\n", "\n");
  let raw_blocks = raw_blocks(&body);
  let mut blocks = Vec::new();
  // the raw block (a run of lines between blank lines) each block comes from
  let mut raw_of = Vec::new();
  let mut trailer_lines = Vec::new();
  let last = raw_blocks.len().saturating_sub(1);
  for (index, raw) in raw_blocks.into_iter().enumerate() {
    match raw {
      RawBlock::Fenced(lines) => blocks.push(CommitBodyBlock::Preformatted { text: dedent(&lines).join("\n") }),
      RawBlock::Lines(lines) => match trailers(&lines).filter(|_| index == last) {
        Some(trailers) => trailer_lines = trailers,
        None => parse_block(&lines, &mut blocks),
      },
    }
    raw_of.resize(blocks.len(), index);
  }
  let (blocks, raw_of) = labelled(blocks, &raw_of);
  let lead = matches!(blocks.first(), Some(CommitBodyBlock::Paragraph { .. }));
  let parts = parts(&blocks, &raw_of, lead);
  let code = code_of(&blocks);
  CommitBody {
    blocks,
    trailers: trailer_lines,
    lead,
    parts,
    code,
  }
}

/// The lines of a block a reader reads as prose: a paragraph its lines, a list its items, a label or a heading its text
pub fn prose_lines(block: &CommitBodyBlock) -> Vec<&str> {
  match block {
    CommitBodyBlock::Paragraph { lines } => lines.iter().map(String::as_str).collect(),
    CommitBodyBlock::List { items, .. } => items.iter().map(String::as_str).collect(),
    CommitBodyBlock::Heading { text, .. } | CommitBodyBlock::Label { text, .. } => vec![text.as_str()],
    CommitBodyBlock::Preformatted { .. } | CommitBodyBlock::Rule => Vec::new(),
  }
}

/// The code of each line of prose of `blocks` that holds some (`CommitBody::code`)
pub fn code_of(blocks: &[CommitBodyBlock]) -> Vec<LineCode> {
  let mut code = Vec::new();
  for (block, of) in blocks.iter().enumerate() {
    for (item, line) in prose_lines(of).into_iter().enumerate() {
      let spans = code_spans(line);
      if !spans.is_empty() {
        code.push(LineCode {
          block: index(block),
          item: index(item),
          spans,
        });
      }
    }
  }
  code
}

/// `blocks` with their labels: a paragraph that is only `Label:` before a block of content, and the label of a
/// paragraph that starts `Label: text` before the rest of it; with the raw block of each (`raw_of` of `blocks`)
fn labelled(blocks: Vec<CommitBodyBlock>, raw_of: &[usize]) -> (Vec<CommitBodyBlock>, Vec<usize>) {
  let mut labelled = Vec::with_capacity(blocks.len());
  let mut labelled_raw = Vec::with_capacity(blocks.len());
  let mut blocks = blocks.into_iter().zip(raw_of.iter().copied()).peekable();
  while let Some((block, raw)) = blocks.next() {
    let CommitBodyBlock::Paragraph { lines } = block else {
      labelled.push(block);
      labelled_raw.push(raw);
      continue;
    };
    let content_follows = matches!(
      blocks.peek(),
      Some((CommitBodyBlock::Paragraph { .. } | CommitBodyBlock::List { .. } | CommitBodyBlock::Preformatted { .. }, _))
    );
    if let [only] = lines.as_slice()
      && let Some(text) = lead_in(only).filter(|_| content_follows)
    {
      labelled.push(label(text, false));
      labelled_raw.push(raw);
    } else if let Some((text, rest)) = lines.first().and_then(|first| run_in(first)) {
      let rest = rest.to_string();
      labelled.push(label(text, true));
      labelled.push(CommitBodyBlock::Paragraph {
        lines: std::iter::once(rest).chain(lines.into_iter().skip(1)).collect(),
      });
      labelled_raw.extend([raw, raw]);
    } else {
      labelled.push(CommitBodyBlock::Paragraph { lines });
      labelled_raw.push(raw);
    }
  }
  (labelled, labelled_raw)
}

fn label(text: &str, run_in: bool) -> CommitBodyBlock {
  CommitBodyBlock::Label {
    text: text.to_string(),
    kind: label_kind(text),
    run_in,
  }
}

/// The label of a line that is only `Label:`: a capital letter of any script, up to `LEAD_IN_MAX` characters without a
/// colon, then the colon
fn lead_in(line: &str) -> Option<&str> {
  let text = line.strip_suffix(':')?;
  (text.starts_with(char::is_uppercase) && !text.contains(':') && text.chars().count() <= LEAD_IN_MAX).then_some(text)
}

/// The label of a line that starts `Label: text` and the text after it: a capitalised word and up to two lowercase
/// ones of any script, at most `RUN_IN_MAX` characters
fn run_in(line: &str) -> Option<(&str, &str)> {
  let (text, rest) = line.split_once(": ")?;
  let mut words = text.split(' ');
  let first = words.next()?;
  let capitalised = first.starts_with(char::is_uppercase) && first.chars().skip(1).all(char::is_lowercase);
  let others: Vec<&str> = words.collect();
  let lowercase = others.len() <= 2 && others.iter().all(|word| !word.is_empty() && word.chars().all(char::is_lowercase));
  (capitalised && lowercase && text.chars().count() <= RUN_IN_MAX && !rest.is_empty()).then_some((text, rest))
}

/// The kind of a label by its words: a leading "The", the case and what follows a comma aside, it starts with a word of
/// `LOOK_CLOSELY` or `VERIFICATION_LABELS`
fn label_kind(text: &str) -> LabelKind {
  let lower = text.to_lowercase();
  let head = lower.split(',').next().unwrap_or_default().trim();
  let head = head.strip_prefix("the ").unwrap_or(head);
  let starts = |word: &str| head.strip_prefix(word).is_some_and(|rest| rest.is_empty() || rest.starts_with(' '));
  if LOOK_CLOSELY.into_iter().any(starts) {
    LabelKind::LookClosely
  } else if VERIFICATION_LABELS.into_iter().any(starts) {
    LabelKind::Verification
  } else {
    LabelKind::Plain
  }
}

/// The parts of `blocks` and their fold (`CommitBody::parts`), `raw_of` the raw block of each. The units of the fold in
/// order: the lead, a labelled section whole, any other block alone; the first unit shows however long it is, so
/// something shows before the fold, then each while the lines so far stay within `FOLD_LINES`, and every unit after the
/// first that does not fit folds. A verification section folds and a Look closely section shows wherever it is, at no
/// cost. A run of other blocks on one side of the fold is one part
fn parts(blocks: &[CommitBodyBlock], raw_of: &[usize], lead: bool) -> Vec<CommitBodyPart> {
  let heads = |block: &CommitBodyBlock| matches!(block, CommitBodyBlock::Label { .. } | CommitBodyBlock::Heading { .. });
  let mut parts: Vec<CommitBodyPart> = Vec::new();
  let mut lines = 0;
  let mut cut = false;
  let mut start = 0;
  while start < blocks.len() {
    let headed = heads(&blocks[start]);
    let end = match blocks[start] {
      // the author's question is the paragraph the label starts, with a list or code written right under it (no blank
      // line between): what follows a blank line is the story again
      CommitBodyBlock::Label {
        kind: LabelKind::LookClosely,
        run_in: true,
        ..
      } => {
        let question = (start + 2).min(blocks.len());
        let raw = raw_of.get(start).copied();
        question + blocks[question..].iter().zip(&raw_of[question..]).take_while(|(block, of)| !heads(block) && Some(**of) == raw).count()
      }
      _ if headed => blocks[start + 1..].iter().position(heads).map_or(blocks.len(), |next| start + 1 + next),
      _ => start + 1,
    };
    let kind = match blocks[start] {
      CommitBodyBlock::Label { kind, .. } => kind,
      _ => LabelKind::Plain,
    };
    let folded = match kind {
      LabelKind::Verification => true,
      LabelKind::LookClosely => false,
      LabelKind::Plain => {
        let cost: u32 = blocks[start..end].iter().map(lines_of).sum();
        cut |= lines > 0 && lines + cost > FOLD_LINES;
        if !cut {
          lines += cost;
        }
        cut
      }
    };
    let part = CommitBodyPart {
      start: index(start),
      end: index(end),
      kind: match () {
        () if headed => CommitBodyPartKind::Section,
        () if lead && start == 0 => CommitBodyPartKind::Lead,
        () => CommitBodyPartKind::Run,
      },
      tone: kind,
      folded,
    };
    match parts.last_mut() {
      // a run of blocks of no label on one side of the fold, the lead apart
      Some(last) if !headed && last.folded == folded && last.end == part.start && !heads(&blocks[last.start as usize]) && !(lead && last.start == 0) => {
        last.end = part.end;
      }
      _ => parts.push(part),
    }
    start = end;
  }
  parts
}

fn index(place: usize) -> u32 {
  u32::try_from(place).unwrap_or(u32::MAX)
}

/// The lines `body` reads in, as its fold counts them (`FOLD_LINES`): a line of prose or an item by `LINE_CHARS`
/// characters, a line of code, a label or a heading one
pub fn reading_lines(body: &CommitBody) -> u32 {
  body.blocks.iter().map(lines_of).sum()
}

/// The lines the story of `body` reads in (`reading_lines`), its verification sections left out: how the author checked
/// the change tells no story
pub fn story_lines(body: &CommitBody) -> u32 {
  body
    .parts
    .iter()
    .map(|part| &body.blocks[part.start as usize..part.end as usize])
    .filter(|blocks| {
      !matches!(
        blocks.first(),
        Some(CommitBodyBlock::Label {
          kind: LabelKind::Verification,
          ..
        })
      )
    })
    .flatten()
    .map(lines_of)
    .sum()
}

/// The lines a block reads in: a line of prose or an item by `LINE_CHARS` characters, a line of code one
fn lines_of(block: &CommitBodyBlock) -> u32 {
  let wrapped = |text: &String| u32::try_from(text.chars().count().div_ceil(LINE_CHARS).max(1)).unwrap_or(u32::MAX);
  match block {
    CommitBodyBlock::Paragraph { lines } => lines.iter().map(wrapped).sum(),
    CommitBodyBlock::List { items, .. } => items.iter().map(wrapped).sum(),
    CommitBodyBlock::Preformatted { text } => u32::try_from(text.lines().count().max(1)).unwrap_or(u32::MAX),
    CommitBodyBlock::Heading { .. } | CommitBodyBlock::Label { .. } | CommitBodyBlock::Rule => 1,
  }
}

/// A run of lines between blank lines, or the lines inside a fence
enum RawBlock<'a> {
  Lines(Vec<&'a str>),
  Fenced(Vec<&'a str>),
}

/// The fence a line opens or closes: ```` ``` ```` or `~~~` after its indent
fn fence_of(line: &str) -> Option<&'static str> {
  let trimmed = line.trim_start();
  ["```", "~~~"].into_iter().find(|fence| trimmed.starts_with(fence))
}

fn raw_blocks(body: &str) -> Vec<RawBlock<'_>> {
  let mut blocks = Vec::new();
  let mut current = Vec::new();
  let mut lines = body.split('\n');
  while let Some(line) = lines.next() {
    if let Some(fence) = fence_of(line) {
      if !current.is_empty() {
        blocks.push(RawBlock::Lines(std::mem::take(&mut current)));
      }
      let fenced: Vec<&str> = lines.by_ref().take_while(|line| !line.trim_start().starts_with(fence)).collect();
      blocks.push(RawBlock::Fenced(trim_blank_edges(fenced)));
    } else if line.trim().is_empty() {
      if !current.is_empty() {
        blocks.push(RawBlock::Lines(std::mem::take(&mut current)));
      }
    } else {
      current.push(line);
    }
  }
  if !current.is_empty() {
    blocks.push(RawBlock::Lines(current));
  }
  blocks
}

fn trim_blank_edges(mut lines: Vec<&str>) -> Vec<&str> {
  while lines.last().is_some_and(|line| line.trim().is_empty()) {
    lines.pop();
  }
  let start = lines.iter().position(|line| !line.trim().is_empty()).unwrap_or(lines.len());
  lines.split_off(start)
}

/// `lines` without the indent they all share (blank lines aside)
fn dedent(lines: &[&str]) -> Vec<String> {
  let common = lines
    .iter()
    .filter(|line| !line.trim().is_empty())
    .map(|line| line.len() - line.trim_start().len())
    .min()
    .unwrap_or(0);
  lines.iter().map(|line| line.get(common..).unwrap_or_default().trim_end().to_string()).collect()
}

/// The trailers of a block, each with its indented continuation lines joined: every line a trailer or (but the first) a
/// continuation, and one of them a token of words joined by `-` or the cherry-pick note; `None` otherwise
fn trailers(lines: &[&str]) -> Option<Vec<String>> {
  let mut trailers: Vec<String> = Vec::new();
  let mut named = false;
  for line in lines {
    match trailer(line) {
      Some(well_known) => {
        named |= well_known;
        trailers.push(line.trim_end().to_string());
      }
      None if line.starts_with(char::is_whitespace) => {
        let last = trailers.last_mut()?;
        last.push(' ');
        last.push_str(line.trim());
      }
      None => return None,
    }
  }
  named.then_some(trailers)
}

/// Whether `line` is a trailer, and whether one that only a trailer can be: `Some(true)` for the cherry-pick note and a
/// token of words joined by `-` (`Co-Authored-By`, `Signed-off-by`), `Some(false)` for a one-word token (`Fixes`)
fn trailer(line: &str) -> Option<bool> {
  if line.starts_with("(cherry picked from commit ") && line.trim_end().ends_with(')') {
    return Some(true);
  }
  let (token, value) = line.split_once(':')?;
  let words: Vec<&str> = token.split('-').collect();
  let is_token = token.starts_with(|c: char| c.is_ascii_alphanumeric()) && words.iter().all(|word| !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric()));
  (is_token && value.starts_with(char::is_whitespace) && !value.trim().is_empty()).then_some(words.len() >= 2)
}

/// The marker a list item starts with: whether it is numbered, its number, and the item text
fn list_item(line: &str) -> Option<(bool, u32, &str)> {
  let trimmed = line.trim_start();
  if is_rule(trimmed) {
    return None;
  }
  for marker in ["- ", "* ", "+ ", "• "] {
    if let Some(text) = trimmed.strip_prefix(marker) {
      return Some((false, 1, text.trim()));
    }
  }
  let digits = trimmed.len() - trimmed.trim_start_matches(|c: char| c.is_ascii_digit()).len();
  let rest = &trimmed[digits..];
  let text = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")).filter(|_| digits > 0)?;
  Some((true, trimmed[..digits].parse().unwrap_or(1), text.trim()))
}

/// `---`, `***`, `___`, also with spaces between
fn is_rule(trimmed: &str) -> bool {
  ['-', '*', '_'].iter().any(|&mark| {
    let marks = trimmed.chars().filter(|&c| c == mark).count();
    marks >= 3 && trimmed.chars().all(|c| c == mark || c == ' ')
  })
}

/// A Markdown heading: one to six `#`, then a space or nothing
fn is_heading(trimmed: &str) -> bool {
  let text = trimmed.trim_start_matches('#');
  let level = trimmed.len() - text.len();
  (1..=6).contains(&level) && (text.is_empty() || text.starts_with(' '))
}

/// The heading of a line: its level and its text, `None` for no heading or one without text
fn heading(trimmed: &str) -> Option<CommitBodyBlock> {
  let text = trimmed.trim_start_matches('#');
  let level = u8::try_from(trimmed.len() - text.len()).ok()?;
  (is_heading(trimmed) && !text.trim().is_empty()).then(|| CommitBodyBlock::Heading {
    level,
    text: text.trim().to_string(),
  })
}

/// A line a label heads: prose or code, not a heading, a rule, a quote line, a table row or a trailer
fn is_content(line: &str) -> bool {
  let trimmed = line.trim();
  !trimmed.is_empty() && !is_heading(trimmed) && !is_rule(trimmed) && !is_own_line(trimmed) && trailer(line).is_none()
}

/// A line that is a line of its own in prose: a quote line or a table row
fn is_own_line(trimmed: &str) -> bool {
  trimmed.starts_with('>') || trimmed.starts_with('|')
}

/// A line indented as code: four spaces or a tab
fn is_code_indented(line: &str) -> bool {
  line.starts_with("    ") || line.starts_with('\t')
}

/// The lines of a block as paragraphs, lists and preformatted runs
fn parse_block(lines: &[&str], blocks: &mut Vec<CommitBodyBlock>) {
  let mut prose = Prose::default();
  let mut preformatted: Vec<&str> = Vec::new();
  let mut list: Option<(bool, u32, Vec<String>)> = None;
  let flush_list = |list: &mut Option<(bool, u32, Vec<String>)>, blocks: &mut Vec<CommitBodyBlock>| {
    if let Some((ordered, start, items)) = list.take() {
      blocks.push(CommitBodyBlock::List { ordered, start, items });
    }
  };
  let flush_preformatted = |preformatted: &mut Vec<&str>, blocks: &mut Vec<CommitBodyBlock>| {
    if !preformatted.is_empty() {
      blocks.push(CommitBodyBlock::Preformatted {
        text: dedent(preformatted).join("\n"),
      });
      preformatted.clear();
    }
  };
  for (index, &line) in lines.iter().enumerate() {
    let trimmed = line.trim();
    // a `Label:` line right before an item heads the list: it ends the list or the prose before it; one that starts its
    // block or follows a list heads the paragraph or the code right after it too
    let heads_next = lines
      .get(index + 1)
      .is_some_and(|&next| list_item(next).is_some() || (prose.lines.is_empty() && is_content(next)));
    if !line.starts_with(char::is_whitespace) && lead_in(trimmed).is_some() && heads_next {
      prose.flush(blocks);
      flush_preformatted(&mut preformatted, blocks);
      flush_list(&mut list, blocks);
      blocks.push(CommitBodyBlock::Paragraph { lines: vec![trimmed.to_string()] });
      continue;
    }
    let block_start = is_heading(trimmed) || is_rule(trimmed) || is_own_line(trimmed);
    if let Some((ordered, number, text)) = list_item(line) {
      prose.flush(blocks);
      flush_preformatted(&mut preformatted, blocks);
      if list.as_ref().is_some_and(|(current, _, _)| *current != ordered) {
        flush_list(&mut list, blocks);
      }
      list.get_or_insert_with(|| (ordered, number, Vec::new())).2.push(text.to_string());
      continue;
    }
    if let Some((_, _, items)) = list.as_mut().filter(|_| !block_start) {
      let last = items.last_mut().expect("a list has an item");
      last.push(' ');
      last.push_str(trimmed);
      continue;
    }
    flush_list(&mut list, blocks);
    if is_code_indented(line) {
      prose.flush(blocks);
      preformatted.push(line);
      continue;
    }
    flush_preformatted(&mut preformatted, blocks);
    prose.push(line, blocks);
  }
  prose.flush(blocks);
  flush_preformatted(&mut preformatted, blocks);
  flush_list(&mut list, blocks);
}

/// The paragraph being read: its lines, and whether the next plain line continues the last one
#[derive(Default)]
struct Prose {
  lines: Vec<String>,
  joinable: bool,
}

impl Prose {
  fn push(&mut self, line: &str, blocks: &mut Vec<CommitBodyBlock>) {
    let trimmed = line.trim();
    if is_rule(trimmed) {
      self.flush(blocks);
      blocks.push(CommitBodyBlock::Rule);
      return;
    }
    if is_heading(trimmed) {
      self.flush(blocks);
      // a `#` without text is a line of prose
      blocks.push(heading(trimmed).unwrap_or_else(|| CommitBodyBlock::Paragraph { lines: vec![trimmed.to_string()] }));
      return;
    }
    let hard_break = line.ends_with("  ") || trimmed.ends_with('\\');
    let text = if hard_break { trimmed.trim_end_matches('\\').trim_end() } else { trimmed };
    let own = is_own_line(trimmed);
    match self.lines.last_mut() {
      Some(last) if self.joinable && !own => {
        last.push(' ');
        last.push_str(text);
      }
      _ => self.lines.push(text.to_string()),
    }
    self.joinable = !hard_break && !own;
  }

  fn flush(&mut self, blocks: &mut Vec<CommitBodyBlock>) {
    if !self.lines.is_empty() {
      blocks.push(CommitBodyBlock::Paragraph {
        lines: std::mem::take(&mut self.lines),
      });
    }
    self.joinable = false;
  }
}

#[cfg(test)]
#[path = "commit_body_test.rs"]
mod commit_body_test;
