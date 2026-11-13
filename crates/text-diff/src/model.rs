//! The wire model of the diff of two texts: what the backend sends as JSON, the views read (`src/lib/bindings.ts`,
//! with the `specta` feature) and the `file_diff` store keeps. Branch Deck's own types, so that a change of the
//! comparison or of the marks changes no field, no name and no word of the bindings that this crate does not change:
//! the values come from `line_diff::model` (the blocks, the runs, the moves) and `diff_marks::model` (the marks), and
//! each type here maps to its value and back (`From`, field by field, so a field added there fails the build here
//! until it is described). The scopes and the blocks of the imports alone are the app's (`TextDiff::old_scopes`,
//! `import_blocks`): the file-diffs service fills them from the summaries of the texts.
//!
//! Lines are IntelliJ's (`line_diff::text::SourceText`): `\n`, `\r\n` and a lone `\r` end a line, and a text has one
//! line more than it has line ends. Line ends compare as equal; `TextDiff::line_ends` names them when the files
//! differ in them.

use serde::{Deserialize, Serialize};

// jscpd:ignore-start: the wire model describes the values of line-diff and diff-marks field by field, on purpose: the
// bindings and the stored layout change here alone
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
/// new lines `[new_start, new_end)` (0-based, the ends exclusive).
/// Lines on both sides, but for the part of a run read across the blocks the comparison cut out of one change
/// (`diff_marks::marks_joined`) in a block with lines on one side alone
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

/// The whitespace option of a diff view (`IgnorePolicy`)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum WhitespacePolicy {
  /// Every char counts
  #[default]
  Default,
  /// The whitespace at the start and the end of a line does not count
  Trim,
  /// No whitespace counts
  Ignore,
  /// No whitespace counts, and a block that changed only in whitespace or empty lines at its start or end is cut off
  /// (`IGNORE_WHITESPACES_CHUNKS`)
  IgnoreBlankLines,
}

/// The line ends of a whole file
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum LineEndKind {
  Lf,
  Crlf,
  Cr,
  /// More than one kind
  Mixed,
}

// jscpd:ignore-end

impl From<line_diff::model::LineDiff> for TextDiff {
  /// The diff of the comparison alone: no scopes, no blocks of the imports alone, and of the marks only where the
  /// reading of each block starts
  fn from(diff: line_diff::model::LineDiff) -> Self {
    let starts = line_diff::model::starts_of(&diff.blocks, &[], &[]);
    let line_diff::model::LineDiff {
      old_line_count,
      new_line_count,
      additions,
      deletions,
      line_ends,
      final_newline,
      empty_last_line,
      line_level_only,
      blocks,
    } = diff;
    Self {
      old_line_count,
      new_line_count,
      additions,
      deletions,
      line_ends: line_ends.map(Into::into),
      final_newline: final_newline.map(Into::into),
      empty_last_line: empty_last_line.into(),
      line_level_only,
      blocks: mapped(blocks),
      old_scopes: Vec::new(),
      new_scopes: Vec::new(),
      import_blocks: Vec::new(),
      marks: DiffMarks { starts, ..DiffMarks::default() },
    }
  }
}

/// Compares two texts under `policy` (`line_diff::model::diff_texts`): the diff of the comparison alone
pub fn diff_texts(old: &str, new: &str, policy: WhitespacePolicy, budget: &line_diff::model::DiffBudget) -> TextDiff {
  line_diff::model::diff_texts(old, new, policy.into(), budget).into()
}

impl TextDiff {
  /// The diff of the comparison this one holds, without its scopes and its marks: what the comparison and the
  /// marks read
  pub fn line_diff(&self) -> line_diff::model::LineDiff {
    line_diff::model::LineDiff {
      old_line_count: self.old_line_count,
      new_line_count: self.new_line_count,
      additions: self.additions,
      deletions: self.deletions,
      line_ends: self.line_ends.map(Into::into),
      final_newline: self.final_newline.map(Into::into),
      empty_last_line: self.empty_last_line.into(),
      line_level_only: self.line_level_only,
      blocks: self.line_blocks(),
    }
  }

  /// The blocks as the comparison and the marks read them
  pub fn line_blocks(&self) -> Vec<line_diff::model::DiffBlock> {
    mapped(self.blocks.clone())
  }
}

impl From<&DiffBlock> for line_diff::apply::LineBlock {
  /// The lines of a block, as the blocks are applied to a text
  fn from(block: &DiffBlock) -> Self {
    Self::of_lines([block.old_start, block.old_end, block.new_start, block.new_end])
  }
}

/// Each value of `values` as the type of the other side
pub fn mapped<A: Into<B>, B>(values: Vec<A>) -> Vec<B> {
  values.into_iter().map(Into::into).collect()
}

