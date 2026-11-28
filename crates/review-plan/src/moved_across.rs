//! The declarations moved whole to another file (openspec/specs/branch/review-units/spec.md, "Units: A declaration
//! moved whole to another file says where"): a unit `removed` whose declaration has the same code as a declaration
//! added to another file says where it went (`ReviewUnit::moved_to`), and a unit `added` whose declaration has the same
//! code as one removed from another file where it came from (`ReviewUnit::moved_from`), each with the unit that shows
//! the other place (`MovedPlace::unit`). A word of each unit, never another kind: the units, their order, their
//! fingerprints and the story are those without it. Not the mechanical kind `moved`, a reorder inside one file.
//!
//! - **The same code** is the same content of the tokens of the lines of the declaration, the doc comments and
//!   attributes above it with them, in the same grammar (`whitespace::Sides::code`), as `Sides::same_code` compares a
//!   pair that moved inside its file, and of a member of an `impl` of Rust the code of the header of that `impl`
//!   (`MoveEnd::within`: from `impl Area for Shape` to `impl Shape` is no move): a declaration moved with a changed
//!   line, another comment or another indent of Starlark is no move.
//! - **The places** of a move are the declarations of one side alone that are units (`added`, `removed`), and the
//!   declarations of a source file the commits add or delete (`newFile`, `deletedFile`, in `deletedFiles` too) that
//!   stand in modules alone (`units::in_modules`: a test of `mod tests` too), as the units of a file that stays take the
//!   outermost declaration of one side alone: a member of a type is part of the type, no place of its own, nor a
//!   declaration of a file taken whole by a kind (`mechanical`, `other`).
//! - **One place on each side**: a code of exactly one place before the commits and one after them, in two files, one
//!   of them a unit, is a move; of a code of several places on a side, each two of one member (`MoveEnd::member`, what
//!   it is a member of but the modules, and its name) that no other place of the code on its side is of. The unit says
//!   the file and the line of the name of the other place (`MovedPlace`) and the unit there: the unit of its
//!   declaration, or the unit that takes its file whole; a file taken whole says nothing, its unit stays as it is. Any
//!   other code of two places on a side is no move: a word on two files must not guess. Two places in one file (a
//!   declaration moved to another container of its file) are no move between files: the diff of the file shows that one.
//! - **A reach that changed with the move**: the places left without a pair pair by their code but the modifiers of
//!   their reach (`Sides::code_but_reach`) and their member, one on each side, and say it (`MovedPlace::visibility`):
//!   `fn helper` made `pub(crate)` by one commit and moved by the next is a move of the range of both.
//! - **The place a move left** is nothing to read where it holds the move alone (openspec/specs/branch/review-units/spec.md,
//!   "Units: The place a move left is nothing to read where it holds the move alone"): the slice of a unit whose blocks
//!   change the old side as removing the declarations whose lines they change would, blank lines aside, with the
//!   modules and the `impl` blocks they emptied and the imports beside them, and add nothing but the skeleton of the move
//!   (`mod x;` of a file of the change, `pub use x::*;`; `units::SourceFile::removed_alone`), and of a file the commits
//!   delete whose every line of code is a line of an outermost declaration or of an import, is `UnitSlice::moved_out`
//!   once each of those declarations moved (`MovedOut`, `Held`: a module of a deleted file by its members too); the
//!   other places show it. A line of no declaration that is an item of its own (a call of a macro, a `pub use` they
//!   remove, which may be API) goes with them where a file the commits add holds its code as often
//!   (`MoveEnds::lines_left`). Any other slice is read where it stands.
//! - **Cost**: no git process and no query of the index. The tokens of a file that stays are those its fingerprints
//!   read already; a file the commits add or delete is read only where one of its declarations has the name of a unit
//!   `added` or `removed`, which every place of the code of that unit has.

use crate::units::{MovedPlace, ReviewUnit, UnitKind};
use crate::whitespace::Code;
use std::collections::{HashMap, HashSet};

/// A place a declaration may have moved from or to (`MoveEnds`)
#[derive(Debug, Clone)]
pub(crate) struct MoveEnd {
  /// The unit of the declaration, by its id; `None` for a declaration of a file taken whole
  pub(crate) unit: Option<String>,
  /// Its file, as the units name it (`UnitSlice::path`)
  pub(crate) path: String,
  /// The line of its name on its side, from 1 (`ReviewUnit::line`)
  pub(crate) line: u32,
  pub(crate) name: String,
  /// What it is a member of, but the modules, and its name (`SearchAnswer.total`, `helper` of `mod tests`): what tells
  /// apart two places of one code
  pub(crate) member: String,
  pub(crate) code: Code,
  /// Its code but the modifiers of its reach (`Sides::code_but_reach`); `None` where the grammar has none
  pub(crate) loose: Option<Code>,
  /// The code of the header of the `impl` of Rust it is a member of (`whitespace::ImplBlock`): a member moves with it
  pub(crate) within: Option<u64>,
}

