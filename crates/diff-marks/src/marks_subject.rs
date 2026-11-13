//! The marks of the files of a subject that only its other files explain
//! (openspec/specs/commit/quiet-changes/spec.md, "Subject"; docs/design/diff-view.md, "Marks"): the lines that moved
//! from one file to another, and the names one file declares and others use. A pure function of what the pairs of
//! the subject are, in the order of git: their diffs with the marks of each pair (`marks.rs`) and what each pair
//! left (`leftovers`). No text is read here, and nothing here is kept with a pair: the same pair of blobs stands in
//! other commits beside other files.
//!
//! - **Moves between files** (`line_diff::moves_across`): rows of the same lines among the changed lines no mark of
//!   their pairs took, deleted in one file and added in another. A label at both ends (`SubjectMarks::moved_out`,
//!   `moved_in`): never quiet, no line of it hidden, no stop taken away.
//! - **Declarations moved whole** (`Subject::with_moves`, the answer of `accio moves`): a declaration the index says
//!   moved to another file or under other owners with the same words is a move between files of its lines, its
//!   layout another too (`FileMove::declaration`), where each end is the changed lines of one block that no mark of
//!   its pair took. One that a row of the same lines holds as it is stays that row, which names no declaration. One of
//!   another layout takes its lines: of a row that shares lines with it, the parts beside it stay moves where each is
//!   a row by itself (`LeftRow::without`), so no blank line or lone brace between two declarations is a move of its
//!   own. The same label as a row of the same lines, and a line of it is read as the line it was (`Subject::is_moved`).
//!   A declaration the index says was put in another order among those of its owner (`MovesAnswer::reordered`) is
//!   such a move within its file where its layout is another: the move of the pair does not hold it then.
//! - **A move of declarations whose order is free** (`SubjectMarks::reordered`, `reorder::ReorderMark`): a move within
//!   a file, of the pair or of a declaration to the file itself, whose every declaration the index reordered stands
//!   within both its ends with an order no reader of the code sees. A label: its notes say so, and its lines are read
//!   where they stand now as those of any move.
//! - **The owner in front of the uses of a declaration moved under another owner** (`Used::moves`,
//!   `UsedRename::moved`): a name that changed at every place of a file that declares neither, from the old owner of
//!   such a declaration to its new one, may follow the move. The index is asked of the uses of those declarations
//!   after the moves (`Subject::following_moves`, `move_questions`), and proves the name (`Subject::followed`) when
//!   the declaration moved with its words and its name and every use it lists is the declaration, a line read anyway,
//!   a line the change left alone with its twin, or behind an owner of the name with its twin behind the twin of the
//!   owner, and every place of the name is such an owner. Only a proven name is a mark (`Subject::with_followed`):
//!   the runs its blocks have with it read as one hold, quiet. One the index did not prove says nothing, since a
//!   label that a call follows a move must be true and the call may be of another declaration of the same owners.
//! - **A name another file declares** (`SubjectMarks::used`): a name that changed at every place of a file, which
//!   declares none of them (`Leftovers::undeclared`), is a rename once another file of the subject declares a name
//!   that is not local and renamed it so. The runs its blocks have with the name read as one, kept by the pair
//!   (`Leftovers::pending`), hold then: lines that changed in such a name alone are runs of the kind `Renamed`, and
//!   they add runs to those of the pair and take none (`marks_renames::with_the_first`). A run that leans on a name
//!   no file of the subject declares does not hold. Labels: the tie is of the two spellings. Nothing else is added:
//!   the quiet runs and the stops of a file are those of its pair, which went out with its rows.
//! - **How far a rename reaches** (`RenameCheck::sites`, `files`): the file that declares a renamed name says how
//!   many places of the files of the subject hold the two names, its own and those of every file that names the
//!   rename (`SubjectMarks::used`), and in how many files: what its proof speaks of. A file that uses the name and
//!   changed more than it (a call added or deleted) names no rename, and its places are not counted.
//! - **What the index proves** (`Subject::questions`, `IndexAnswer`): every declaration of a renamed name that is
//!   not local is asked of the index of the repository, all in one call. A declaration is proven renamed
//!   (`RenameProof::Confirmed`) when the index says that the two places are one declaration, that its answer is
//!   complete, that the rename changes the meaning of no name, and when every use it lists in either tree is the
//!   declaration, a place of the rename with its twin of the other tree listed too, a link of a doc comment, a use
//!   that writes no word (a positional argument that sets a property), a use by another word (an alias, an accessor)
//!   that the other tree lists at the same place, or stands on a line that is read anyway (a line of a change, in
//!   no run and in no move). Every other answer, and no answer, names why not. A name whose declaration changed in
//!   more than names (`Declarations::redeclared`: a constant with another value, a function with another body) is
//!   another thing under another name: it is not asked, and its proof is `OtherDeclaration` in every file that
//!   names it. The places a proven declaration lists
//!   are proven, and a run whose every place is proven, by the index or, for a local name, by the scopes of its
//!   file, is one of `SubjectMarks::confirmed`: quiet, as its kind is (`marks::is_quiet`). The declaration itself
//!   is in no run and stays a stop.
//! - **A line a move took is read as the line it was** (`Subject::is_moved`): a view shows it as the same line at
//!   another place, within its file or in another, and no reader asks what its names mean there. So a use on such a
//!   line is neither read anyway nor a place of a rename: the old name on it is a use the change left behind
//!   (`UsesOutside`), the new name one that meant something else (`OtherMeaning`). An edited line of a move that
//!   differs in more than whitespace is a changed line like any other (`Leftovers::rewritten`). A pale block
//!   (`WhitespaceOnly`) is the lines it was too.
//! - **Only Kotlin and Java are proven**: accio checks each tie by the scopes of the file for these two; of another
//!   language its `complete` is the word of its resolver (its decision 2026-11-11-rename-read-in-two-trees). So a
//!   name declared in a file of another language is not asked at all: its proof is `Incomplete` without a process.
//! - **The lines of the index are the lines of the diff** only in a text without a lone carriage return
//!   (`Leftovers::lone_returns`): a use the index lists in a text with one is on no line the diff knows, and the
//!   answer proves nothing (`Incomplete`).
//!
//! The answer of the index comes after the rows of the files (in Branch Deck `file_diffs::service::FileDiffs::index_marks`,
//! asked by the window that shows them), so nothing it says takes a line out of a view: a view passes a confirmed run and shows each of its
//! lines.

use crate::index::{Access, MovedDeclaration, MovesAnswer, RenameAnswer, RenamePair, RenameUse, RenameUses, RenamedDeclaration, Reordered, Resolved};
use crate::leftovers::{Leftovers, PendingRun};
use crate::marks::is_quiet;
use crate::marks_renames::with_the_first;
use crate::model::{DeclarationMove, DiffMarks, FileMove, Rename, RenameCheck, RenameProof, SubjectMarks, UsedRename};
use crate::reorder::ReorderMark;
use code_tokens::{Grammar, grammar_of};
use line_diff::model::to_u32;
use line_diff::model::{BlockKind, DiffBlock, LineMove, LineRun, RunKind, starts_of};
use line_diff::moves_across::{AcrossWork, FileRows, LeftRow, MoveAcross, find_moves_across};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

/// The languages whose uses accio ties by the scopes of their files, as it names them: only of these is a complete
/// answer a proof
const PROVEN_LANGUAGES: [&str; 2] = ["kotlin", "java"];

/// Whether the file at `path` is of a language of `PROVEN_LANGUAGES`: only a declaration of such a file is asked
fn is_of_proven_language(path: &str) -> bool {
  matches!(grammar_of(path), Some(Grammar::Kotlin | Grammar::KotlinScript | Grammar::Java))
}

/// A file of a subject whose content is a text: the blocks of its diff, the marks of its pair, and what the pair left
#[derive(Debug, Clone, Copy)]
pub struct PairFacts<'a> {
  /// The path of each side, as the trees of the subject hold it; `None` for a side the file does not exist on
  pub old_path: Option<&'a str>,
  pub new_path: Option<&'a str>,
  pub blocks: &'a [DiffBlock],
  pub marks: &'a DiffMarks,
  pub left: &'a Leftovers,
}

