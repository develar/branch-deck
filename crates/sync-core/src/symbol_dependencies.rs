//! Which commits need which by the names they declare and use, from the syntax summaries of the files they change
//! (`symbol_scan`). The edges are of the class `Order` (docs/design/symbol-dependencies.md): a commit that uses a name
//! another commit adds can't be built without it, but the two may still be in two branches, one built after the other;
//! git's three-way merge can't refute such an edge, so it never joins two branches the way a line dependency does.
//!
//! - **The index is the range.** Only the declarations the commits ahead of the baseline add are looked up, so a
//!   repository of a million files costs only the blobs the commits change. A use of a name the range doesn't add is
//!   no edge.
//! - **Evidence, not names.** A use links to a declaration only with evidence that it means that one: an import of
//!   it, a qualified name, the same file, the same Rust module, or a type of the same package in the same module. A
//!   call of a function of the same package without an import, or a member of an unknown receiver, is no evidence:
//!   Kotlin's implicit receivers and extensions decide those, which only a compiler sees.
//! - **Degraded files decide nothing**: a file with a twentieth of its bytes in parse errors makes no edge.

use crate::diff_scan::CommitDiff;
use crate::module_roots::ModuleRoots;
use crate::symbol_scan::{SymbolInput, TsAliases, join, normalize};
use code_symbols::language::{Family, Language};
use code_symbols::summary::{DeclKind, Declaration, FileSummary, ImportTarget, MemberKind, Reach, Reference, Role};
use object_cache::commit_diff::FileChange;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Why a use means a declaration another commit adds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Evidence {
  /// An import or `use` of it, an alias included
  Import,
  /// A wildcard import of its package or module, for a type
  StarImport,
  /// Its qualified name, or `Type.member` with a resolved `Type`
  Qualified,
  /// The same file
  SameFile,
  /// The same Rust module
  SameModule,
  /// A type of the same JVM package in the same module
  SamePackageType,
  /// The later commit removes a declaration whose uses the earlier one removed
  RemovesAfter,
}

impl Evidence {
  pub const fn name(self) -> &'static str {
    match self {
      Self::Import => "import",
      Self::StarImport => "star_import",
      Self::Qualified => "qualified",
      Self::SameFile => "same_file",
      Self::SameModule => "same_module",
      Self::SamePackageType => "same_package_type",
      Self::RemovesAfter => "removes_after",
    }
  }
}

/// One name that makes a symbol edge
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolLink {
  pub name: String,
  /// The file of the declaration
  pub declared_in: String,
  /// The file of the use, and its line there
  pub used_in: String,
  pub line: u32,
  pub evidence: Evidence,
}

/// `from` needs `to`, an older commit, by the names of `links`; both are positions in the input
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolEdge {
  pub from: usize,
  pub to: usize,
  pub links: Vec<SymbolLink>,
}

/// The dependencies by symbols of a range of commits
#[derive(Debug, Clone, Default)]
pub struct SymbolGraph {
  /// Ordered by `from`, then `to`
  pub edges: Vec<SymbolEdge>,
  /// Per commit, the declarations it changes inside (functions and types, by qualified name): the entities two
  /// commits share
  pub entities: Vec<BTreeSet<String>>,
}

/// A file of a commit, before or after it, with what resolution needs to know of it, computed once
struct FileCtx<'a> {
  path: &'a str,
  language: Language,
  summary: &'a FileSummary,
  /// The JVM package, the Rust module path, or the TypeScript file path
  namespace: Vec<String>,
  module_root: Option<&'a str>,
  crate_root: Option<&'a str>,
  aliases: Option<&'a TsAliases>,
  /// The path of each import of `summary.imports`, as names
  import_paths: Vec<Vec<String>>,
}

/// A use is resolved in the context of its file
type UseSite<'a> = FileCtx<'a>;

/// A declaration of the range, where the lookup finds it
#[derive(Clone)]
struct Site<'a> {
  commit: usize,
  ctx: &'a FileCtx<'a>,
  declaration: &'a Declaration,
  /// The names of the declarations it is a member of, the outermost first; for a Rust `impl` member its receiver
  containers: Vec<String>,
  /// Namespace, containers and name: what identifies it across the commits
  key: String,
}

