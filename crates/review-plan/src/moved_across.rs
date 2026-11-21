//! The declarations moved whole to another file (openspec/specs/branch/review-units/spec.md, "Units: A declaration
//! moved whole to another file says where"): a unit `removed` whose declaration has the same code as a declaration
//! added to another file says where it went (`ReviewUnit::moved_to`), and a unit `added` whose declaration has the same
//! code as one removed from another file where it came from (`ReviewUnit::moved_from`), each with the unit that shows
//! the other place (`MovedPlace::unit`). A word of each unit, never another kind: the units, their order, their
//! fingerprints and the story are those without it.
//!
//! - **The same code** is the same content of the tokens of the lines of the declaration, the doc comments and
//!   attributes above it with them, in the same grammar (`whitespace::Sides::code`), as `Sides::same_code` compares a
//!   pair that moved inside its file: a declaration moved with a changed line, another modifier, another comment or
//!   another indent of Starlark is no move.
//! - **The places** of a move are the declarations of one side alone that are units (`added`, `removed`), and the
//!   outermost declarations of a source file the commits add or delete (`newFile`, `deletedFile`, in `deletedFiles`
//!   too), as the units of a file that stays take the outermost declaration of one side alone: a member of a new or a
//!   removed type is no place of its own, nor a declaration of a file taken whole by a kind (`mechanical`, `other`).
//! - **One place on each side**: a code of exactly one place before the commits and one after them, in two files, one
//!   of them a unit, is a move. The unit says the file and the line of the name of the other place (`MovedPlace`) and
//!   the unit there: the unit of its declaration, or the unit that takes its file whole; a file taken whole says
//!   nothing, its unit stays as it is. A code of two places on a side is no move: a word on two files must not guess.
//!   Two places in one file (a declaration moved to another container of its file) are no move between files: the diff
//!   of the file shows that one.
//! - **The place a move left** is nothing to read where it holds the move alone (openspec/specs/branch/review-units/spec.md,
//!   "Units: The place a move left is nothing to read where it holds the move alone"): the slice of a unit `removed` whose
//!   blocks change the old side as removing the declarations whose lines they change would, blank lines aside, and of a
//!   file the commits delete whose every line of code is a line of an outermost declaration or of an import, is
//!   `UnitSlice::moved_out` once each of those declarations moved (`MovedOut`); the other places show it. Any other slice
//!   is read where it stands.
//! - **Cost**: no git process and no query of the index. The tokens of a file that stays are those its fingerprints
//!   read already; a file the commits add or delete is read only where one of its outermost declarations has the name
//!   of a unit `added` or `removed`, which every place of the code of that unit has.

use crate::units::{MovedPlace, ReviewUnit, UnitKind};
use crate::whitespace::Code;
use std::collections::{HashMap, HashSet};

/// A place a declaration may have moved from or to (`MoveEnds`)
#[derive(Debug, Clone)]
pub(crate) struct MoveEnd {
  /// The unit of the declaration, by its id; `None` for an outermost declaration of a file taken whole
  pub(crate) unit: Option<String>,
  /// Its file, as the units name it (`UnitSlice::path`)
  pub(crate) path: String,
  /// The line of its name on its side, from 1 (`ReviewUnit::line`)
  pub(crate) line: u32,
  pub(crate) name: String,
  pub(crate) code: Code,
}

/// A slice of the place declarations left that holds nothing else (`UnitSlice::moved_out` once each of them moved): the
/// slice of a unit `removed` whose blocks remove those declarations alone, blank lines aside, or a file the commits
/// delete whose lines of code are those declarations and its imports
#[derive(Debug, Clone)]
pub(crate) struct MovedOut {
  /// The unit whose slice it is, by its id; `None` for a file the commits delete, whose unit takes it whole (that of
  /// its path: `deletedFile`, or `deletedFiles` with its slice of `path`)
  pub(crate) unit: Option<String>,
  pub(crate) path: String,
  /// The declarations it holds, by their place in `MoveEnds::left`
  pub(crate) ends: Vec<usize>,
}

/// The places of the declarations of one side alone: those before the commits (`left`) and those after them
/// (`arrived`), and the slices of the places they left that hold them alone (`out`)
#[derive(Debug, Default)]
pub(crate) struct MoveEnds {
  pub(crate) left: Vec<MoveEnd>,
  pub(crate) arrived: Vec<MoveEnd>,
  pub(crate) out: Vec<MovedOut>,
}

impl MoveEnds {
  /// The place `end`, after the commits (`new_side`) or before them; its place in `left` or `arrived`
  pub(crate) fn push(&mut self, new_side: bool, end: MoveEnd) -> usize {
    let ends = if new_side { &mut self.arrived } else { &mut self.left };
    ends.push(end);
    ends.len() - 1
  }