/// What the index of the repository said of the declarations of a subject (`Subject::questions`)
#[derive(Debug, Clone, Copy)]
pub enum IndexAnswer<'a> {
  /// It is being asked: its answer follows
  Asked,
  /// No index answers: none on this machine, one without the answer, no index of the repository, or a subject it
  /// cannot be asked of
  Unavailable,
  /// Its answer of each question, in their order
  Answered(&'a [RenameAnswer]),
}

/// A name that is not local among the renames of a file: one its marks hold (it declares the name), by its number
/// among them, or one it declares none of, by its number among `Leftovers::undeclared`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Name {
  Declared(u32),
  Used(u32),
}

/// A place of a rename that is not local, by its file among the files of the subject and its number among the
/// places of the rename
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Site {
  file: u32,
  name: Name,
  place: u32,
}

/// A place in a tree of the subject: the file among its files, the line from 0 and the UTF-16 columns of the name
type Place = (u32, u32, u32, u32);

/// The places of the renames of one pair of names in the files of the subject, by where they stand in each tree
#[derive(Debug, Default)]
struct Sites {
  old: HashMap<Place, Site>,
  new: HashMap<Place, Site>,
}

/// A name of a file that another file of the subject declares, or the owner of declarations that moved whole under
/// another owner (`moves`)
#[derive(Debug, Clone)]
struct Used {
  /// Its number among `Leftovers::undeclared`
  name: u32,
  /// The file that declares it and the new line of its declaration there; of an owner that follows moves, the file
  /// and the first line of the first of those declarations where it stands now
  file: u32,
  line: u32,
  /// The moves it follows, by their numbers among `Subject::move_questions`; empty for a name another file declares
  moves: Vec<u32>,
}

/// A declaration the index says moved whole under other owners, its name the same, whose uses a file of the subject
/// follows: the name of the owner in front of each use changed from the old owner to the new one
/// (`Subject::move_questions`). The index is asked of its uses by the places of the two declarations
#[derive(Debug, Clone)]
struct MoveQuestion {
  pair: RenamePair,
  /// The name of the declaration, and those of its old owner and of its new one
  name: String,
  owners: (String, String),
  /// Its files among the files of the subject, and its lines in each, from 0, the end exclusive
  old_file: u32,
  old: Range<u32>,
  new_file: u32,
  new: Range<u32>,
}

/// Which names of the files of a subject follow a move under another owner, as the index proved it
/// (`Subject::followed`): each by its file and its number among `Leftovers::undeclared`, with its places
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Followed {
  names: HashSet<(u32, u32)>,
  sites: HashSet<Site>,
}

impl Followed {
  /// Whether the index proved no name
  pub fn is_empty(&self) -> bool {
    self.names.is_empty()
  }
}

/// Which names of a file that may follow a move under another owner a subject reads as such
#[derive(Debug, Clone, Copy)]
enum Follow<'f> {
  None,
  All,
  /// Those of these files and numbers among `Leftovers::undeclared`
  Only(&'f HashSet<(u32, u32)>),
}

/// A file of the subject after the pass
#[derive(Debug)]
struct FileSubject<'a> {
  facts: PairFacts<'a>,
  /// The runs of its pair and those the names of `used` add, each quiet when its lines are proven without the index;
  /// borrowed while they are the runs of the pair
  runs: Cow<'a, [LineRun]>,
  moved_out: Vec<FileMove>,
  moved_in: Vec<FileMove>,
  used: Vec<Used>,
  /// The moves within it whose declarations were reordered where their order is free
  reordered: Vec<ReorderMark>,
}

/// A declaration the index is asked of: the place `place` of the rename `rename` of the marks of the file `file`
#[derive(Debug, Clone, Copy)]
struct Question {
  file: u32,
  rename: u32,
  place: u32,
}

/// The files of a subject put together: the moves between them and the names one declares and another uses. What
/// the index says of those names is read by `marks`
#[derive(Debug)]
pub struct Subject<'a> {
  files: Vec<Option<FileSubject<'a>>>,
  pairs: Vec<RenamePair>,
  questions: Vec<Question>,
  /// The declarations that moved whole under other owners whose uses a file follows, and their places as the index
  /// is asked of them
  move_pairs: Vec<RenamePair>,
  move_questions: Vec<MoveQuestion>,
  /// The places of the names that follow a move that the index proved (`with_followed`)
  followed_sites: HashSet<Site>,
  /// The places of the renames that are not local, by their two names
  sites: HashMap<(&'a str, &'a str), Sites>,
  /// The names, by the two of each, that a file of a language the index proves nothing of declares: not asked
  unread: HashSet<(&'a str, &'a str)>,
  /// The names, by the two of each, that a file declares by other code than before (`Declarations::redeclared`):
  /// another thing under another name, which no answer of the index makes a rename. Not asked
  redeclared: HashSet<(&'a str, &'a str)>,
  /// The file of each path of the old tree, and of the new one
  paths: [HashMap<&'a str, u32>; 2],
  /// What the search for the moves between the files cost and left out
  pub work: AcrossWork,
}

/// The names of a rename, the old one first
const fn names_of(rename: &Rename) -> (&str, &str) {
  (rename.old.as_str(), rename.new.as_str())
}

/// The lines of a run on a side
const fn side_of(run: &LineRun, new_side: bool) -> Range<u32> {
  if new_side { run.new_start..run.new_end } else { run.old_start..run.old_end }
}

