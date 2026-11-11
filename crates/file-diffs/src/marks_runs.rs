//! The runs of one block of a diff, from the tokens of its two sides (`marks.rs`; openspec/specs/commit/quiet-changes/spec.md).
//!
//! 1. **The tokens of the code are aligned.** Lines whose code is the same on both sides (`compare` is not
//!    `Different`) pair token by token. Else the longest common subsequence of the keys of the tokens
//!    (`text_diff::lcs::diff::build_changes_ints_within`), within `MAX_BLOCK_TOKENS`, `MAX_TOKEN_EDITS` and the work
//!    left of the file; a block over a limit has no run. Tokens only one side has are put on whole lines where the
//!    subsequence leaves the choice (`LineEnds::slide`).
//! 2. **The block is cut at the line boundaries both sides share**: a boundary of each side that no token crosses,
//!    with as many paired tokens before it on both sides. Between two cuts stands a segment: lines of both sides
//!    with paired tokens, or lines without any (a zone: what stands between two paired tokens).
//! 3. **A segment is asked whole** (`code_symbols::tokens::compare`): the same code on both sides, no token on either
//!    side (blank lines), or different. The alignment only finds the cuts; what a run is, the tokens of its lines
//!    alone prove.
//! 4. **Segments with the same code in a row are cut into runs** (`runs_of_same_code`): the longest groups of them
//!    whose tokens are the same, comments too, are runs of formatting, with the blank lines inside them; what stands
//!    between two such groups differs in its comments alone and is a run of comments.
//! 5. **The ends of the block.** Blank lines at an end of the block join the run beside them when the code around the
//!    block reads the same there. A run that reaches an end of its block is a run only then (`same_before`,
//!    `same_after`): at its other ends stand lines of the same block that are no quiet run, where the reader stops.
//!
//! A run of formatting has a character that is no blank; a run always has lines on both sides.
//!
//! **A reading with the two names of a rename read as one** (`marks_renames`, the second reading of a block) says
//! which lines hold a place of a rename (`renamed`), and a run is of one kind there too: a group of the same tokens
//! is cut where it may be (`runs_of_group`) into the lines that hold a place, a run of the kind `Renamed`, and the
//! lines that hold none, a run of formatting as without the rename; lines that hold a place and differ in their
//! comments are in no run and end the run of comments beside them. The first reading of a block has no such lines.
//!
//! The alignment also hands out the tokens it put at the same place of the two sides without pairing them: where as
//! many tokens of each side stand between two paired ones, the first of one side with the first of the other, and so
//! on. Two names among them are what a rename is looked for in (`marks_renames`).

use crate::marks::{MAX_BLOCK_TOKENS, MAX_TOKEN_EDITS, has_proof};
use code_symbols::tokens::{Equality, Token, Tokens, compare, same_after, same_before};
use std::collections::HashMap;
use std::ops::Range;
use text_diff::lcs::diff::build_changes_ints_within;
use text_diff::model::{LineRun, RunKind};

/// The lines of a block on each side, from 0, the ends excluded
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BlockLines {
  pub old: Range<u32>,
  pub new: Range<u32>,
}

/// What a segment of a block is, by its tokens
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
  /// The same code on both sides: the same tokens, or the same but for its comments
  SameCode,
  /// No token on either side
  Blank,
  Different,
}

/// A run before the ends of its block are asked: the segments `first..=last` of the block
struct Found {
  kind: RunKind,
  first: usize,
  last: usize,
}

/// What a reading of a block asks of its lines beside their tokens
pub(crate) struct Asked<'a> {
  /// Whether lines of a side (`true` the new one) hold a character that is no blank
  pub has_text: &'a dyn Fn(bool, Range<u32>) -> bool,
  /// Whether lines of the old side hold a place of a rename, a name the tokens read as one with its name of the
  /// new side: never in the first reading of a block (`NO_RENAME`)
  pub renamed: &'a dyn Fn(Range<u32>) -> bool,
}

/// No line holds a place of a rename: the tokens are those of the texts
pub(crate) const NO_RENAME: &dyn Fn(Range<u32>) -> bool = &|_| false;

