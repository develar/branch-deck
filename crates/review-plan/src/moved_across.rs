//! The declarations moved whole to another file (openspec/specs/branch/review-units/spec.md, "Units: A declaration
//! moved whole to another file says where"): a unit `removed` whose declaration has the same code as a declaration
//! added to another file says where it went (`ReviewUnit::moved_to`), and a unit `added` whose declaration has the same
//! code as one removed from another file where it came from (`ReviewUnit::moved_from`). A word of each unit, never
//! another kind: the units, their order, their fingerprints and the story are those without it.
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
//!   of them a unit, is a move. The unit says the file and the line of the name of the other place (`MovedPlace`); a
//!   file taken whole says nothing, its unit stays as it is. A code of two places on a side is no move: a word on two
//!   files must not guess. Two places in one file (a declaration moved to another container of its file) are no move
//!   between files: the diff of the file shows that one.
//! - **Cost**: no git process and no query of the index. The tokens of a file that stays are those its fingerprints
//!   read already; a file the commits add or delete is read only where one of its outermost declarations has the name
//!   of a unit `added` or `removed`, which every place of the code of that unit has.

use crate::units::{MovedPlace, ReviewUnit};
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

/// The places of the declarations of one side alone: those before the commits (`left`) and those after them
/// (`arrived`)
#[derive(Debug, Default)]
pub(crate) struct MoveEnds {
  pub(crate) left: Vec<MoveEnd>,
  pub(crate) arrived: Vec<MoveEnd>,
}

impl MoveEnds {
  /// The place `end`, after the commits (`new_side`) or before them
  pub(crate) fn push(&mut self, new_side: bool, end: MoveEnd) {
    if new_side { &mut self.arrived } else { &mut self.left }.push(end);
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

impl MoveEnd {
  fn place(&self) -> MovedPlace {
    MovedPlace {
      path: self.path.clone(),
      line: self.line,
    }
  }
}

/// Tell each unit of `units` that `ends` say moved whole to or from another file where it went (`moved_to`) or came
/// from (`moved_from`); every other unit has neither
pub(crate) fn mark_moves(units: &mut [ReviewUnit], ends: &MoveEnds) {
  let mut by_code: HashMap<Code, (Vec<&MoveEnd>, Vec<&MoveEnd>)> = HashMap::new();
  for end in &ends.left {
    by_code.entry(end.code).or_default().0.push(end);
  }
  for end in &ends.arrived {
    by_code.entry(end.code).or_default().1.push(end);
  }
  let at: HashMap<&str, usize> = units.iter().enumerate().map(|(index, unit)| (unit.id.as_str(), index)).collect();
  let unit_of = |end: &MoveEnd| end.unit.as_deref().and_then(|id| at.get(id).copied());
  // (the unit, the place it moved to) and (the unit, the place it moved from)
  let mut moved_to: Vec<(usize, MovedPlace)> = Vec::new();
  let mut moved_from: Vec<(usize, MovedPlace)> = Vec::new();
  for (left, arrived) in by_code.values() {
    let ([from], [to]) = (left.as_slice(), arrived.as_slice()) else { continue };
    if from.path == to.path {
      continue;
    }
    moved_to.extend(unit_of(from).map(|unit| (unit, to.place())));
    moved_from.extend(unit_of(to).map(|unit| (unit, from.place())));
  }
  for unit in units.iter_mut() {
    unit.moved_from = None;
    unit.moved_to = None;
  }
  for (unit, place) in moved_to {
    units[unit].moved_to = Some(place);
  }
  for (unit, place) in moved_from {
    units[unit].moved_from = Some(place);
  }
}