impl<'a> Subject<'a> {
  /// The subject of the files `pairs`, in the order of git; `None` for a file without a text to compare
  pub fn of(pairs: &[Option<PairFacts<'a>>]) -> Self {
    Self::read(pairs, None, Follow::None)
  }

  /// The subject of the files `pairs`, as `of`, with the declarations the index says it moved whole (`moves`, the
  /// answer of `accio moves` between its two trees): each a move between files of its lines where no row of the same
  /// lines holds it as it is. No owner follows a move here: it is a mark only once the index proved it
  /// (`following_moves`, `with_followed`)
  pub fn with_moves(pairs: &[Option<PairFacts<'a>>], moves: &MovesAnswer) -> Self {
    Self::read(pairs, Some(moves), Follow::None)
  }

  /// `with_moves`, with every name of a file that may follow a move under another owner: what the index is asked of
  /// (`move_questions`) and what its answer proves (`followed`). Its marks are never shown: an owner is a label only
  /// where it is proven
  pub fn following_moves(pairs: &[Option<PairFacts<'a>>], moves: &MovesAnswer) -> Self {
    Self::read(pairs, Some(moves), Follow::All)
  }

  /// `with_moves`, with the names of `followed` (`following_moves` and `followed` of the same `pairs` and `moves`):
  /// each a name of its file that follows a move, proven, its runs quiet
  pub fn with_followed(pairs: &[Option<PairFacts<'a>>], moves: &MovesAnswer, followed: &Followed) -> Self {
    let mut subject = Self::read(pairs, Some(moves), Follow::Only(&followed.names));
    subject.followed_sites.clone_from(&followed.sites);
    subject
  }

  /// Whether a declaration of the subject may have moved whole: a file of a text deletes a line and one adds a line.
  /// Else the index is not asked of its moves
  pub fn may_move(&self) -> bool {
    let changed = |new_side: bool| {
      self.files.iter().flatten().any(|file| {
        file.facts.blocks.iter().any(|block| {
          let lines = if new_side { block.new_start..block.new_end } else { block.old_start..block.old_end };
          block.kind == BlockKind::Changed && !lines.is_empty()
        })
      })
    };
    changed(false) && changed(true)
  }

  fn read(pairs: &[Option<PairFacts<'a>>], moves: Option<&MovesAnswer>, follow: Follow<'_>) -> Self {
    // the files that declare a renamed name that is not local, by its two names
    let mut declaring: HashMap<(&str, &str), Vec<(u32, u32)>> = HashMap::new();
    for (file, facts) in pairs.iter().enumerate() {
      let renames = facts.iter().flat_map(|facts| (0u32..).zip(&facts.marks.renames));
      for (number, rename) in renames.filter(|(_, rename)| !rename.local) {
        declaring.entry(names_of(rename)).or_default().push((to_u32(file), number));
      }
    }
    let declaration_line = |file: u32, number: u32| -> Option<u32> {
      let rename = pairs.get(file as usize)?.as_ref()?.marks.renames.get(number as usize)?;
      rename.new_sites.get(*rename.declared.first()? as usize * 3).copied()
    };
    // the declarations the index says moved whole under other owners, by the names of the two owners: a name that
    // changed from the one to the other at every place of a file that declares neither follows such a move
    let candidates = moves.map_or_default(|moves| owner_moves(pairs, &moves.moved));
    let mut by_owners: HashMap<(&str, &str), Vec<u32>> = HashMap::new();
    for (number, candidate) in (0u32..).zip(&candidates) {
      by_owners.entry((candidate.owners.0.as_str(), candidate.owners.1.as_str())).or_default().push(number);
    }
    let mut asked: Vec<Option<u32>> = vec![None; candidates.len()];
    let mut move_questions: Vec<MoveQuestion> = Vec::new();

    let mut files: Vec<Option<FileSubject<'a>>> = (0u32..)
      .zip(pairs)
      .map(|(index, facts)| {
        let facts = (*facts)?;
        let used: Vec<Used> = (0u32..)
          .zip(&facts.left.undeclared)
          .filter_map(|(name, rename)| {
            if let Some(&(file, number)) = declaring.get(&names_of(rename)).and_then(|it| it.first()) {
              return Some(Used {
                name,
                file,
                line: declaration_line(file, number)?,
                moves: Vec::new(),
              });
            }
            let follows = match follow {
              Follow::None => false,
              Follow::All => true,
              Follow::Only(names) => names.contains(&(index, name)),
            };
            let followed = by_owners.get(&names_of(rename)).filter(|_| follows)?;
            let moves: Vec<u32> = followed
              .iter()
              .map(|&candidate| {
                *asked[candidate as usize].get_or_insert_with(|| {
                  move_questions.push(candidates[candidate as usize].clone());
                  to_u32(move_questions.len() - 1)
                })
              })
              .collect();
            let first = &candidates[followed[0] as usize];
            Some(Used {
              name,
              file: first.new_file,
              line: first.new.start,
              moves,
            })
          })
          .collect();
        Some(FileSubject {
          runs: merged_runs(&facts, &used),
          facts,
          moved_out: Vec::new(),
          moved_in: Vec::new(),
          used,
          reordered: Vec::new(),
        })
      })
      .collect();

    // the moves between the files, among the lines no run took: those of the pairs, and those added here. Then the
    // declarations the index says moved whole that no row of the same lines holds as it is (another layout), whose
    // lines are theirs: of a row that shares a line with one, the parts that are still a row. A declaration put in
    // another order among those of its owner is one too, within its file
    let whole: Vec<MovedDeclaration> = moves.map_or_default(|moves| {
      let reordered = moves.reordered.iter().map(|reordered| MovedDeclaration {
        old: reordered.old.clone(),
        new: reordered.new.clone(),
        file: false,
        owner: false,
      });
      moves.moved.iter().cloned().chain(reordered).collect()
    });
    let (found, declared, work) = {
      let rows: Vec<[Cow<'_, [LeftRow]>; 2]> = files.iter().map(|file| file.as_ref().map_or_else(Default::default, rows_of)).collect();
      let file_rows: Vec<FileRows<'_>> = rows.iter().map(|[old, new]| FileRows { old, new }).collect();
      let (found, work) = find_moves_across(&file_rows);
      let declared = declaration_moves(&files, &whole, &found);
      (outside_declarations(found, &declared, &rows), declared, work)
    };
    for moved in &declared {
      let (old, new) = (moved.old.clone(), moved.new.clone());
      let end = |lines: &Range<u32>, other: &Range<u32>, file: u32| FileMove {
        start: lines.start,
        len: lines.end - lines.start,
        file,
        line: other.start,
        declaration: Some(DeclarationMove {
          name: moved.name.to_string(),
          other_len: other.end - other.start,
        }),
      };
      if let Some(Some(file)) = files.get_mut(moved.old_file as usize) {
        file.moved_out.push(end(&old, &new, moved.new_file));
      }
      if let Some(Some(file)) = files.get_mut(moved.new_file as usize) {
        file.moved_in.push(end(&new, &old, moved.old_file));
      }
    }

    for found in found {
      if let Some(Some(file)) = files.get_mut(found.old_file as usize) {
        file.moved_out.push(FileMove {
          start: found.old_start,
          len: found.len,
          file: found.new_file,
          line: found.new_start,
          declaration: None,
        });
      }
      if let Some(Some(file)) = files.get_mut(found.new_file as usize) {
        file.moved_in.push(FileMove {
          start: found.new_start,
          len: found.len,
          file: found.old_file,
          line: found.old_start,
          declaration: None,
        });
      }
    }
    for file in files.iter_mut().flatten() {
      file.moved_out.sort_unstable_by_key(|moved| moved.start);
      file.moved_in.sort_unstable_by_key(|moved| moved.start);
    }
    if let Some(reordered) = moves.map(|moves| moves.reordered.as_slice()).filter(|reordered| !reordered.is_empty()) {
      for (index, file) in (0u32..).zip(files.iter_mut()) {
        if let Some(file) = file {
          file.reordered = reorder_marks(file, index, reordered);
        }
      }
    }

    let mut subject = Self {
      files,
      pairs: Vec::new(),
      questions: Vec::new(),
      move_pairs: move_questions.iter().map(|question| question.pair.clone()).collect(),
      move_questions,
      followed_sites: HashSet::new(),
      sites: HashMap::new(),
      unread: HashSet::new(),
      redeclared: HashSet::new(),
      paths: [HashMap::new(), HashMap::new()],
      work,
    };
    subject.read_names();
    subject
  }

  /// The places of the renames that are not local, the paths of the files, and the questions for the index
  fn read_names(&mut self) {
    // a name a file of a language the index proves nothing of declares is asked of no declaration: no answer
    // would prove its uses
    for facts in self.files.iter().flatten().map(|file| &file.facts) {
      let proven = facts.old_path.is_some_and(is_of_proven_language) && facts.new_path.is_some_and(is_of_proven_language);
      if !proven {
        self.unread.extend(facts.marks.renames.iter().filter(|rename| !rename.local).map(names_of));
      }
      // and a name that one of its declarations declares by other code is asked of none of them
      let redeclared = facts.left.declarations.iter().filter(|declarations| declarations.redeclared);
      self
        .redeclared
        .extend(redeclared.filter_map(|declarations| facts.marks.renames.get(declarations.rename as usize)).map(names_of));
    }
    for (index, file) in self.files.iter().enumerate() {
      let Some(file) = file else { continue };
      let (index, facts) = (to_u32(index), &file.facts);
      for (side, path) in [facts.old_path, facts.new_path].into_iter().enumerate() {
        // a path of the old tree is the old side of the file that stays there, before that of a copy of it
        if let Some(path) = path
          && (facts.old_path == facts.new_path || !self.paths[side].contains_key(path))
        {
          self.paths[side].insert(path, index);
        }
      }
      let declared = (0u32..)
        .zip(&facts.marks.renames)
        .filter(|(_, rename)| !rename.local)
        .map(|(number, rename)| (Name::Declared(number), rename));
      let used = file
        .used
        .iter()
        .filter_map(|used| Some((Name::Used(used.name), facts.left.undeclared.get(used.name as usize)?)));
      for (name, rename) in declared.chain(used) {
        let sites = self.sites.entry(names_of(rename)).or_default();
        for place in 0..rename.sites {
          let site = Site { file: index, name, place };
          if let (Some(old), Some(new)) = (place_in(&rename.old_sites, index, place), place_in(&rename.new_sites, index, place)) {
            sites.old.insert(old, site);
            sites.new.insert(new, site);
          }
        }
      }
      // a path with a tab or a line end cannot be written into the list of the pairs
      let listable = |path: &str| !path.contains(['\t', '\n', '\r']);
      let (Some(old_path), Some(new_path)) = (facts.old_path.filter(|path| listable(path)), facts.new_path.filter(|path| listable(path))) else {
        continue;
      };
      for declarations in &facts.left.declarations {
        let Some(rename) = facts
          .marks
          .renames
          .get(declarations.rename as usize)
          .filter(|rename| !self.unread.contains(&names_of(rename)) && !self.redeclared.contains(&names_of(rename)))
        else {
          continue;
        };
        for (place, [old_line, old_column, new_line, new_column]) in rename.declared.iter().zip(&declarations.places) {
          self.pairs.push(RenamePair {
            old: format!("{old_path}:{old_line}:{old_column}"),
            new: format!("{new_path}:{new_line}:{new_column}"),
          });
          self.questions.push(Question {
            file: index,
            rename: declarations.rename,
            place: *place,
          });
        }
      }
    }
  }

  /// The declarations of the subject the index is asked of, all in one call: each by the place of its name in the
  /// old tree and in the new one. None when the subject renamed no name that is not local in a file of a language
  /// the index proves the uses of
  pub fn questions(&self) -> &[RenamePair] {
    &self.pairs
  }

  /// The declarations that moved whole under other owners whose uses a file of the subject follows (a name that
  /// changed from the old owner to the new one at every place of the file), all in one call of the index after the
  /// moves: each by the line of its name in the old tree and in the new one. None without the moves of the index
  pub fn move_questions(&self) -> &[RenamePair] {
    &self.move_pairs
  }

  /// The marks of each file that the subject says something of, with what `answer` says of its names; `None` for a
  /// file whose marks are those of its pair. A name that follows a move is one the index proved (`with_followed`)
  pub fn marks(&self, answer: IndexAnswer<'_>) -> Vec<Option<DiffMarks>> {
    let checked = match answer {
      IndexAnswer::Answered(answers) => Some(self.check(answers)),
      IndexAnswer::Asked | IndexAnswer::Unavailable => None,
    };
    // the places proven, by a declaration renamed or by declarations moved
    let proven: Cow<'_, HashSet<Site>> = match &checked {
      Some(checked) if self.followed_sites.is_empty() => Cow::Borrowed(&checked.sites),
      Some(checked) => Cow::Owned(checked.sites.union(&self.followed_sites).copied().collect()),
      None => Cow::Borrowed(&self.followed_sites),
    };
    // the questions of each pair of names, by their numbers
    let mut questions: HashMap<(&str, &str), Vec<usize>> = HashMap::new();
    for (number, question) in self.questions.iter().enumerate() {
      if let Some(rename) = self.rename(question.file, Name::Declared(question.rename)) {
        questions.entry(names_of(rename)).or_default().push(number);
      }
    }
    let proof = |file: u32, name: Name, rename: &Rename| -> RenameProof {
      if self.redeclared.contains(&names_of(rename)) {
        return RenameProof::OtherDeclaration;
      }
      if self.unread.contains(&names_of(rename)) {
        return RenameProof::Incomplete;
      }
      let Some(asked) = questions.get(&names_of(rename)) else {
        return RenameProof::Unavailable;
      };
      let Some(checked) = &checked else {
        return if matches!(answer, IndexAnswer::Asked) {
          RenameProof::Asked
        } else {
          RenameProof::Unavailable
        };
      };
      let worst = asked.iter().filter_map(|number| checked.verdicts.get(*number)).copied().min_by_key(|proof| rank(*proof));
      match worst {
        None => RenameProof::Unavailable,
        // a place of the name that no declaration asked lists as its use is one the index tied to none of them
        Some(RenameProof::Confirmed) if !uses_of(rename).all(|place| checked.sites.contains(&Site { file, name, place })) => RenameProof::Incomplete,
        Some(worst) => worst,
      }
    };

    // how far each pair of names reaches in the subject: its places, and the files they are in
    let reach = |rename: &Rename| -> (u32, u32) {
      self.sites.get(&names_of(rename)).map_or((rename.sites, 1), |sites| {
        let files: HashSet<u32> = sites.new.values().map(|site| site.file).collect();
        (to_u32(sites.new.len()), to_u32(files.len()))
      })
    };

    (0u32..)
      .zip(&self.files)
      .map(|(index, file)| {
        let file = file.as_ref()?;
        let (pair, left) = (&file.facts.marks, file.facts.left);
        let declared: Vec<RenameCheck> = (0u32..)
          .zip(&pair.renames)
          .filter(|(_, rename)| !rename.local)
          .map(|(number, rename)| {
            let (sites, files) = reach(rename);
            RenameCheck {
              rename: number,
              proof: proof(index, Name::Declared(number), rename),
              sites,
              files,
            }
          })
          .collect();
        let used: Vec<UsedRename> = file
          .used
          .iter()
          .filter_map(|used| {
            let rename = left.undeclared.get(used.name as usize)?;
            let name = Name::Used(used.name);
            let followed = used.moves.first().and_then(|number| self.move_questions.get(*number as usize));
            Some(UsedRename {
              rename: rename.clone(),
              file: used.file,
              line: used.line,
              // a name that follows a move is here only where the index proved it
              proof: if used.moves.is_empty() { proof(index, name, rename) } else { RenameProof::Confirmed },
              moved: followed.map(|question| question.name.clone()),
            })
          })
          .collect();
        if file.moved_out.is_empty() && file.moved_in.is_empty() && declared.is_empty() && used.is_empty() && file.reordered.is_empty() {
          return None;
        }
        let confirmed = if checked.is_some() || !self.followed_sites.is_empty() {
          confirmed_runs(index, file, &proven)
        } else {
          Vec::new()
        };
        // the one place the kind of a run that came from what a pair left is asked
        let runs: Vec<LineRun> = file
          .runs
          .iter()
          .map(|run| LineRun {
            quiet: run.quiet && is_quiet(run.kind),
            ..run.clone()
          })
          .collect();
        let blocks = &file.facts.blocks;
        let marks = DiffMarks {
          starts: starts_of(blocks, &runs, &pair.moves),
          runs,
          moves: pair.moves.clone(),
          renames: pair.renames.clone(),
          subject: Some(Box::new(SubjectMarks {
            moved_out: file.moved_out.clone(),
            moved_in: file.moved_in.clone(),
            declared,
            used,
            confirmed,
            reordered: file.reordered.clone(),
          })),
        };
        Some(if is_quiet(RunKind::Renamed) { as_confirmed(&marks, blocks) } else { marks })
      })
      .collect()
  }

  /// A rename that is not local of a file
  fn rename(&self, file: u32, name: Name) -> Option<&'a Rename> {
    let facts = &self.files.get(file as usize)?.as_ref()?.facts;
    match name {
      Name::Declared(number) => facts.marks.renames.get(number as usize),
      Name::Used(number) => facts.left.undeclared.get(number as usize),
    }
  }

  /// Where a place of a rename stands in a tree of the subject
  fn place_of(&self, site: Site, new_side: bool) -> Option<Place> {
    let rename = self.rename(site.file, site.name)?;
    place_in(if new_side { &rename.new_sites } else { &rename.old_sites }, site.file, site.place)
  }

  /// Whether the line `line` of a side of the file `file` is read whatever a rename is: a line of a change, in no
  /// run and in no move. A reader may pass the lines of a run, and reads a pale block (equal to itself under the
  /// policy) and a block the policy cut off as the lines they were; a line a move took is the same line at another
  /// place to him (`is_moved`)
  fn is_read(&self, file: u32, new_side: bool, line: u32) -> bool {
    let Some(Some(subject)) = self.files.get(file as usize) else { return false };
    let blocks = &subject.facts.blocks;
    let lines = |block: &DiffBlock| if new_side { block.new_start..block.new_end } else { block.old_start..block.old_end };
    let changed = blocks
      .get(blocks.partition_point(|block| lines(block).end <= line))
      .is_some_and(|block| block.kind == BlockKind::Changed && lines(block).contains(&line));
    changed && !subject.runs.iter().any(|run| side_of(run, new_side).contains(&line)) && !self.is_moved(file, new_side, line)
  }

  /// Whether a move took the line `line` of a side of the file `file` as it is: a line of a move between two files
  /// (exact lines only), or of a move within the file that did not rewrite it (`Leftovers::rewritten`). A view shows
  /// such a line as the same line at another place, so a name on it is read as what it was: no use on it is read
  /// anyway, and none is a place of a rename
  fn is_moved(&self, file: u32, new_side: bool, line: u32) -> bool {
    let Some(Some(subject)) = self.files.get(file as usize) else { return false };
    let between = if new_side { &subject.moved_in } else { &subject.moved_out };
    let (pair, left) = (&subject.facts.marks, subject.facts.left);
    let within = |moved: &LineMove| if new_side { moved.new_lines() } else { moved.old_lines() }.contains(&line);
    between.iter().any(|moved| (moved.start..moved.start + moved.len).contains(&line))
      || (pair.moves.iter().any(within) && left.rewritten[usize::from(new_side)].binary_search(&line).is_err())
  }

  /// What the index proved: of each question whether its declaration is renamed and nothing else, and the places the
  /// proven ones list as uses in both trees
  fn check(&self, answers: &[RenameAnswer]) -> Checked {
    let mut checked = Checked::default();
    for (question, answer) in self.questions.iter().zip(answers) {
      let mut listed = Vec::new();
      let verdict = self.verdict(*question, answer, &mut listed);
      if verdict == RenameProof::Confirmed {
        checked.sites.extend(listed);
      }
      checked.verdicts.push(verdict);
    }
    checked
  }

  /// The line of the other side that the line `line` of a side of the file `file` is, when the change left it
  /// alone: a line of no block
  fn same_line(&self, file: u32, new_side: bool, line: u32) -> Option<u32> {
    let blocks = &self.files.get(file as usize)?.as_ref()?.facts.blocks;
    // the lines of a block on the side asked, and on the other one
    let sides = |block: &DiffBlock| {
      let (old, new) = (block.old_start..block.old_end, block.new_start..block.new_end);
      if new_side { (new, old) } else { (old, new) }
    };
    let before = blocks.partition_point(|block| sides(block).0.end <= line);
    if blocks.get(before).is_some_and(|block| sides(block).0.contains(&line)) {
      return None;
    }
    Some(match before.checked_sub(1).and_then(|last| blocks.get(last)) {
      Some(last) => {
        let (own, other) = sides(last);
        line - own.end + other.end
      }
      None => line,
    })
  }

  /// What the answer of the index says of a declaration; `listed` gets the places of renames it lists as a use in
  /// both trees
  fn verdict(&self, question: Question, answer: &RenameAnswer, listed: &mut Vec<Site>) -> RenameProof {
    if answer.unavailable {
      return RenameProof::Unavailable;
    }
    if !answer.same {
      return RenameProof::OtherDeclaration;
    }
    let declaration = Site {
      file: question.file,
      name: Name::Declared(question.rename),
      place: question.place,
    };
    let sites = self.rename(question.file, declaration.name).and_then(|rename| self.sites.get(&names_of(rename)));
    let proven_language = |declaration: &Option<RenamedDeclaration>| declaration.as_ref().is_some_and(|it| PROVEN_LANGUAGES.contains(&it.language.as_str()));
    // a use the change left behind in a file it did not touch is the index's own finding; every other conflict is a
    // name that means something else by the rename
    let mut outside = answer.conflicts.iter().any(|conflict| conflict.kind == "left");
    let mut meaning = answer.conflicts.iter().any(|conflict| conflict.kind != "left");
    let mut incomplete = !answer.complete || !proven_language(&answer.old) || !proven_language(&answer.new);
    if let (Some(sites), Some(base), Some(head)) = (sites, &answer.base, &answer.head) {
      incomplete |= base.uses.len() < base.total || head.uses.len() < head.total;
      let (base, head) = (self.uses_in(base, false), self.uses_in(head, true));
      // what the uses say by where they stand
      let (mut left, mut captured) = (false, false);
      for (new_side, uses, others) in [(false, &base, &head), (true, &head, &base)] {
        for (used, place) in &uses.uses {
          // a place of a rename on a line a move took as it is is none: the line stands at the other end of its
          // move with the name it had, which is no use of the declaration there
          let site = place
            .filter(|(file, line)| !self.is_moved(*file, new_side, *line))
            .map(|(file, line)| (file, line, used.column, used.end_column))
            .and_then(|place| if new_side { sites.new.get(&place) } else { sites.old.get(&place) });
          match (site, &used.text) {
            (Some(site), _) if *site == declaration => {}
            (Some(_), _) if used.resolved == Resolved::Name => incomplete = true,
            (Some(site), _) => match self.place_of(*site, !new_side) {
              Some(twin) if others.places.contains(&twin) => {
                if !new_side {
                  listed.push(*site);
                }
              }
              // the place of the other tree is no use of the declaration there: the name means something else at one
              _ => captured = true,
            },
            (None, _) if place.is_some_and(|(file, line)| self.is_read(file, new_side, line)) => {}
            // another word than the name (an alias, an accessor): the rename asks nothing of it where the other
            // tree has the same word at the same place, a line the change left alone. Else nothing is known of it
            (None, Some(word)) => {
              let twin = match *place {
                Some((file, line)) => self.same_line(file, new_side, line).map(|line| (Spot::File(file), line)),
                None => used.line.checked_sub(1).map(|line| (Spot::Path(&used.path), line)),
              };
              let same = twin.is_some_and(|(spot, line)| others.words.contains(&(spot, line, used.column, used.end_column, word.as_str())));
              incomplete |= !same;
            }
            // the old name where the change left the line alone or moved it as it is, or in a file it did not touch
            (None, None) if !new_side => left = true,
            // the new name where the change left the line alone or moved it as it is: it named something else before
            (None, None) => captured = true,
          }
        }
      }
      if base.unmapped || head.unmapped {
        // a use on a line the diff cannot name: where the others stand says nothing either
        incomplete = true;
      } else {
        outside |= left;
        meaning |= captured;
      }
    } else {
      incomplete = true;
    }
    if meaning {
      RenameProof::OtherMeaning
    } else if outside {
      RenameProof::UsesOutside
    } else if incomplete {
      RenameProof::Incomplete
    } else {
      RenameProof::Confirmed
    }
  }

  /// The names of the files that follow a move which `answers`, the answer of the index to `move_questions` in their
  /// order, proves: the index says of every declaration a name follows that it moved with its words and its name,
  /// that the answer is complete and changes the meaning of no name, and every use it lists in either tree is the
  /// declaration or one of its lines, stands on a line read anyway or a line the change left alone with its twin
  /// listed at the same place, or has an owner of the name right before it with its twin listed behind the twin of
  /// that owner; and every place of the name is such an owner
  pub fn followed(&self, answers: &[RenameAnswer]) -> Followed {
    let checked = self.check_moves(answers);
    let mut followed = Followed::default();
    for (index, file) in (0u32..).zip(&self.files) {
      let Some(file) = file else { continue };
      for used in file.used.iter().filter(|used| !used.moves.is_empty()) {
        let Some(rename) = file.facts.left.undeclared.get(used.name as usize) else { continue };
        let name = Name::Used(used.name);
        let proven = used.moves.iter().all(|number| checked.verdicts.get(*number as usize) == Some(&RenameProof::Confirmed));
        let sites: Vec<Site> = uses_of(rename).map(|place| Site { file: index, name, place }).collect();
        if proven && sites.iter().all(|site| checked.sites.contains(site)) {
          followed.names.insert((index, used.name));
          followed.sites.extend(sites);
        }
      }
    }
    followed
  }

  /// What the index proved of the declarations that moved under other owners (`move_questions`): of each whether
  /// every use the change updated is one of the new owner, and the places of the names that follow the moves which it
  /// lists as the owners of its uses in both trees
  fn check_moves(&self, answers: &[RenameAnswer]) -> Checked {
    let mut checked = Checked::default();
    for (number, (question, answer)) in (0u32..).zip(self.move_questions.iter().zip(answers)) {
      let mut listed = Vec::new();
      let verdict = self.move_verdict(number, question, answer, &mut listed);
      if verdict == RenameProof::Confirmed {
        checked.sites.extend(listed);
      }
      checked.verdicts.push(verdict);
    }
    checked
  }

  /// The place of a name that follows the move `question` and stands right before the use at `column` of the line
  /// `line` of a side of the file `file`, its owner there: on the same line, with at most two characters between
  /// them (`.`, `::`)
  fn owner_before(&self, question: u32, file: u32, new_side: bool, line: u32, column: u32) -> Option<Site> {
    let subject = self.files.get(file as usize)?.as_ref()?;
    subject.used.iter().filter(|used| used.moves.contains(&question)).find_map(|used| {
      let rename = subject.facts.left.undeclared.get(used.name as usize)?;
      let sites = if new_side { &rename.new_sites } else { &rename.old_sites };
      (0..rename.sites)
        .find(|&place| place_in(sites, file, place).is_some_and(|(_, at, _, end)| at == line && end <= column && column - end <= 2))
        .map(|place| Site {
          file,
          name: Name::Used(used.name),
          place,
        })
    })
  }

  /// What the answer of the index says of a declaration that moved under another owner (`MoveQuestion`, by its number
  /// `number`); `listed` gets the places of the names that follow the move it lists as the owner of a use in both
  /// trees
  fn move_verdict(&self, number: u32, question: &MoveQuestion, answer: &RenameAnswer, listed: &mut Vec<Site>) -> RenameProof {
    if answer.unavailable {
      return RenameProof::Unavailable;
    }
    // the same declaration at another place: its name, its kind, its language, its signature and its words the same
    if !answer.moved || answer.renamed || !answer.body_equal || answer.differs.iter().any(|differs| differs != "owner") {
      return RenameProof::OtherDeclaration;
    }
    let proven_language = |declaration: &Option<RenamedDeclaration>| declaration.as_ref().is_some_and(|it| PROVEN_LANGUAGES.contains(&it.language.as_str()));
    let mut outside = answer.conflicts.iter().any(|conflict| conflict.kind == "left");
    let meaning = answer.conflicts.iter().any(|conflict| conflict.kind != "left");
    let mut incomplete = !answer.complete || !proven_language(&answer.old) || !proven_language(&answer.new);
    let (Some(base), Some(head)) = (&answer.base, &answer.head) else {
      return if meaning { RenameProof::OtherMeaning } else { RenameProof::Incomplete };
    };
    incomplete |= base.uses.len() < base.total || head.uses.len() < head.total;
    let (base, head) = (self.uses_in(base, false), self.uses_in(head, true));
    let own = |new_side: bool| {
      if new_side {
        (question.new_file, &question.new)
      } else {
        (question.old_file, &question.old)
      }
    };
    for (new_side, uses, others) in [(false, &base, &head), (true, &head, &base)] {
      for (used, place) in &uses.uses {
        let Some((file, line)) = *place else {
          // in a file the subject did not change: the other tree lists it at the same place
          incomplete |= !others.outside.contains(&(used.path.as_str(), used.line, used.column, used.end_column));
          continue;
        };
        // the declaration and its own lines, which moved with it
        let (own_file, own_lines) = own(new_side);
        if used.kind == Access::Declaration || (file == own_file && own_lines.contains(&line)) {
          continue;
        }
        // the owner in front of it changed with the move: the other tree lists the use behind the twin of the owner
        if let Some(site) = self.owner_before(number, file, new_side, line, used.column) {
          let twin = self
            .place_of(site, new_side)
            .zip(self.place_of(site, !new_side))
            .map(|((_, _, _, end), (_, twin_line, _, twin_end))| {
              let column = twin_end + (used.column - end);
              (file, twin_line, column, column + (used.end_column - used.column))
            });
          if twin.is_some_and(|twin| others.places.contains(&twin)) {
            if !new_side {
              listed.push(site);
            }
          } else {
            incomplete = true;
          }
          continue;
        }
        if self.is_read(file, new_side, line) {
          continue;
        }
        // a line the change left alone: the other tree lists the use at its twin
        if self
          .same_line(file, new_side, line)
          .is_some_and(|twin| others.places.contains(&(file, twin, used.column, used.end_column)))
        {
          continue;
        }
        if new_side {
          incomplete = true;
        } else {
          outside = true;
        }
      }
    }
    if base.unmapped || head.unmapped {
      incomplete = true;
    }
    if meaning {
      RenameProof::OtherMeaning
    } else if outside {
      RenameProof::UsesOutside
    } else if incomplete {
      RenameProof::Incomplete
    } else {
      RenameProof::Confirmed
    }
  }

  /// The uses of a tree that a rename may ask something of, each with where it stands in the subject. A link of a
  /// doc comment runs nothing, and a use that writes no word (the index lists it with no column between its ends: a
  /// positional argument that sets a property, a constructor delegation) has no name to change: neither is one
  fn uses_in<'u>(&self, uses: &'u RenameUses, new_side: bool) -> TreeUses<'u> {
    let side = usize::from(new_side);
    let mut read = TreeUses::default();
    for used in uses.uses.iter().filter(|used| used.kind != Access::Doc && used.column != used.end_column) {
      let file = self.paths[side].get(used.path.as_str()).copied();
      let lone_return = |file: u32| {
        self
          .files
          .get(file as usize)
          .and_then(Option::as_ref)
          .is_some_and(|file| file.facts.left.lone_returns[side])
      };
      let Some(line) = used.line.checked_sub(1).filter(|_| !file.is_some_and(lone_return)) else {
        read.unmapped = true;
        continue;
      };
      match file {
        Some(file) => read.places.insert((file, line, used.column, used.end_column)),
        None => read.outside.insert((used.path.as_str(), used.line, used.column, used.end_column)),
      };
      if let Some(word) = &used.text {
        let spot = file.map_or(Spot::Path(&used.path), Spot::File);
        read.words.insert((spot, line, used.column, used.end_column, word.as_str()));
      }
      read.uses.push((used, file.map(|file| (file, line))));
    }
    read
  }
}

/// Where a use stands: in a file of the subject, by its index, or in a file the subject did not change, by its path
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Spot<'u> {
  File(u32),
  Path(&'u str),
}

/// The uses of a declaration in one tree (`Subject::uses_in`)
#[derive(Debug, Default)]
struct TreeUses<'u> {
  /// Each with its file among the files of the subject and its line there from 0; `None` in a file the subject
  /// did not change
  uses: Vec<(&'u RenameUse, Option<(u32, u32)>)>,
  /// Where those in the files of the subject stand
  places: HashSet<Place>,
  /// Where those in the files the subject did not change stand: the path, the line from 1 and the columns
  outside: HashSet<(&'u str, u32, u32, u32)>,
  /// The words written in place of the name of the declaration, each by where it stands: the line from 0 and the
  /// columns
  words: HashSet<(Spot<'u>, u32, u32, u32, &'u str)>,
  /// A use stands on a line the diff cannot name: in a text of the subject with a lone carriage return, whose lines
  /// the index counts otherwise, or on no line at all
  unmapped: bool,
}

/// What the index proved of the questions of a subject
#[derive(Debug, Default)]
struct Checked {
  /// Of each question, in their order
  verdicts: Vec<RenameProof>,
  /// The places a proven declaration lists as its uses in both trees
  sites: HashSet<Site>,
}

/// The order a rename with several declarations takes its proof in, the first first: what the index found wrong
/// before what it could not say, and proven only when every one of them is
const fn rank(proof: RenameProof) -> u8 {
  match proof {
    RenameProof::OtherDeclaration => 0,
    RenameProof::OtherMeaning => 1,
    RenameProof::UsesOutside => 2,
    RenameProof::Incomplete => 3,
    RenameProof::Unavailable => 4,
    RenameProof::Asked => 5,
    RenameProof::Confirmed => 6,
  }
}

/// The place `place` of flat sites (`[line, start, end]` each) in the file `file`
fn place_in(sites: &[u32], file: u32, place: u32) -> Option<Place> {
  let at = place as usize * 3;
  Some((file, *sites.get(at)?, *sites.get(at + 1)?, *sites.get(at + 2)?))
}

/// The places of a rename that use the name: all but those that declare it
fn uses_of(rename: &Rename) -> impl Iterator<Item = u32> + '_ {
  (0..rename.sites).filter(|place| !rename.declared.contains(place))
}

/// The runs of a file: those of its pair, and in the blocks that hold a place of a name of `used` the runs of the
/// kind `Renamed` of the reading that read such names as one (`Leftovers::pending`), as far as every name they lean
/// on is of `used`. They add runs and take none, as the second reading of a pair does; and they add labels alone:
/// no run of a kind that may be quiet without the index comes from here, and none of them takes a line of a quiet
/// run of the pair, so the quiet runs and the stops of the file stay those of its pair
fn merged_runs<'a>(facts: &PairFacts<'a>, used: &[Used]) -> Cow<'a, [LineRun]> {
  let pair = &facts.marks.runs;
  let holds = |run: &&PendingRun| run.run.kind == RunKind::Renamed && !run.names.is_empty() && run.names.iter().all(|name| used.iter().any(|used| used.name == *name));
  let mut pending = facts.left.pending.iter().filter(|block| block.runs.iter().any(|run| holds(&run))).peekable();
  if used.is_empty() || pending.peek().is_none() {
    return Cow::Borrowed(pair);
  }
  let blocks = &facts.blocks;
  let mut runs = Vec::with_capacity(pair.len());
  let mut from = 0;
  for block in pending {
    let Some(lines) = blocks.get(block.block as usize) else { continue };
    // the runs of the pair before the block, then those inside it: a run has lines on both sides
    let before = from + pair[from..].partition_point(|run| run.old_start < lines.old_start);
    let inside = before + pair[before..].partition_point(|run| run.old_start < lines.old_end);
    runs.extend_from_slice(&pair[from..before]);
    // a quiet run of the pair stays the run it is: its lines went out with the rows of the file, some of them
    // shown once. A run of this reading that shares a line with it (the line of a renamed local it holds beside a
    // call of the name another file declares) is none, and the line of that call is read as any changed line
    let quiet = &pair[before..inside];
    let meets =
      |run: &LineRun, other: &LineRun| (run.old_start < other.old_end && other.old_start < run.old_end) || (run.new_start < other.new_end && other.new_start < run.new_end);
    let read: Vec<LineRun> = block
      .runs
      .iter()
      .filter(holds)
      .map(|run| run.run.clone())
      .filter(|run| !quiet.iter().any(|other| other.quiet && meets(run, other)))
      .collect();
    runs.extend(with_the_first(pair[before..inside].to_vec(), read));
    from = inside;
  }
  runs.extend_from_slice(&pair[from..]);
  Cow::Owned(runs)
}

