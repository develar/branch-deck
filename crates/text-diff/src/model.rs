//! The diff of two texts for the views of the app: the changed blocks of lines with their kind and the changed ranges
//! inside their lines, built straight from the comparison. Branch Deck's own: the wire model that the backend sends as
//! JSON. The views take the texts by blob id and build the context, the hunks and the expansion from the blocks.
//!
//! Lines are IntelliJ's (`SourceText`): `\n`, `\r\n` and a lone `\r` end a line, and a text has one line more than it
//! has line ends. Line ends compare as equal; `TextDiff::line_ends` names them when the files differ in them.

use crate::cancellation::{Deadline, DiffTooBig};
use crate::comparison_util::is_equal_texts;
use crate::inline::{LineRanges, inline_pairs, ranges_dropped};
use crate::policy::{ComparisonPolicy, HighlightPolicy, WhitespacePolicy};
use crate::provider::{ViewFragment, compare_for_view};
use crate::text::{LineEndKind, SourceText};
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// The inner ranges are compared only when each side has at most this many lines
pub const MAX_INNER_LINES: usize = 20_000;
/// The inner ranges are compared only when each side has at most this many UTF-8 bytes
pub const MAX_INNER_BYTES: usize = 1 << 20;

/// The diff of two texts
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TextDiff {
  pub old_line_count: u32,
  pub new_line_count: u32,
  /// The changed lines of the new text under the policy: an `Ignored` block, and the empty line after a final line
  /// end, do not count
  pub additions: u32,
  /// The same for the old text
  pub deletions: u32,
  /// The line ends of the two files when they differ; `None` when they are the same, or a file has no line end
  pub line_ends: Option<LineEnds>,
  /// Whether each text ends with a line end, when only one does ("Newline at end of file added"); `None` when they agree,
  /// or a text is empty (an added or a deleted file)
  pub final_newline: Option<FinalNewline>,
  /// Whether the last line of each text is the empty line that does not count: the line after a final line end, or
  /// the one line of an empty text. A view hides it when no block touches it, so a file shows no empty last row
  pub empty_last_line: EmptyLastLine,
  /// The inner ranges were not compared (a side over a limit, or out of time): the blocks are still exact
  pub line_level_only: bool,
  /// In order and without overlap; the lines between two blocks are equal under the policy
  pub blocks: Vec<DiffBlock>,
  /// The declarations of the old text, for the labels of the hunks; empty when not known
  pub old_scopes: Vec<Scope>,
  pub new_scopes: Vec<Scope>,
  /// The blocks, by index, whose every changed line of each side is an import or a package statement or blank: a change
  /// of the imports alone, from the syntax of the texts; empty when not known
  pub import_blocks: Vec<u32>,
  /// What a reader may pass and what a view labels: the runs of lines that changed in their formatting or their
  /// comments alone, the moved lines, the renamed names, and the line the reading of each block starts at. The
  /// comparison fills `starts` alone; the file-diffs service fills the rest: the runs from the tokens of the texts,
  /// the moves from their lines (`moves::find_moves`)
  pub marks: DiffMarks,
}

/// The marks of a diff (openspec/specs/commit/quiet-changes/spec.md): they change no block. `starts` is the one source
/// of where a reader stops; the runs, the moves and the renames are what a view draws
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DiffMarks {
  /// By `old_start`, in the order of both sides, each inside one block, without overlap
  pub runs: Vec<LineRun>,
  /// By `new_start`, a line of a side in one move at most and in no run: the old lines of a move inside one block
  /// (where the lines left: nothing to read there), its new lines inside another (where they stand now: read there),
  /// the blank lines that moved with it counted beside them
  pub moves: Vec<LineMove>,
  pub renames: Vec<Rename>,
  /// Flat `[block, side, line]` for each block with something to read, in the order of the blocks: the index of the
  /// block, the side of the line (0 the old text, 1 the new one) and the 0-based line the reading starts at
  /// (`starts_of`). A block without an entry has nothing to read: an `Ignored` one, or one whose every new line is in
  /// a quiet run and every old line in a quiet run or in a move
  pub starts: Vec<u32>,
  /// What the other files of the subject say of this one (`SubjectMarks`): the lines that moved between two files
  /// and the names another file declares or the index of the repository checked. `None` in the diff of a pair of
  /// blobs alone, which is what the caches keep: the same pair stands in other commits beside other files. Boxed:
  /// nearly every diff in memory has none
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub subject: Option<Box<SubjectMarks>>,
}

