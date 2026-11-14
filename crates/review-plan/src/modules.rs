//! The modules of a review (openspec/specs/branch/review-plan/spec.md, "Modules"): the shape of a change before any of its words,
//! which modules it adds, changes or deletes, how large each part is, and what each newly uses.
//!
//! - **The module of a file** is the root accio's `modules` answers for its path at the head reviewed (the nearest
//!   directory above it whose build files declare a module: JPS, Bazel, Cargo, npm, Go, Python, Maven, Gradle,
//!   .NET), or the deeper root it answers at the base, a module the commits delete; the root of the repository
//!   (`""`) for a file under none, and for every file without both answers (`DeclaredRoots`).
//! - **Its state**: `Deleted` when its root is of the base alone, `Added` when it is of the head alone (no file of it has
//!   that root at the base), else `Changed`.
//! - **Its name**: the name its build files give it (its `*.iml`, its crate, its package, …), else the name a dependency
//!   change gives its build file, else its directory; `(root)` for the root. **Its label**, where the review names it:
//!   the shortest suffix of its root that no other module of the review shares, of two directories at least when its
//!   last has no `.`, `-` or `_` (`shared/code`, not `code`; `view.gui.media` and `review-plan` alone).
//! - **Its files** by role: `build` (the build model, a manifest, the list of a module: `MechanicalKind::BuildModel` and
//!   `ModuleList`), `prose` (Markdown and the like), `tests` (the test paths of `files::test_subject`), `code` (a
//!   language `code_symbols` parses), the rest `resources`; its lines added and removed over the range, as
//!   `git diff --numstat` counts them (`line_counts`).
//! - **Its directories**: below the root, without the leading source directories (`src`, `testSrc`, `main`, `kotlin`,
//!   …) and the folders its code and its tests share (the packages `com/example/player` under `src` and under
//!   `testSrc`), its resources apart (a bundle under `resources/messages` shares none of them), a file right in its root
//!   (its build file) taking no part, by their first two levels, the most files first; each file its `folder` below
//!   them, all of its levels.
//! - **What it newly uses**: the modules of the review its files link to by links the commits add
//!   (`FileLink::new_between_modules`), and those a dependency entry its build files add names (`declared`); the plugin
//!   modules whose descriptors add it as content (`content_of`); the entries its build files add that name no module
//!   of the review (`new_dependencies`, by their place in `ReviewPlan::dependencies`).
//! - **The order**: a module the commits add first, then the most changed lines of code and tests, then tree order.

use crate::dependencies::{AddedManifest, DependencyChange, DependencyChangeKind, DependencyKind, is_manifest};
use crate::entries::ModuleEntries;
use crate::fan_in::ask_index;
use crate::files::{ChangedFile, MechanicalKind, is_build_model, is_module_file, test_subject};
use crate::links::FileLink;
use crate::pair::PairBlocks;
use anyhow::Result;
use code_symbols::language::language_of;
use domain_types::range_reading::FileRole;
use file_diffs::accio::{DeclaredModule, ModulesAnswer, SymbolIndex};
use file_diffs::service::compare_tree_paths;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, HashSet};
use sync_utils::span::in_current_span;

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
  /// Of a module the change adds: what enters it, from accio (`entries::add_entries`); `None` before the index answers,
  /// without accio, and for a module the change changes or deletes
  pub entries: Option<ModuleEntries>,
}

/// The modules of a review and the module of each changed file
#[derive(Debug, Default)]
pub(crate) struct Modules {
  pub shapes: Vec<ModuleShape>,
  /// By the place of each file of `changed`: its module in `shapes`
  pub of_file: Vec<u32>,
  /// By the place of each file of `changed`: its directory below the folders the files of its module share
  /// (`SharedFolders::below`)
  pub folder_of: Vec<String>,
}