/// What a file leaves to the moves between files: the rows of its pair, without the lines of the runs the subject
/// added to them
fn rows_of<'s>(file: &'s FileSubject<'_>) -> [Cow<'s, [LeftRow]>; 2] {
  let left = &file.facts.left.lines;
  if matches!(file.runs, Cow::Borrowed(_)) {
    return [Cow::Borrowed(&left[0]), Cow::Borrowed(&left[1])];
  }
  let side = |new_side: bool| -> Cow<'s, [LeftRow]> {
    let taken: Vec<Range<u32>> = file.runs.iter().map(|run| side_of(run, new_side)).collect();
    Cow::Owned(left[usize::from(new_side)].iter().flat_map(|row| row.without(&taken)).collect())
  };
  [side(false), side(true)]
}

/// The moves between files `found`, the lines of each that no declaration of `declared` took: a line is in one move
/// at most, and a declaration of another layout takes its own. Of a move that shares a line with one, the parts that
/// are still a row of the same lines by themselves (`LeftRow::without`: at least `MIN_ACROSS_LINES` lines that are not
/// blank and `MIN_ACROSS_CHARS` characters, without the blank lines at their ends), so no blank line or lone brace
/// between two declarations is a move of its own; `rows`, the rows of the files the moves were found in, say the
/// characters of their lines. A move no declaration touches is what it was
fn outside_declarations(found: Vec<MoveAcross>, declared: &[DeclaredMove<'_>], rows: &[[Cow<'_, [LeftRow]>; 2]]) -> Vec<MoveAcross> {
  if declared.is_empty() {
    return found;
  }
  let mut parts = Vec::with_capacity(found.len());
  for moved in found {
    let (old, new) = (moved.old_start..moved.old_start + moved.len, moved.new_start..moved.new_start + moved.len);
    // the lines the declarations took, as old lines of the move: a new line taken is the old line it is the same as
    let mut taken: Vec<Range<u32>> = Vec::new();
    for declaration in declared {
      if declaration.old_file == moved.old_file {
        taken.extend(overlap(&declaration.old, &old));
      }
      if declaration.new_file == moved.new_file {
        taken.extend(overlap(&declaration.new, &new).map(|lines| lines.start - new.start + old.start..lines.end - new.start + old.start));
      }
    }
    if taken.is_empty() {
      parts.push(moved);
      continue;
    }
    taken.sort_unstable_by_key(|lines| lines.start);
    let mut merged: Vec<Range<u32>> = Vec::with_capacity(taken.len());
    for lines in taken {
      match merged.last_mut() {
        Some(last) if lines.start <= last.end => last.end = last.end.max(lines.end),
        _ => merged.push(lines),
      }
    }
    // the old row the move lies in: a move is lines that follow each other in one row of each side
    let row = rows
      .get(moved.old_file as usize)
      .and_then(|[old_rows, _]| old_rows.iter().find(|row| row.start <= old.start && old.end <= row.start + to_u32(row.keys.len())));
    let Some(row) = row else { continue };
    let (from, to) = ((old.start - row.start) as usize, (old.end - row.start) as usize);
    let lines = LeftRow {
      start: old.start,
      keys: row.keys[from..to].to_vec(),
      solid: row.solid[from..to].to_vec(),
    };
    for part in lines.without(&merged) {
      parts.push(MoveAcross {
        old_start: part.start,
        new_start: part.start - old.start + new.start,
        len: to_u32(part.keys.len()),
        ..moved
      });
    }
  }
  parts
}