impl Site<'_> {
  fn name(&self) -> &str {
    self.ctx.summary.name(self.declaration.name)
  }
}

/// The names of the declarations that `declaration` is a member of, the outermost first
fn containers_of(summary: &FileSummary, declaration: &Declaration) -> Vec<String> {
  let mut names = Vec::new();
  let mut current = declaration.container;
  while let Some(index) = current {
    let Some(container) = summary.declarations.get(index as usize) else { break };
    names.push(summary.name(container.name).to_string());
    current = container.container;
  }
  if names.is_empty()
    && let Some(receiver) = declaration.receiver.filter(|_| summary.language == Language::Rust)
  {
    names.push(summary.name(receiver).to_string());
  }
  names.reverse();
  names
}

/// The Rust module path of a file of the crate at `crate_root`: `src/lib.rs` and `src/main.rs` are the crate, `src/a.rs`
/// and `src/a/mod.rs` are `a`; a file outside `src` (a test or a benchmark target) is a crate of its own
fn rust_module(path: &str, crate_root: Option<&str>) -> Vec<String> {
  let relative = crate_root.map_or(path, |root| {
    if root.is_empty() {
      path
    } else {
      path.strip_prefix(root).map_or(path, |rest| rest.trim_start_matches('/'))
    }
  });
  let Some(in_src) = relative.strip_prefix("src/") else {
    return vec![format!("#{}", relative.trim_end_matches(".rs"))];
  };
  let mut segments: Vec<String> = in_src.trim_end_matches(".rs").split('/').map(str::to_string).collect();
  if segments
    .last()
    .is_some_and(|last| last == "mod" || (segments.len() == 1 && (last == "lib" || last == "main")))
  {
    segments.pop();
  }
  segments
}

impl SymbolInput {
  fn file_ctx<'a>(&'a self, path: &'a str, summary: &'a FileSummary) -> FileCtx<'a> {
    let crate_root = self.roots.crate_of(path);
    FileCtx {
      path,
      language: summary.language,
      summary,
      namespace: match summary.language.family() {
        Family::Jvm => summary.package.as_deref().map_or_else(Vec::new, |package| package.split('.').map(str::to_string).collect()),
        Family::Rust => rust_module(path, crate_root),
        Family::Script => vec![path.to_string()],
      },
      module_root: self.roots.module_of(path),
      crate_root,
      aliases: aliases_of(&self.roots, &self.aliases, path),
      import_paths: summary.imports.iter().map(|import| names(summary, &import.path)).collect(),
    }
  }
}

/// The aliases of the nearest `tsconfig*.json` above `path` that has any
fn aliases_of<'a>(roots: &ModuleRoots, aliases: &'a HashMap<String, TsAliases>, path: &str) -> Option<&'a TsAliases> {
  roots
    .tsconfigs_of(path)
    .iter()
    .find_map(|(config, _)| aliases.get(config.rsplit_once('/').map_or("", |(dir, _)| dir)))
}

/// Whether a declaration can be used from other files at all: a binding of an import only where it is re-exported
fn is_exported(declaration: &Declaration, summary: &FileSummary) -> bool {
  match declaration.kind {
    DeclKind::Binding => summary.imports.iter().any(|import| import.exported && import.line == declaration.name_line),
    _ => true,
  }
}

/// Line ranges `[start, end)`
type LineRanges = Vec<(u32, u32)>;

/// The line ranges of the new side of `change`'s hunks, and of the old side
fn changed_lines(change: &FileChange) -> (LineRanges, LineRanges) {
  let added = change
    .hunks
    .iter()
    .filter(|hunk| hunk.new_lines > 0)
    .map(|hunk| (hunk.new_start, hunk.new_start + hunk.new_lines))
    .collect();
  let removed = change
    .hunks
    .iter()
    .filter(|hunk| hunk.old_lines > 0)
    .map(|hunk| (hunk.old_start, hunk.old_start + hunk.old_lines))
    .collect();
  (added, removed)
}