impl Modules {
  /// The module of `path`, a file of the review
  pub(crate) fn of_path(&self, changed: &[ChangedFile<'_>], path: &str) -> Option<u32> {
    changed.iter().position(|file| file.path == path).and_then(|index| self.of_file.get(index).copied())
  }

  /// The directory of `path`, a file of the review, below the folders the files of its module share
  pub(crate) fn folder_of_path(&self, changed: &[ChangedFile<'_>], path: &str) -> String {
    changed
      .iter()
      .position(|file| file.path == path)
      .and_then(|index| self.folder_of.get(index).cloned())
      .unwrap_or_default()
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

/// The roots of the modules of the changed files of a review, as accio's `modules` answers them at the head reviewed
/// and at the base: none, every file of the root of the repository, without both answers (no accio, no index, an accio
/// that gives another revision of the answer, `MODULES_ANSWER`)
#[derive(Debug, Default)]
pub(crate) struct DeclaredRoots {
  /// Path -> the root of its module at the head, `None` outside every one
  head: HashMap<String, Option<String>>,
  /// Path -> the root of its module at the base
  base: HashMap<String, Option<String>>,
  /// Root -> what declares a module there, at the head, else at the base
  declared: HashMap<String, DeclaredModule>,
}

impl DeclaredRoots {
  /// The roots of `paths` at `head` and at `base`, one accio process of each side, at once
  pub(crate) fn ask(index: Option<&dyn SymbolIndex>, repository_path: &str, base: &str, head: &str, paths: &[String]) -> Result<Self> {
    if paths.is_empty() || index.is_none() {
      return Ok(Self::default());
    }
    let ask = |revision: &str| ask_index(index, |lenient| lenient.modules(repository_path, paths, revision));
    let (head, base) = std::thread::scope(|scope| {
      let base = scope.spawn(in_current_span(|| ask(base)));
      let head = ask(head);
      (head, base.join().unwrap_or_else(|_| Err(anyhow::anyhow!("the query of the modules panicked"))))
    });
    Ok(match (head?, base?) {
      (Ok(head), Ok(base)) => {
        // a build file accio could not read whole declares what it read of it: the answer is still the modules
        for (side, answer) in [("head", &head), ("base", &base)].into_iter().filter(|(_, answer)| !answer.complete) {
          tracing::debug!(side, note = answer.note.as_deref().unwrap_or_default(), "the modules of a side are not complete");
        }
        Self::of(head, base)
      }
      (head, base) => {
        let why = head.err().or_else(|| base.err()).unwrap_or_default();
        tracing::debug!(why, "no modules: the index answered them of one side or of none");
        Self::default()
      }
    })
  }

  /// The roots of the two answers
  fn of(head: ModulesAnswer, base: ModulesAnswer) -> Self {
    let roots = |answer: &ModulesAnswer| -> HashMap<String, Option<String>> { answer.paths.iter().map(|path| (path.path.clone(), path.root.clone())).collect() };
    let (head_roots, base_roots) = (roots(&head), roots(&base));
    let mut declared: HashMap<String, DeclaredModule> = base.modules.into_iter().map(|module| (module.root.clone(), module)).collect();
    declared.extend(head.modules.into_iter().map(|module| (module.root.clone(), module)));
    Self {
      head: head_roots,
      base: base_roots,
      declared,
    }
  }

  /// The root of the module of `path`, a changed file, and whether it is of the base alone: the root at the head, else
  /// the root at the base where that one is deeper, which the head has no module at (a module the commits delete)
  pub(crate) fn root_of(&self, path: &str) -> (&str, bool) {
    let head = self.head.get(path).and_then(Option::as_deref);
    match self.base.get(path).and_then(Option::as_deref) {
      Some(base) if head.is_none_or(|head| head != base && is_under(base, head)) => (base, true),
      _ => (head.unwrap_or(""), false),
    }
  }

  /// `path` has `root` for the root of its module at the base
  fn of_base(&self, path: &str, root: &str) -> bool {
    self.base.get(path).and_then(Option::as_deref) == Some(root)
  }

  /// `path` has `root` for the root of its module at the head
  fn of_head(&self, path: &str, root: &str) -> bool {
    self.head.get(path).and_then(Option::as_deref) == Some(root)
  }

  /// The name the build files at `root` give its module: of its `*.iml`, its crate, its package, its Maven, Gradle,
  /// .NET, Go or Python project
  fn name_of(&self, root: &str) -> Option<&str> {
    let module = self.declared.get(root)?;
    [
      &module.jps,
      &module.cargo,
      &module.npm,
      &module.maven,
      &module.gradle,
      &module.msbuild,
      &module.go,
      &module.python,
    ]
    .into_iter()
    .flatten()
    .find_map(|block| block.name.as_deref())
  }
}

/// What `module_shapes` reads
pub(crate) struct ModulesInput<'a> {
  pub changed: &'a [ChangedFile<'a>],
  /// The roots of the modules of `changed`
  pub roots: &'a DeclaredRoots,
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
  let root_of = |path: &str| -> String { input.roots.root_of(path).0.to_string() };
  // the roots of the modules the commits delete: of the base alone
  let deleted_roots: HashSet<&str> = changed
    .iter()
    .map(|file| input.roots.root_of(file.path))
    .filter_map(|(root, deleted)| deleted.then_some(root))
    .collect();
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
      state: state_of(root, changed, input.roots, &deleted_roots),
      files: ModuleFiles::default(),
      additions: 0,
      deletions: 0,
      directories: Vec::new(),
      uses: Vec::new(),
      used_by: Vec::new(),
      content_of: Vec::new(),
      new_dependencies: Vec::new(),
      entries: None,
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
  }
  // the folders of each module below the packages its code and tests share (`com/example/player`), its resources apart
  let mut folder_of: Vec<String> = vec![String::new(); changed.len()];
  for (module, root) in distinct.iter().enumerate() {
    let files: Vec<(usize, &str)> = changed
      .iter()
      .enumerate()
      .filter(|(index, _)| place[roots[*index].as_str()] == module)
      .map(|(index, file)| (index, file.path))
      .collect();
    let paths: Vec<&str> = files.iter().map(|&(_, path)| path).collect();
    let shared = SharedFolders::of(&paths, root);
    for (index, path) in files {
      *directories[module].entry(shared.directory(path)).or_default() += 1;
      folder_of[index] = shared.below(path);
    }
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
    folder_of,
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
  if matches!(mechanical, Some(MechanicalKind::BuildModel | MechanicalKind::ModuleList)) || is_module_file(name) || dependencies.iter().any(|change| change.manifest == path) {
    Role::Build
  } else if is_prose(path) {
    Role::Prose
  } else if test_subject(path).is_some() {
    Role::Tests
  } else if language_of(path).is_some() {
    Role::Code
  } else {
    Role::Resources
  }
}

/// The role of a changed file by its path alone, as what a range leaves to read sums it (`reading`): `role_of` with a
/// file of the build model or a manifest of dependencies a build file, whatever its entries did
pub(crate) fn reading_role(path: &str) -> FileRole {
  let build = is_build_model(path) || is_manifest(path);
  match role_of(path, build.then_some(MechanicalKind::BuildModel), &[]) {
    Role::Code => FileRole::Code,
    Role::Tests => FileRole::Tests,
    Role::Resources => FileRole::Resources,
    Role::Build => FileRole::Build,
    Role::Prose => FileRole::Prose,
  }
}

/// A file of prose by its extension: Markdown, `reStructuredText`, `AsciiDoc`, text
pub(crate) fn is_prose(path: &str) -> bool {
  file_name(path)
    .rsplit_once('.')
    .is_some_and(|(_, extension)| PROSE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()))
}

/// The lines the range adds and removes in `file`, as `git diff --numstat` of the range counts them: git's counts of its
/// commits (`FileChange::git_additions`), which the engine's can differ from. A file that several commits change counts
/// the blocks of its pair (`pair::LineBlock::added`), the engine's count of the range: git's would take a process of
/// its own, and the sum of the counts of its commits is no count of the range
fn line_counts(file: &ChangedFile<'_>, pair: Option<&PairBlocks>) -> (u32, u32) {
  let (added, removed) = file
    .changes
    .iter()
    .fold((0u32, 0u32), |(added, removed), change| (added + change.git_additions, removed + change.git_deletions));
  match pair {
    Some(PairBlocks::Blocks(blocks)) if file.changes.len() > 1 => blocks.iter().fold((0, 0), |(added, removed), block| (added + block.added, removed + block.removed)),
    // a file the range adds has the lines its commits leave, one it deletes the lines it had: as the diff of the range
    // counts them, though a later commit changed what an earlier one added
    Some(PairBlocks::Added) => (added.saturating_sub(removed), 0),
    Some(PairBlocks::Deleted) => (0, removed.saturating_sub(added)),
    _ => (added, removed),
  }
}

/// The source directories of resources, whose folders are no packages of the code
const RESOURCE_DIRECTORIES: [&str; 2] = ["resources", "testResources"];

/// What a file of a module is to the folders its files share
#[derive(Clone, Copy, PartialEq, Eq)]
enum FolderKind {
  /// Code and tests, which share the packages
  Code,
  /// Under a source directory of resources (`resources/messages`, `src/main/resources`)
  Resources,
  /// Right in the root of the module, under no source directory (its `BUILD.bazel`, its `*.iml`): in no folder, so it
  /// takes no part in what the others share
  Root,
}

/// What `path` below `root` is to the folders of its module, and its folders after its leading source directories
fn source_and_folders<'a>(path: &'a str, root: &str) -> (FolderKind, Vec<&'a str>) {
  let below = if root.is_empty() {
    path
  } else {
    path.strip_prefix(root).and_then(|rest| rest.strip_prefix('/')).unwrap_or(path)
  };
  let directory = below.rsplit_once('/').map_or("", |(directory, _)| directory);
  let segments: Vec<&str> = directory.split('/').filter(|segment| !segment.is_empty()).collect();
  let source = segments.iter().take_while(|segment| SOURCE_DIRECTORIES.contains(segment)).count();
  let kind = if segments.is_empty() {
    FolderKind::Root
  } else if segments[..source].iter().any(|segment| RESOURCE_DIRECTORIES.contains(segment)) {
    FolderKind::Resources
  } else {
    FolderKind::Code
  };
  (kind, segments[source..].to_vec())
}

