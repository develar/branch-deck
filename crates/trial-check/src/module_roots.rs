//! Where the modules of source files begin: the nearest directory above a file that holds a build file. Kotlin's and
//! C#'s `internal` (a .NET assembly, a `*.csproj`) and a Rust crate end there, and a TypeScript alias of `tsconfig.json` starts there. Read from the
//! tree of the revision the caller names: HEAD for the commits ahead of the baseline (it has their newest structure), the
//! head of a reviewed range for the summaries of its review plan (a pull request is not checked out), with one
//! `cat-file --batch` of every directory above the files: the paths go on stdin, so the length of a command line never
//! limits them. The modules a review plan shows are accio's (`review_plan::modules::DeclaredRoots`). The files of those
//! directories are known by name too (`file_at`): a name the `extends` of a `tsconfig.json` tries that none of them has
//! costs no read, and one they have is read by its blob.

use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::object_texts::{TreeEntry, read_trees};
use std::collections::{BTreeSet, HashMap, HashSet};
use tracing::instrument;

/// The build files that make a directory the root of a JVM or Bazel module
const MODULE_FILES: [&str; 6] = ["BUILD.bazel", "BUILD", "build.gradle", "build.gradle.kts", "pom.xml", "MODULE.bazel"];

/// What one directory holds of the build files
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DirFacts {
  /// An `*.iml`, a `*.csproj` or a file of `MODULE_FILES`
  module: bool,
  /// The blob of its `Cargo.toml`
  cargo: Option<String>,
  /// `package.json` is there
  package: bool,
  /// Its `tsconfig*.json` files: (name, blob)
  tsconfigs: Vec<(String, String)>,
  /// Every regular file: (name, blob), by name
  files: Vec<(String, String)>,
}

/// What a revision holds at a path, as the directories `ModuleRoots::read` read know it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAt<'a> {
  /// A regular file, by its blob
  Blob(&'a str),
  /// No regular file: its directory has none of that name (a symlink, a submodule or a directory neither), or the
  /// revision lacks the directory
  Absent,
  /// Its directory is none of those read
  Unknown,
}

/// The directories above some files and their build files, at one revision
#[derive(Debug, Clone, Default)]
pub struct ModuleRoots {
  /// Directory (`""` for the root, else without a trailing `/`) -> its facts; a directory the revision lacks is absent
  dirs: HashMap<String, DirFacts>,
  /// Every directory above the paths `read` was given, those the revision lacks too (a module a commit deletes)
  looked_up: HashSet<String>,
}

/// The directories above `path`, the nearest first, the root (`""`) last
fn ancestors(path: &str) -> impl Iterator<Item = &str> {
  path.match_indices('/').rev().map(move |(slash, _)| &path[..slash]).chain(std::iter::once(""))
}

