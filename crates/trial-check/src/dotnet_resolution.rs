//! How a C# use in another file means a declaration of the range (`Family::Dotnet`), by C#'s name lookup as far as
//! the summaries tell it: a namespace sees the types of itself and of every namespace around it (`namespace A.B`
//! sees `A.B.T`, `A.T` and the global `T`), a `using N;` the types of `N` (not its nested namespaces), a
//! `using static N.T;` the static members and nested types of `T`, an alias `using X = N.T;` the type `T` as `X`. A
//! qualified name is resolved from its head: the full name, a name relative to a namespace around the use (`B.T` in
//! `namespace A`), an alias, or a type the use sees. Namespaces span assemblies, so a type of a namespace the use
//! sees means it whatever its assembly; `internal` reaches its assembly only (`ModuleRoots::module_of`, a `*.csproj`).
//!
//! - **An extension method** (a static method of a static class whose first parameter is `this T`, or a member of an
//!   `extension(T t)` block) is called on a receiver of `T` where the use sees its class's namespace (or imports the
//!   class with `using static`): evidence only when the receiver's type is known and is `T` by its last name
//!   (`Reference::receiver_type`), as an instance member of the receiver wins over it.
//! - **An instance member** links by the JVM rules of a receiver of a known type or an inherited call (`inherited`,
//!   `through_receiver`), its type's path resolving by these rules (`type_path_resolves`).
//! - **Not read**: `global using` of another file, which the summary of the use doesn't hold; a `using` inside a block
//!   namespace counts for the whole file, and a file's namespace is its first one (`FileSummary::package`).

use super::{Evidence, Site, UseSite, bare, import_binding, inherited, nested_shadows, through_receiver};
use code_symbols::summary::{DeclKind, ImportTarget, MemberKind, Reach, Reference, Role};

/// The evidence that the C# use `reference` (its qualifier's names `qualifier`) in another file means `site`
pub(super) fn resolve_dotnet(use_site: &UseSite<'_>, reference: &Reference, qualifier: Option<&[String]>, site: &Site<'_>) -> Option<Evidence> {
  if !reaches_assembly(use_site, site, site.declaration.reach) {
    return None;
  }
  let declaration = site.declaration;
  let full = full_path(site);
  if let Some(qualifier) = qualifier.filter(|qualifier| !qualifier.is_empty() && !bare(Some(qualifier))) {
    // `N.T`, `T.Member`, `Outer.Inner`, `Ext.Method(x)`: the qualifier names what declares the site
    let owner = &full[..full.len() - 1];
    if qualifies(use_site, qualifier, owner) {
      return Some(Evidence::Qualified);
    }
  }
  if is_extension(site) {
    return extension(use_site, reference, site);
  }
  match declaration.member {
    MemberKind::TopLevel => top_level(use_site, reference, qualifier, site),
    MemberKind::Static => {
      // a bare name of a member that `using static N.T;` imports
      let containers = &full[..full.len() - 1];
      (qualifier.is_none() && star_imports(use_site, containers)).then_some(Evidence::StarImport)
    }
    MemberKind::Instance => {
      // C#'s `base.Member` is the JVM rules' `super.member`
      let as_super = [String::from("super")];
      let qualifier = match qualifier {
        Some([only]) if only == "base" => Some(&as_super[..]),
        _ => qualifier,
      };
      if inherited(use_site, reference, qualifier, site) {
        return Some(Evidence::Inherited);
      }
      through_receiver(use_site, reference, qualifier, site).then_some(Evidence::Receiver)
    }
  }
}

/// A type of a namespace by a bare name: through an alias of it (`using X = N.T;`), or where the use sees its namespace
/// and nothing of the file shadows its name (a declaration of it, an alias of it to another type)
fn top_level(use_site: &UseSite<'_>, reference: &Reference, qualifier: Option<&[String]>, site: &Site<'_>) -> Option<Evidence> {
  if qualifier.is_some() {
    return None;
  }
  let summary = use_site.summary;
  let used = summary.name(reference.name);
  if let Some((path, imported)) = import_binding(use_site, used) {
    return (path == site.ctx.namespace.as_slice() && imported == site.name()).then_some(Evidence::Import);
  }
  let is_type = matches!(site.declaration.kind, DeclKind::Type | DeclKind::TypeAlias) && matches!(reference.role, Role::Type | Role::Call);
  if !is_type || summary.declares(used) {
    return None;
  }
  sees_namespace(use_site, &site.ctx.namespace)
}

/// An extension method or a member of an extension block on a receiver of its type, where the use sees the namespace of
/// its class or imports the class (`using static`)
fn extension(use_site: &UseSite<'_>, reference: &Reference, site: &Site<'_>) -> Option<Evidence> {
  if !matches!(reference.role, Role::Call | Role::Value) || !sees_extension(use_site, site) {
    return None;
  }
  let summary = use_site.summary;
  let receiver = reference.receiver_type.as_deref().and_then(<[u32]>::last).map(|it| summary.name(*it))?;
  let declared = site.declaration.receiver.map(|it| site.ctx.summary.name(it))?;
  (receiver == declared).then_some(Evidence::Receiver)
}

