//! The groups of a large step (openspec/specs/branch/review-tour/spec.md, "Groups of a Step"): a reading order inside a step its
//! author told, never a task of its own. A commit of 125 files across four modules is one step; its stops in one list
//! read as a list of files. Cut by the modules of the plan, the step reads as its parts.
//!
//! - **Which steps**: a step of the commits or of the message, never one of the concerns (its evidence is the group).
//!   Cut by module when its stops of code, tests and resources lie in two modules or more and the step holds more than
//!   `MIN_GROUPED_FILES` files, of which a module the commits add or two modules of `MIN_MODULE_FILES` files each; else
//!   by the folders of its one module when it holds more than `MIN_FOLDER_FILES` files there; else no groups.
//! - **The groups**: `Contract` first (the stops of the entries of the brief and of the other files of prose: the rules of
//!   the specs, the docs, the guides, a table of a doc the brief leaves out), then a
//!   group per module (`Module`) or per folder below the root of the module (`Folder`, the first two directories without
//!   a leading source directory, as `ModuleShape::directories`), then the mechanical stops (`Mechanical`: the build model,
//!   the lists of modules, imports alone, …) last, as they were.
//! - **A small module** (fewer than `MIN_MODULE_FILES` files of the step) joins the group of the module of the step that
//!   newly uses it (`ModuleShape::uses`), which names it (`also`); a small module no module of the step uses stays a group.
//! - **The order of the modules**: a module whose declarations another of the step newly uses comes before it (their
//!   `uses`), the order of their first stops between modules no use orders and inside a cycle.
//! - **Inside a group**: the stops in the order of the step (a declaration before its uses), its tests after its code
//!   (`tests`: the first of them), so a reader meets the design before the code that checks it.

use crate::modules::{ModuleShape, ModuleState, SharedFolders, is_prose};
use crate::steps::{ReviewStep, StepStop, StepTests, StepsInput};
use crate::units::{ReviewUnit, UnitKind};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};

/// A step with fewer files is read as one list
pub(crate) const MIN_GROUPED_FILES: usize = 12;
/// A module with fewer files of the step joins the group of the module that uses it
pub(crate) const MIN_MODULE_FILES: usize = 4;
/// Two modules of this many files each cut a step without a new module
const LARGE_MODULE_FILES: usize = 5;
/// A step of one module with more files is cut by its folders
pub(crate) const MIN_FOLDER_FILES: usize = 25;

/// What a group of a step holds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum GroupKind {
  /// The rules of the specs and the docs the step changes: the entries of the brief
  Contract,
  /// The code, tests and resources of one module
  Module,
  /// Of a step of one module: the files of one of its folders
  Folder,
  /// The mechanical stops: the build model, the lists of modules, imports alone, whitespace, moved declarations,
  /// renames, generated files
  Mechanical,
}

/// A run of the stops of a step that reads as one part of it (`ReviewStep::groups`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StepGroup {
  pub kind: GroupKind,
  /// Of a group of a module or of a folder: its module, by its place in `ReviewPlan::modules`
  pub module: Option<u32>,
  /// Of a group of a folder: the folder below the root of its module (`""` for the files at its top)
  pub folder: Option<String>,
  /// The small modules whose stops it holds, by their place in `ReviewPlan::modules`
  pub also: Vec<u32>,
  /// Its stops are `start..end` of `ReviewStep::stops`; those from `tests` on are tests (`tests == end`: none)
  pub start: u32,
  pub tests: u32,
  pub end: u32,
  /// The tests of the change that reach the code of its files and its code none reaches (`ReviewSteps::apply_tests`), as
  /// `ReviewStep::tests` are of the whole step: what the opening of the group says, the step's opening no more
  pub tested: StepTests,
}

/// What a stop is to the groups
#[derive(Debug, Clone, PartialEq, Eq)]
enum Place {
  Contract,
  Mechanical,
  /// Code, tests and resources of a module, `None` for a file of no module
  Module(Option<u32>),
}

/// Cut `step` into its groups and reorder its stops so that each group is one run of them: nothing when it is no step
/// to cut
pub(crate) fn cut_groups(input: &StepsInput<'_>, step: &mut ReviewStep) {
  if step.concern.is_some() || step.stops.is_empty() {
    return;
  }
  let old = std::mem::take(&mut step.stops);
  let (stops, groups) = match cut(input, &old) {
    Some(cut) => cut,
    None => (old, Vec::new()),
  };
  step.stops = stops;
  step.groups = groups;
}

