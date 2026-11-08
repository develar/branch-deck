//! The modules of a review (spec/branch/review-plan.spec.md, "Modules"): the shape of a change before any of its words,
//! which modules it adds, changes or deletes, how large each part is, and what each newly uses.
//!
//! - **The module of a file** is the nearest directory above it with a build file at the head reviewed
//!   (`ModuleRoots::root_of`: an `*.iml`, a `*.csproj`, a Bazel, Gradle or Maven build file, a `Cargo.toml`, a
//!   `package.json`), or of a module the commits delete, whose build file only the base has; the root of the
//!   repository (`""`) for a file under none.
//! - **Its state**: `Added` when the commits add a build file at its root, `Deleted` when they delete one and the head
//!   has none there, else `Changed`.
//! - **Its name**: of its `*.iml`, its crate, else the name a dependency change gives its build file, else its
//!   directory; `(root)` for the root. **Its label**, where the review names it: the shortest suffix of its root that
//!   no other module of the review shares, of two directories at least when its last has no `.`, `-` or `_` (`shared/code`,
//!   not `code`; `view.gui.media` and `review-plan` alone).
//! - **Its files** by role: `build` (the build model, a manifest, the list of a module: `MechanicalKind::BuildModel` and
//!   `ModuleList`), `prose` (Markdown and the like), `tests` (the test paths of `files::test_subject`), `code` (a
//!   language `code_symbols` parses), the rest `resources`; its lines added and removed over the range.
//! - **Its directories**: below the root, without a leading source directory (`src`, `testSrc`, `main`, `kotlin`, …),
//!   by their first two levels, the most files first.
//! - **What it newly uses**: the modules of the review its files link to by links the commits add
//!   (`FileLink::new_between_modules`), and those a dependency entry its build files add names (`declared`); the plugin
//!   modules whose descriptors add it as content (`content_of`); the entries its build files add that name no module
//!   of the review (`new_dependencies`, by their place in `ReviewPlan::dependencies`).
//! - **The order**: a module the commits add first, then the most changed lines of code and tests, then tree order.

use crate::dependencies::{AddedManifest, DependencyChange, DependencyChangeKind, DependencyKind};
use crate::files::{ChangedFile, MechanicalKind, test_subject};
use crate::links::FileLink;
use crate::pair::PairBlocks;
use code_symbols::language::language_of;
use file_diffs::service::compare_tree_paths;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, HashSet};
use trial_check::module_roots::{ModuleRoots, is_module_file};

/// What the commits do to a module
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum ModuleState {
  Added,
  Changed,
  Deleted,
}

/// The changed files of a module by their role
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ModuleFiles {
  pub code: u32,
  pub tests: u32,
  pub resources: u32,
  pub build: u32,
  pub prose: u32,
}

/// A directory of a module with changed files, below its root
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ModuleDirectory {
  /// Below the root, without a leading source directory, at most two levels (`ui/embeds`); `""` for the files at the top
  pub path: String,
  pub files: u32,
}

/// Another module of the review that a module newly uses
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ModuleUse {
  /// By its place in `ReviewPlan::modules`
  pub module: u32,
  /// The links the commits add from its files to that module's, by their place in `ReviewPlan::links`
  pub links: Vec<u32>,
  /// A dependency entry its build files add names that module
  pub declared: bool,
}

/// A module the commits change, with the shape of its change
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ModuleShape {
  /// The directory of its build file, `""` for the root of the repository
  pub root: String,
  pub name: String,
  /// How the review names it: the shortest suffix of its root no other module of the review shares
  pub label: String,
  pub state: ModuleState,
  pub files: ModuleFiles,
  pub additions: u32,
  pub deletions: u32,
  /// The most files first, then in tree order
  pub directories: Vec<ModuleDirectory>,
  /// The most links first, the declared ones before the rest of equal count
  pub uses: Vec<ModuleUse>,
  /// The modules of the review that newly use it, by their place in `ReviewPlan::modules`
  pub used_by: Vec<u32>,
  /// The plugin modules whose descriptors the commits make hold it as content
  pub content_of: Vec<String>,
  /// The entries its build files add that name no module of the review, by their place in `ReviewPlan::dependencies`
  pub new_dependencies: Vec<u32>,
}

