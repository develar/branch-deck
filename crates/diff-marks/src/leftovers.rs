//! What the diff of a pair of blobs leaves to the other files of a subject it is in
//! (openspec/specs/commit/quiet-changes/spec.md, "Subject"; docs/design/diff-view.md, "Marks"): the pass over the
//! files of a commit, a range or the working tree (`marks_subject`) reads it of every pair and reads no blob again.
//!
//! - **The lines no mark took** (`line_diff::moves_across::left_rows`): the changed lines of each side that are in no
//!   run and in no move of the file, as hashed keys with their weight, in rows. A move between two files is found
//!   among them.
//! - **The names the file declares none of** (`undeclared`): they changed at every place of the file, and only the
//!   file that declares them says whether they were renamed. With them, **the runs of their blocks read once more**
//!   (`pending`): the blocks that hold a place of such a name, with those names read as one too, and of the runs of
//!   that reading those of the kind `Renamed` that lean on such a name. A run of them holds once every name it leans
//!   on is a rename of the subject, and no text is read for it then. No other run of that reading is kept: a run of
//!   formatting is quiet, and the files of a subject are put together after the rows of the file were drawn.
//! - **The declarations of the names that are not local** (`declarations`): the places the index of the repository
//!   is asked by, in its lines and columns, made here where the texts are, and whether the code that declares the
//!   name changed in more than names (`Declarations::redeclared`): the index is asked of no such name.
//! - **The renames the scopes of the file prove** (`proven`): what a run that holds a local name beside a name that
//!   is not local still needs to be proven whole.
//! - **The lines a move of the file rewrote** (`rewritten`, `line_diff::moves::rewritten_lines`): the edited lines of
//!   its moves that differ in more than whitespace. Every other line of a move reads as the same line at another
//!   place, and what a name on it means is asked of no reader.
//! - **Whether a side ends a line with a lone carriage return** (`lone_returns`): the comparison counts it as a line
//!   end and the index does not, so no line the index names of such a text is a line of the diff.
//!
//! It is a value of its own beside the diff, which Branch Deck keeps in memory (`file_diffs::service`,
//! `MarksMemory::leftovers`) and in the `file_leftovers` store, under the key of its diff and the version of its
//! model: a scan decodes the diff of a pair at every hit (`FileDiffs::blocks_of`) and never pays for what the pair
//! left. Nothing here depends on the subject: the same
//! pair stands in other commits beside other files.

use crate::index::Columns;
use crate::marks::PairMarks;
use crate::model::{DiffMarks, Rename};
use line_diff::model::{DiffBlock, LineRun};
use line_diff::moves::rewritten_lines;
use line_diff::moves_across::{LeftRow, left_rows};
use serde::{Deserialize, Serialize};

/// What the diff of a pair leaves to the other files of a subject
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leftovers {
  /// The changed lines of each side that no mark of the pair took, the old side first, in rows that can each hold
  /// a move between files
  pub lines: [Vec<LeftRow>; 2],
  /// The names that changed at every place of the file and are declared in none of them, by their first place
  pub undeclared: Vec<Rename>,
  /// The blocks that hold a place of an `undeclared` name, read with those names as one too
  pub pending: Vec<PendingBlock>,
  /// The places of the declarations of the renames of the marks that are not local, as the index is asked by them
  pub declarations: Vec<Declarations>,
  /// The renames of the marks, by their numbers, whose places the scopes of the file prove
  pub proven: Vec<u32>,
  /// Whether the old text, and the new one, ends a line with a carriage return that no line feed follows
  pub lone_returns: [bool; 2],
  /// The lines of each side, the old one first and each in order, that a move of the marks took and rewrote: an
  /// edited line of it whose two texts differ in more than whitespace, read as a changed line
  pub rewritten: [Vec<u32>; 2],
}

/// The runs of one block, by its number among the blocks of the diff, that it has once the names the file declares
/// none of are renames: those of the kind `Renamed` that lean on such a name, in the order of the lines
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingBlock {
  pub block: u32,
  pub runs: Vec<PendingRun>,
}

/// A run of a `PendingBlock`: it holds when every name of `names` is a rename
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingRun {
  /// Of the kind `Renamed` and never `quiet`: only the index proves a name another file declares
  pub run: LineRun,
  /// The `Leftovers::undeclared` names, by their numbers, that the run leans on, in order, one at least: those with
  /// a place in its lines, and the one whose place is the first token of the code after its block when the run
  /// reaches the end of the block (it is a run there only while the code after the block reads the same,
  /// `code_tokens::same_after`, which that name read as one makes it)
  pub names: Vec<u32>,
}

