//! The plain imports of Kotlin and Java that the range adds, an import of a name without an alias (decision 30 of
//! docs/design/symbol-dependencies.md). The summary binds no name for such an import (an alias is a `Binding`, decision
//! 19), so the index of the declarations can't hold it; this one follows the files of the range apart from it.
//!
//! - **The commit of the import.** Per file, each name that a plain import binds has the commit of the range that
//!   added that import to the file, or changed the path it imports the name from; an import of the baseline has none,
//!   nor has a file the commit adds, which a later change of it needs by its lines already. A commit that removes the
//!   import removes the name, a rename takes the names along, and a side the summary can't read (truncated, degraded,
//!   absent) drops what the file had: an unknown import is no edge.
//! - **A use that only the import gives.** A bare use of the name on an added line of a later commit of the same file
//!   needs that commit: a type, a value or a call, and the head of a qualified name of a type or a constant
//!   (`JBUI.scale`, `TimeUnit.SECONDS`). Not where the file gives the name another way: a declaration of the name, a
//!   wildcard import of the import's own package, an import of the file's own package or of a default one. A function
//!   or a property by a lower-case name only where the import decides it (`import_decides`): a lambda's receiver or a
//!   supertype may take the call first.
//! - **Wildcard imports** bind no name the summary knows, so a use through a wildcard import of another commit is no
//!   edge: a limit.

use super::{Evidence, FileCtx, UseSite, import_decides};
use crate::lost_bindings::is_default_package;
use code_symbols::language::Language;
use code_symbols::summary::{FileSummary, ImportTarget, Reference, Role, Shape};
use object_cache::commit_diff::FileChange;
use std::collections::{BTreeMap, HashMap};

/// Per file of the range, the names that a plain import of a commit binds, each with the position of that commit
#[derive(Default)]
pub(super) struct JvmImports {
  files: HashMap<String, HashMap<String, usize>>,
}

impl JvmImports {
  /// Follows the change of one file by the commit at `position`, the contexts of its sides: before the uses of that
  /// commit, so that an import the commit adds itself is its own
  pub(super) fn follow(&mut self, change: &FileChange, before: Option<&FileCtx<'_>>, after: Option<&FileCtx<'_>>, position: usize) {
    let mut owners = self.files.remove(change.old_path.as_deref().unwrap_or(&change.path)).unwrap_or_default();
    let Some(after) = after.filter(|ctx| is_jvm(ctx.language) && readable(ctx.summary)) else {
      return;
    };
    // a file the commit adds brings its imports along: a later change of it needs the commit by its lines already
    let Some(before) = before.filter(|before| before.language == after.language && readable(before.summary)) else {
      return;
    };
    let old = plain_imports(before.summary);
    let new = plain_imports(after.summary);
    for (name, path) in &new {
      if old.get(name) != Some(path) {
        owners.insert((*name).to_string(), position);
      }
    }
    owners.retain(|name, _| new.contains_key(name.as_str()));
    if !owners.is_empty() {
      self.files.insert(change.path.clone(), owners);
    }
  }

  /// What `reference`, on an added line of `use_site` of the commit at `position`, needs of the plain imports of its
  /// file: the commit that added the import of its name, the name and the file, and the evidence
  pub(super) fn resolve(&self, use_site: &UseSite<'_>, reference: &Reference, position: usize) -> Option<(usize, String, String, Evidence)> {
    let owners = self.files.get(use_site.path)?;
    let summary = use_site.summary;
    let name = match reference.qualifier.as_deref() {
      None if matches!(reference.role, Role::Value | Role::Call | Role::Type) => summary.name(reference.name),
      // `JBUI.scale`: the head of a qualified name of a type or a constant; a lower-case head may be a property
      Some([head, ..]) if matches!(reference.role, Role::Value | Role::Call | Role::Type) && is_capitalized(summary.name(*head)) => summary.name(*head),
      _ => return None,
    };
    let &owner = owners.get(name)?;
    if owner == position || summary.declares(name) {
      return None;
    }
    let path = plain_imports(summary).remove(name)?;
    let star_of_package = summary
      .imports
      .iter()
      .any(|import| import.target == ImportTarget::All && import.path.iter().map(|id| summary.name(*id)).eq(path.iter().copied()));
    if star_of_package {
      return None;
    }
    if reference.qualifier.is_none() && !is_capitalized(name) {
      let shape = reference.shape.and_then(|shape| summary.shapes.get(shape as usize));
      if !import_decides(summary, reference, matches!(shape, Some(Shape::Call { enclosed: true, .. }))) {
        return None;
      }
    }
    Some((owner, name.to_string(), use_site.path.to_string(), Evidence::Import))
  }
}

const fn is_jvm(language: Language) -> bool {
  matches!(language, Language::Kotlin | Language::Java)
}

/// A summary whose imports and uses are trusted: neither truncated nor degraded
fn readable(summary: &FileSummary) -> bool {
  !summary.truncated && !summary.is_degraded()
}

/// A type or a constant by convention: the first letter is a capital
fn is_capitalized(name: &str) -> bool {
  name.chars().next().is_some_and(char::is_uppercase)
}

/// The names the plain imports of `summary` bind, each with the path it imports the name from (`a.b` of `import a.b.C`),
/// the first import of a name: an import of a name without an alias, Java's `import static` included, of a package other
/// than the file's own and the default ones
fn plain_imports(summary: &FileSummary) -> BTreeMap<&str, Vec<&str>> {
  let own_package: Vec<&str> = summary.package.as_deref().map_or_else(Vec::new, |package| package.split('.').collect());
  let mut imports = BTreeMap::new();
  for import in summary.imports.iter().filter(|import| import.alias.is_none() && import.exported.is_none()) {
    let ImportTarget::Name(target) = import.target else { continue };
    let path: Vec<&str> = import.path.iter().map(|id| summary.name(*id)).collect();
    if path == own_package || is_default_package(summary.language, &path) {
      continue;
    }
    imports.entry(summary.name(target)).or_insert(path);
  }
  imports
}