/// The modules of a review and the module of each changed file
#[derive(Debug, Default)]
pub(crate) struct Modules {
  pub shapes: Vec<ModuleShape>,
  /// By the place of each file of `changed`: its module in `shapes`
  pub of_file: Vec<u32>,
}

impl Modules {
  /// The module of `path`, a file of the review
  pub(crate) fn of_path(&self, changed: &[ChangedFile<'_>], path: &str) -> Option<u32> {
    changed.iter().position(|file| file.path == path).and_then(|index| self.of_file.get(index).copied())
  }
}

/// The directories a source tree starts with, which say nothing of what a directory below them holds
const SOURCE_DIRECTORIES: [&str; 11] = ["src", "testSrc", "test", "tests", "main", "kotlin", "java", "resources", "lib", "gen", "testResources"];

/// The extensions of prose
const PROSE_EXTENSIONS: [&str; 6] = ["md", "markdown", "mdx", "rst", "adoc", "txt"];

/// What a changed file is to its module
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
  Code,
  Tests,
  Resources,
  Build,
  Prose,
}

/// What `module_shapes` reads
pub(crate) struct ModulesInput<'a> {
  pub changed: &'a [ChangedFile<'a>],
  /// The module structure at the head, `None` without a summary read
  pub roots: Option<&'a ModuleRoots>,
  /// Crate directory -> crate name
  pub crates: Option<&'a HashMap<String, String>>,
  /// By the place of each file of `changed`
  pub mechanical: &'a [Option<MechanicalKind>],
  pub pairs: &'a [PairBlocks],
  pub links: &'a [FileLink],
  pub dependencies: &'a [DependencyChange],
  /// The entries of the manifests the commits add
  pub added_manifests: &'a [AddedManifest],
}