  /// The names of the units of both sides: what an outermost declaration of a file taken whole must be named to be a
  /// place of the code of a unit
  pub(crate) fn unit_names(&self) -> HashSet<String> {
    self.left.iter().chain(&self.arrived).filter(|end| end.unit.is_some()).map(|end| end.name.clone()).collect()
  }

  /// Whether a unit of a declaration of one side alone has its code: without one no move can be told
  pub(crate) fn has_units(&self) -> bool {
    self.left.iter().chain(&self.arrived).any(|end| end.unit.is_some())
  }
}

/// Tell each unit of `units` that `ends` say moved whole to or from another file where it went (`moved_to`) or came
/// from (`moved_from`) and the unit there, and each slice of `ends.out` whose declarations all moved that it is the move
/// alone (`UnitSlice::moved_out`); every other unit has neither word, every other slice is read
pub(crate) fn mark_moves(units: &mut [ReviewUnit], ends: &MoveEnds) {
  let mut by_code: HashMap<Code, (Vec<usize>, Vec<usize>)> = HashMap::new();
  for (index, end) in ends.left.iter().enumerate() {
    by_code.entry(end.code).or_default().0.push(index);
  }
  for (index, end) in ends.arrived.iter().enumerate() {
    by_code.entry(end.code).or_default().1.push(index);
  }
  let at: HashMap<&str, usize> = units.iter().enumerate().map(|(index, unit)| (unit.id.as_str(), index)).collect();
  // the unit that takes a file whole, by the path of the file: the place of an outermost declaration of it
  let whole: HashMap<&str, &str> = units
    .iter()
    .filter(|unit| matches!(unit.kind, UnitKind::NewFile | UnitKind::DeletedFile | UnitKind::DeletedFiles { .. }))
    .flat_map(|unit| unit.slices.iter().map(|slice| (slice.path.as_str(), unit.id.as_str())))
    .collect();
  let unit_of = |end: &MoveEnd| end.unit.as_deref().and_then(|id| at.get(id).copied());
  let shown_by = |end: &MoveEnd| end.unit.as_deref().or_else(|| whole.get(end.path.as_str()).copied()).map(str::to_string);
  let place = |end: &MoveEnd| {
    shown_by(end).map(|unit| MovedPlace {
      path: end.path.clone(),
      line: end.line,
      unit,
    })
  };
  // (the unit, the place it moved to) and (the unit, the place it moved from); which ends before the commits moved
  let mut moved_to: Vec<(usize, MovedPlace)> = Vec::new();
  let mut moved_from: Vec<(usize, MovedPlace)> = Vec::new();
  let mut left_moved = vec![false; ends.left.len()];
  for (left, arrived) in by_code.values() {
    let (&[from], &[to]) = (left.as_slice(), arrived.as_slice()) else { continue };
    let (from_end, to_end) = (&ends.left[from], &ends.arrived[to]);
    // one of them a unit: a new and a deleted file alone are no move
    let (from_unit, to_unit) = (unit_of(from_end), unit_of(to_end));
    if from_end.path == to_end.path || (from_unit.is_none() && to_unit.is_none()) {
      continue;
    }
    let (Some(to_place), Some(from_place)) = (place(to_end), place(from_end)) else {
      continue;
    };
    left_moved[from] = true;
    moved_to.extend(from_unit.map(|unit| (unit, to_place)));
    moved_from.extend(to_unit.map(|unit| (unit, from_place)));
  }
  // the units whose slice of a path is the move alone: each declaration it holds moved
  let moved_out: Vec<(usize, &str)> = ends
    .out
    .iter()
    .filter(|out| !out.ends.is_empty() && out.ends.iter().all(|&end| left_moved[end]))
    .filter_map(|out| {
      let holder = out.unit.as_deref().or_else(|| whole.get(out.path.as_str()).copied());
      holder.and_then(|id| at.get(id).copied()).map(|unit| (unit, out.path.as_str()))
    })
    .collect();
  for unit in units.iter_mut() {
    unit.moved_from = None;
    unit.moved_to = None;
    for slice in &mut unit.slices {
      slice.moved_out = false;
    }
  }
  for (unit, place) in moved_to {
    units[unit].moved_to = Some(place);
  }
  for (unit, place) in moved_from {
    units[unit].moved_from = Some(place);
  }
  for (unit, path) in moved_out {
    if let Some(slice) = units[unit].slices.iter_mut().find(|slice| slice.path == path) {
      slice.moved_out = true;
    }
  }
}
