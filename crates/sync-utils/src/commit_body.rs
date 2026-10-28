//! The grammar of a commit body (`CommitBodyBlock`), one for the commit preview of the page (`commit_body_blocks`) and
//! the body of a pull request (`remote_facts::pull_request_text`), which writes the blocks as Markdown:
//! - blocks are separated by blank lines; a fence (```` ``` ```` or `~~~`) starts a preformatted block that runs to the
//!   closing fence or the end of the body, blank lines included;
//! - a last block whose lines are all trailers (`Token: value`, the token letters, digits and `-`, an indented line
//!   continuing one; `(cherry picked from commit …)`), at least one of them a token of words joined by `-` or the
//!   cherry-pick note, is the trailers: a one-word `Note: …` alone is prose;
//! - inside a block, a line starting with `- `, `* `, `+ `, `• `, `1. ` or `1) ` (not a rule) starts a list, and every
//!   following line starts an item or continues the last one, until a heading, a quote, a table row or a rule; a
//!   change between bullets and numbers starts another list;
//! - a run of lines indented by four spaces or a tab is preformatted, without the common indent;
//! - the other lines are prose: hard-wrapped lines joined, a line after a hard break, a quote line and a table row a
//!   line of their own, a heading and a rule a paragraph of their own.

use domain_types::commit_body::CommitBodyBlock;

/// The body of `message`: the lines after its first one, the subject line the rows show
pub fn message_body(message: &str) -> &str {
  message.split_once('\n').map_or("", |(_, body)| body)
}

/// The blocks of `body`, a commit message without its subject line
pub fn body_blocks(body: &str) -> Vec<CommitBodyBlock> {
  let body = body.replace("\r\n", "\n");
  let raw_blocks = raw_blocks(&body);
  let mut blocks = Vec::new();
  let last = raw_blocks.len().saturating_sub(1);
  for (index, raw) in raw_blocks.into_iter().enumerate() {
    match raw {
      RawBlock::Fenced(lines) => blocks.push(CommitBodyBlock::Preformatted { text: dedent(&lines).join("\n") }),
      RawBlock::Lines(lines) => match trailers(&lines).filter(|_| index == last) {
        Some(trailers) => blocks.push(CommitBodyBlock::Trailers { lines: trailers }),
        None => parse_block(&lines, &mut blocks),
      },
    }
  }
  blocks
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
  for &line in lines {
    let trimmed = line.trim();
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
    if is_heading(trimmed) || is_rule(trimmed) {
      self.flush(blocks);
      blocks.push(CommitBodyBlock::Paragraph { lines: vec![trimmed.to_string()] });
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