/// The runs of the block at `lines` of two texts with the tokens `old` and `new`, in the order of the lines; `None`
/// for a block over a limit of work, which is not compared: nothing is cut out of it. `asked` answers for the lines
/// what the tokens do not say; `work` is what is left of `MAX_FILE_TOKEN_WORK`, which a comparison of tokens takes
/// its part of. `replaced` is given each token of the old side with the token of the new side the alignment put at
/// its place without pairing the two. A run is quiet when its lines are proven (`has_proof`: every run of
/// formatting, and a run of a renamed name as far as the tokens go: its reader asks the places); whether its kind is
/// quiet its caller says
pub(crate) fn runs_of_block(old: &Tokens, new: &Tokens, lines: &BlockLines, asked: &Asked<'_>, work: &mut u64, replaced: &mut dyn FnMut(&Token, &Token)) -> Option<Vec<LineRun>> {
  let (old_at, new_at) = (old.places_in_lines(lines.old.clone()), new.places_in_lines(lines.new.clone()));
  let (old_tokens, new_tokens) = (&old.all()[old_at.clone()], &new.all()[new_at.clone()]);
  let [old_paired, new_paired] = paired(old_tokens, new_tokens, work, replaced)?;
  let segments = segments(
    &cuts(old, lines.old.clone(), old_at.start, old_tokens, &old_paired),
    &cuts(new, lines.new.clone(), new_at.start, new_tokens, &new_paired),
  );
  let classes: Vec<Class> = segments
    .iter()
    .map(|segment| {
      let (old_run, new_run) = (tokens_of(old, &segment.old), tokens_of(new, &segment.new));
      if old_run.is_empty() && new_run.is_empty() {
        Class::Blank
      } else if compare(old_run, new_run) == Equality::Different {
        Class::Different
      } else {
        Class::SameCode
      }
    })
    .collect();

  // the runs of each row of segments with the same code
  let rows = Rows {
    old,
    new,
    segments: &segments,
    classes: &classes,
    asked,
  };
  let mut found = Vec::new();
  let mut from = 0;
  for index in 0..=segments.len() {
    if classes.get(index).is_none_or(|class| *class == Class::Different) {
      rows.runs_of_same_code(from..index, &mut found);
      from = index + 1;
    }
  }

  // the code around the block reads the same before it, and after it
  let same_start = || same_before(&old.all()[..old_at.start], &new.all()[..new_at.start]);
  let same_end = || same_after(&old.all()[old_at.end..], &new.all()[new_at.end..]);
  let first_with_tokens = classes.iter().position(|class| *class != Class::Blank);
  let last_with_tokens = classes.iter().rposition(|class| *class != Class::Blank);
  let mut runs = Vec::new();
  for Found { kind, first, last } in found {
    let mut run = BlockLines {
      old: segments[first].old.start..segments[last].old.end,
      new: segments[first].new.start..segments[last].new.end,
    };
    if run.old.start == lines.old.start && run.new.start == lines.new.start {
      if !same_start() {
        continue;
      }
    } else if Some(first) == first_with_tokens && same_start() {
      // the blank lines before it reach the start of the block
      (run.old.start, run.new.start) = (lines.old.start, lines.new.start);
    }
    if run.old.end == lines.old.end && run.new.end == lines.new.end {
      if !same_end() {
        continue;
      }
    } else if Some(last) == last_with_tokens && same_end() {
      (run.old.end, run.new.end) = (lines.old.end, lines.new.end);
    }
    runs.push(LineRun {
      old_start: run.old.start,
      old_end: run.old.end,
      new_start: run.new.start,
      new_end: run.new.end,
      kind,
      quiet: has_proof(kind),
    });
  }
  Some(runs)
}

/// The segments of a block with what each is, and what its reading asks of their lines
struct Rows<'a> {
  old: &'a Tokens,
  new: &'a Tokens,
  segments: &'a [BlockLines],
  classes: &'a [Class],
  asked: &'a Asked<'a>,
}