fn in_ranges(ranges: &[(u32, u32)], line: u32) -> bool {
  ranges.iter().any(|&(start, end)| (start..end).contains(&line))
}

/// A changed source file of a commit and the contexts of its two sides
type ChangedFile<'a> = (&'a FileChange, Option<FileCtx<'a>>, Option<FileCtx<'a>>);

/// The edges by symbols between `commits` (oldest first) and the entities each changes
pub fn compute_symbol_dependencies(commits: &[CommitDiff], input: &SymbolInput) -> SymbolGraph {
  let summary_of = |blob: Option<&String>| blob.and_then(|blob| input.summaries.get(blob)).map(AsRef::as_ref);
  // per commit its changed source files, each side's context computed once
  let files: Vec<Vec<ChangedFile<'_>>> = commits
    .iter()
    .map(|commit| {
      commit
        .files
        .iter()
        .filter_map(|change| {
          let old_path = change.old_path.as_deref().unwrap_or(&change.path);
          let before = summary_of(change.old_blob.as_ref()).map(|summary| input.file_ctx(old_path, summary));
          let after = summary_of(change.new_blob.as_ref()).map(|summary| input.file_ctx(&change.path, summary));
          (before.is_some() || after.is_some()).then_some((change, before, after))
        })
        .collect()
    })
    .collect();
  // name -> the sites the range added so far, by key
  let mut index: HashMap<String, BTreeMap<String, Site<'_>>> = HashMap::new();
  // (from, to) -> links
  let mut edges: BTreeMap<(usize, usize), Vec<SymbolLink>> = BTreeMap::new();
  let mut entities: Vec<BTreeSet<String>> = vec![BTreeSet::new(); commits.len()];

  for (position, changed) in files.iter().enumerate() {
    // the declarations of the files before and after, by key
    let mut added: Vec<Site<'_>> = Vec::new();
    let mut removed: Vec<Site<'_>> = Vec::new();
    for (change, before, after) in changed {
      let (added_lines, removed_lines) = changed_lines(change);
      match (before, after) {
        // the same namespace on both sides: a declaration added or removed lies on the changed lines of its side, so
        // only those are compared, by the declarations of the same name on the other side
        (Some(before), Some(after)) if before.namespace == after.namespace => {
          added.extend(changed_sites(after, &added_lines, before, position));
          removed.extend(changed_sites(before, &removed_lines, after, position));
        }
        // a new file, a deleted one, or a moved namespace (a rename, a new package): every key may differ
        _ => {
          let before_sites = before.as_ref().map_or_else(BTreeMap::new, |ctx| declared_sites(ctx, position));
          let after_sites = after.as_ref().map_or_else(BTreeMap::new, |ctx| declared_sites(ctx, position));
          added.extend(after_sites.iter().filter(|(key, _)| !before_sites.contains_key(*key)).map(|(_, site)| site.clone()));
          removed.extend(before_sites.iter().filter(|(key, _)| !after_sites.contains_key(*key)).map(|(_, site)| site.clone()));
        }
      }

      // the functions and types around the changed lines, on the side that has them
      for (ctx, ranges) in [(after, &added_lines), (before, &removed_lines)] {
        let Some(ctx) = ctx else { continue };
        for declaration in ctx
          .summary
          .declarations
          .iter()
          .filter(|declaration| matches!(declaration.kind, DeclKind::Function | DeclKind::Type))
        {
          if ranges.iter().any(|&(start, end)| declaration.start_line < end && start <= declaration.end_line) {
            entities[position].insert(site_of(ctx, position, declaration).key);
          }
        }
      }
    }
    // a declaration that moved to another file of the same commit is neither added nor removed
    let added_keys: BTreeSet<String> = added.iter().map(|site| site.key.clone()).collect();
    let removed_keys: BTreeSet<String> = removed.iter().map(|site| site.key.clone()).collect();
    added.retain(|site| !removed_keys.contains(&site.key));
    removed.retain(|site| !added_keys.contains(&site.key));

    // its own declarations first, so that its uses of them resolve to it
    for site in added
      .iter()
      .filter(|site| !site.ctx.summary.is_degraded() && !site.declaration.in_error && is_exported(site.declaration, site.ctx.summary))
    {
      index.entry(site.name().to_string()).or_default().insert(site.key.clone(), site.clone());
    }

    // the uses on the added lines
    for (change, _, after) in changed {
      let Some(use_site) = after.as_ref().filter(|ctx| !ctx.summary.is_degraded()) else {
        continue;
      };
      let (added_lines, _) = changed_lines(change);
      for reference in use_site.summary.references.iter().filter(|reference| in_ranges(&added_lines, reference.line)) {
        let Some(candidates) = index.get(use_site.summary.name(reference.name)) else { continue };
        // the names of the qualifier once, not once per candidate
        let qualifier = reference.qualifier.as_deref().map(|qualifier| names(use_site.summary, qualifier));
        let resolved: Vec<(&Site<'_>, Evidence)> = candidates
          .values()
          .filter(|site| site.commit != position)
          .filter_map(|site| resolve(input, use_site, reference, qualifier.as_deref(), site).map(|evidence| (site, evidence)))
          .collect();
        let owners: BTreeSet<usize> = resolved.iter().map(|(site, _)| site.commit).collect();
        // every declaration it may mean is of one older commit, else it is ambiguous
        let (1, Some(&(site, evidence))) = (owners.len(), resolved.first()) else { continue };
        edges.entry((position, site.commit)).or_default().push(SymbolLink {
          name: site.name().to_string(),
          declared_in: site.ctx.path.to_string(),
          used_in: change.path.clone(),
          line: reference.line,
          evidence,
        });
      }
    }

    // a removal needs the older commits that removed the uses of the name
    for site in removed.iter().filter(|site| !site.ctx.summary.is_degraded()) {
      for (earlier, earlier_files) in files.iter().enumerate().take(position) {
        let Some(link) = removed_uses(input, earlier_files, site) else { continue };
        edges.entry((position, earlier)).or_default().push(link);
      }
      if let Some(sites) = index.get_mut(site.name()) {
        sites.remove(&site.key);
      }
    }
  }

  SymbolGraph {
    edges: edges
      .into_iter()
      .map(|((from, to), mut links)| {
        // one link per name and pair of files: the first use, an import before the uses below it
        links.sort_by(|a, b| (&a.name, &a.declared_in, &a.used_in, a.line).cmp(&(&b.name, &b.declared_in, &b.used_in, b.line)));
        links.dedup_by(|later, first| (&later.name, &later.declared_in, &later.used_in) == (&first.name, &first.declared_in, &first.used_in));
        SymbolEdge { from, to, links }
      })
      .collect(),
    entities,
  }
}

/// The site of `declaration`, of the file of `ctx` in commit `commit`
fn site_of<'a>(ctx: &'a FileCtx<'a>, commit: usize, declaration: &'a Declaration) -> Site<'a> {
  let containers = containers_of(ctx.summary, declaration);
  let mut key = ctx.namespace.join(".");
  for segment in containers.iter().map(String::as_str).chain([ctx.summary.name(declaration.name)]) {
    key.push('.');
    key.push_str(segment);
  }
  Site {
    commit,
    ctx,
    declaration,
    containers,
    key,
  }
}