/// `From` both ways between a type of the model and its value, field by field: the destructuring names every field,
/// so a field the value gains fails the build until the model describes it
#[cfg_attr(not(feature = "marks"), expect(unused_macro_rules, reason = "the lists and the boxes are only of the marks"))]
macro_rules! mirror {
  ($wire:ident = $($value:ident)::+ { $($field:ident: $map:ident),* $(,)? }) => {
    impl From<$($value)::+> for $wire {
      fn from(value: $($value)::+) -> Self {
        let $($value)::+ { $($field),* } = value;
        Self { $($field: mirror!(@$map $field)),* }
      }
    }

    impl From<$wire> for $($value)::+ {
      fn from(value: $wire) -> Self {
        let $wire { $($field),* } = value;
        Self { $($field: mirror!(@$map $field)),* }
      }
    }
  };
  (@same $field:ident) => { $field };
  (@into $field:ident) => { $field.into() };
  (@vec $field:ident) => { mapped($field) };
  (@option $field:ident) => { $field.map(|it| Box::new((*it).into())) };
}

/// `From` both ways between an enum of the model and its value, variant by variant
macro_rules! mirror_enum {
  ($wire:ident = $($value:ident)::+ { $($variant:ident),* $(,)? }) => {
    impl From<$($value)::+> for $wire {
      fn from(value: $($value)::+) -> Self {
        type Value = $($value)::+;
        match value {
          $(Value::$variant => Self::$variant),*
        }
      }
    }

    impl From<$wire> for $($value)::+ {
      fn from(value: $wire) -> Self {
        match value {
          $($wire::$variant => Self::$variant),*
        }
      }
    }
  };
}

mirror_enum!(
  WhitespacePolicy = line_diff::policy::WhitespacePolicy {
    Default,
    Trim,
    Ignore,
    IgnoreBlankLines
  }
);
mirror_enum!(LineEndKind = line_diff::text::LineEndKind { Lf, Crlf, Cr, Mixed });
mirror_enum!(BlockKind = line_diff::model::BlockKind { Changed, WhitespaceOnly, Ignored });
mirror_enum!(RunKind = line_diff::model::RunKind { Formatting, Comments, Renamed });
mirror!(LineEnds = line_diff::model::LineEnds { old: into, new: into, only: same });
mirror!(FinalNewline = line_diff::model::FinalNewline { old: same, new: same });
mirror!(EmptyLastLine = line_diff::model::EmptyLastLine { old: same, new: same });
mirror!(
  DiffBlock = line_diff::model::DiffBlock {
    old_start: same,
    old_end: same,
    new_start: same,
    new_end: same,
    kind: into,
    inline_pairs: same,
    old_ranges: same,
    new_ranges: same,
  }
);
mirror!(
  LineRun = line_diff::model::LineRun {
    old_start: same,
    old_end: same,
    new_start: same,
    new_end: same,
    kind: into,
    quiet: same,
  }
);
mirror!(
  LineMove = line_diff::model::LineMove {
    old_start: same,
    new_start: same,
    len: same,
    indent: same,
    blanks_before: same,
    blanks_after: same,
    edited: same,
    old_ranges: same,
    new_ranges: same,
  }
);

/// The marks of `diff_marks::model` and their values: the crate of the marks is only there for a reader that makes
/// them (the file-diffs service), not for one that only merges
#[cfg(feature = "marks")]
mod marks {
  use super::{DiffMarks, FileMove, Rename, RenameCheck, RenameProof, SubjectMarks, UsedRename, mapped};

  mirror_enum!(
    RenameProof = diff_marks::model::RenameProof {
      Asked,
      Confirmed,
      UsesOutside,
      OtherMeaning,
      OtherDeclaration,
      Incomplete,
      Unavailable,
    }
  );
  mirror!(
    DiffMarks = diff_marks::model::DiffMarks {
      runs: vec,
      moves: vec,
      renames: vec,
      starts: same,
      subject: option,
    }
  );
  mirror!(
    SubjectMarks = diff_marks::model::SubjectMarks {
      moved_out: vec,
      moved_in: vec,
      declared: vec,
      used: vec,
      confirmed: same,
    }
  );
  mirror!(
    FileMove = diff_marks::model::FileMove {
      start: same,
      len: same,
      file: same,
      line: same
    }
  );
  mirror!(
    RenameCheck = diff_marks::model::RenameCheck {
      rename: same,
      proof: into,
      sites: same,
      files: same
    }
  );
  mirror!(
    UsedRename = diff_marks::model::UsedRename {
      rename: into,
      file: same,
      line: same,
      proof: into
    }
  );
  mirror!(
    Rename = diff_marks::model::Rename {
      old: same,
      new: same,
      sites: same,
      local: same,
      declared: same,
      old_sites: same,
      new_sites: same,
    }
  );
}