impl Rows<'_> {
  /// The runs among the segments `row` of the block, a row of segments with the same code on both sides (none is
  /// `Different`), added to `found`. From each segment with tokens, the longest group of segments whose tokens are
  /// the same on both sides, comments too, is a run of formatting, the blank lines inside it with it: a comment that
  /// moved to the line before is in it, though the two lines by themselves differ in their comments. What stands
  /// between two such groups differs in its comments alone: a run of comments, when it has lines on both sides. A
  /// segment that holds a place of a rename is in no run of comments, and ends the one before it: its code is the
  /// same only by the rename
  fn runs_of_same_code(&self, row: Range<usize>, found: &mut Vec<Found>) {
    let Self {
      old,
      new,
      segments,
      classes,
      asked,
    } = *self;
    if row.is_empty() {
      return;
    }
    let (old_start, new_start) = (segments[row.start].old.start, segments[row.start].new.start);
    let row_end = &segments[row.end - 1];
    let (old_tokens, new_tokens) = (tokens_of(old, &(old_start..row_end.old.end)), tokens_of(new, &(new_start..row_end.new.end)));
    // the tokens of each side before the start of each segment of the row, and before its end
    let before = |index: usize| -> (usize, usize) {
      let (old_line, new_line) = segments
        .get(index)
        .filter(|_| index < row.end)
        .map_or((row_end.old.end, row_end.new.end), |it| (it.old.start, it.new.start));
      (tokens_of(old, &(old_start..old_line)).len(), tokens_of(new, &(new_start..new_line)).len())
    };

    // the segments since the last run of formatting
    let mut between: Option<usize> = None;
    let comments = |between: &mut Option<usize>, end: usize, found: &mut Vec<Found>| {
      let Some(first) = between.take() else { return };
      let Some(last) = (first..end).rev().find(|index| classes[*index] != Class::Blank) else {
        return;
      };
      let (old_lines, new_lines) = (segments[first].old.start..segments[last].old.end, segments[first].new.start..segments[last].new.end);
      if !old_lines.is_empty() && !new_lines.is_empty() && compare(tokens_of(old, &old_lines), tokens_of(new, &new_lines)) == Equality::CodeOnly {
        found.push(Found {
          kind: RunKind::Comments,
          first,
          last,
        });
      }
    };

    let mut index = row.start;
    while index < row.end {
      if classes[index] == Class::Blank {
        index += 1;
        continue;
      }
      let (old_from, new_from) = before(index);
      let same = old_tokens[old_from..].iter().zip(&new_tokens[new_from..]).take_while(|(old, new)| old.same(new)).count();
      // the ends of the segments with tokens that as many tokens of both sides stand before, all of them the same:
      // the last one ends the group, and at each the group may be cut into two that are the same tokens too
      let mut ends = Vec::new();
      for next in index + 1..=row.end {
        let (old_to, new_to) = before(next);
        if old_to - old_from > same {
          break;
        }
        if old_to - old_from == new_to - new_from && classes[next - 1] != Class::Blank {
          ends.push(next);
        }
      }
      match ends.last() {
        Some(&end) if (asked.has_text)(false, segments[index].old.start..segments[end - 1].old.end) => {
          comments(&mut between, index, found);
          self.runs_of_group(index, &ends, found);
          index = end;
        }
        _ => {
          if (asked.renamed)(segments[index].old.clone()) {
            comments(&mut between, index, found);
          } else {
            between.get_or_insert(index);
          }
          index += 1;
        }
      }
    }
    comments(&mut between, row.end, found);
  }

  /// The runs of a group of segments whose tokens are the same on both sides, added to `found`: `first` is its first
  /// segment and `ends` are the ends of its parts, the smallest rows of its segments that are the same tokens by
  /// themselves, the last one its own end. The group is one run of formatting when none of its lines holds a place
  /// of a rename: every group of a first reading. Else its parts that hold a place, in a row, are a run of the kind
  /// `Renamed`, and those that hold none a run of formatting, as they are without the rename: a line that changed in
  /// its formatting alone is no line of a rename for standing beside one. The blank lines between two such runs go
  /// with the formatting
  fn runs_of_group(&self, first: usize, ends: &[usize], found: &mut Vec<Found>) {
    let Self { segments, classes, asked, .. } = *self;
    let mut push = |from: usize, to: usize, renamed: bool| {
      if renamed || (asked.has_text)(false, segments[from].old.start..segments[to - 1].old.end) {
        found.push(Found {
          kind: if renamed { RunKind::Renamed } else { RunKind::Formatting },
          first: from,
          last: to - 1,
        });
      }
    };
    // where the run in the making starts, where the part does, and whether the parts of that run hold a place
    let (mut from, mut part, mut renamed) = (first, first, None);
    for &end in ends {
      let holds = (asked.renamed)(segments[part].old.start..segments[end - 1].old.end);
      if let Some(renamed) = renamed.filter(|renamed| *renamed != holds) {
        // a part ends at a segment with tokens, so the blank lines between the two start the later one
        let cut = if holds {
          (part..end).find(|index| classes[*index] != Class::Blank).unwrap_or(part)
        } else {
          part
        };
        push(from, cut, renamed);
        from = cut;
      }
      (part, renamed) = (end, Some(holds));
    }
    push(from, part, renamed == Some(true));
  }
}