/// The folders the files of one module share, as the packages of a Java or Kotlin module do (`com/example/player` under
/// `src` and under `testSrc`), the last folder of the deepest kept: what tells them apart is below them. Its code and
/// its tests share them; its resources apart, as a bundle under `resources/messages` shares no package of the code; a
/// file right in its root (its build file) takes no part
pub(crate) struct SharedFolders<'a> {
  root: &'a str,
  /// Of the code and the tests, of the resources: every folder they share
  shared: [usize; 2],
  /// The same, but the last folder of the shallowest kept, so a module whose files share every folder still names one
  named: [usize; 2],
}

impl<'a> SharedFolders<'a> {
  pub(crate) fn of(paths: &[&str], root: &'a str) -> Self {
    let mut by_kind: [Vec<Vec<&str>>; 2] = [Vec::new(), Vec::new()];
    for path in paths {
      match source_and_folders(path, root) {
        (FolderKind::Code, folders) => by_kind[0].push(folders),
        (FolderKind::Resources, folders) => by_kind[1].push(folders),
        (FolderKind::Root, _) => {}
      }
    }
    let counts = by_kind.map(|folders| {
      let Some(first) = folders.first() else { return (0, 0) };
      let shared = (0..first.len()).take_while(|&at| folders.iter().all(|other| other.get(at) == first.get(at))).count();
      // files that share every folder keep their last one, so their one folder still has a name
      (shared, shared.min(folders.iter().map(Vec::len).min().unwrap_or(0).saturating_sub(1)))
    });
    Self {
      root,
      shared: counts.map(|(shared, _)| shared),
      named: counts.map(|(_, named)| named),
    }
  }

