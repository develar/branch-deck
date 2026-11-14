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

use crate::units::{RenameWait, RenamedName, RenamedUses, ReviewUnit, UnitBlock, UnitKind};
use file_diffs::model::DiffContent;
use file_diffs::service::Collected;
use std::collections::HashMap;
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
        match proof {
          RenameProof::Asked => checking = true,
          RenameProof::Unavailable | RenameProof::Incomplete => {}
          RenameProof::Confirmed | RenameProof::UsesOutside | RenameProof::OtherMeaning | RenameProof::OtherDeclaration => return None,
        }
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