/// The modules of the files of a review, in their order
pub(crate) fn module_shapes(input: &ModulesInput<'_>) -> Modules {
  let changed = input.changed;
  // the roots of the modules the commits delete: their build files are in no tree of the head
  let deleted_roots: HashSet<&str> = changed
    .iter()
    .filter(|file| file.deleted && is_module_file(file_name(file.path)))
    .map(|file| directory_of(file.path))
    .collect();
  let root_of = |path: &str| -> String {
    let head = input.roots.and_then(|roots| roots.root_of(path)).unwrap_or("");
    let deleted = deleted_roots.iter().copied().filter(|root| is_under(path, root)).max_by_key(|root| root.len());
    match deleted {
      Some(deleted) if deleted.len() > head.len() => deleted.to_string(),
      _ => head.to_string(),
    }
  };
  let roots: Vec<String> = changed.iter().map(|file| root_of(file.path)).collect();
  // the roots in tree order, each once
  let mut distinct: Vec<&str> = roots.iter().map(String::as_str).collect::<HashSet<_>>().into_iter().collect();
  distinct.sort_by(|a, b| compare_tree_paths(a, b));
  let labels = labels_of(&distinct);
  let mut shapes: Vec<ModuleShape> = distinct
    .iter()
    .zip(labels)
    .map(|(&root, label)| ModuleShape {
      root: root.to_string(),
      name: name_of(root, input),
      label,
      state: state_of(root, changed, &deleted_roots),
      files: ModuleFiles::default(),
      additions: 0,
      deletions: 0,
      directories: Vec::new(),
      uses: Vec::new(),
      used_by: Vec::new(),
      content_of: Vec::new(),
      new_dependencies: Vec::new(),
    })
    .collect();
  let place: HashMap<&str, usize> = distinct.iter().enumerate().map(|(index, &root)| (root, index)).collect();
  let mut directories: Vec<BTreeMap<String, u32>> = vec![BTreeMap::new(); shapes.len()];
  for (index, file) in changed.iter().enumerate() {
    let module = place[roots[index].as_str()];
    let shape = &mut shapes[module];
    match role_of(file.path, input.mechanical.get(index).copied().flatten(), input.dependencies) {
      Role::Code => shape.files.code += 1,
      Role::Tests => shape.files.tests += 1,
      Role::Resources => shape.files.resources += 1,
      Role::Build => shape.files.build += 1,
      Role::Prose => shape.files.prose += 1,
    }
    let (additions, deletions) = line_counts(file, input.pairs.get(index));
    shape.additions += additions;
    shape.deletions += deletions;
    *directories[module].entry(directory_below(file.path, &roots[index])).or_default() += 1;
  }
  for (shape, found) in shapes.iter_mut().zip(directories) {
    let mut found: Vec<ModuleDirectory> = found.into_iter().map(|(path, files)| ModuleDirectory { path, files }).collect();
    found.sort_by(|a, b| b.files.cmp(&a.files).then_with(|| compare_tree_paths(&a.path, &b.path)));
    shape.directories = found;
  }
  add_uses(&mut shapes, &place, input, &root_of);
  // the order: an added module first, then the most changed lines of code and tests, then tree order (`distinct`)
  let weight: Vec<u32> = (0..shapes.len())
    .map(|module| {
      changed
        .iter()
        .enumerate()
        .filter(|(index, file)| {
          place[roots[*index].as_str()] == module
            && matches!(
              role_of(file.path, input.mechanical.get(*index).copied().flatten(), input.dependencies),
              Role::Code | Role::Tests
            )
        })
        .map(|(index, file)| {
          let (additions, deletions) = line_counts(file, input.pairs.get(index));
          additions + deletions
        })
        .sum()
    })
    .collect();
  let mut order: Vec<usize> = (0..shapes.len()).collect();
  order.sort_by_key(|&module| (shapes[module].state != ModuleState::Added, Reverse(weight[module]), module));
  let new_place: Vec<u32> = {
    let mut new_place = vec![0; order.len()];
    for (at, &module) in order.iter().enumerate() {
      new_place[module] = u32::try_from(at).unwrap_or(u32::MAX);
    }
    new_place
  };
  let mut ordered: Vec<ModuleShape> = order.iter().map(|&module| shapes[module].clone()).collect();
  for shape in &mut ordered {
    for used in &mut shape.uses {
      used.module = new_place[used.module as usize];
    }
    for user in &mut shape.used_by {
      *user = new_place[*user as usize];
    }
    shape.used_by.sort_unstable();
  }
  Modules {
    shapes: ordered,
    of_file: roots.iter().map(|root| new_place[place[root.as_str()]]).collect(),
  }
}