/// Whether `site` is a C# extension: a function with a receiver (`this T` or an `extension(T)` block)
pub(super) const fn is_extension(site: &Site<'_>) -> bool {
  site.declaration.receiver.is_some() && matches!(site.declaration.kind, DeclKind::Function | DeclKind::Value)
}

/// Whether `site` is a C# extension (`is_extension` of a C# file)
pub(super) fn is_extension_site(site: &Site<'_>) -> bool {
  site.ctx.language.family() == code_symbols::language::Family::Dotnet && is_extension(site)
}

/// Whether the file of `use_site` sees the C# extension `site`: the namespace of its class, or its class by
/// `using static`
pub(super) fn sees_extension(use_site: &UseSite<'_>, site: &Site<'_>) -> bool {
  let full = full_path(site);
  is_extension(site) && (sees_namespace(use_site, &site.ctx.namespace).is_some() || star_imports(use_site, &full[..full.len() - 1]))
}

/// Whether the type path `path` (a supertype, a receiver's type) of a use names the type `site` is a member of: by
/// the rules of a qualifier (`qualifies`), a bare type name where the use sees its namespace, and no type of the name
/// nested in a declaration around the reference
pub(super) fn type_path_resolves(use_site: &UseSite<'_>, reference: &Reference, path: &[String], site: &Site<'_>) -> bool {
  let Some(head) = path.first() else { return false };
  if nested_shadows(use_site, reference, head) {
    return false;
  }
  let full = full_path(site);
  qualifies(use_site, path, &full[..full.len() - 1])
}

/// Whether `qualifier` names the namespace or the type `target` (its full path): the full name, a name relative to a
/// namespace around the use, an alias at its head (`using X = N.T;`, `X.Member`), or a type path whose head the use
/// sees by its namespace (`T.Member`, `Outer.Inner`) and the file doesn't declare
fn qualifies(use_site: &UseSite<'_>, qualifier: &[String], target: &[String]) -> bool {
  // `global::N.T` is the full name
  let qualifier = match qualifier.split_first() {
    Some((head, rest)) if head == "global" => rest,
    _ => qualifier,
  };
  if enclosing(&use_site.namespace).any(|around| around.iter().chain(qualifier).eq(target.iter())) {
    return true;
  }
  let Some((head, rest)) = qualifier.split_first() else { return false };
  if let Some((path, imported)) = import_binding(use_site, head) {
    return path
      .iter()
      .map(String::as_str)
      .chain([imported])
      .chain(rest.iter().map(String::as_str))
      .eq(target.iter().map(String::as_str));
  }
  // a type path of a namespace the use sees by `using`: `T.Member` with `using N;`
  if use_site.summary.declares(head) {
    return false;
  }
  target.len() > qualifier.len() && target.ends_with(qualifier) && star_imports(use_site, &target[..target.len() - qualifier.len()])
}

/// The evidence that the use sees the namespace `namespace`: the namespace of its file or one around it, the global
/// one included (`SamePackageType`), or a `using` of it (`StarImport`)
fn sees_namespace(use_site: &UseSite<'_>, namespace: &[String]) -> Option<Evidence> {
  if use_site.namespace.starts_with(namespace) {
    return Some(Evidence::SamePackageType);
  }
  star_imports(use_site, namespace).then_some(Evidence::StarImport)
}

/// Whether the file of `use_site` has `using path;` (`using static path;` too: the summary holds both as a wildcard)
fn star_imports(use_site: &UseSite<'_>, path: &[String]) -> bool {
  use_site
    .summary
    .imports
    .iter()
    .zip(&use_site.import_paths)
    .any(|(import, import_path)| import.target == ImportTarget::All && import_path == path)
}

/// The namespace of a file and every namespace around it, the innermost first, the global one (empty) last
fn enclosing(namespace: &[String]) -> impl Iterator<Item = &[String]> {
  (0..=namespace.len()).rev().map(move |length| &namespace[..length])
}

/// The full path of a site: its namespace, its containers and its name
fn full_path(site: &Site<'_>) -> Vec<String> {
  site.ctx.namespace.iter().chain(&site.containers).cloned().chain([site.name().to_string()]).collect()
}

/// Whether the declaration `site` of `reach` reaches the file of `use_site`: `internal` reaches its assembly (the
/// nearest `*.csproj` above it). A file of no project is of no assembly, but an older form (`Site::older`), whose
/// rejection makes an edge, reaches it, as the compiler may see one
fn reaches_assembly(use_site: &UseSite<'_>, site: &Site<'_>, reach: Reach) -> bool {
  reach != Reach::Module
    || match (use_site.module_root, site.ctx.module_root) {
      (Some(used), Some(declared)) => used == declared,
      _ => site.older,
    }
}