/// What a slice of the place declarations left holds (`MovedOut`): a declaration, by its place in `MoveEnds::left`; a
/// module of a file the commits delete, gone where it moved itself (`end`) or where each of its members did; a line of
/// no declaration that is an item of its own, by its place in `MoveEnds::lines_left`
#[derive(Debug, Clone)]
pub(crate) enum Held {
  End(usize),
  Module { end: Option<usize>, members: Vec<Self> },
  Line(usize),
}

impl Held {
  /// Whether it went where another place shows it: the declaration moved, the module moved whole or member by member,
  /// the line stands in a file the commits add
  fn gone(&self, left_moved: &[bool], lines_moved: &[bool]) -> bool {
    match self {
      Self::End(end) => left_moved[*end],
      Self::Module { end, members } => end.is_some_and(|end| left_moved[end]) || (!members.is_empty() && members.iter().all(|member| member.gone(left_moved, lines_moved))),
      Self::Line(line) => lines_moved[*line],
    }
  }
}

/// A slice of the place declarations left that holds nothing else (`UnitSlice::moved_out` once each of them moved): the
/// slice of a unit whose blocks remove those declarations alone, blank lines aside, the lines of a module or an `impl`
/// they emptied with them, and in the place of them the lines of a module declared in a file of the commits and of
/// imports alone (`SourceFile::removed_alone`), or a file the commits delete whose lines of code are those declarations
/// and its imports
#[derive(Debug, Clone)]
pub(crate) struct MovedOut {
  /// The unit whose slice it is, by its id; `None` for a file the commits delete, whose unit takes it whole (that of
  /// its path: `deletedFile`, or `deletedFiles` with its slice of `path`)
  pub(crate) unit: Option<String>,
  pub(crate) path: String,
  /// The declarations it holds
  pub(crate) held: Vec<Held>,
}

/// The places of the declarations of one side alone: those before the commits (`left`) and those after them
/// (`arrived`), and the slices of the places they left that hold them alone (`out`)
#[derive(Debug, Default)]
pub(crate) struct MoveEnds {
  pub(crate) left: Vec<MoveEnd>,
  pub(crate) arrived: Vec<MoveEnd>,
  pub(crate) out: Vec<MovedOut>,
  /// The code of each line of no declaration that is an item of its own (`Sides::item_line_code`: a call of a macro
  /// at the top of a file) that a slice of `out` removes, and of each of a file the commits add
  pub(crate) lines_left: Vec<Code>,
  pub(crate) lines_arrived: Vec<Code>,
}

impl MoveEnds {
  /// The place `end`, after the commits (`new_side`) or before them; its place in `left` or `arrived`
  pub(crate) fn push(&mut self, new_side: bool, end: MoveEnd) -> usize {
    let ends = if new_side { &mut self.arrived } else { &mut self.left };
    ends.push(end);
    ends.len() - 1
  }

  /// The names of the units of both sides: what a declaration of a file taken whole must be named to be a place of the
  /// code of a unit
  pub(crate) fn unit_names(&self) -> HashSet<String> {
    self.left.iter().chain(&self.arrived).filter(|end| end.unit.is_some()).map(|end| end.name.clone()).collect()
  }

  /// Whether a unit of a declaration of one side alone has its code: without one no move can be told
  pub(crate) fn has_units(&self) -> bool {
    self.left.iter().chain(&self.arrived).any(|end| end.unit.is_some())
  }
}

/// The places of one code on each side, by their place in `MoveEnds::left` and `MoveEnds::arrived`
type Sides = (Vec<usize>, Vec<usize>);

/// The places of `left` and `arrived` (of those `open` keeps), by `key`
fn by_key<K: std::hash::Hash + Eq>(ends: &MoveEnds, open: (&[bool], &[bool]), key: impl Fn(&MoveEnd) -> Option<K>) -> HashMap<K, Sides> {
  let mut by: HashMap<K, Sides> = HashMap::new();
  for (index, end) in ends.left.iter().enumerate().filter(|(index, _)| open.0[*index]) {
    if let Some(key) = key(end) {
      by.entry(key).or_default().0.push(index);
    }
  }
  for (index, end) in ends.arrived.iter().enumerate().filter(|(index, _)| open.1[*index]) {
    if let Some(key) = key(end) {
      by.entry(key).or_default().1.push(index);
    }
  }
  by
}