/// `uses`, `used_by`, `content_of` and `new_dependencies` of every module
fn add_uses(shapes: &mut [ModuleShape], place: &HashMap<&str, usize>, input: &ModulesInput<'_>, root_of: &dyn Fn(&str) -> String) {
  let modules = shapes.len();
  // (user, used) -> the links
  let mut linked: BTreeMap<(usize, usize), Vec<u32>> = BTreeMap::new();
  for (index, link) in input.links.iter().enumerate().filter(|(_, link)| link.new_between_modules) {
    let (Some(&from), Some(&to)) = (place.get(root_of(&link.from).as_str()), place.get(root_of(&link.to).as_str())) else {
      continue;
    };
    if from != to {
      linked.entry((from, to)).or_default().push(u32::try_from(index).unwrap_or(u32::MAX));
    }
  }
  let by_name: HashMap<&str, usize> = shapes.iter().enumerate().map(|(index, shape)| (shape.name.as_str(), index)).collect();
  let mut declared: HashSet<(usize, usize)> = HashSet::new();
  let mut content: Vec<Vec<String>> = vec![Vec::new(); modules];
  let mut external: Vec<Vec<u32>> = vec![Vec::new(); modules];
  for (index, change) in input.dependencies.iter().enumerate().filter(|(_, change)| change.change == DependencyChangeKind::Added) {
    let Some(&owner) = place.get(root_of(&change.manifest).as_str()) else {
      continue;
    };
    let named = by_name.get(change.name.as_str()).copied();
    match change.kind {
      DependencyKind::NewModule => {}
      DependencyKind::Content => match named {
        Some(module) => content[module].push(change.module.clone()),
        None => external[owner].push(u32::try_from(index).unwrap_or(u32::MAX)),
      },
      DependencyKind::Module | DependencyKind::Library | DependencyKind::Plugin | DependencyKind::Crate | DependencyKind::Package => match named {
        Some(module) if module != owner => {
          declared.insert((owner, module));
        }
        Some(_) => {}
        None => external[owner].push(u32::try_from(index).unwrap_or(u32::MAX)),
      },
    }
  }
  // what a new manifest declares: its uses of the other modules of the review
  for manifest in input.added_manifests {
    let Some(&owner) = place.get(root_of(&manifest.manifest).as_str()) else {
      continue;
    };
    for (kind, name) in &manifest.entries {
      if matches!(kind, DependencyKind::Module | DependencyKind::Crate | DependencyKind::Package)
        && let Some(&module) = by_name.get(name.as_str())
        && module != owner
      {
        declared.insert((owner, module));
      }
    }
  }
  let mut uses: Vec<Vec<ModuleUse>> = vec![Vec::new(); modules];
  let mut pairs: Vec<(usize, usize)> = linked.keys().copied().chain(declared.iter().copied()).collect::<HashSet<_>>().into_iter().collect();
  pairs.sort_unstable();
  for (from, to) in pairs {
    uses[from].push(ModuleUse {
      module: u32::try_from(to).unwrap_or(u32::MAX),
      links: linked.get(&(from, to)).cloned().unwrap_or_default(),
      declared: declared.contains(&(from, to)),
    });
  }
  for (module, shape) in shapes.iter_mut().enumerate() {
    let mut found = std::mem::take(&mut uses[module]);
    found.sort_by(|a, b| {
      b.links
        .len()
        .cmp(&a.links.len())
        .then_with(|| b.declared.cmp(&a.declared))
        .then_with(|| a.module.cmp(&b.module))
    });
    shape.uses = found;
    shape.content_of = std::mem::take(&mut content[module]);
    shape.content_of.dedup();
    shape.new_dependencies = std::mem::take(&mut external[module]);
  }
  let users: Vec<Vec<u32>> = (0..modules)
    .map(|used| {
      (0..modules)
        .filter(|&user| shapes[user].uses.iter().any(|it| it.module as usize == used))
        .map(|user| u32::try_from(user).unwrap_or(u32::MAX))
        .collect()
    })
    .collect();
  for (shape, users) in shapes.iter_mut().zip(users) {
    shape.used_by = users;
  }
}

/// The role of a changed file in its module
fn role_of(path: &str, mechanical: Option<MechanicalKind>, dependencies: &[DependencyChange]) -> Role {
  let name = file_name(path);
  let extension = name.rsplit_once('.').map(|(_, extension)| extension.to_ascii_lowercase());
  if matches!(mechanical, Some(MechanicalKind::BuildModel | MechanicalKind::ModuleList)) || is_module_file(name) || dependencies.iter().any(|change| change.manifest == path) {
    Role::Build
  } else if extension.as_deref().is_some_and(|extension| PROSE_EXTENSIONS.contains(&extension)) {
    Role::Prose
  } else if test_subject(path).is_some() {
    Role::Tests
  } else if language_of(path).is_some() {
    Role::Code
  } else {
    Role::Resources
  }
}