/// What the pass over the files of a subject (a commit, a range, the working tree) adds to the marks of one of them
/// (openspec/specs/commit/quiet-changes/spec.md, "Subject"). A file of the subject is named by its index in the list
/// of its files (`FileDiffEvent::Files`). Nothing here is quiet by itself, and nothing here changes a line of the
/// views: the moves are labels at both their ends, and what the index confirmed of a name is in `confirmed`
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SubjectMarks {
  /// The old lines of this file that stand in another file of the subject now, by `start`, without overlap
  pub moved_out: Vec<FileMove>,
  /// The new lines of this file that stood in another file of the subject, by `start`, without overlap
  pub moved_in: Vec<FileMove>,
  /// For each rename of `DiffMarks::renames` that is not local, in their order: what the index says of its uses
  pub declared: Vec<RenameCheck>,
  /// The names that changed at every place of this file, which declares none of them, and that another file of the
  /// subject declares and renamed so, by their first place: the lines that changed in such a name alone are runs of
  /// the kind `Renamed` among `DiffMarks::runs`
  pub used: Vec<UsedRename>,
  /// The runs of `DiffMarks::runs`, by their numbers, that hold a place of a name that is not local and whose every
  /// place is proven: by the index for such a name, by the scopes of the file for a local one. Quiet; they came
  /// after the rows of the file, so a view passes them and still shows each of their lines
  pub confirmed: Vec<u32>,
}

/// One end of lines that moved between two files of a subject without a change: `len` lines from `start` of this
/// file (0-based: old lines where they left, new lines where they stand now) are the lines from `line` of the file
/// `file` of the subject, on its other side. The same lines but for the spaces and tabs at their ends; where the
/// indentation of the file has a meaning, at the same depth too. Never quiet, and no line of it is hidden: both
/// ends are read
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FileMove {
  pub start: u32,
  pub len: u32,
  /// The other file, by its index among the files of the subject
  pub file: u32,
  /// The first line of the other end, 0-based
  pub line: u32,
}

/// What the subject says of a rename of `DiffMarks::renames` that is not local: how far it reaches in the files of
/// the subject, and what the index says of its uses
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RenameCheck {
  /// The number of the rename among `DiffMarks::renames`
  pub rename: u32,
  pub proof: RenameProof,
  /// The places that hold the two names in the files of the subject: those of every file that declares the name and
  /// renamed it so, this one among them, and of every file that uses it and names it a rename (`used`). What the
  /// proof speaks of, where it is one: a view says it beside the rename, where the name is declared
  pub sites: u32,
  /// The files of the subject those places are in, this one among them: 1 for a name no other file renamed so
  pub files: u32,
}

/// A name renamed at every place of a file that another file of the subject declares
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UsedRename {
  /// Its places in this file; `declared` is empty
  pub rename: Rename,
  /// The file that declares it, by its index among the files of the subject: the first one that does
  pub file: u32,
  /// The new line of its declaration there, 0-based
  pub line: u32,
  pub proof: RenameProof,
}

