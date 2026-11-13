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
//! The answer of the index comes after the rows of the files (in Branch Deck `file_diffs::service::FileDiffs::rename_proofs`,
//! asked by the window that shows them), so nothing it says takes a line out of a view: a view passes a confirmed run and shows each of its
//! lines.

use crate::index::{Access, RenameAnswer, RenamePair, RenameUse, RenameUses, RenamedDeclaration, Resolved};
use crate::leftovers::{Leftovers, PendingRun};
use crate::marks::is_quiet;
use crate::marks_renames::with_the_first;
use crate::model::{DiffMarks, FileMove, Rename, RenameCheck, RenameProof, SubjectMarks, UsedRename};
use code_tokens::{Grammar, grammar_of};
use line_diff::model::to_u32;
use line_diff::model::{BlockKind, DiffBlock, LineMove, LineRun, RunKind, starts_of};
use line_diff::moves_across::{AcrossWork, FileRows, LeftRow, find_moves_across};
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

/// A name of a file that another file of the subject declares
#[derive(Debug, Clone, Copy)]
struct Used {
  /// Its number among `Leftovers::undeclared`
  name: u32,
  /// The file that declares it and the new line of its declaration there
  file: u32,
  line: u32,
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

    let mut files: Vec<Option<FileSubject<'a>>> = pairs
      .iter()
      .map(|facts| {
        let facts = (*facts)?;
        let used: Vec<Used> = (0u32..)
          .zip(&facts.left.undeclared)
          .filter_map(|(name, rename)| {
            let (file, number) = *declaring.get(&names_of(rename))?.first()?;
            Some(Used {
              name,
              file,
              line: declaration_line(file, number)?,
            })
          })
          .collect();
        Some(FileSubject {
          runs: merged_runs(&facts, &used),
          facts,
          moved_out: Vec::new(),
          moved_in: Vec::new(),
          used,
        })
      })
      .collect();

    // the moves between the files, among the lines no run took: those of the pairs, and those added here
    let rows: Vec<[Cow<'_, [LeftRow]>; 2]> = files.iter().map(|file| file.as_ref().map_or_else(Default::default, rows_of)).collect();
    let file_rows: Vec<FileRows<'_>> = rows.iter().map(|[old, new]| FileRows { old, new }).collect();
    let (moves, work) = find_moves_across(&file_rows);
    for found in moves {
      if let Some(Some(file)) = files.get_mut(found.old_file as usize) {
        file.moved_out.push(FileMove {
          start: found.old_start,
          len: found.len,
          file: found.new_file,
          line: found.new_start,
        });
      }
      if let Some(Some(file)) = files.get_mut(found.new_file as usize) {
        file.moved_in.push(FileMove {
          start: found.new_start,
          len: found.len,
          file: found.old_file,
          line: found.old_start,
        });
      }
    }
    for file in files.iter_mut().flatten() {
      file.moved_out.sort_unstable_by_key(|moved| moved.start);
      file.moved_in.sort_unstable_by_key(|moved| moved.start);
    }

    let mut subject = Self {
      files,
      pairs: Vec::new(),
      questions: Vec::new(),
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

  /// The marks of each file that the subject says something of, with what `answer` says of its names; `None` for a
  /// file whose marks are those of its pair
  pub fn marks(&self, answer: IndexAnswer<'_>) -> Vec<Option<DiffMarks>> {
    let checked = match answer {
      IndexAnswer::Answered(answers) => Some(self.check(answers)),
      IndexAnswer::Asked | IndexAnswer::Unavailable => None,
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
            Some(UsedRename {
              rename: rename.clone(),
              file: used.file,
              line: used.line,
              proof: proof(index, Name::Used(used.name), rename),
            })
          })
          .collect();
        if file.moved_out.is_empty() && file.moved_in.is_empty() && declared.is_empty() && used.is_empty() {
          return None;
        }
        let confirmed = checked.as_ref().map_or_default(|checked| confirmed_runs(index, file, &checked.sites));
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
      if let Some(file) = file {
        read.places.insert((file, line, used.column, used.end_column));
      }
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