/// The lines the range adds and removes in `file`: of its blocks, else of its commits
fn line_counts(file: &ChangedFile<'_>, pair: Option<&PairBlocks>) -> (u32, u32) {
  match pair {
    Some(PairBlocks::Blocks(blocks)) => blocks.iter().fold((0, 0), |(added, removed), block| (added + span(&block.new), removed + span(&block.old))),
    _ => file
      .changes
      .iter()
      .fold((0, 0), |(added, removed), change| (added + change.additions, removed + change.deletions)),
  }
}

/// The directory of `path` below `root`, without a leading source directory, at most two levels
fn directory_below(path: &str, root: &str) -> String {
  let below = if root.is_empty() {
    path
  } else {
    path.strip_prefix(root).and_then(|rest| rest.strip_prefix('/')).unwrap_or(path)
  };
  let directory = below.rsplit_once('/').map_or("", |(directory, _)| directory);
  let segments: Vec<&str> = directory
    .split('/')
    .filter(|segment| !segment.is_empty())
    .skip_while(|segment| SOURCE_DIRECTORIES.contains(segment))
    .take(2)
    .collect();
  segments.join("/")
}

/// The shortest suffix of each root no other root shares, of two directories at least when the last has no `.`, `-`
/// or `_`; `(root)` for the root
pub(crate) fn labels_of(roots: &[&str]) -> Vec<String> {
  let segments: Vec<Vec<&str>> = roots.iter().map(|root| root.split('/').filter(|segment| !segment.is_empty()).collect()).collect();
  let suffix = |segments: &[&str], count: usize| segments[segments.len().saturating_sub(count)..].join("/");
  segments
    .iter()
    .map(|own| {
      if own.is_empty() {
        return "(root)".to_string();
      }
      let distinct = own.last().is_some_and(|last| last.contains(['.', '-', '_']));
      let mut count = if distinct { 1 } else { 2.min(own.len()) };
      while count < own.len()
        && segments
          .iter()
          .filter(|other| !other.is_empty())
          .filter(|other| *other != own)
          .any(|other| suffix(other, count) == suffix(own, count))
      {
        count += 1;
      }
      suffix(own, count)
    })
    .collect()
}

/// The name of the module at `root`: its `*.iml`, its crate, the module a dependency change gives a build file at its
/// root, else its directory
fn name_of(root: &str, input: &ModulesInput<'_>) -> String {
  if root.is_empty() {
    return "(root)".to_string();
  }
  input
    .roots
    .and_then(|roots| roots.iml_of(root))
    .map(str::to_string)
    .or_else(|| input.crates.and_then(|crates| crates.get(root)).cloned())
    .or_else(|| {
      input
        .dependencies
        .iter()
        .find(|change| directory_of(&change.manifest) == root && is_module_file(file_name(&change.manifest)))
        .map(|change| change.module.clone())
    })
    .unwrap_or_else(|| file_name(root).to_string())
}

/// `Added` when the commits add a build file at `root`, `Deleted` when they delete one and the head has no module there
fn state_of(root: &str, changed: &[ChangedFile<'_>], deleted_roots: &HashSet<&str>) -> ModuleState {
  let build_files = || changed.iter().filter(|file| directory_of(file.path) == root && is_module_file(file_name(file.path)));
  if deleted_roots.contains(root) && build_files().all(|file| file.deleted) {
    ModuleState::Deleted
  } else if build_files().any(|file| file.before.is_none() && !file.deleted && file.changes.first().is_some_and(|change| change.status == 'A')) {
    ModuleState::Added
  } else {
    ModuleState::Changed
  }
}

/// The lines of a range of lines
const fn span(lines: &std::ops::Range<u32>) -> u32 {
  lines.end.saturating_sub(lines.start)
}

fn file_name(path: &str) -> &str {
  path.rsplit('/').next().unwrap_or(path)
}

fn directory_of(path: &str) -> &str {
  path.rsplit_once('/').map_or("", |(directory, _)| directory)
}

/// `path` is inside the directory `root`
fn is_under(path: &str, root: &str) -> bool {
  root.is_empty() || path.strip_prefix(root).is_some_and(|rest| rest.starts_with('/'))
}