/// The lines `lines` shares with `other`, if any
fn overlap(lines: &Range<u32>, other: &Range<u32>) -> Option<Range<u32>> {
  let shared = lines.start.max(other.start)..lines.end.min(other.end);
  (!shared.is_empty()).then_some(shared)
}

/// The file of each path of the old tree and of the new one among the files `pairs` of a subject: a path of the old
/// tree is the old side of the file that stays there, before that of a copy of it
fn paths_of<'p>(pairs: impl Iterator<Item = Option<&'p PairFacts<'p>>>) -> [HashMap<&'p str, u32>; 2] {
  let mut paths: [HashMap<&str, u32>; 2] = [HashMap::new(), HashMap::new()];
  for (index, facts) in (0u32..).zip(pairs) {
    let Some(facts) = facts else { continue };
    for (side, path) in [facts.old_path, facts.new_path].into_iter().enumerate() {
      if let Some(path) = path
        && (facts.old_path == facts.new_path || !paths[side].contains_key(path))
      {
        paths[side].insert(path, index);
      }
    }
  }
  paths
}

/// The declarations among `moved`, the answer of the index, that moved whole under another owner and kept their name,
/// both places in files of the subject `pairs` of a language whose uses the index proves: what a use of one in a file
/// of the subject follows, where the name of its owner in front of it changed so (`MoveQuestion`)
fn owner_moves(pairs: &[Option<PairFacts<'_>>], moved: &[MovedDeclaration]) -> Vec<MoveQuestion> {
  let paths = paths_of(pairs.iter().map(Option::as_ref));
  let listable = |path: &str| !path.contains(['\t', '\n', '\r']);
  moved
    .iter()
    .filter_map(|moved| {
      let (old, new) = (&moved.old, &moved.new);
      let owners = (old.parent.clone()?, new.parent.clone()?);
      if !moved.owner || old.name != new.name || owners.0 == owners.1 || ![&old.file, &new.file].into_iter().all(|path| is_of_proven_language(path) && listable(path)) {
        return None;
      }
      let lines = |at: &crate::index::DeclaredAt| (at.line >= 1 && at.end_line >= at.line).then(|| at.line - 1..at.end_line);
      Some(MoveQuestion {
        // the place of the line of its name, which holds no other declaration before it
        pair: RenamePair {
          old: format!("{}:{}", old.file, old.line),
          new: format!("{}:{}", new.file, new.line),
        },
        name: new.name.clone(),
        owners,
        old_file: *paths[0].get(old.file.as_str())?,
        old: lines(old)?,
        new_file: *paths[1].get(new.file.as_str())?,
        new: lines(new)?,
      })
    })
    .collect()
}

/// A declaration of a subject that moved whole (`declaration_moves`): the files of its two ends, by their index, and
/// the lines of each, 0-based, the end exclusive
#[derive(Debug)]
struct DeclaredMove<'m> {
  name: &'m str,
  old_file: u32,
  old: Range<u32>,
  new_file: u32,
  new: Range<u32>,
}

