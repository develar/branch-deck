//! What the diff of a pair of blobs leaves to the other files of a subject it is in
//! (openspec/specs/commit/quiet-changes/spec.md, "Subject"; docs/design/diff-view.md, "Marks"): the pass over the
//! files of a commit, a range or the working tree (`marks_subject`) reads it of every pair and reads no blob again.
//!
//! - **The lines no mark took** (`text_diff::moves_across::left_rows`): the changed lines of each side that are in no
//!   run and in no move of the file, as hashed keys with their weight, in rows. A move between two files is found
//!   among them.
//! - **The names the file declares none of** (`undeclared`): they changed at every place of the file, and only the
//!   file that declares them says whether they were renamed. With them, **the runs of their blocks read once more**
//!   (`pending`): the blocks that hold a place of such a name, with those names read as one too. A run of them holds
//!   once the names in it are renames of the subject, and no text is read for it then.
//! - **The declarations of the names that are not local** (`declarations`): the places the index of the repository
//!   is asked by, in its lines and columns, made here where the texts are.
//! - **The renames the scopes of the file prove** (`proven`): what a run that holds a local name beside a name that
//!   is not local still needs to be proven whole.
//!
//! It is a value of its own beside the diff, in memory (`MarksMemory::leftovers`) and in the `file_leftovers` store,
//! under the key of its diff and the version of its model: a scan decodes the diff of a pair at every hit
//! (`FileDiffs::blocks_of`) and never pays for what the pair left. Nothing here depends on the subject: the same
//! pair stands in other commits beside other files.

use crate::accio::Columns;
use crate::marks::PairMarks;
use serde::{Deserialize, Serialize};
use text_diff::model::{DiffBlock, DiffMarks, LineRun, Rename};
use text_diff::moves_across::{LeftRow, left_rows};

/// The leftovers that memory keeps, weighed by their size
pub const LEFTOVERS_CACHE_BYTES: u64 = 16 << 20;

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
}

/// The runs of one block, by its number among the blocks of the diff, read with the names the file declares none
/// of as one name each: all of them, in the order of the lines
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingBlock {
  pub block: u32,
  pub runs: Vec<PendingRun>,
}

/// A run of a `PendingBlock`: it holds when every name of `names` is a rename
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingRun {
  /// `quiet` when its lines are proven without the index: never with a name in `names`
  pub run: LineRun,
  /// The `Leftovers::undeclared` names, by their numbers, that have a place in its lines, in order
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
}

impl Leftovers {
  /// The marks of a diff with the blocks `blocks` of the texts `old` and `new`, and what the diff leaves beside
  /// them: `pair` is what `marks_of` read of it
  pub fn of(blocks: &[DiffBlock], pair: PairMarks, old: &str, new: &str) -> (DiffMarks, Self) {
    let PairMarks {
      marks,
      undeclared,
      pending,
      proven_renames,
      proven: _,
    } = pair;
    let left = Self {
      lines: left_rows(old, new, blocks, &marks.runs, &marks.moves),
      declarations: declarations_of(&marks.renames, old, new),
      undeclared,
      pending,
      proven: proven_renames,
    };
    (marks, left)
  }

  /// Its weight in the memory cache, roughly in bytes
  pub(crate) fn weight(&self) -> u32 {
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
    u32::try_from(64 + rows + renames + pending + declarations + self.proven.len() * 4).unwrap_or(u32::MAX)
  }
}

/// The places the index is asked by of the declarations of the `renames` that are not local. A rename a place of
/// which the index cannot be given is left out: its uses are never proven. So is every rename of a text with a lone
/// carriage return, where the lines of a view are not the lines of the index
fn declarations_of(renames: &[Rename], old: &str, new: &str) -> Vec<Declarations> {
  if !renames.iter().any(|rename| !rename.local) || has_lone_carriage_return(old) || has_lone_carriage_return(new) {
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
      Some(Declarations { rename: number, places: places? })
    })
    .collect()
}

/// Whether `text` ends a line with a carriage return that no line feed follows
fn has_lone_carriage_return(text: &str) -> bool {
  let bytes = text.as_bytes();
  bytes.iter().enumerate().any(|(index, byte)| *byte == b'\r' && bytes.get(index + 1) != Some(&b'\n'))
}