/// The pairs of places of one code (`left`, `arrived`): the place of each side where there is one, else each two of one
/// member (`MoveEnd::member`) where no other place of the code on its side is of it
fn one_to_one(ends: &MoveEnds, left: &[usize], arrived: &[usize]) -> Vec<(usize, usize)> {
  if let (&[from], &[to]) = (left, arrived) {
    return vec![(from, to)];
  }
  let alone = |places: &[usize], side: &[MoveEnd], member: &str| {
    let mut of = places.iter().filter(|&&place| side[place].member == member);
    of.next().filter(|_| of.next().is_none()).copied()
  };
  left
    .iter()
    .filter_map(|&from| {
      let member = ends.left[from].member.as_str();
      Some((alone(left, &ends.left, member)?, alone(arrived, &ends.arrived, member)?))
    })
    .collect()
}

/// Tell each unit of `units` that `ends` say moved whole to or from another file where it went (`moved_to`) or came
/// from (`moved_from`) and the unit there, and each slice of `ends.out` whose declarations all moved that it is the move
/// alone (`UnitSlice::moved_out`); every other unit has neither word, every other slice is read. The places pair by
/// their code first, the places of one code by their member; those left without a pair by their code but the modifiers
/// of their reach and their member, which says the reach changed (`MovedPlace::visibility`)
pub(crate) fn mark_moves(units: &mut [ReviewUnit], ends: &MoveEnds) {
  let at: HashMap<&str, usize> = units.iter().enumerate().map(|(index, unit)| (unit.id.as_str(), index)).collect();
  // the unit that takes a file whole, by the path of the file: the place of a declaration of it
  let whole: HashMap<&str, &str> = units
    .iter()
    .filter(|unit| matches!(unit.kind, UnitKind::NewFile | UnitKind::DeletedFile | UnitKind::DeletedFiles { .. }))
    .flat_map(|unit| unit.slices.iter().map(|slice| (slice.path.as_str(), unit.id.as_str())))
    .collect();
  let unit_of = |end: &MoveEnd| end.unit.as_deref().and_then(|id| at.get(id).copied());
  let shown_by = |end: &MoveEnd| end.unit.as_deref().or_else(|| whole.get(end.path.as_str()).copied()).map(str::to_string);
  let place = |end: &MoveEnd, visibility: bool| {
    shown_by(end).map(|unit| MovedPlace {
      path: end.path.clone(),
      line: end.line,
      unit,
      visibility,
    })
  };
  // (the unit, the place it moved to) and (the unit, the place it moved from); which ends before the commits moved
  let mut moved_to: Vec<(usize, MovedPlace)> = Vec::new();
  let mut moved_from: Vec<(usize, MovedPlace)> = Vec::new();
  let mut left_moved = vec![false; ends.left.len()];
  // the places without a pair yet, of each side
  let (mut open_left, mut open_arrived) = (vec![true; ends.left.len()], vec![true; ends.arrived.len()]);
  let exact = by_key(ends, (&open_left, &open_arrived), |end| Some((end.code, end.within)));
  let mut pairs: Vec<(usize, usize)> = exact.values().flat_map(|(left, arrived)| one_to_one(ends, left, arrived)).collect();
  for &(from, to) in &pairs {
    open_left[from] = false;
    open_arrived[to] = false;
  }
  let loose = by_key(ends, (&open_left, &open_arrived), |end| end.loose.map(|loose| (loose, end.within, end.member.clone())));
  pairs.extend(loose.values().filter_map(|places| match places {
    (left, arrived) if left.len() == 1 && arrived.len() == 1 => Some((left[0], arrived[0])),
    _ => None,
  }));
  for (from, to) in pairs {
    let (from_end, to_end) = (&ends.left[from], &ends.arrived[to]);
    // one of them a unit: a new and a deleted file alone are no move
    let (from_unit, to_unit) = (unit_of(from_end), unit_of(to_end));
    if from_end.path == to_end.path || (from_unit.is_none() && to_unit.is_none()) {
      continue;
    }
    let visibility = from_end.code != to_end.code;
    let (Some(to_place), Some(from_place)) = (place(to_end, visibility), place(from_end, visibility)) else {
      continue;
    };
    left_moved[from] = true;
    moved_to.extend(from_unit.map(|unit| (unit, to_place)));
    moved_from.extend(to_unit.map(|unit| (unit, from_place)));
  }
  // a line of no declaration went where the files the commits add hold its code as often as the places left it
  let mut arrived: HashMap<Code, usize> = HashMap::new();
  for code in &ends.lines_arrived {
    *arrived.entry(*code).or_default() += 1;
  }
  let mut left: HashMap<Code, usize> = HashMap::new();
  for code in &ends.lines_left {
    *left.entry(*code).or_default() += 1;
  }
  let lines_moved: Vec<bool> = ends.lines_left.iter().map(|code| left[code] <= arrived.get(code).copied().unwrap_or_default()).collect();
  // the units whose slice of a path is the move alone: each declaration it holds moved
  let moved_out: Vec<(usize, &str)> = ends
    .out
    .iter()
    .filter(|out| out.held.iter().any(|held| !matches!(held, Held::Line(_))) && out.held.iter().all(|held| held.gone(&left_moved, &lines_moved)))
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