/// The declarations among `moved`, the answer of the index, that are a move between files of the subject `files`:
/// each end the changed lines of one block of its file, no line of which a run or a move of its pair took, in a text
/// whose lines the index counts as the diff does (no lone carriage return), and no line of which another declaration
/// took. A declaration a row of the same lines of `rows` holds as it is, its old lines on the new lines they are, is
/// none: the row says it moved, and stays the row it is. By the old place, as the index lists them
fn declaration_moves<'m>(files: &[Option<FileSubject<'_>>], moved: &'m [MovedDeclaration], rows: &[MoveAcross]) -> Vec<DeclaredMove<'m>> {
  let paths = paths_of(files.iter().map(|file| file.as_ref().map(|file| &file.facts)));
  let mut taken: [HashSet<(u32, u32)>; 2] = [HashSet::new(), HashSet::new()];
  let mut found = Vec::new();
  for moved in moved {
    let lines = |line: u32, end_line: u32| (line >= 1 && end_line >= line).then(|| line - 1..end_line);
    let (Some(old), Some(new)) = (lines(moved.old.line, moved.old.end_line), lines(moved.new.line, moved.new.end_line)) else {
      continue;
    };
    let (Some(&old_file), Some(&new_file)) = (paths[0].get(moved.old.file.as_str()), paths[1].get(moved.new.file.as_str())) else {
      continue;
    };
    let end_is_free = |file: u32, new_side: bool, lines: &Range<u32>| {
      files.get(file as usize).and_then(Option::as_ref).is_some_and(|file| is_free_end(file, new_side, lines))
        && lines.clone().all(|line| !taken[usize::from(new_side)].contains(&(file, line)))
    };
    if !end_is_free(old_file, false, &old) || !end_is_free(new_file, true, &new) {
      continue;
    }
    // the same lines in the same layout: a row holds them already
    let in_row = |row: &MoveAcross| {
      row.old_file == old_file
        && row.new_file == new_file
        && row.old_start <= old.start
        && old.end <= row.old_start + row.len
        && old.start - row.old_start == new.start.wrapping_sub(row.new_start)
        && old.len() == new.len()
    };
    if rows.iter().any(in_row) {
      continue;
    }
    taken[0].extend(old.clone().map(|line| (old_file, line)));
    taken[1].extend(new.clone().map(|line| (new_file, line)));
    found.push(DeclaredMove {
      name: &moved.new.name,
      old_file,
      old,
      new_file,
      new,
    });
  }
  found
}