/// Whether the uses of a renamed name that is not local are proven to be the rename and nothing else, and why not:
/// what a view says beside the name. Only `Confirmed` proves anything; every other one leaves the rename a label
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum RenameProof {
  /// The index of the repository is there to be asked: the window that shows the file asks it
  /// (`get_rename_proofs`), and the marks of the file come once more with its answer
  Asked,
  /// The index answered that the two names are one declaration, that it listed every use of it in both trees, that
  /// each of them is a place of this rename in the subject or stands on a line that is read anyway, and that no name
  /// means something else by the rename
  Confirmed,
  /// A use of the name stands where the subject did not change it: in a file it left alone, on a line it left
  /// alone, or on a line it only moved
  UsesOutside,
  /// The rename changes what a name means: a use that names another declaration now, an overload, an override, a
  /// declaration it hides or that hides it, a namesake
  OtherMeaning,
  /// The two names are not one declaration: its kind, its owner or its signature changed with the name, as the
  /// index says; or the code that declares the name changed in more than names (a constant with another value, a
  /// function with another body), which the diff of its file says, and no index is asked then
  OtherDeclaration,
  /// The index did not list every use, or tied one by its name alone, or reads the language of the declaration
  /// without the scopes that would prove a use, or lists a use in a text whose lines it counts otherwise than the
  /// diff does (a lone carriage return), or by another word that the other tree does not hold at its place
  Incomplete,
  /// No index answered: none on this machine, one that knows no such answer, no index of the repository, a subject
  /// it cannot be asked of (the working tree), or a declaration it cannot be asked by its place
  Unavailable,
}

/// A run of lines inside one block that changed in one way alone: the old lines `[old_start, old_end)` became the
/// new lines `[new_start, new_end)` (0-based, the ends exclusive)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LineRun {
  pub old_start: u32,
  pub old_end: u32,
  pub new_start: u32,
  pub new_end: u32,
  pub kind: RunKind,
  /// The run provably changes no behaviour: a reader passes it, the navigation of changes does not stop in it, and
  /// the Inline view shows its new lines alone. A run that is not quiet is a label
  pub quiet: bool,
}

/// What alone changed in a run
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum RunKind {
  /// The same code and the same comments: line breaks, indents, spaces between tokens
  Formatting,
  /// The same code, other comments
  Comments,
  /// The same code but for a renamed name
  Renamed,
}

/// `len` lines that moved from `old_start` of the old text to `new_start` of the new one (0-based): old line
/// `old_start + i` is new line `new_start + i`. Never quiet: the new lines are read where they stand now. The blank
/// lines that left with it and stand at its other end now are counted beside its lines (`blanks_before`,
/// `blanks_after`): they are lines of the move on both sides, and no pair of its lines
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LineMove {
  pub old_start: u32,
  pub new_start: u32,
  pub len: u32,
  /// The columns of whitespace its lines gained at their start, negative for lost ones: "re-indented". Of every line
  /// that is the same on both sides where the indentation of the file has a meaning, of most of them elsewhere
  pub indent: i32,
  /// The blank lines right before its old lines that stand right after its new lines: the old lines
  /// `[old_start - blanks_before, old_start)`, the new lines `[new_start + len, new_start + len + blanks_before)`
  pub blanks_before: u32,
  /// The blank lines right after its old lines that stand right before its new lines: the old lines
  /// `[old_start + len, old_start + len + blanks_after)`, the new lines `[new_start - blanks_after, new_start)`
  pub blanks_after: u32,
  /// The lines of the move that were edited on the way, by their place in it from 0, in order: a line whose text
  /// changed, a line whose indent or whose whitespace after its text changed otherwise than the move says where the
  /// policy of the diff counts that whitespace, and a line whose indent did not move as the indents of the move
  /// where the indentation of the file has a meaning (its nesting changed, also when no character of it did)
  pub edited: Vec<u32>,
  /// For each line of the move, `[start, end, kind, ...]` as the ranges of a `DiffBlock`, the two lines of a pair
  /// with as many ranges of the same kinds, each a text that is another on the other side: what an edited line
  /// changed in. A line without a range is the same on both sides but for the indent of the move, and for the
  /// whitespace at its ends where the policy of the diff leaves that out. Empty lists for a move none of whose
  /// lines has a range
  pub old_ranges: Vec<Vec<u32>>,
  pub new_ranges: Vec<Vec<u32>>,
}