/// The declarations of `side` on its `ranges` (the changed lines of that side) that `other` (the other side of the
/// same file, in the same namespace) lacks, by key
fn changed_sites<'a>(side: &'a FileCtx<'a>, ranges: &[(u32, u32)], other: &'a FileCtx<'a>, commit: usize) -> Vec<Site<'a>> {
  let declarations = &side.summary.declarations;
  let on_changed_lines = |start_line: u32, end_line: u32| ranges.iter().any(|&(start, end)| start_line < end && start <= end_line);
  // a container whose name changed gives its members new keys, though their lines stay
  let renamed_container = |declaration: &Declaration| {
    let mut current = declaration.container;
    while let Some(container) = current.and_then(|index| declarations.get(index as usize)) {
      if on_changed_lines(container.name_line, container.name_line) {
        return true;
      }
      current = container.container;
    }
    false
  };
  declarations
    .iter()
    .filter(|declaration| side.summary.name(declaration.name).len() >= MIN_NAME_LENGTH)
    .filter(|declaration| on_changed_lines(declaration.start_line, declaration.end_line) || renamed_container(declaration))
    .map(|declaration| site_of(side, commit, declaration))
    .filter(|site| {
      let Some(name) = other.summary.name_id(site.name()) else { return true };
      !other
        .summary
        .declarations
        .iter()
        .filter(|declaration| declaration.name == name)
        .any(|declaration| site_of(other, commit, declaration).key == site.key)
    })
    .collect()
}