/// The moves within `file`, the file `index` of its subject, whose declarations `reordered` (the answer of the index)
/// put in another order where their order is free (`ReorderMark::of_move`): the moves of its pair and those of a
/// declaration to the file itself, in the order of their new lines. None in a text whose lines the index counts
/// otherwise than the diff does (a lone carriage return)
fn reorder_marks(file: &FileSubject<'_>, index: u32, reordered: &[Reordered]) -> Vec<ReorderMark> {
  let facts = &file.facts;
  let (Some(old_path), Some(new_path)) = (facts.old_path, facts.new_path) else {
    return Vec::new();
  };
  if facts.left.lone_returns.iter().any(|&lone| lone) {
    return Vec::new();
  }
  let pair = facts
    .marks
    .moves
    .iter()
    .map(|moved| (moved.old_start..moved.old_start + moved.len, moved.new_start..moved.new_start + moved.len));
  let declared = file.moved_out.iter().filter(|moved| moved.file == index).filter_map(|moved| {
    let other = moved.declaration.as_ref()?.other_len;
    Some((moved.start..moved.start + moved.len, moved.line..moved.line + other))
  });
  let mut marks: Vec<ReorderMark> = pair
    .chain(declared)
    .filter_map(|(old, new)| ReorderMark::of_move(old_path, new_path, old, new, reordered))
    .collect();
  marks.sort_unstable_by_key(|mark| (mark.new_start, mark.old_start));
  marks
}