/// The tokens of `lines` of a side; none of no lines (`Tokens::in_lines` of an empty range is the token across it)
fn tokens_of<'t>(tokens: &'t Tokens, lines: &Range<u32>) -> &'t [Token] {
  if lines.is_empty() { &[] } else { tokens.in_lines(lines.clone()) }
}

/// For each token of the code of each side, in order, whether the alignment pairs it with a token of the other side;
/// `None` when the block is over a limit. Sides with the same code pair token by token, without a comparison.
/// `replaced` is given the tokens of a change of as many tokens on each side, the first with the first and so on
fn paired(old: &[Token], new: &[Token], work: &mut u64, replaced: &mut dyn FnMut(&Token, &Token)) -> Option<[Vec<bool>; 2]> {
  let code = |tokens: &[Token]| tokens.iter().filter(|token| !token.is_comment()).count();
  let (old_count, new_count) = (code(old), code(new));
  if compare(old, new) != Equality::Different {
    return Some([vec![true; old_count], vec![true; new_count]]);
  }
  if old_count > MAX_BLOCK_TOKENS || new_count > MAX_BLOCK_TOKENS {
    return None;
  }
  let total = old_count + new_count;
  let cost = (total as u64).saturating_mul(total.min(MAX_TOKEN_EDITS) as u64);
  *work = work.checked_sub(cost)?;

  // the keys of the tokens as small numbers, the same number for the same token
  let mut numbers: HashMap<u64, u32> = HashMap::with_capacity(total);
  let mut number = |tokens: &[Token]| -> Vec<u32> {
    tokens
      .iter()
      .filter(|token| !token.is_comment())
      .map(|token| {
        let next = u32::try_from(numbers.len()).unwrap_or(u32::MAX);
        *numbers.entry(token.key()).or_insert(next)
      })
      .collect()
  };
  let (old_numbers, new_numbers) = (number(old), number(new));
  let changes = build_changes_ints_within(&old_numbers, &new_numbers, MAX_TOKEN_EDITS).ok()?;
  if changes.iter().any(|change| change.deleted == change.inserted) {
    let (old_code, new_code) = (code_of(old), code_of(new));
    for change in changes.iter().filter(|change| change.deleted == change.inserted) {
      for offset in 0..change.deleted {
        replaced(old_code[change.line0 + offset], new_code[change.line1 + offset]);
      }
    }
  }
  let (mut old_paired, mut new_paired) = (vec![true; old_count], vec![true; new_count]);
  let (old_lines, new_lines) = (LineEnds::of(old), LineEnds::of(new));
  // where the change before ends on each side
  let (mut old_low, mut new_low) = (0, 0);
  for (index, change) in changes.iter().enumerate() {
    let next = changes.get(index + 1);
    let (mut old_start, mut new_start) = (change.line0, change.line1);
    // tokens only one side has, between tokens that pair: they may stand a few tokens earlier or later
    if change.inserted == 0 {
      old_start = old_lines.slide(&old_numbers, old_start..old_start + change.deleted, old_low, next.map_or(old_count, |next| next.line0));
    } else if change.deleted == 0 {
      new_start = new_lines.slide(&new_numbers, new_start..new_start + change.inserted, new_low, next.map_or(new_count, |next| next.line1));
    }
    (old_low, new_low) = (old_start + change.deleted, new_start + change.inserted);
    old_paired[old_start..old_low].fill(false);
    new_paired[new_start..new_low].fill(false);
  }
  Some([old_paired, new_paired])
}

/// The tokens of the code among `tokens`: those that are no comments
fn code_of(tokens: &[Token]) -> Vec<&Token> {
  tokens.iter().filter(|token| !token.is_comment()).collect()
}

/// Where the lines of the tokens of the code of one side of a block start and end
struct LineEnds {
  /// For each token, whether it is the first of its line, and whether it is the last
  ends: Vec<(bool, bool)>,
}

impl LineEnds {
  fn of(tokens: &[Token]) -> Self {
    let code = code_of(tokens);
    let ends = (0..code.len())
      .map(|index| {
        let first = index == 0 || code[index - 1].end_line < code[index].line;
        let last = index + 1 == code.len() || code[index].end_line < code[index + 1].line;
        (first, last)
      })
      .collect();
    Self { ends }
  }