/// The declarations of one rename of the marks that is not local
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declarations {
  /// The number of the rename among `DiffMarks::renames`
  pub rename: u32,
  /// For each place of `Rename::declared`, in their order: the line and the column of the name in the old text,
  /// then in the new one, both from 1, the column in characters (`Columns::Characters`)
  pub places: Vec<[u32; 4]>,
  /// The code that declares the name changed in more than names (`PairMarks::redeclared`): what the new name
  /// declares is another thing than what the old one did, and the index is not asked of it
  pub redeclared: bool,
}

impl Leftovers {
  /// The marks of a diff with the blocks `blocks` of the texts `old` and `new`, and what the diff leaves beside
  /// them: `pair` is what `marks_of` read of it, `indented` whether the indentation of the file has a meaning
  /// (`marks::pair_indented`), as `marks_of` was told
  pub fn of(blocks: &[DiffBlock], pair: PairMarks, old: &str, new: &str, indented: bool) -> (DiffMarks, Self) {
    let PairMarks {
      marks,
      undeclared,
      pending,
      proven_renames,
      redeclared,
      proven: _,
    } = pair;
    let lone_returns = [has_lone_carriage_return(old), has_lone_carriage_return(new)];
    let left = Self {
      lines: left_rows(old, new, blocks, &marks.runs, &marks.moves, indented),
      declarations: declarations_of(&marks.renames, [old, new], lone_returns, &redeclared),
      undeclared,
      pending,
      proven: proven_renames,
      lone_returns,
      rewritten: rewritten_lines(old, new, &marks.moves),
    };
    (marks, left)
  }

  /// Its weight in the memory cache, roughly in bytes
  pub fn weight(&self) -> u32 {
    let rows: usize = self.lines.iter().flatten().map(|row| 32 + row.keys.len() * 12).sum();
    let renames: usize = self
      .undeclared
      .iter()
      .map(|it| 88 + it.old.len() + it.new.len() + (it.old_sites.len() + it.new_sites.len()) * 4)
      .sum();
    let pending: usize = self
      .pending
      .iter()
      .map(|block| 32 + block.runs.iter().map(|run| 48 + run.names.len() * 4).sum::<usize>())
      .sum();
    let declarations: usize = self.declarations.iter().map(|it| 32 + it.places.len() * 16).sum();
    let rewritten: usize = self.rewritten.iter().map(|lines| lines.len() * 4).sum();
    u32::try_from(64 + rows + renames + pending + declarations + self.proven.len() * 4 + rewritten).unwrap_or(u32::MAX)
  }
}

/// The places the index is asked by of the declarations of the `renames` that are not local, in the texts `old` and
/// `new`. A rename a place of which the index cannot be given is left out: its uses are never proven. So is every
/// rename of a text with a lone carriage return (`lone_returns`), where the lines of a view are not the lines of the
/// index
fn declarations_of(renames: &[Rename], [old, new]: [&str; 2], lone_returns: [bool; 2], redeclared: &[u32]) -> Vec<Declarations> {
  if !renames.iter().any(|rename| !rename.local) || lone_returns != [false; 2] {
    return Vec::new();
  }
  let place = |text: &str, sites: &[u32], site: u32| -> Option<[u32; 2]> {
    let at = site as usize * 3;
    let (line, column) = (*sites.get(at)?, *sites.get(at + 1)?);
    Some([line + 1, Columns::Characters.at(text, line, column)?])
  };
  (0u32..)
    .zip(renames)
    .filter(|(_, rename)| !rename.local)
    .filter_map(|(number, rename)| {
      let places: Option<Vec<[u32; 4]>> = rename
        .declared
        .iter()
        .map(|site| {
          let ([old_line, old_column], [new_line, new_column]) = (place(old, &rename.old_sites, *site)?, place(new, &rename.new_sites, *site)?);
          Some([old_line, old_column, new_line, new_column])
        })
        .collect();
      Some(Declarations {
        rename: number,
        places: places?,
        redeclared: redeclared.contains(&number),
      })
    })
    .collect()
}

/// Whether `text` ends a line with a carriage return that no line feed follows
fn has_lone_carriage_return(text: &str) -> bool {
  // nearly every text has no carriage return at all: the search for one character is the fast one
  text.contains('\r') && {
    let bytes = text.as_bytes();
    bytes.iter().enumerate().any(|(index, byte)| *byte == b'\r' && bytes.get(index + 1) != Some(&b'\n'))
  }
}