/// The declarations of a file other files can name, by key
fn declared_sites<'a>(ctx: &'a FileCtx<'a>, commit: usize) -> BTreeMap<String, Site<'a>> {
  ctx
    .summary
    .declarations
    .iter()
    .filter(|declaration| ctx.summary.name(declaration.name).len() >= MIN_NAME_LENGTH)
    .map(|declaration| site_of(ctx, commit, declaration))
    .map(|site| (site.key.clone(), site))
    .collect()
}

/// Names shorter than this are everywhere
const MIN_NAME_LENGTH: usize = 3;

/// The link of a use that the commit of `files` removed of the declaration `site`: a file whose before side has more
/// uses that resolve to it than its after side
fn removed_uses(input: &SymbolInput, files: &[ChangedFile<'_>], site: &Site<'_>) -> Option<SymbolLink> {
  let name = site.name();
  files.iter().find_map(|(_, before, after)| {
    let before = before.as_ref().filter(|ctx| ctx.summary.name_id(name).is_some())?;
    let count = |ctx: &FileCtx<'_>| {
      if ctx.summary.is_degraded() {
        return (0, None);
      }
      let uses: Vec<&Reference> = ctx
        .summary
        .references_to(name)
        .filter(|reference| {
          let qualifier = reference.qualifier.as_deref().map(|qualifier| names(ctx.summary, qualifier));
          resolve(input, ctx, reference, qualifier.as_deref(), site).is_some()
        })
        .collect();
      (uses.len(), uses.first().map(|reference| reference.line))
    };
    let (removed_count, line) = count(before);
    let kept = after.as_ref().filter(|ctx| ctx.summary.name_id(name).is_some()).map_or(0, |after| count(after).0);
    (removed_count > kept).then(|| SymbolLink {
      name: name.to_string(),
      declared_in: site.ctx.path.to_string(),
      used_in: before.path.to_string(),
      line: line.unwrap_or_default(),
      evidence: Evidence::RemovesAfter,
    })
  })
}

/// The evidence that `reference` in `use_site` means the declaration `site`, `None` without any; the name an import
/// brings in is the import's evidence
fn resolve(input: &SymbolInput, use_site: &UseSite<'_>, reference: &Reference, qualifier: Option<&[String]>, site: &Site<'_>) -> Option<Evidence> {
  let evidence = resolve_any(input, use_site, reference, qualifier, site)?;
  Some(if reference.role == Role::Import && evidence == Evidence::Qualified {
    Evidence::Import
  } else {
    evidence
  })
}

fn resolve_any(input: &SymbolInput, use_site: &UseSite<'_>, reference: &Reference, qualifier: Option<&[String]>, site: &Site<'_>) -> Option<Evidence> {
  if use_site.language.family() != site.ctx.language.family() {
    return None;
  }
  if use_site.path == site.ctx.path {
    // the extraction drops locals; a member of another container of the file is no use of a private one
    return match site.declaration.reach {
      Reach::Container => contains(use_site.summary, reference, &site.containers).then_some(Evidence::SameFile),
      _ => Some(Evidence::SameFile),
    };
  }
  if matches!(site.declaration.reach, Reach::File | Reach::Container) && site.ctx.language != Language::Rust {
    return None;
  }
  match site.ctx.language.family() {
    Family::Jvm => resolve_jvm(use_site, reference, qualifier, site),
    Family::Rust => resolve_rust(input, use_site, reference, qualifier, site),
    Family::Script => resolve_script(use_site, reference, qualifier, site),
  }
}