impl LineMove {
  /// Its old lines with the blank lines that left with it
  pub const fn old_lines(&self) -> Range<u32> {
    self.old_start - self.blanks_before..self.old_start + self.len + self.blanks_after
  }

  /// Its new lines with the blank lines that stand beside them now
  pub const fn new_lines(&self) -> Range<u32> {
    self.new_start - self.blanks_after..self.new_start + self.len + self.blanks_before
  }
}

/// A name that the change renamed: one declaration inside the body of a function with every name bound to it
/// (`local`), else every name of that spelling in the file that no such declaration binds. Its places are the same
/// on both sides, in the order of the texts: place `i` of the old text is place `i` of the new one, both in one
/// block. A place whose line changed in nothing else is in a run of the kind `Renamed`; a place that makes more of
/// the name than its binding (a shorthand property, a destructuring), the declaration of a name that is not local
/// and a place on a line with another change are in none, and are read
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Rename {
  pub old: String,
  pub new: String,
  /// How many places hold the name
  pub sites: u32,
  /// The name is bound inside the body of a function
  pub local: bool,
  /// The places that declare the name, by their number among the places from 0, in order: where the words of the
  /// rename stand. A rename among the marks of a diff has one at least
  pub declared: Vec<u32>,
  /// Flat `[line, start, end]` of each place: the 0-based line and the UTF-16 offsets inside it, the end exclusive
  pub old_sites: Vec<u32>,
  pub new_sites: Vec<u32>,
}

impl Rename {
  /// The lines of its places on each side, the old one first, from the first place to the last: what lies between
  /// the declaration of the name and its last use
  pub fn spans(&self) -> [Range<u32>; 2] {
    let span = |sites: &[u32]| {
      let lines = || sites.as_chunks::<3>().0.iter().map(|site| site[0]);
      lines().min().map_or(0..0, |first| first..lines().max().unwrap_or(first) + 1)
    };
    [span(&self.old_sites), span(&self.new_sites)]
  }
}

/// A block of changed lines
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DiffBlock {
  /// 0-based line indices, the end exclusive; `start == end` is a side without lines
  pub old_start: u32,
  pub old_end: u32,
  pub new_start: u32,
  pub new_end: u32,
  pub kind: BlockKind,
  /// The Inline view may draw pair i as one row (`pairs_align`): the sides have the same number of lines, each pair is
  /// similar, each inner change lies in one pair, and outside its changes a pair is the same text on both sides but
  /// for whitespace. So the ranges of line i of both sides align by index, and the row of a pair shows all of both
  /// its lines.
  pub inline_pairs: bool,
  /// For each line of the old side, `[start, end, kind, start, end, kind, ...]`: UTF-16 offsets inside the line, the
  /// end exclusive, never the line end, and the `RangeKind` of the inner change, as IntelliJ types a fragment.
  /// `start == end` marks the empty side of an inserted or a deleted change, which a view draws as a thin bar. Empty
  /// when the block has no inner ranges: `Ignored`, `WhitespaceOnly`, ranges dropped, or `line_level_only`.
  pub old_ranges: Vec<Vec<u32>>,
  pub new_ranges: Vec<Vec<u32>>,
}

/// The kind of an inner change of a block by its sides, as IntelliJ's `DiffUtil.getDiffType` types a fragment; its
/// number is the third value of each range of `DiffBlock`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeKind {
  /// The old side is empty
  Inserted = 0,
  /// The new side is empty
  Deleted = 1,
  Modified = 2,
}

