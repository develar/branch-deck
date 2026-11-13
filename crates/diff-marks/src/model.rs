//! The marks of a diff as values (openspec/specs/commit/quiet-changes/spec.md): what `marks::marks_of` reads of a
//! pair of texts and `marks_subject` of the files of a subject. They change no block of the diff; `DiffMarks::starts`
//! is the one source of where a reader stops, the runs, the moves and the renames are what a view draws. A consumer
//! that sends them to another program or keeps them on disk describes them by types of its own.

use line_diff::model::{LineMove, LineRun};
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// The marks of a diff
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffMarks {
  /// By `old_start`, in the order of both sides, each inside one block, without overlap
  pub runs: Vec<LineRun>,
  /// By `new_start`, a line of a side in one move at most and in no run: the old lines of a move inside one block
  /// (where the lines left: nothing to read there), its new lines inside another (where they stand now: read there),
  /// the blank lines that moved with it counted beside them
  pub moves: Vec<LineMove>,
  pub renames: Vec<Rename>,
  /// Where the reading of each block starts (`line_diff::model::starts_of`): a block without an entry has nothing to
  /// read, an `Ignored` one, or one whose every new line is in a quiet run and every old line in a quiet run or in a
  /// move
  pub starts: Vec<u32>,
  /// What the other files of the subject say of this one (`SubjectMarks`). `None` in the marks of a pair of texts
  /// alone: the same pair stands in other subjects beside other files. Boxed: nearly every diff has none
  #[serde(default)]
  pub subject: Option<Box<SubjectMarks>>,
}

/// What the pass over the files of a subject (a commit, a range, the working tree) adds to the marks of one of them
/// (openspec/specs/commit/quiet-changes/spec.md, "Subject"). A file of the subject is named by its index in the list
/// of its files. Nothing here is quiet by itself, and nothing here changes a line of a view: the moves are labels at
/// both their ends, and what the index confirmed of a name is in `confirmed`
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
/// indentation of the file has a meaning, at the same depth too. Or, with `declaration`, a declaration the index of
/// the repository says moved whole to another file or under other owners with the same words, its layout maybe
/// another: the lines of the declaration at each end, `file` the file itself for a move under other owners within it.
/// Never quiet: both ends are read
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMove {
  pub start: u32,
  pub len: u32,
  /// The other file, by its index among the files of the subject
  pub file: u32,
  /// The first line of the other end, 0-based
  pub line: u32,
  /// The declaration the index says moved whole (`index::MovesAnswer`); `None` for a row of the same lines
  #[serde(default)]
  pub declaration: Option<DeclarationMove>,
}

/// A declaration that moved whole, as an end of its move names it (`FileMove::declaration`)
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclarationMove {
  /// Its name, as the index answers it
  pub name: String,
  /// The lines of the other end: another layout has another number of them
  pub other_len: u32,
}

/// What the subject says of a rename of `DiffMarks::renames` that is not local: how far it reaches in the files of
/// the subject, and what the index says of its uses
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameCheck {
  /// The number of the rename among `DiffMarks::renames`
  pub rename: u32,
  pub proof: RenameProof,
  /// The places that hold the two names in the files of the subject: those of every file that declares the name and
  /// renamed it so, this one among them, and of every file that uses it and names it a rename (`used`)
  pub sites: u32,
  /// The files of the subject those places are in, this one among them: 1 for a name no other file renamed so
  pub files: u32,
}

/// A name renamed at every place of a file that another file of the subject declares
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsedRename {
  /// Its places in this file; `declared` is empty
  pub rename: Rename,
  /// The file that declares it, by its index among the files of the subject: the first one that does
  pub file: u32,
  /// The new line of its declaration there, 0-based
  pub line: u32,
  pub proof: RenameProof,
  /// Of a name that is the owner of a declaration that moved whole under another owner, the name of that declaration:
  /// the uses of it in this file changed their owner from the old name to the new one, which nothing renamed. `file`
  /// and `line` are where the declaration stands now. `None` for a name another file declares
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub moved: Option<String>,
}

/// Whether the uses of a renamed name that is not local are proven to be the rename and nothing else, and why not.
/// Only `Confirmed` proves anything; every other one leaves the rename a label
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenameProof {
  /// The index of the repository is there to be asked, and the marks come once more with its answer
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
  /// index says; or the code that declares the name changed in more than names, which the diff of its file says,
  /// and no index is asked then
  OtherDeclaration,
  /// The index did not list every use, or tied one by its name alone, or reads the language of the declaration
  /// without the scopes that would prove a use, or lists a use in a text whose lines it counts otherwise than the
  /// diff does (a lone carriage return), or by another word that the other tree does not hold at its place
  Incomplete,
  /// No index answered, or none can be asked of the subject or of the declaration
  Unavailable,
}

/// A name that the change renamed: one declaration inside the body of a function with every name bound to it
/// (`local`), else every name of that spelling in the file that no such declaration binds. Its places are the same
/// on both sides, in the order of the texts: place `i` of the old text is place `i` of the new one, both in one
/// block. A place whose line changed in nothing else is in a run of the kind `Renamed`; a place that makes more of
/// the name than its binding (a shorthand property, a destructuring), the declaration of a name that is not local
/// and a place on a line with another change are in none, and are read
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