/// Whether `reference` is inside a declaration of `containers` (the innermost container chain of its file)
fn contains(summary: &FileSummary, reference: &Reference, containers: &[String]) -> bool {
  let Some(last) = containers.last() else { return true };
  let mut current = reference.container;
  while let Some(index) = current {
    let Some(declaration) = summary.declarations.get(index as usize) else { return false };
    if summary.name(declaration.name) == last {
      return true;
    }
    current = declaration.container;
  }
  false
}

/// The names of `segments` in `summary`
fn names(summary: &FileSummary, segments: &[u32]) -> Vec<String> {
  segments.iter().map(|&segment| summary.name(segment).to_string()).collect()
}

fn resolve_jvm(use_site: &UseSite<'_>, reference: &Reference, qualifier: Option<&[String]>, site: &Site<'_>) -> Option<Evidence> {
  let summary = use_site.summary;
  let name = site.name();
  // Kotlin `internal` reaches its module, Java's package-private its package
  if site.declaration.reach == Reach::Module {
    let reachable = match site.ctx.language {
      Language::Kotlin => use_site.module_root.is_some() && use_site.module_root == site.ctx.module_root,
      _ => use_site.namespace == site.ctx.namespace,
    };
    if !reachable {
      return None;
    }
  }
  let imports_of = |path: &[String], target: &str| {
    summary.imports.iter().zip(&use_site.import_paths).any(|(import, import_path)| {
      import_path == path
        && matches!(import.target, ImportTarget::Name(imported) if summary.name(imported) == target)
        && import.alias.map_or(target, |alias| summary.name(alias)) == reference_head(summary, reference, qualifier)
    })
  };
  let star_imports = |path: &[String]| {
    summary
      .imports
      .iter()
      .zip(&use_site.import_paths)
      .any(|(import, import_path)| import.target == ImportTarget::All && import_path == path)
  };
  match site.declaration.member {
    MemberKind::TopLevel => {
      if qualifier.is_none() && imports_of(&site.ctx.namespace, name) {
        return Some(Evidence::Import);
      }
      if qualifier == Some(site.ctx.namespace.as_slice()) {
        return Some(Evidence::Qualified);
      }
      let is_type = matches!(site.declaration.kind, DeclKind::Type | DeclKind::TypeAlias) && matches!(reference.role, Role::Type | Role::Call);
      if qualifier.is_none() && is_type {
        // a declaration of its own, or an explicit import of another one, shadows the package's
        let shadowed = summary.declares(name)
          || summary
            .imports
            .iter()
            .any(|import| matches!(import.target, ImportTarget::Name(imported) if summary.name(imported) == name));
        if !shadowed && use_site.namespace == site.ctx.namespace && use_site.module_root.is_some() && use_site.module_root == site.ctx.module_root {
          return Some(Evidence::SamePackageType);
        }
        if !shadowed && star_imports(&site.ctx.namespace) {
          return Some(Evidence::StarImport);
        }
      }
      None
    }
    MemberKind::Static => {
      let (Some(qualifier), Some(owner)) = (qualifier, site.containers.last()) else {
        // Java's `import static a.B.member`
        let mut path = site.ctx.namespace.clone();
        path.extend(site.containers.iter().cloned());
        return (reference.qualifier.is_none() && imports_of(&path, name)).then_some(Evidence::Import);
      };
      // `Owner.member`, `Owner.Companion.member`, `a.b.Owner.member`
      let qualifier: Vec<&str> = qualifier.iter().map(String::as_str).filter(|segment| *segment != "Companion").collect();
      let mut full: Vec<&str> = site.ctx.namespace.iter().map(String::as_str).collect();
      full.extend(site.containers.iter().map(String::as_str));
      if qualifier == full {
        return Some(Evidence::Qualified);
      }
      let owner_resolves = imports_of(&site.ctx.namespace, owner) || (use_site.namespace == site.ctx.namespace && !summary.declares(owner));
      (qualifier == [owner.as_str()] && owner_resolves).then_some(Evidence::Qualified)
    }
    MemberKind::Instance => None,
  }
}