/// How a view draws a block
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum BlockKind {
  /// A change
  Changed,
  /// The lines are equal under the policy, an empty list of inner changes (whitespace only, or a blank line under
  /// `Ignore`): drawn pale, still a change that counts and that the navigation stops at, as in IntelliJ
  WhitespaceOnly,
  /// `IgnoreBlankLines` cut the block off the ends of a run of changes, where IntelliJ drops it: drawn as unchanged,
  /// not counted, and the change navigation skips it. A view needs it to align the two sides.
  Ignored,
}

/// The line ends of two files that differ in them
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LineEnds {
  pub old: LineEndKind,
  pub new: LineEndKind,
  /// The files differ in their line ends only
  pub only: bool,
}

/// Whether each text ends with a line end
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FinalNewline {
  pub old: bool,
  pub new: bool,
}

/// Whether the last line of each text is empty and does not count
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct EmptyLastLine {
  pub old: bool,
  pub new: bool,
}

/// A declaration of a text: its lines and its label, `Outer › inner`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Scope {
  /// 0-based, the end exclusive
  pub start_line: u32,
  pub end_line: u32,
  pub label: String,
}

/// The time of one `diff_texts`
#[derive(Clone, Copy, Debug)]
pub struct DiffBudget {
  /// The deadline of the comparison with inner ranges; `None` compares the lines only
  pub inner: Option<Deadline>,
  /// The deadline of the line comparison, when the inner one was skipped or ran out of time. When it runs out too,
  /// the diff is one block of both whole texts.
  pub lines: Deadline,
}

impl DiffBudget {
  /// No time limit
  pub const NONE: Self = Self {
    inner: Some(Deadline::NONE),
    lines: Deadline::NONE,
  };
}

/// Compares two texts under `policy`, by word inside the changed lines (`diff_texts_in`)
pub fn diff_texts(old: &str, new: &str, policy: WhitespacePolicy, budget: &DiffBudget) -> TextDiff {
  diff_texts_in(old, new, policy, budget).diff
}

/// A diff, and whether the time of its budget cut it short
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetedDiff {
  pub diff: TextDiff,
  /// A deadline of the budget ran out: the inner ranges (the diff is by lines alone) or the lines (one block of both
  /// whole texts). A side over the size limits or a comparison of too many changes is a limit of the work, the same on
  /// any machine, and is not this
  pub out_of_time: bool,
}

/// Compares two texts under `policy`, by word inside the changed lines, within `budget`; says whether its time ran out
pub fn diff_texts_in(old: &str, new: &str, policy: WhitespacePolicy, budget: &DiffBudget) -> BudgetedDiff {
  let (old_source, new_source) = (SourceText::new(old), SourceText::new(new));
  let (old_text, new_text) = (old_source.text(), new_source.text());
  let fits = |text: &str, source: &SourceText| text.len() <= MAX_INNER_BYTES && source.line_count() <= MAX_INNER_LINES;

  let inner = budget.inner.filter(|_| fits(old, &old_source) && fits(new, &new_source));
  let mut line_level_only = inner.is_none();
  let mut out_of_time = false;
  let mut compare = |highlight: HighlightPolicy, deadline: &Deadline| match compare_for_view(old_text, new_text, policy, highlight, deadline) {
    Ok(fragments) => Some(fragments),
    Err(error) => {
      out_of_time |= error == DiffTooBig::DeadlineExceeded;
      None
    }
  };
  let mut fragments = inner.and_then(|deadline| compare(HighlightPolicy::ByWord, &deadline));
  if fragments.is_none() {
    line_level_only = true;
    fragments = compare(HighlightPolicy::ByLine, &budget.lines);
  }
  drop(compare);
  let blocks = match fragments {
    Some(fragments) => fragments.iter().map(|fragment| block_of(fragment, &old_source, &new_source)).collect(),
    // out of time: the whole texts, unless they are equal
    None if old_text == new_text => Vec::new(),
    None => vec![DiffBlock {
      old_start: 0,
      old_end: to_u32(old_source.line_count()),
      new_start: 0,
      new_end: to_u32(new_source.line_count()),
      kind: BlockKind::Changed,
      inline_pairs: false,
      old_ranges: Vec::new(),
      new_ranges: Vec::new(),
    }],
  };

  let counted = |source: &SourceText, lines: Vec<Range<u32>>| -> u32 {
    let limit = to_u32(source.counted_lines());
    lines.into_iter().map(|range| range.end.min(limit).saturating_sub(range.start)).sum()
  };
  let changed = || blocks.iter().filter(|block| block.kind != BlockKind::Ignored);
  let marks = DiffMarks {
    starts: starts_of(&blocks, &[], &[]),
    ..DiffMarks::default()
  };
  let diff = TextDiff {
    old_line_count: to_u32(old_source.line_count()),
    new_line_count: to_u32(new_source.line_count()),
    additions: counted(&new_source, changed().map(|block| block.new_start..block.new_end).collect()),
    deletions: counted(&old_source, changed().map(|block| block.old_start..block.old_end).collect()),
    line_ends: line_ends(&old_source, &new_source),
    final_newline: final_newline(&old_source, &new_source),
    empty_last_line: EmptyLastLine {
      old: has_empty_last_line(&old_source),
      new: has_empty_last_line(&new_source),
    },
    line_level_only,
    blocks,
    old_scopes: Vec::new(),
    new_scopes: Vec::new(),
    import_blocks: Vec::new(),
    marks,
  };
  BudgetedDiff { diff, out_of_time }
}

