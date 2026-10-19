//! Where the modules of source files begin: the nearest directory above a file that holds a build file. Kotlin's
//! `internal` and a Rust crate end there, and a TypeScript alias of `tsconfig.json` starts there. Read from HEAD's tree
//! (it has the newest structure of the commits ahead of the baseline) with one `cat-file --batch` of every directory
//! above the files: the paths go on stdin, so the length of a command line never limits them.

use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::object_texts::{TreeEntry, read_trees};
use std::collections::{BTreeSet, HashMap};
use tracing::instrument;

/// The build files that make a directory the root of a JVM or Bazel module
const MODULE_FILES: [&str; 6] = ["BUILD.bazel", "BUILD", "build.gradle", "build.gradle.kts", "pom.xml", "MODULE.bazel"];

/// What one directory holds of the build files
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DirFacts {
  /// An `*.iml` or a file of `MODULE_FILES`
  module: bool,
  /// The blob of its `Cargo.toml`
  cargo: Option<String>,
  /// `package.json` is there
  package: bool,
  /// Its `tsconfig*.json` files: (name, blob)
  tsconfigs: Vec<(String, String)>,
}

/// The directories above some files and their build files, at HEAD
#[derive(Debug, Clone, Default)]
pub struct ModuleRoots {
  /// Directory (`""` for the root, else without a trailing `/`) -> its facts; a directory HEAD lacks is absent
  dirs: HashMap<String, DirFacts>,
}

/// The directories above `path`, the nearest first, the root (`""`) last
fn ancestors(path: &str) -> impl Iterator<Item = &str> {
  path.match_indices('/').rev().map(move |(slash, _)| &path[..slash]).chain(std::iter::once(""))
}

impl ModuleRoots {
  /// The directories above `paths` at HEAD, with one `cat-file --batch`
  #[instrument(skip_all)]
  pub fn read<'a>(git: &GitCommandExecutor, repository_path: &str, paths: impl IntoIterator<Item = &'a str>) -> Result<Self> {
    let dirs: Vec<String> = paths.into_iter().flat_map(ancestors).collect::<BTreeSet<&str>>().into_iter().map(str::to_string).collect();
    let names: Vec<String> = dirs.iter().map(|dir| format!("HEAD:{dir}")).collect();
    let trees = read_trees(git, repository_path, &names)?;
    Ok(Self {
      dirs: dirs.into_iter().zip(trees).filter_map(|(dir, entries)| Some((dir, facts_of(&entries?)))).collect(),
    })
  }

  /// The directories and their entries as given: for a test
  #[cfg(test)]
  pub(crate) fn from_entries(entries: &[(&str, &[TreeEntry])]) -> Self {
    Self {
      dirs: entries.iter().map(|&(dir, entries)| (dir.to_string(), facts_of(entries))).collect(),
    }
  }

  /// The nearest directory above `path` for which `has` holds
  fn nearest(&self, path: &str, has: impl Fn(&DirFacts) -> bool) -> Option<&str> {
    ancestors(path).find_map(|dir| self.dirs.get_key_value(dir).filter(|(_, facts)| has(facts)).map(|(dir, _)| dir.as_str()))
  }

  /// The root of the module of `path`: the nearest directory above it with an `*.iml`, a Bazel, Gradle or Maven build
  /// file; `None` when no directory has one
  pub fn module_of(&self, path: &str) -> Option<&str> {
    self.nearest(path, |facts| facts.module)
  }

  /// The root of the Rust crate of `path`: the nearest directory with a `Cargo.toml`
  pub fn crate_of(&self, path: &str) -> Option<&str> {
    self.nearest(path, |facts| facts.cargo.is_some())
  }

  /// `crate_of` of a path whose directory `read` was given: `None` for a path in no crate, and for a path of a
  /// directory not read (a path of the baseline after the base), whose crate is not known
  pub fn read_crate_of(&self, path: &str) -> Option<&str> {
    let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
    self.dirs.contains_key(dir).then(|| self.crate_of(path)).flatten()
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

fn facts_of(entries: &[TreeEntry]) -> DirFacts {
  let mut facts = DirFacts::default();
  for entry in entries.iter().filter(|entry| entry.mode.starts_with("100")) {
    let name = entry.name.as_str();
    let extension = name.rsplit_once('.').map(|(_, extension)| extension);
    if extension.is_some_and(|extension| extension.eq_ignore_ascii_case("iml")) || MODULE_FILES.contains(&name) {
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
  }
  facts
}