/// The name a reference starts with: the first segment of its qualifier, else its own name
fn reference_head<'a>(summary: &'a FileSummary, reference: &Reference, qualifier: Option<&'a [String]>) -> &'a str {
  qualifier.and_then(<[String]>::first).map_or_else(|| summary.name(reference.name), String::as_str)
}

/// The module a Rust path names from the file of `use_site`, `crate`, `self`, `super` and crate names resolved: (crate
/// root, module path)
fn rust_path<'a>(input: &'a SymbolInput, use_site: &UseSite<'a>, segments: &[String]) -> Option<(Option<&'a str>, Vec<String>)> {
  let (first, rest) = segments.split_first()?;
  match first.as_str() {
    "crate" => Some((use_site.crate_root, rest.to_vec())),
    "self" => Some((use_site.crate_root, [use_site.namespace.clone(), rest.to_vec()].concat())),
    "super" => {
      let mut module = use_site.namespace.clone();
      module.pop();
      let mut rest = rest;
      while let Some((next, after)) = rest.split_first().filter(|(next, _)| *next == "super") {
        let _ = next;
        module.pop();
        rest = after;
      }
      Some((use_site.crate_root, [module, rest.to_vec()].concat()))
    }
    name => {
      if let Some((root, _)) = input.crates.iter().find(|(_, crate_name)| *crate_name == name) {
        return Some((input.roots.crate_of(&format!("{root}/src/lib.rs")).or(Some(root.as_str())), rest.to_vec()));
      }
      // a name a `use` of the file brings in: `use a::b; b::C`
      let summary = use_site.summary;
      let id = summary.name_id(name);
      if let Some((import, import_path)) = summary
        .imports
        .iter()
        .zip(&use_site.import_paths)
        .find(|(import, _)| matches!(import.target, ImportTarget::Name(imported) if Some(import.alias.unwrap_or(imported)) == id))
      {
        let mut path = import_path.clone();
        if let ImportTarget::Name(imported) = import.target {
          path.push(summary.name(imported).to_string());
        }
        path.extend(rest.iter().cloned());
        return rust_path(input, use_site, &path);
      }
      // a module of this one (`mod a;` and `a::b`)
      Some((use_site.crate_root, [use_site.namespace.clone(), segments.to_vec()].concat()))
    }
  }
}

fn resolve_rust(input: &SymbolInput, use_site: &UseSite<'_>, reference: &Reference, qualifier: Option<&[String]>, site: &Site<'_>) -> Option<Evidence> {
  let summary = use_site.summary;
  let same_crate = use_site.crate_root.is_some() && use_site.crate_root == site.ctx.crate_root;
  // the module that holds the item, with the inline modules it is in
  let mut item_module = site.ctx.namespace.clone();
  let inline_modules: Vec<String> = site.containers.clone();
  let visible = match site.declaration.reach {
    Reach::Public => true,
    Reach::Module => same_crate,
    Reach::Parent => same_crate && use_site.namespace.starts_with(&item_module[..item_module.len().saturating_sub(1)]),
    Reach::File | Reach::Container => same_crate && use_site.namespace.starts_with(&item_module),
  };
  if !visible {
    return None;
  }
  let name = site.name();
  match site.declaration.member {
    MemberKind::TopLevel => {
      // a top-level item is a member of inline modules only
      item_module.extend(inline_modules);
      if reference.qualifier.is_none() && same_crate && use_site.namespace == item_module && reference.role != Role::Import {
        return Some(Evidence::SameModule);
      }
      let is_item = |path: &[String]| rust_path(input, use_site, path).is_some_and(|(root, module)| root == site.ctx.crate_root && module == item_module);
      for (import, path) in summary.imports.iter().zip(&use_site.import_paths) {
        match import.target {
          ImportTarget::Name(imported) if summary.name(imported) == name && binds(import.alias, imported, reference) && is_item(path) => {
            return Some(Evidence::Import);
          }
          ImportTarget::All if reference.qualifier.is_none() && is_item(path) => return Some(Evidence::StarImport),
          _ => {}
        }
      }
      let qualifier = qualifier?;
      (!qualifier.is_empty() && is_item(qualifier)).then_some(Evidence::Qualified)
    }
    MemberKind::Static => {
      // `Type::new()`: the type resolves to the receiver of the impl
      let owner = site.containers.last()?;
      let (last, path) = qualifier?.split_last()?;
      if last != owner {
        return None;
      }
      if path.is_empty() {
        let imported = summary.imports.iter().zip(&use_site.import_paths).any(|(import, import_path)| {
          matches!(import.target, ImportTarget::Name(imported) if summary.name(imported) == owner)
            && rust_path(input, use_site, import_path).is_some_and(|(root, module)| root == site.ctx.crate_root && module == item_module)
        });
        return (imported || (same_crate && use_site.namespace == item_module)).then_some(Evidence::Qualified);
      }
      rust_path(input, use_site, path)
        .is_some_and(|(root, module)| root == site.ctx.crate_root && module == item_module)
        .then_some(Evidence::Qualified)
    }
    MemberKind::Instance => None,
  }
}