impl ModuleRoots {
  /// The directories above `paths` at `rev`, with one `cat-file --batch`
  #[instrument(skip_all, fields(rev))]
  pub fn read<'a>(git: &GitCommandExecutor, repository_path: &str, rev: &str, paths: impl IntoIterator<Item = &'a str>) -> Result<Self> {
    let dirs: Vec<String> = paths.into_iter().flat_map(ancestors).collect::<BTreeSet<&str>>().into_iter().map(str::to_string).collect();
    let names: Vec<String> = dirs.iter().map(|dir| format!("{rev}:{dir}")).collect();
    let trees = read_trees(git, repository_path, &names)?;
    Ok(Self {
      looked_up: dirs.iter().cloned().collect(),
      dirs: dirs.into_iter().zip(trees).filter_map(|(dir, entries)| Some((dir, facts_of(entries?)))).collect(),
    })
  }

  /// The directories and their entries as given: for a test
  #[cfg(test)]
  pub(crate) fn from_entries(entries: &[(&str, &[TreeEntry])]) -> Self {
    Self {
      dirs: entries.iter().map(|&(dir, entries)| (dir.to_string(), facts_of(entries.to_vec()))).collect(),
      looked_up: entries.iter().map(|&(dir, _)| dir.to_string()).collect(),
    }
  }

  /// The nearest directory above `path` for which `has` holds
  fn nearest(&self, path: &str, has: impl Fn(&DirFacts) -> bool) -> Option<&str> {
    ancestors(path).find_map(|dir| self.dirs.get_key_value(dir).filter(|(_, facts)| has(facts)).map(|(dir, _)| dir.as_str()))
  }

  /// The root of the module of `path`: the nearest directory above it with an `*.iml`, a `*.csproj`, a Bazel, Gradle or
  /// Maven build file; `None` when no directory has one
  pub fn module_of(&self, path: &str) -> Option<&str> {
    self.nearest(path, |facts| facts.module)
  }

  /// The root of the Rust crate of `path`: the nearest directory with a `Cargo.toml`
  pub fn crate_of(&self, path: &str) -> Option<&str> {
    self.nearest(path, |facts| facts.cargo.is_some())
  }

  /// `crate_of` of a path `read` was given: `None` for a path in no crate, and for a path of a directory not looked up (a
  /// path of the baseline after the base), whose crate is not known. A path of a directory the revision lacks (a module
  /// a commit deletes) is in the crate of its nearest directory the revision has
  pub fn read_crate_of(&self, path: &str) -> Option<&str> {
    let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
    self.looked_up.contains(dir).then(|| self.crate_of(path)).flatten()
  }

  /// The root of the TypeScript or JavaScript package of `path`: the nearest directory with a `package.json`
  pub fn package_of(&self, path: &str) -> Option<&str> {
    self.nearest(path, |facts| facts.package)
  }

  /// The `tsconfig*.json` files of the directory nearest above `path` that has any: (path, blob)
  pub fn tsconfigs_of(&self, path: &str) -> Vec<(String, &str)> {
    let Some(dir) = self.nearest(path, |facts| !facts.tsconfigs.is_empty()) else {
      return Vec::new();
    };
    self.dirs[dir].tsconfigs.iter().map(|(name, blob)| (join(dir, name), blob.as_str())).collect()
  }

  /// The crate directories and the blobs of their `Cargo.toml`
  pub fn manifests(&self) -> impl Iterator<Item = (&str, &str)> {
    self.dirs.iter().filter_map(|(dir, facts)| Some((dir.as_str(), facts.cargo.as_deref()?)))
  }

  /// What the revision holds at `path`, a path from the root
  pub fn file_at(&self, path: &str) -> FileAt<'_> {
    let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
    if !self.looked_up.contains(dir) {
      return FileAt::Unknown;
    }
    let Some(facts) = self.dirs.get(dir) else { return FileAt::Absent };
    facts
      .files
      .binary_search_by(|(file, _)| file.as_str().cmp(name))
      .map_or(FileAt::Absent, |index| FileAt::Blob(&facts.files[index].1))
  }

  /// Every `tsconfig*.json`: (path, blob)
  pub fn all_tsconfigs(&self) -> impl Iterator<Item = (String, &str)> {
    self
      .dirs
      .iter()
      .flat_map(|(dir, facts)| facts.tsconfigs.iter().map(move |(name, blob)| (join(dir, name), blob.as_str())))
  }
}

/// `dir/name`, or `name` for the root
fn join(dir: &str, name: &str) -> String {
  if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") }
}

/// The facts of a directory from its entries, whose names and ids it keeps: moved, not copied, since the directories
/// above the files of a scan can hold thousands
fn facts_of(entries: Vec<TreeEntry>) -> DirFacts {
  let mut facts = DirFacts::default();
  for entry in entries.into_iter().filter(|entry| entry.mode.starts_with("100")) {
    let name = entry.name.as_str();
    let extension = name.rsplit_once('.').map(|(_, extension)| extension);
    if extension.is_some_and(|extension| extension.eq_ignore_ascii_case("iml") || extension.eq_ignore_ascii_case("csproj")) || MODULE_FILES.contains(&name) {
      facts.module = true;
    }
    if name == "Cargo.toml" {
      facts.cargo = Some(entry.id.clone());
    }
    if name == "package.json" {
      facts.package = true;
    }
    if name.starts_with("tsconfig") && extension.is_some_and(|extension| extension.eq_ignore_ascii_case("json")) {
      facts.tsconfigs.push((name.to_string(), entry.id.clone()));
    }
    facts.files.push((entry.name, entry.id));
  }
  // git sorts the entries of a tree as if a directory's name ended with `/`, so the files alone come by their bytes
  // already, and the sort only confirms it in one pass; a tree git did not write may come otherwise
  facts.files.sort_unstable();
  facts
}