/// The stops of a step in the order of its groups, and the groups; `None` for a step read as one list
fn cut(input: &StepsInput<'_>, old: &[StepStop]) -> Option<(Vec<StepStop>, Vec<StepGroup>)> {
  let units: HashMap<&str, &ReviewUnit> = input.units.iter().map(|unit| (unit.id.as_str(), unit)).collect();
  let contract: HashSet<&str> = input.contract.iter().map(|entry| entry.path.as_str()).collect();
  let shapes = &input.modules.shapes;
  let places: Vec<Place> = old
    .iter()
    .map(|stop| {
      if units.get(stop.unit.as_str()).is_some_and(|unit| matches!(unit.kind, UnitKind::Mechanical { .. })) {
        Place::Mechanical
      } else if contract.contains(stop.path.as_str()) || is_prose(&stop.path) {
        Place::Contract
      } else {
        Place::Module(input.modules.of_path(input.changed, &stop.path))
      }
    })
    .collect();
  // the files of the step in each module, by the order of their first stop
  let mut module_files: Vec<(Option<u32>, BTreeSet<&str>)> = Vec::new();
  for (stop, place) in old.iter().zip(&places) {
    if let Place::Module(module) = place {
      match module_files.iter_mut().find(|(known, _)| known == module) {
        Some((_, files)) => {
          files.insert(stop.path.as_str());
        }
        None => module_files.push((*module, BTreeSet::from([stop.path.as_str()]))),
      }
    }
  }
  let files: usize = old.iter().map(|stop| stop.path.as_str()).collect::<HashSet<_>>().len();
  let added = |module: Option<u32>| module.and_then(|module| shapes.get(module as usize)).is_some_and(|shape| shape.state == ModuleState::Added);
  let by_module = module_files.len() >= 2
    && files > MIN_GROUPED_FILES
    && (module_files.iter().any(|(module, _)| added(*module)) || module_files.iter().filter(|(_, files)| files.len() >= LARGE_MODULE_FILES).count() >= 2);
  let by_folder = module_files.len() == 1 && module_files[0].1.len() > MIN_FOLDER_FILES;
  if !by_module && !by_folder {
    return None;
  }
  // the key of the group of each stop of code
  let keys: Vec<Option<GroupKey>> = if by_module {
    let home = homes(&module_files, shapes);
    places
      .iter()
      .map(|place| match place {
        Place::Module(module) => Some(GroupKey::Module(home.get(module).copied().flatten())),
        _ => None,
      })
      .collect()
  } else {
    let root = module_files[0].0.and_then(|module| shapes.get(module as usize)).map_or("", |shape| shape.root.as_str());
    let paths: Vec<&str> = module_files[0].1.iter().copied().collect();
    let shared = SharedFolders::of(&paths, root);
    old
      .iter()
      .zip(&places)
      .map(|(stop, place)| matches!(place, Place::Module(_)).then(|| GroupKey::Folder(shared.directory(&stop.path))))
      .collect()
  };
  // the groups of code in their order, each with its stops
  let mut order: Vec<GroupKey> = Vec::new();
  for key in keys.iter().flatten() {
    if !order.contains(key) {
      order.push(key.clone());
    }
  }
  if by_module {
    order = dependency_order(&order, shapes);
  }
  let is_test = |stop: &StepStop| units.get(stop.unit.as_str()).is_some_and(|unit| unit.test) || crate::files::test_subject(&stop.path).is_some();
  let mut stops: Vec<StepStop> = Vec::with_capacity(old.len());
  let mut groups: Vec<StepGroup> = Vec::new();
  let mut taken = vec![false; old.len()];
  let mut push_group = |kind: GroupKind, key: Option<&GroupKey>, wanted: &dyn Fn(usize) -> bool, stops: &mut Vec<StepStop>, taken: &mut Vec<bool>| {
    let start = stops.len();
    let mut tests: Vec<StepStop> = Vec::new();
    for (index, stop) in old.iter().enumerate() {
      if !taken[index] && wanted(index) {
        taken[index] = true;
        if kind == GroupKind::Mechanical || kind == GroupKind::Contract || !is_test(stop) {
          stops.push(stop.clone());
        } else {
          tests.push(stop.clone());
        }
      }
    }
    let tests_at = stops.len();
    stops.extend(tests);
    if stops.len() > start {
      let (module, folder, also) = match key {
        Some(GroupKey::Module(module)) => (*module, None, also_of(*module, &module_files, shapes)),
        Some(GroupKey::Folder(folder)) => (module_files[0].0, Some(folder.clone()), Vec::new()),
        None => (None, None, Vec::new()),
      };
      groups.push(StepGroup {
        kind,
        module,
        folder,
        also,
        start: to_u32(start),
        tests: to_u32(tests_at),
        end: to_u32(stops.len()),
        tested: StepTests::default(),
      });
    }
  };
  push_group(GroupKind::Contract, None, &|index| places[index] == Place::Contract, &mut stops, &mut taken);
  for key in &order {
    let kind = if matches!(key, GroupKey::Folder(_)) { GroupKind::Folder } else { GroupKind::Module };
    push_group(kind, Some(key), &|index| keys[index].as_ref() == Some(key), &mut stops, &mut taken);
  }
  push_group(GroupKind::Mechanical, None, &|index| places[index] == Place::Mechanical, &mut stops, &mut taken);
  // every stop once: what no group took stays at the end, in its order
  stops.extend(old.iter().zip(&taken).filter(|(_, taken)| !**taken).map(|(stop, _)| stop.clone()));
  Some((stops, groups))
}