/// Whether `reference` is the name an import of `imported` (as `alias`) binds, or the imported name in the import itself
fn binds(alias: Option<u32>, imported: u32, reference: &Reference) -> bool {
  alias.unwrap_or(imported) == reference.name || (reference.role == Role::Import && imported == reference.name)
}

/// The files a TypeScript import specifier names from `from` (relative, or by a path alias), without extensions
fn ts_targets(specifier: &str, from: &str, aliases: Option<&TsAliases>) -> Vec<String> {
  let dir = from.rsplit_once('/').map_or("", |(dir, _)| dir);
  let mut bases = Vec::new();
  if specifier.starts_with("./") || specifier.starts_with("../") {
    bases.push(normalize(&join(dir, specifier)));
  } else if let Some(aliases) = aliases {
    for (pattern, targets) in &aliases.paths {
      let matched = match pattern.split_once('*') {
        Some((prefix, suffix)) => specifier.strip_prefix(prefix).and_then(|rest| rest.strip_suffix(suffix)),
        None => (specifier == pattern).then_some(""),
      };
      let Some(matched) = matched else { continue };
      bases.extend(targets.iter().map(|target| normalize(&target.replacen('*', matched, 1))));
    }
  }
  bases.into_iter().map(|base| strip_script_extension(&base).to_string()).collect()
}

/// `path` without `.ts`, `.tsx`, `.js`, … and without a trailing `/index`
fn strip_script_extension(path: &str) -> &str {
  let path = ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"]
    .iter()
    .find_map(|extension| path.strip_suffix(extension).and_then(|rest| rest.strip_suffix('.')))
    .unwrap_or(path);
  path.strip_suffix("/index").unwrap_or(path)
}

fn resolve_script(use_site: &UseSite<'_>, reference: &Reference, qualifier: Option<&[String]>, site: &Site<'_>) -> Option<Evidence> {
  let summary = use_site.summary;
  let declared = strip_script_extension(site.ctx.path);
  let means = |specifier: Option<&str>| specifier.is_some_and(|specifier| ts_targets(specifier, use_site.path, use_site.aliases).iter().any(|target| target == declared));
  let name = site.name();
  for import in &summary.imports {
    match import.target {
      ImportTarget::Name(imported) if summary.name(imported) == name && binds(import.alias, imported, reference) && qualifier.is_none() && means(import.specifier.as_deref()) => {
        return Some(Evidence::Import);
      }
      ImportTarget::Namespace
        if qualifier.is_some_and(|qualifier| import.alias.is_some_and(|alias| qualifier == [summary.name(alias).to_string()])) && means(import.specifier.as_deref()) =>
      {
        return Some(Evidence::Qualified);
      }
      _ => {}
    }
  }
  None
}
