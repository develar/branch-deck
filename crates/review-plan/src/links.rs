//! The links between the files of a review (spec/branch/review-plan.spec.md, "Links"): which changed file uses which,
//! before and after the commits, from the syntax summaries the API already read (`SymbolInput`), no git.
//!
//! - **After**: the uses of the dependency order (`order::review_order`), each file as the last reviewed commit leaves
//!   it. **Before**: the same evidence (`trial_check::symbol_dependencies::file_links`) on each file as it was before
//!   the first commit that changes it, at its path then, named by its last path.
//! - **Per pair of files** (`from` uses `to`): `Added` when it links only after the commits, `Removed` only before,
//!   `Kept` both; the symbols and lines are those of the side it links on, after the commits for a kept link.
//! - **Across modules** when the nearest module root above the two files differs: an `*.iml`, a Bazel, Gradle or Maven
//!   build file, a `Cargo.toml` or a `package.json` (`ModuleRoots`), read at the checked out HEAD.
//! - A deleted file, a file the summaries don't read and a degraded one link nothing on that side.

use crate::files::ChangedFile;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use trial_check::module_roots::ModuleRoots;
use trial_check::symbol_dependencies::{SymbolLink, file_links};
use trial_check::symbol_scan::SymbolInput;

/// Whether the commits added, removed or kept a link
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum LinkState {
  /// Only after the commits
  Added,
  /// Only before the commits
  Removed,
  /// Before and after the commits
  Kept,
}

/// A name that links two files, and the line of its first use in the file that uses it
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct LinkedSymbol {
  pub name: String,
  pub line: u32,
}

/// A changed file that uses another changed file
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FileLink {
  /// The file that uses, at its last path
  pub from: String,
  /// The file that declares, at its last path
  pub to: String,
  /// By name: of the side it links on, after the commits when it links on both
  pub symbols: Vec<LinkedSymbol>,
  pub state: LinkState,
  /// The two files are in different modules
  pub cross_module: bool,
}

/// The names that link a pair of files before and after the commits
#[derive(Default)]
struct Sides {
  before: Vec<LinkedSymbol>,
  after: Vec<LinkedSymbol>,
}

/// The root of the module of `path`: the nearest directory above it with a build file
fn module_root<'a>(roots: &'a ModuleRoots, path: &str) -> Option<&'a str> {
  [roots.module_of(path), roots.crate_of(path), roots.package_of(path)]
    .into_iter()
    .flatten()
    .max_by_key(|root| root.len())
}

/// The links between `files` (`files::changed_files`): those of `after` (the uses of `order::review_order`) against
/// the uses of the files before the commits; sorted by the file that uses, then the file that declares
pub(crate) fn link_files(input: &SymbolInput, files: &[ChangedFile<'_>], after: &[SymbolLink]) -> Vec<FileLink> {
  let before_files: Vec<(&str, &str)> = files.iter().filter_map(|file| Some((file.first_path, file.before?))).collect();
  // the path before the commits -> the last path
  let last_path: HashMap<&str, &str> = files.iter().filter(|file| file.before.is_some()).map(|file| (file.first_path, file.path)).collect();
  let before = file_links(input, &before_files);

  // (from, to) -> the names that link them
  let mut pairs: BTreeMap<(&str, &str), Sides> = BTreeMap::new();
  let symbol = |link: &SymbolLink| LinkedSymbol {
    name: link.name.clone(),
    line: link.line,
  };
  for link in &before {
    let (Some(&from), Some(&to)) = (last_path.get(link.used_in.as_str()), last_path.get(link.declared_in.as_str())) else {
      continue;
    };
    if from != to {
      pairs.entry((from, to)).or_default().before.push(symbol(link));
    }
  }
  for link in after {
    pairs.entry((link.used_in.as_str(), link.declared_in.as_str())).or_default().after.push(symbol(link));
  }
  pairs
    .into_iter()
    .map(|((from, to), Sides { before, after })| {
      let (state, symbols) = match (before.is_empty(), after.is_empty()) {
        (true, _) => (LinkState::Added, after),
        (false, true) => (LinkState::Removed, before),
        (false, false) => (LinkState::Kept, after),
      };
      FileLink {
        from: from.to_string(),
        to: to.to_string(),
        symbols,
        state,
        cross_module: module_root(&input.roots, from) != module_root(&input.roots, to),
      }
    })
    .collect()
}