/// Where the reading of each block starts (`DiffMarks::starts`), from the quiet runs among `runs` (by `old_start`,
/// each inside one block, without overlap) and from `moves`. The quiet runs of a block cut it into parts in the
/// order of both sides; the reading starts at the first part with a line to read: at its first new line, at its
/// first old line that is in no move when it has no new one. The old lines of a move, with the blank lines that
/// left with it (`LineMove::old_lines`), are read where they stand now, so they are nothing to read where they
/// left; its new lines are in no quiet run and always read. An `Ignored`
/// block and a block without a line to read have no entry. The one place that reads `LineRun::quiet` and the moves
/// for the stops
pub fn starts_of(blocks: &[DiffBlock], runs: &[LineRun], moves: &[LineMove]) -> Vec<u32> {
  let mut starts = Vec::new();
  let mut quiet = runs.iter().filter(|run| run.quiet).peekable();
  // the old lines of the moves, in their order
  let mut left: Vec<Range<u32>> = moves.iter().map(LineMove::old_lines).collect();
  left.sort_unstable_by_key(|lines| lines.start);
  // the first of `lines` that no move took
  let first_kept = |lines: Range<u32>| {
    let mut line = lines.start;
    for moved in &left[left.partition_point(|moved| moved.end <= line)..] {
      if moved.start > line {
        break;
      }
      line = moved.end;
    }
    (line < lines.end).then_some(line)
  };
  for (index, block) in blocks.iter().enumerate() {
    let (mut old, mut new) = (block.old_start, block.new_start);
    let mut start = None;
    // the first part with a line to read between `old`, `new` and the next quiet run, or the end of the block
    let mut part = |old: u32, new: u32, old_end: u32, new_end: u32| {
      if start.is_none() {
        start = if new < new_end {
          Some((1, new))
        } else {
          first_kept(old..old_end).map(|line| (0, line))
        };
      }
    };
    while let Some(run) = quiet.next_if(|run| run.old_start < block.old_end || run.new_start < block.new_end) {
      part(old, new, run.old_start, run.new_start);
      (old, new) = (run.old_end.max(old), run.new_end.max(new));
    }
    part(old, new, block.old_end, block.new_end);
    if block.kind != BlockKind::Ignored
      && let Some((side, line)) = start
    {
      starts.extend([to_u32(index), side, line]);
    }
  }
  starts
}

fn final_newline(old: &SourceText, new: &SourceText) -> Option<FinalNewline> {
  let (old_end, new_end) = (old.ends_with_line_end(), new.ends_with_line_end());
  (old_end != new_end && !old.text().is_empty() && !new.text().is_empty()).then_some(FinalNewline { old: old_end, new: new_end })
}