  /// The directory of `path` below every folder the files of its kind share, all of its levels: "" when that is its
  /// own directory (`FileFacts::folder`, after the name of a file in the tree)
  pub(crate) fn below(&self, path: &str) -> String {
    let (kind, folders) = source_and_folders(path, self.root);
    folders[self.shared[usize::from(kind == FolderKind::Resources)].min(folders.len())..].join("/")
  }

  /// The directory of `path` below the root, without its leading source directories and the folders the files of its
  /// kind share (the code and the tests, or the resources), at most two levels
  pub(crate) fn directory(&self, path: &str) -> String {
    let (kind, folders) = source_and_folders(path, self.root);
    folders
      .into_iter()
      .skip(self.named[usize::from(kind == FolderKind::Resources)])
      .take(2)
      .collect::<Vec<_>>()
      .join("/")
  }
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

/// The name of the module at `root`: the one its build files give it, the module a dependency change gives a build file
/// at its root, else its directory
fn name_of(root: &str, input: &ModulesInput<'_>) -> String {
  if root.is_empty() {
    return "(root)".to_string();
  }
  input
    .roots
    .name_of(root)
    .map(str::to_string)
    .or_else(|| {
      input
        .dependencies
        .iter()
        .find(|change| directory_of(&change.manifest) == root && is_module_file(file_name(&change.manifest)))
        .map(|change| change.module.clone())
    })
    .unwrap_or_else(|| file_name(root).to_string())
}

/// `Deleted` when `root` is of the base alone, `Added` when the head declares a module there and no file of it has that
/// root at the base
fn state_of(root: &str, changed: &[ChangedFile<'_>], roots: &DeclaredRoots, deleted_roots: &HashSet<&str>) -> ModuleState {
  let files = || changed.iter().filter(|file| roots.root_of(file.path).0 == root);
  if deleted_roots.contains(root) {
    ModuleState::Deleted
  } else if files().all(|file| roots.of_head(file.path, root) && !roots.of_base(file.path, root)) {
    ModuleState::Added
  } else {
    ModuleState::Changed
  }
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
