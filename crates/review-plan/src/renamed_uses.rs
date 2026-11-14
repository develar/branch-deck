//! The bodies that changed in renamed names alone (openspec/specs/branch/review-units/spec.md, "Units: A body changed
//! in a rename the index has not proven says so"): a unit `Behaviour` whose every changed line is a run of the kind
//! `Renamed` of a name that is not local, which the index of the repository has not proven, is told by those renames and
//! what they wait on (`ReviewUnit::renamed`), never as a body that changed. Read from the marks of the diffs the reading
//! of the plan proves (`PlanReading`), so it says what the header of the review says of the same file
//! (`Unproven::RenamesPending`, `Unproven::RenamesUnanswered`), and read again with the answer of the index.
//!
//! - **A run** that is quiet, or that the index confirmed (`SubjectMarks::confirmed`), is passed: a body whose every run
//!   is passed has nothing to read, and stays `Behaviour`, as the diffs pass it.
//! - **A rename** of a run is the rename of this file (`DiffMarks::renames`, its proof in `SubjectMarks::declared`) or
//!   of another file of the subject (`SubjectMarks::used`) with a place on a line of the run. Its proof `Asked` waits for
//!   the index (`RenameWait::Checking`), `Unavailable` and `Incomplete` are not proven (`RenameWait::NotProven`); any
//!   other proof (`UsesOutside`, `OtherMeaning`, `OtherDeclaration`), a local name and a run of no rename leave the unit a
//!   body that changed: more than a rename the index cannot prove is to be read.
//! - **A block** of the unit is matched to a block of the diff by its lines; a unit whose blocks the diff does not hold
//!   so (another pair, a lone carriage return) is a body that changed.
//! - **A declaration renamed** (openspec/specs/branch/review-units/spec.md, "Units: A declaration renamed where the
//!   index has not proven it is one unit"): a rename this file declares (`DiffMarks::renames`, its proof in
//!   `SubjectMarks::declared`) whose proof waits for the index or is not proven names a declaration whose code changed
//!   in names alone: else the diff says `OtherDeclaration`, and no index is asked. Such renames, by the path of their
//!   file (`DeclaredRenames`), pair the declaration of the old name with the one of the new name as one unit
//!   (`units::SourceFile::cut`, `ReviewUnit::renamed_from`) in place of a removed and an added one. A rename the index
//!   confirmed, or said is more than a rename, pairs nothing: the two stay as they are. The plan reads them again with
//!   the answer of the index, and tells its story again only where the pairs changed (`DeclaredRenames::pairs`).

use crate::units::{RenameWait, RenamedName, RenamedUses, ReviewUnit, UnitBlock, UnitKind};
use file_diffs::model::DiffContent;
use file_diffs::service::Collected;
use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use text_diff::model::{LineRun, Rename, RenameProof, RunKind, TextDiff};

/// Tell each unit of `units` whose body changed in renamed names alone that the index has not proven by those renames
/// (`ReviewUnit::renamed`), by the marks of the diffs of `read`; every other unit has none
pub(crate) fn mark_renamed_uses(units: &mut [ReviewUnit], read: &Collected) {
  let diffs: HashMap<&str, &TextDiff> = read
    .files()
    .filter_map(|(header, content)| match content {
      Some(DiffContent::Text(diff)) => Some((header.path.as_str(), diff)),
      _ => None,
    })
    .collect();
  for unit in units {
    unit.renamed = renamed_uses(unit, &diffs);
  }
}