/// Whether the lines `lines` of a side of `file` may be an end of a declaration that moved whole: the changed lines
/// of one block, in no run and in no move of the pair, of a text without a lone carriage return
fn is_free_end(file: &FileSubject<'_>, new_side: bool, lines: &Range<u32>) -> bool {
  let facts = &file.facts;
  if facts.left.lone_returns[usize::from(new_side)] || lines.is_empty() {
    return false;
  }
  let side = |block: &DiffBlock| if new_side { block.new_start..block.new_end } else { block.old_start..block.old_end };
  let in_block = facts
    .blocks
    .iter()
    .any(|block| block.kind == BlockKind::Changed && side(block).start <= lines.start && lines.end <= side(block).end);
  let meets = |other: Range<u32>| other.start < lines.end && lines.start < other.end;
  let moved = |moved: &LineMove| if new_side { moved.new_lines() } else { moved.old_lines() };
  in_block && !file.runs.iter().any(|run| meets(side_of(run, new_side))) && !facts.marks.moves.iter().any(|it| meets(moved(it)))
}

/// The runs of `file`, the file `index` of its subject, by their numbers, that hold a place of a name that is not
/// local and whose every place is proven: one of `sites` for such a name, one of a rename the scopes of the file
/// prove for a local one
fn confirmed_runs(index: u32, file: &FileSubject<'_>, sites: &HashSet<Site>) -> Vec<u32> {
  let (marks, left) = (&file.facts.marks, file.facts.left);
  // the old line of each place of a rename, whether it is proven, and whether the index was asked of it
  let mut places: Vec<(u32, bool, bool)> = Vec::new();
  let mut read = |rename: &Rename, proven: &dyn Fn(u32) -> bool, asked: bool| {
    places.extend((0..rename.sites).filter_map(|place| Some((*rename.old_sites.get(place as usize * 3)?, proven(place), asked))));
  };
  for (number, rename) in (0u32..).zip(&marks.renames) {
    if rename.local {
      read(rename, &|_| left.proven.contains(&number), false);
    } else {
      let name = Name::Declared(number);
      read(rename, &|place| sites.contains(&Site { file: index, name, place }), true);
    }
  }
  for used in &file.used {
    if let Some(rename) = left.undeclared.get(used.name as usize) {
      let name = Name::Used(used.name);
      read(rename, &|place| sites.contains(&Site { file: index, name, place }), true);
    }
  }
  places.sort_unstable();
  (0u32..)
    .zip(file.runs.iter())
    .filter(|(_, run)| run.kind == RunKind::Renamed)
    .filter(|(_, run)| {
      let held = &places[places.partition_point(|place| place.0 < run.old_start)..places.partition_point(|place| place.0 < run.old_end)];
      held.iter().any(|place| place.2) && held.iter().all(|place| place.1)
    })
    .map(|(number, _)| number)
    .collect()
}

/// The marks of a file of a subject with the kind `Renamed` switched on (`marks::is_quiet`), which it is: the runs
/// the index confirmed (`SubjectMarks::confirmed`) quiet, and the starts of `blocks`, the blocks of its diff, that
/// follow from them. What `Subject::marks` answers, and what a measurement reads of a kind that is off, as
/// `PairMarks::as_proven` is for the runs of a pair
#[must_use]
pub fn as_confirmed(marks: &DiffMarks, blocks: &[DiffBlock]) -> DiffMarks {
  let mut marks = marks.clone();
  let confirmed = marks.subject.as_ref().map_or_default(|subject| subject.confirmed.clone());
  for run in confirmed {
    if let Some(run) = marks.runs.get_mut(run as usize) {
      run.quiet = true;
    }
  }
  marks.starts = starts_of(blocks, &marks.runs, &marks.moves);
  marks
}

/// The facts of a text file of a subject at `path`, `old_path` before a rename, with the blob ids of its sides, the
/// blocks of its diff and the marks of its pair
pub fn facts_of<'a>(old_path: Option<&'a str>, path: &'a str, ids: [Option<&str>; 2], blocks: &'a [DiffBlock], marks: &'a DiffMarks, left: &'a Leftovers) -> PairFacts<'a> {
  PairFacts {
    old_path: ids[0].map(|_| old_path.unwrap_or(path)),
    new_path: ids[1].map(|_| path),
    blocks,
    marks,
    left,
  }
}