/// The last line of the text is the empty one that does not count
pub(crate) fn has_empty_last_line(source: &SourceText) -> bool {
  source.counted_lines() < source.line_count()
}

fn line_ends(old: &SourceText, new: &SourceText) -> Option<LineEnds> {
  let (old_kind, new_kind) = (old.separator()?, new.separator()?);
  (old_kind != new_kind).then(|| LineEnds {
    old: old_kind,
    new: new_kind,
    only: old.text() == new.text(),
  })
}

fn block_of(view: &ViewFragment, old: &SourceText, new: &SourceText) -> DiffBlock {
  let fragment = &view.fragment;
  let mut block = DiffBlock {
    old_start: to_u32(fragment.start_line1),
    old_end: to_u32(fragment.end_line1),
    new_start: to_u32(fragment.start_line2),
    new_end: to_u32(fragment.end_line2),
    kind: BlockKind::Changed,
    inline_pairs: false,
    old_ranges: Vec::new(),
    new_ranges: Vec::new(),
  };
  if view.ignored {
    block.kind = BlockKind::Ignored;
    return block;
  }
  match &fragment.inner_fragments {
    // no similar parts, or not compared
    None => {}
    Some(inner) if inner.is_empty() => {
      block.kind = BlockKind::WhitespaceOnly;
      block.inline_pairs = pairs_align(old, new, fragment.start_line1..fragment.end_line1, fragment.start_line2..fragment.end_line2, &[], &[]);
    }
    Some(inner) => {
      let ranges1: Vec<Range<usize>> = inner
        .iter()
        .map(|it| fragment.start_offset1 + it.start_offset1..fragment.start_offset1 + it.end_offset1)
        .collect();
      let ranges2: Vec<Range<usize>> = inner
        .iter()
        .map(|it| fragment.start_offset2 + it.start_offset2..fragment.start_offset2 + it.end_offset2)
        .collect();
      let old_lines = line_ranges(old, fragment.start_line1..fragment.end_line1, &ranges1);
      let new_lines = line_ranges(new, fragment.start_line2..fragment.end_line2, &ranges2);
      if !ranges_dropped(&old_lines, &new_lines) {
        block.inline_pairs = inline_pairs(&old_lines, &new_lines)
          && pairs_align(
            old,
            new,
            fragment.start_line1..fragment.end_line1,
            fragment.start_line2..fragment.end_line2,
            &ranges1,
            &ranges2,
          );
        let kinds: Vec<RangeKind> = inner
          .iter()
          .map(|it| match (it.start_offset1 == it.end_offset1, it.start_offset2 == it.end_offset2) {
            (true, _) => RangeKind::Inserted,
            (_, true) => RangeKind::Deleted,
            _ => RangeKind::Modified,
          })
          .collect();
        block.old_ranges = typed_line_ranges(old, fragment.start_line1..fragment.end_line1, &ranges1, &kinds);
        block.new_ranges = typed_line_ranges(new, fragment.start_line2..fragment.end_line2, &ranges2, &kinds);
      }
    }
  }
  block
}