/// The renames `unit` changed alone, as `mark_renamed_uses` tells them
fn renamed_uses(unit: &ReviewUnit, diffs: &HashMap<&str, &TextDiff>) -> Option<RenamedUses> {
  let UnitKind::Behaviour = unit.kind else { return None };
  let [slice] = unit.slices.as_slice() else { return None };
  if slice.whole || slice.blocks.is_empty() {
    return None;
  }
  let diff = diffs.get(slice.path.as_str())?;
  let marks = &diff.marks;
  let confirmed = marks.subject.as_deref().map_or(&[][..], |subject| subject.confirmed.as_slice());
  let mut names: Vec<RenamedName> = Vec::new();
  let mut checking = false;
  for block in &slice.blocks {
    let lines = (block.old_start, block.old_end, block.new_start, block.new_end);
    if !diff.blocks.iter().any(|at| (at.old_start, at.old_end, at.new_start, at.new_end) == lines) {
      return None;
    }
    let runs: Vec<(usize, &LineRun)> = marks
      .runs
      .iter()
      .enumerate()
      .filter(|(_, run)| run.kind == RunKind::Renamed && within(run, block))
      .collect();
    let (old, new): (Vec<Range<u32>>, Vec<Range<u32>>) = runs.iter().map(|(_, run)| (run.old_start..run.old_end, run.new_start..run.new_end)).unzip();
    if !covered(block.old_start..block.old_end, &old) || !covered(block.new_start..block.new_end, &new) {
      return None;
    }
    let passed = |index: usize| u32::try_from(index).is_ok_and(|index| confirmed.contains(&index));
    for (_, run) in runs.into_iter().filter(|(index, run)| !run.quiet && !passed(*index)) {
      let renames = renames_of(diff, run.new_start..run.new_end);
      if renames.is_empty() {
        return None;
      }
      for (rename, proof) in renames {
        checking |= wait_of(proof)? == RenameWait::Checking;
        if !names.iter().any(|name| name.old == rename.old && name.new == rename.new) {
          names.push(RenamedName {
            old: rename.old.clone(),
            new: rename.new.clone(),
          });
        }
      }
    }
  }
  // every run passed: nothing to read, as the diffs pass it
  if names.is_empty() {
    return None;
  }
  Some(RenamedUses {
    names,
    wait: if checking { RenameWait::Checking } else { RenameWait::NotProven },
  })
}

/// The run lies inside the lines of `block`
const fn within(run: &LineRun, block: &UnitBlock) -> bool {
  block.old_start <= run.old_start && run.old_end <= block.old_end && block.new_start <= run.new_start && run.new_end <= block.new_end
}

/// Every line of `lines` is in one of `runs`
fn covered(mut lines: Range<u32>, runs: &[Range<u32>]) -> bool {
  lines.all(|line| runs.iter().any(|run| run.contains(&line)))
}

/// The renames that are not local with a place on one of the new lines `lines`, each with what the index proved of it:
/// those this file declares (`SubjectMarks::declared`) and those another file of the subject declares
/// (`SubjectMarks::used`). A local one has no proof of the index: it is left out, and a run of it alone has none
fn renames_of(diff: &TextDiff, lines: Range<u32>) -> Vec<(&Rename, RenameProof)> {
  let Some(subject) = diff.marks.subject.as_deref() else { return Vec::new() };
  let placed = |rename: &Rename| rename.new_sites.as_chunks::<3>().0.iter().any(|site| lines.contains(&site[0]));
  let declared = subject.declared.iter().filter_map(|check| {
    let rename = diff.marks.renames.get(check.rename as usize)?;
    (!rename.local && placed(rename)).then_some((rename, check.proof))
  });
  let used = subject.used.iter().filter(|used| placed(&used.rename)).map(|used| (&used.rename, used.proof));
  declared.chain(used).collect()
}

/// What a rename the index has not proven waits on, by its proof; `None` for a proof that says something: confirmed, or
/// more than a rename
const fn wait_of(proof: RenameProof) -> Option<RenameWait> {
  match proof {
    RenameProof::Asked => Some(RenameWait::Checking),
    RenameProof::Unavailable | RenameProof::Incomplete => Some(RenameWait::NotProven),
    RenameProof::Confirmed | RenameProof::UsesOutside | RenameProof::OtherMeaning | RenameProof::OtherDeclaration => None,
  }
}

/// A rename of a declaration of its file that the index has not proven (`DeclaredRenames`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeclaredRename {
  pub(crate) old: String,
  pub(crate) new: String,
  pub(crate) wait: RenameWait,
}

/// The renames of one file (`DeclaredRenames`)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FileRenames {
  /// Those it declares that the index has not proven
  pub(crate) declared: Vec<DeclaredRename>,
  /// Every name the diff of the file says the commits renamed, local or not, the old and the new spelling: what a
  /// declaration renamed may differ in beside its own name (`same_but_names`)
  pub(crate) names: Vec<(String, String)>,
}

