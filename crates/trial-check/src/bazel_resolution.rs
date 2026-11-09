//! How a Bazel use in another file means a declaration of the range (`Family::Bazel`): a `load` names the file it
//! loads from by a label, and the name it binds is the declaration of that file.
//!
//! - **A label is a path from the root of the repository**: `//pkg:defs.bzl` is `pkg/defs.bzl`, `:defs.bzl` and
//!   `defs.bzl` are relative to the directory of the file that loads them. Where no file of the range has that exact
//!   path, a path that ends in it counts: a Bazel workspace in a subdirectory of the repository names its packages
//!   from its own root, which the diff does not know.
//! - **Another repository** (`@repo//pkg:defs.bzl`) is out of the range by definition: it is no file of this diff.
//! - **A target is no code**: only a `def` and a name a top-level `=` assigns are loaded, and `DeclKind::Target` is
//!   never a declaration a use can mean (openspec/specs/commit/symbol-dependencies/spec.md).

use super::{Evidence, Site, UseSite, binds};
use code_symbols::summary::{ImportTarget, Reference};

/// The evidence that the Bazel use `reference` in another file means `site`: a `load` of the file of `site` that
/// binds the name
pub(super) fn resolve_bazel(use_site: &UseSite<'_>, reference: &Reference, qualifier: Option<&[String]>, site: &Site<'_>) -> Option<Evidence> {
  // a loaded name is bare: `a.b` in a Bazel file is a member of a provider, never a loaded declaration
  if qualifier.is_some() {
    return None;
  }
  let summary = use_site.summary;
  let name = site.name();
  for import in &summary.imports {
    let ImportTarget::Name(imported) = import.target else { continue };
    if summary.name(imported) != name || !binds(import.alias, imported, reference) {
      continue;
    }
    if import.specifier.as_deref().is_some_and(|label| label_means(label, use_site.path, site.ctx.path)) {
      return Some(Evidence::Import);
    }
  }
  None
}

/// Whether `label`, written in the file at `from`, names the file at `path`
fn label_means(label: &str, from: &str, path: &str) -> bool {
  let Some(wanted) = label_path(label, from) else { return false };
  path == wanted || path.ends_with(&format!("/{wanted}"))
}

/// The path a label names, from the root of the repository; `None` for a label of another repository
fn label_path(label: &str, from: &str) -> Option<String> {
  if label.starts_with('@') {
    return None;
  }
  if let Some(absolute) = label.strip_prefix("//") {
    return Some(absolute.replacen(':', "/", 1));
  }
  let relative = label.strip_prefix(':').unwrap_or(label);
  let directory = from.rsplit_once('/').map_or("", |(directory, _)| directory);
  Some(if directory.is_empty() {
    relative.to_string()
  } else {
    format!("{directory}/{relative}")
  })
}

#[cfg(test)]
mod tests {
  use super::label_means;

  #[test]
  fn a_label_names_a_file_from_the_root_of_the_repository() {
    assert!(label_means("//defs:jvm.bzl", "pkg/BUILD", "defs/jvm.bzl"));
    assert!(label_means("//defs/nested:jvm.bzl", "pkg/BUILD", "defs/nested/jvm.bzl"));
    assert!(!label_means("//defs:jvm.bzl", "pkg/BUILD", "other/jvm.bzl"));
  }

  #[test]
  fn a_relative_label_names_a_file_of_the_package_that_loads_it() {
    assert!(label_means(":local.bzl", "pkg/BUILD", "pkg/local.bzl"));
    assert!(label_means("local.bzl", "pkg/BUILD", "pkg/local.bzl"));
    assert!(label_means(":local.bzl", "BUILD", "local.bzl"));
    assert!(!label_means(":local.bzl", "pkg/BUILD", "other/local.bzl"));
  }

  #[test]
  fn a_workspace_below_the_root_of_the_repository_matches_by_the_tail_of_the_path() {
    assert!(label_means("//defs:jvm.bzl", "community/pkg/BUILD", "community/defs/jvm.bzl"));
    // a tail match needs the whole label: `defs/jvm.bzl` is no `other_defs/jvm.bzl`
    assert!(!label_means("//defs:jvm.bzl", "community/pkg/BUILD", "community/other_defs/jvm.bzl"));
  }

  #[test]
  fn a_label_of_another_repository_names_no_file_of_this_one() {
    assert!(!label_means("@rules_java//java:defs.bzl", "pkg/BUILD", "java/defs.bzl"));
  }
}