/// Whether line i of the old side and line i of the new side of a block are a pair that one row can show whole. The
/// inner changes are of the text of the whole block, not of a pair, so a change may take a line end with it or have
/// its sides in lines apart (a call wrapped again), and under `Ignore` a line end may move without a change. Such a
/// block has no pairs: a row of line i of both sides would lose the deleted text whose inserted side stands in
/// another row. The sides align when they have as many lines, each change lies within line i of both sides for one i
/// (an empty side in the first line that holds its offset, where `typed_line_ranges` puts its marker), and what a pair
/// holds between its changes is the same text on both sides but for whitespace.
fn pairs_align(old: &SourceText, new: &SourceText, old_lines: Range<usize>, new_lines: Range<usize>, old_ranges: &[Range<usize>], new_ranges: &[Range<usize>]) -> bool {
  if old_lines.len() != new_lines.len() {
    return false;
  }
  let (old_text, new_text) = (old.text(), new.text());
  let (old_offsets, new_offsets) = (old_text.line_offsets(), new_text.line_offsets());
  let same = |old_part: Range<usize>, new_part: Range<usize>| {
    let (old_part, new_part) = (&old_text.units()[old_part], &new_text.units()[new_part]);
    old_part == new_part || is_equal_texts(old_part, new_part, ComparisonPolicy::IgnoreWhitespaces)
  };
  let mut changes = old_ranges.iter().zip(new_ranges).peekable();
  for (old_line, new_line) in old_lines.zip(new_lines) {
    let (mut old_at, mut new_at) = (old_offsets.line_start(old_line), new_offsets.line_start(new_line));
    let (old_end, new_end) = (old_offsets.line_end(old_line), new_offsets.line_end(new_line));
    // the changes of this pair: the ones whose old side starts in its old line
    while let Some((old_range, new_range)) = changes.next_if(|(old_range, _)| old_range.start <= old_end) {
      let inside = old_range.start >= old_at && old_range.end <= old_end && new_range.start >= new_at && new_range.end <= new_end;
      if !inside || !same(old_at..old_range.start, new_at..new_range.start) {
        return false;
      }
      (old_at, new_at) = (old_range.end, new_range.end);
    }
    if !same(old_at..old_end, new_at..new_end) {
      return false;
    }
  }
  changes.next().is_none()
}

/// The ranges inside each line of `lines`, from ranges of text offsets; a range is cut at the line ends
pub(crate) fn line_ranges(source: &SourceText, lines: Range<usize>, ranges: &[Range<usize>]) -> Vec<LineRanges> {
  let offsets = source.text().line_offsets();
  lines
    .map(|line| {
      let (line_start, line_end) = (offsets.line_start(line), offsets.line_end(line));
      let ranges = ranges
        .iter()
        .filter_map(|range| {
          let (start, end) = (range.start.max(line_start), range.end.min(line_end));
          (start < end).then(|| (start - line_start, end - line_start))
        })
        .collect();
      LineRanges {
        length: line_end - line_start,
        ranges,
      }
    })
    .collect()
}

/// The typed ranges of each line of `lines`, `[start, end, kind, ...]`: a range is cut at the line ends; an empty range
/// (the empty side of an inserted or a deleted change) is a marker in the first line that holds its offset
fn typed_line_ranges(source: &SourceText, lines: Range<usize>, ranges: &[Range<usize>], kinds: &[RangeKind]) -> Vec<Vec<u32>> {
  let offsets = source.text().line_offsets();
  let mut placed = vec![false; ranges.len()];
  lines
    .map(|line| {
      let (line_start, line_end) = (offsets.line_start(line), offsets.line_end(line));
      let mut typed = Vec::new();
      for (index, (range, kind)) in ranges.iter().zip(kinds).enumerate() {
        let (start, end) = if range.is_empty() {
          if placed[index] || range.start < line_start || range.start > line_end {
            continue;
          }
          placed[index] = true;
          (range.start, range.start)
        } else {
          let (start, end) = (range.start.max(line_start), range.end.min(line_end));
          if start >= end {
            continue;
          }
          (start, end)
        };
        typed.extend([to_u32(start - line_start), to_u32(end - line_start), *kind as u32]);
      }
      typed
    })
    .collect()
}

pub(crate) fn flat_ranges(line: &LineRanges) -> Vec<u32> {
  line.ranges.iter().flat_map(|&(start, end)| [to_u32(start), to_u32(end)]).collect()
}

/// A line index or an offset; the texts are far below 4 GiB
pub(crate) fn to_u32(value: usize) -> u32 {
  u32::try_from(value).expect("a text below 4 GiB")
}