/// The renames each file of the pair declares that the index has not proven, by the path of the file after the
/// commits: what pairs a declaration of the old name with one of the new name (`units::SourceFile::cut`)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DeclaredRenames {
  by_path: HashMap<String, FileRenames>,
}

impl DeclaredRenames {
  /// The renames of the marks of the diffs of `read`
  pub(crate) fn of(read: &Collected) -> Self {
    let mut by_path: HashMap<String, FileRenames> = HashMap::new();
    for (header, content) in read.files() {
      let Some(DiffContent::Text(diff)) = content else { continue };
      let Some(subject) = diff.marks.subject.as_deref() else { continue };
      let mut declared: Vec<DeclaredRename> = Vec::new();
      for check in &subject.declared {
        let Some(rename) = diff.marks.renames.get(check.rename as usize) else { continue };
        let Some(wait) = wait_of(check.proof) else { continue };
        if !rename.local && !rename.declared.is_empty() && !declared.iter().any(|known| known.old == rename.old && known.new == rename.new) {
          declared.push(DeclaredRename {
            old: rename.old.clone(),
            new: rename.new.clone(),
            wait,
          });
        }
      }
      if declared.is_empty() {
        continue;
      }
      let renamed = diff.marks.renames.iter().chain(subject.used.iter().map(|used| &used.rename));
      let names = renamed.map(|rename| (rename.old.clone(), rename.new.clone())).collect();
      by_path.insert(header.path.clone(), FileRenames { declared, names });
    }
    Self { by_path }
  }

  /// The renames of the file at `path` (after the commits), none where it declares none the index has not proven
  pub(crate) fn of_file(&self, path: &str) -> Option<&FileRenames> {
    self.by_path.get(path)
  }

  /// The renames as the units pair by them, without what they wait on: another set cuts other units
  pub(crate) fn pairs(&self) -> BTreeSet<(&str, &str, &str)> {
    self
      .by_path
      .iter()
      .flat_map(|(path, renames)| renames.declared.iter().map(move |rename| (path.as_str(), rename.old.as_str(), rename.new.as_str())))
      .collect()
  }

  /// What the unit of each declaration renamed (`ReviewUnit::renamed_from`) waits on, as these renames say it: the units
  /// cut by renames of the same pairs (`pairs`), read again with the answer of the index
  pub(crate) fn mark_waits(&self, units: &mut [ReviewUnit]) {
    for unit in units {
      let (Some(from), [slice], Some(name)) = (unit.renamed_from.as_mut(), unit.slices.as_slice(), unit.name.as_deref()) else {
        continue;
      };
      let declared = self.of_file(&slice.path).map_or(&[][..], |renames| renames.declared.as_slice());
      if let Some(rename) = declared.iter().find(|rename| rename.old == from.old && rename.new == name) {
        from.wait = rename.wait;
      }
    }
  }
}

/// Whether `old` (a text before the commits) is `new` (after them) but for the names `names` renamed and whitespace: the
/// same words and the same other characters, in order, each old word that a rename names read as its new spelling. A
/// name of the same spelling the commits did not rename reads as renamed too, and so differs: never a false sameness
pub(crate) fn same_but_names(old: &str, new: &str, names: &[(String, String)]) -> bool {
  let renamed: HashMap<&str, &str> = names.iter().map(|(old, new)| (old.as_str(), new.as_str())).collect();
  let old_tokens = tokens(old).map(|token| renamed.get(token).copied().unwrap_or(token));
  old_tokens.eq(tokens(new))
}

/// The words (letters, digits, `_` and `$`) and the other characters of `text` but whitespace, in order
fn tokens(text: &str) -> impl Iterator<Item = &str> {
  let word = |character: char| character.is_alphanumeric() || character == '_' || character == '$';
  let mut rest = text;
  std::iter::from_fn(move || {
    rest = rest.trim_start();
    let first = rest.chars().next()?;
    let end = if word(first) {
      rest.find(|character: char| !word(character)).unwrap_or(rest.len())
    } else {
      first.len_utf8()
    };
    let (token, after) = rest.split_at(end);
    rest = after;
    Some(token)
  })
}