/// The group a stop of code goes to
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum GroupKey {
  Module(Option<u32>),
  Folder(String),
}

/// The module whose group holds the stops of each module of the step: its own, or of a small one the group of the
/// module of the step that newly uses it, the one with the most files of equal ones
fn homes(module_files: &[(Option<u32>, BTreeSet<&str>)], shapes: &[ModuleShape]) -> HashMap<Option<u32>, Option<u32>> {
  let large: Vec<Option<u32>> = module_files
    .iter()
    .filter(|(_, files)| files.len() >= MIN_MODULE_FILES)
    .map(|(module, _)| *module)
    .collect();
  module_files
    .iter()
    .map(|(module, files)| {
      if files.len() >= MIN_MODULE_FILES {
        return (*module, *module);
      }
      let user = module.and_then(|small| {
        large
          .iter()
          .flatten()
          .copied()
          .filter(|&user| shapes.get(user as usize).is_some_and(|shape| shape.uses.iter().any(|used| used.module == small)))
          .max_by_key(|&user| module_files.iter().find(|(known, _)| *known == Some(user)).map_or(0, |(_, files)| files.len()))
      });
      (*module, user.map_or(*module, Some))
    })
    .collect()
}

/// The small modules whose stops the group of `module` holds
fn also_of(module: Option<u32>, module_files: &[(Option<u32>, BTreeSet<&str>)], shapes: &[ModuleShape]) -> Vec<u32> {
  let home = homes(module_files, shapes);
  module_files
    .iter()
    .filter_map(|(small, _)| (*small != module && home.get(small).copied().flatten() == module).then_some(*small).flatten())
    .collect()
}

/// `order` with a module whose declarations another newly uses before that one; their first stops order the rest
fn dependency_order(order: &[GroupKey], shapes: &[ModuleShape]) -> Vec<GroupKey> {
  let module_of = |key: &GroupKey| match key {
    GroupKey::Module(module) => *module,
    GroupKey::Folder(_) => None,
  };
  // the groups each one waits for: those of the modules it newly uses
  let waits: Vec<Vec<usize>> = order
    .iter()
    .map(|key| {
      let Some(shape) = module_of(key).and_then(|module| shapes.get(module as usize)) else {
        return Vec::new();
      };
      order
        .iter()
        .enumerate()
        .filter(|(_, other)| module_of(other).is_some_and(|other| shape.uses.iter().any(|used| used.module == other)))
        .map(|(place, _)| place)
        .collect()
    })
    .collect();
  let mut done = vec![false; order.len()];
  let mut placed: Vec<usize> = Vec::with_capacity(order.len());
  while placed.len() < order.len() {
    // the first group in the order of the step whose modules are placed, else the first left (a cycle)
    let next = (0..order.len())
      .find(|&place| !done[place] && waits[place].iter().all(|&waited| done[waited] || waited == place))
      .or_else(|| (0..order.len()).find(|&place| !done[place]));
    let Some(next) = next else { break };
    done[next] = true;
    placed.push(next);
  }
  placed.into_iter().map(|place| order[place].clone()).collect()
}

fn to_u32(value: usize) -> u32 {
  u32::try_from(value).unwrap_or(u32::MAX)
}

/// The group of each module the change adds starts with the files of its entry points (`ModuleShape::entries`), in
/// their order, before the rest of its code in the order it had; its tests stay at its end; the files of each step
/// (`ReviewStep::paths`) follow its stops
pub(crate) fn order_by_entries(steps: &mut [ReviewStep], modules: &[ModuleShape]) {
  for step in steps {
    let mut moved = false;
    for group in &step.groups {
      let entries = group.module.and_then(|module| modules.get(module as usize)).and_then(|module| module.entries.as_ref());
      let Some(entries) = entries.filter(|_| group.kind == GroupKind::Module) else { continue };
      let rank = |path: &str| entries.points.iter().position(|point| point.path == path).unwrap_or(usize::MAX);
      let code = &mut step.stops[group.start as usize..group.tests as usize];
      if code.windows(2).any(|pair| rank(&pair[0].path) > rank(&pair[1].path)) {
        // stable: the stops of one file keep their order, the files of no entry theirs
        code.sort_by_key(|stop| rank(&stop.path));
        moved = true;
      }
    }
    if moved {
      let mut paths: Vec<String> = Vec::new();
      for stop in &step.stops {
        if !paths.contains(&stop.path) {
          paths.push(stop.path.clone());
        }
      }
      // then the files only another step's stops show, in the order they had
      for path in std::mem::take(&mut step.paths) {
        if !paths.contains(&path) {
          paths.push(path);
        }
      }
      step.paths = paths;
    }
  }
}