  /// Where the tokens at `at`, which only this side has, start when they are put on whole lines. A longest common
  /// subsequence pairs a token with any of its likes: of `b(1, 2)⏎c()` and `b(1, 2)`, the `)` of the other side
  /// pairs with the first `)` as well as with the last. The tokens may move over the paired tokens around them
  /// (between `low` and `high`, the changes beside them) while the token they pass is the one they take in. They
  /// stay where a line starts with them and one ends with them, else where either does
  fn slide(&self, numbers: &[u32], at: Range<usize>, low: usize, high: usize) -> usize {
    let on_lines = |start: usize, end: usize| u8::from(self.ends[start].0) + u8::from(self.ends[end - 1].1);
    if at.is_empty() {
      return at.start;
    }
    let mut best = (on_lines(at.start, at.end), at.start);
    let (mut start, mut end) = (at.start, at.end);
    while start > low && numbers[start - 1] == numbers[end - 1] {
      (start, end) = (start - 1, end - 1);
      if on_lines(start, end) > best.0 {
        best = (on_lines(start, end), start);
      }
    }
    let (mut start, mut end) = (at.start, at.end);
    while end < high && numbers[start] == numbers[end] {
      (start, end) = (start + 1, end + 1);
      if on_lines(start, end) > best.0 {
        best = (on_lines(start, end), start);
      }
    }
    best.1
  }
}

/// The line boundaries of one side of a block where it may be cut, each with the paired tokens before it: the start
/// of the block, every line start inside it that no token crosses, and its end. `first` is the place of the first
/// token of the block in the tokens of its text, `tokens` the tokens of the block, `paired` whether each token of its
/// code is paired
fn cuts(text: &Tokens, lines: Range<u32>, first: usize, tokens: &[Token], paired: &[bool]) -> Vec<(u32, usize)> {
  // the paired tokens among the first tokens of the block, by their number
  let mut before = Vec::with_capacity(tokens.len() + 1);
  let (mut count, mut code) = (0, 0);
  before.push(0);
  for token in tokens {
    if !token.is_comment() {
      count += usize::from(paired[code]);
      code += 1;
    }
    before.push(count);
  }
  let mut cuts = vec![(lines.start, 0)];
  for line in lines.start + 1..lines.end {
    if text.in_lines(line..line).is_empty() {
      let passed = text.places_in_lines(lines.start..line).end - first;
      cuts.push((line, before[passed]));
    }
  }
  cuts.push((lines.end, count));
  cuts
}

/// The segments of a block from the cuts of its two sides (`cuts`), in order: for each number of paired tokens both
/// sides have a cut at, the lines between the first and the last such cut (a zone, left out when it has no line), then
/// the lines as far as the next such number, which hold paired tokens on both sides
fn segments(old: &[(u32, usize)], new: &[(u32, usize)]) -> Vec<BlockLines> {
  // the first and the last cut of each number of paired tokens
  let zones = |cuts: &[(u32, usize)]| -> Vec<(usize, u32, u32)> {
    let mut zones: Vec<(usize, u32, u32)> = Vec::new();
    for &(line, paired) in cuts {
      match zones.last_mut() {
        Some(zone) if zone.0 == paired => zone.2 = line,
        _ => zones.push((paired, line, line)),
      }
    }
    zones
  };
  let (old, new) = (zones(old), zones(new));
  let mut segments = Vec::new();
  // where the segment in the making starts
  let mut from: Option<(u32, u32)> = None;
  let (mut old_index, mut new_index) = (0, 0);
  while let (Some(old_zone), Some(new_zone)) = (old.get(old_index), new.get(new_index)) {
    match old_zone.0.cmp(&new_zone.0) {
      std::cmp::Ordering::Less => old_index += 1,
      std::cmp::Ordering::Greater => new_index += 1,
      std::cmp::Ordering::Equal => {
        if let Some((old_from, new_from)) = from {
          segments.push(BlockLines {
            old: old_from..old_zone.1,
            new: new_from..new_zone.1,
          });
        }
        if old_zone.1 < old_zone.2 || new_zone.1 < new_zone.2 {
          segments.push(BlockLines {
            old: old_zone.1..old_zone.2,
            new: new_zone.1..new_zone.2,
          });
        }
        from = Some((old_zone.2, new_zone.2));
        (old_index, new_index) = (old_index + 1, new_index + 1);
      }
    }
  }
  segments
}
