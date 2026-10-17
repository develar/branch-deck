//! Which commits need which by the names they declare and use, from the syntax summaries of the files they change
//! (`symbol_scan`). The edges are of the class `Order` (docs/design/symbol-dependencies.md): a commit that uses a name
//! another commit adds can't be built without it, but the two may still be in two branches, one built after the other;
//! git's three-way merge can't refute such an edge, so it never joins two branches the way a line dependency does.
//!
//! - **The index is the range.** Only the declarations the commits ahead of the baseline add or change are looked up,
//!   so a repository of a million files costs only the blobs the commits change. A use of a name the range doesn't
//!   add is no edge.
//! - **Evidence, not names.** A use links to a declaration only with evidence that it means that one: an import of
//!   it, a qualified name, the same file (unless another type qualifies the name), the same Rust module, a type of the
//!   same package in the same module, or an override in a subtype whose supertype resolves. A call of a function of
//!   the same package without an import, or a member of an unknown receiver, is no evidence: Kotlin's implicit
//!   receivers and extensions decide those, which only a compiler sees.
//! - **A key has entries.** A key (namespace, containers and name) holds the declarations of its name: for a member
//!   of a JVM type all of them, its overloads, told apart by receiver and parameter types (arrays and a vararg
//!   included), each with the commit it comes from (`Origin`); a commit that removes one entry and adds one changes its
//!   form. An override, and a call by its arguments, link only when every entry they may mean comes from one older
//!   commit: the baseline, its own commit or two commits are no edge. Whether a call's arguments fit an entry errs
//!   toward yes (`accepts_call`), and they tell the overloads apart only of a type whose supertypes are at most `Any`
//!   or `Object`: another supertype may declare more, which the range doesn't show. A call of such a type, a bare name
//!   that something of its file may take before the import that brings it in, and any other use need only the key,
//!   the commit that added it. In the file of a key of the baseline only a use that can mean only a member of its type
//!   means it: a bare name, `this` or `super` in the type, not in a lambda, an object expression, an anonymous class or
//!   a member extension, and in Kotlin not where a receiver further out, the top level or an import may take it.
//!   A top-level function may have overloads in files the range doesn't change, so its key is only present or absent,
//!   with the entries of each file of the range that adds it.
//! - **Degraded files decide nothing**: a file with a twentieth of its bytes in parse errors makes no edge.

use crate::diff_scan::CommitDiff;
use crate::module_roots::ModuleRoots;
use crate::symbol_scan::{SymbolInput, TsAliases, join, normalize};
use code_symbols::language::{Family, Language};
use code_symbols::summary::{Argument, DeclKind, Declaration, FileSummary, ImportTarget, MemberKind, Parameter, Reach, Reference, Role, Shape, TypeRef};
use object_cache::commit_diff::FileChange;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;

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
  /// A Kotlin `override` or Java `@Override` member of a subtype whose supertype resolves (`resolve_override`)
  Override,
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
      Self::Override => "override",
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

/// One declaration of a key in the state of the range, an overload of its name, with the step that made it what it is
#[derive(Clone)]
struct Entry<'a> {
  /// Its form now, in the file of the last commit that changed the key
  site: Site<'a>,
  origin: Rc<Origin<'a>>,
}

/// The step of the range that created an entry or changed its form
struct Origin<'a> {
  /// `None`: the entry was there before the range
  commit: Option<usize>,
  /// The entries of the key the commit removed; when it removed one and added one, that is this entry in its old form
  predecessors: Vec<Entry<'a>>,
}

/// Name -> key -> the entries of each key the range added or changed so far
type Index<'a> = HashMap<String, BTreeMap<String, Vec<Entry<'a>>>>;

/// The declarations of a member key on both sides of the files of a commit that change it
#[derive(Default)]
struct Member<'a> {
  before: Vec<Site<'a>>,
  after: Vec<Site<'a>>,
  /// A side of one of those files is degraded
  degraded: bool,
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
  let mut index: Index<'_> = HashMap::new();
  // (from, to) -> links
  let mut edges: BTreeMap<(usize, usize), Vec<SymbolLink>> = BTreeMap::new();
  let mut entities: Vec<BTreeSet<String>> = vec![BTreeSet::new(); commits.len()];

  for (position, changed) in files.iter().enumerate() {
    // the declarations of the files before and after whose key the other side lacks
    let mut added: Vec<Site<'_>> = Vec::new();
    let mut removed: Vec<Site<'_>> = Vec::new();
    // the member keys of the declarations it changes, with all their declarations on both sides
    let mut members: BTreeMap<String, Member<'_>> = BTreeMap::new();
    for (change, before, after) in changed {
      let (added_lines, removed_lines) = changed_lines(change);
      match (before, after) {
        // the same namespace on both sides: a declaration added or removed lies on the changed lines of its side, so
        // only those are compared, by the declarations of the same name on the other side
        (Some(before), Some(after)) if before.namespace == after.namespace => {
          let after_changed = changed_sites(after, &added_lines);
          let before_changed = changed_sites(before, &removed_lines);
          added.extend(after_changed.iter().filter(|site| sites_of_key(before, site).is_empty()).cloned());
          removed.extend(before_changed.iter().filter(|site| sites_of_key(after, site).is_empty()).cloned());
          let keys: BTreeMap<&str, &Site<'_>> = after_changed
            .iter()
            .chain(&before_changed)
            .filter(|site| has_entries(site))
            .map(|site| (site.key.as_str(), site))
            .collect();
          for site in keys.into_values() {
            let member = members.entry(site.key.clone()).or_default();
            member.before.extend(sites_of_key(before, site));
            member.after.extend(sites_of_key(after, site));
            member.degraded |= before.summary.is_degraded() || after.summary.is_degraded();
          }
        }
        // a new file, a deleted one, or a moved namespace (a rename, a new package): every key may differ
        _ => {
          let before_sites = before.as_ref().map_or_else(Vec::new, |ctx| declared_sites(ctx));
          let after_sites = after.as_ref().map_or_else(Vec::new, |ctx| declared_sites(ctx));
          let before_keys: BTreeSet<&str> = before_sites.iter().map(|site| site.key.as_str()).collect();
          let after_keys: BTreeSet<&str> = after_sites.iter().map(|site| site.key.as_str()).collect();
          added.extend(after_sites.iter().filter(|site| !before_keys.contains(site.key.as_str())).cloned());
          removed.extend(before_sites.iter().filter(|site| !after_keys.contains(site.key.as_str())).cloned());
          let degraded = [before, after].into_iter().flatten().any(|ctx| ctx.summary.is_degraded());
          for (is_after, sites) in [(false, before_sites), (true, after_sites)] {
            for site in sites.into_iter().filter(has_entries) {
              let member = members.entry(site.key.clone()).or_default();
              member.degraded |= degraded;
              if is_after { member.after.push(site) } else { member.before.push(site) }
            }
          }
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
            entities[position].insert(site_of(ctx, declaration).key);
          }
        }
      }
    }
    // a declaration that moved to another file of the same commit is neither added nor removed
    let added_keys: BTreeSet<String> = added.iter().map(|site| site.key.clone()).collect();
    let removed_keys: BTreeSet<String> = removed.iter().map(|site| site.key.clone()).collect();
    added.retain(|site| !removed_keys.contains(&site.key));
    removed.retain(|site| !added_keys.contains(&site.key));
    // a member key goes by its entries, but only on clean sides: a declaration the parser recovered around may be
    // misread, and two declarations of one side that are the same entry can't be matched across the sides, so such a
    // key, as every other, is only present or absent; a removal stays one of the whole key
    members.retain(|_, member| {
      !member.degraded && member.before.iter().chain(&member.after).all(|site| !site.declaration.in_error) && !has_twins(&member.before) && !has_twins(&member.after)
    });
    added.retain(|site| !members.contains_key(&site.key));

    // its own declarations first, so that its uses of them resolve to it
    let origin = Rc::new(Origin {
      commit: Some(position),
      predecessors: Vec::new(),
    });
    let mut fresh: BTreeMap<String, Vec<Entry<'_>>> = BTreeMap::new();
    for site in added
      .iter()
      .filter(|site| !site.ctx.summary.is_degraded() && !site.declaration.in_error && is_exported(site.declaration, site.ctx.summary))
    {
      fresh.entry(site.key.clone()).or_default().push(Entry {
        site: site.clone(),
        origin: Rc::clone(&origin),
      });
    }
    for (key, entries) in fresh {
      let Some(name) = entries.first().map(|entry| entry.site.name().to_string()) else {
        continue;
      };
      // a key the range has in another file, an overload of a top-level function, keeps those entries; one of a file
      // the commit declares it in again is replaced
      let known = index.entry(name).or_default().entry(key).or_default();
      known.retain(|entry| entries.iter().all(|new| new.site.ctx.path != entry.site.ctx.path));
      known.extend(entries);
    }
    for (key, member) in members {
      let Some(name) = member.before.iter().chain(&member.after).next().map(|site| site.name().to_string()) else {
        continue;
      };
      let known = index.get(&name).and_then(|keys| keys.get(&key)).map(Vec::as_slice);
      let Some(entries) = member_entries(known, member, position) else { continue };
      index.entry(name).or_default().insert(key, entries);
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
        let shape = reference.shape.and_then(|shape| use_site.summary.shapes.get(shape as usize));
        // a bare name, `this.name` or `super.name`: a member of the type around it
        let unqualified = qualifier
          .as_deref()
          .is_none_or(|qualifier| matches!(qualifier, [only] if only == "this" || only == "super"));
        // a call in a lambda, an object expression, an anonymous class or a local declaration, whose receiver may be
        // what a bare name or `this` means
        let enclosed = matches!(shape, Some(Shape::Call { enclosed: true, .. }));
        let mut origins: BTreeSet<Option<usize>> = BTreeSet::new();
        let mut first: Option<(&Site<'_>, Evidence)> = None;
        for entries in candidates.values() {
          let Some(head) = entries.first().map(|entry| &entry.site) else { continue };
          let call = match shape {
            Some(Shape::Call { arguments, spread, .. }) if calls_tell_apart(use_site, reference, head, enclosed) => Some((arguments.as_slice(), *spread)),
            _ => None,
          };
          let tells_overloads_apart = call.is_some() || matches!(shape, Some(Shape::Override { .. }));
          // a key of the baseline that a commit gives a new overload or form: in its file only a use that can mean only
          // a member of its type means it (`means_its_type`), else `list.add(0, item)` of a `MutableList`, or
          // `buildList { add(0, item) }`, that the old `add(item)` rejects would link to a new `Lanes.add(index, item)`
          // beside it. A key the range adds keeps the rule before
          let same_file = use_site.path != head.ctx.path || !of_baseline(entries) || (unqualified && means_its_type(use_site.summary, reference, enclosed, head));
          let accepts = |site: &Site<'_>| {
            let evidence = resolve(input, use_site, reference, qualifier.as_deref(), site)?;
            let fits = call.is_none_or(|(arguments, spread)| accepts_call(use_site.summary, arguments, spread, site));
            (fits && (same_file || evidence != Evidence::SameFile)).then_some(evidence)
          };
          let mut needs: BTreeSet<Option<usize>> = BTreeSet::new();
          for entry in entries {
            let Some(evidence) = accepts(&entry.site) else { continue };
            let mut found: BTreeSet<Option<usize>> = BTreeSet::new();
            origins_of(entry, &|site| accepts(site).is_some(), &mut found);
            // a bare name outside the type of a member its own commit adds may mean that member only by an implicit
            // receiver, which the file doesn't show: no origin, as the rule before the entries dropped its own commit
            if evidence == Evidence::SameFile && reference.qualifier.is_none() && !contains(use_site.summary, reference, &entry.site.containers) {
              found.remove(&Some(position));
            }
            if found.is_empty() {
              continue;
            }
            first.get_or_insert((&entry.site, evidence));
            needs.extend(found);
          }
          // a use that tells no overloads of a type apart (a type, a value, a call of a type with supertypes) needs the
          // key, as before the entries: the commit that added it (the oldest origin), or none when it was there before
          // the range. The overloads of a top-level function in several files stay ambiguous
          if !tells_overloads_apart && has_entries(head) {
            needs = needs.first().copied().into_iter().collect();
          }
          origins.extend(needs);
        }
        // every entry it may mean is of one older commit: the baseline, its own commit or two commits are ambiguous
        let (1, Some(&Some(owner))) = (origins.len(), origins.first()) else { continue };
        let Some((site, evidence)) = first.filter(|_| owner != position) else { continue };
        edges.entry((position, owner)).or_default().push(SymbolLink {
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
      // the whole key of a member; of a top-level function the entries of this file, its overloads elsewhere stay, but
      // all of them when none is of this file (it was renamed since)
      if let Some(keys) = index.get_mut(site.name())
        && let Some(entries) = keys.get_mut(&site.key)
      {
        let elsewhere = |entry: &Entry<'_>| entry.site.ctx.path != site.ctx.path;
        if has_entries(site) || entries.iter().all(elsewhere) {
          entries.clear();
        }
        entries.retain(elsewhere);
        if entries.is_empty() {
          keys.remove(&site.key);
        }
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

/// The site of `declaration`, of the file of `ctx`
fn site_of<'a>(ctx: &'a FileCtx<'a>, declaration: &'a Declaration) -> Site<'a> {
  let containers = containers_of(ctx.summary, declaration);
  let mut key = ctx.namespace.join(".");
  for segment in containers.iter().map(String::as_str).chain([ctx.summary.name(declaration.name)]) {
    key.push('.');
    key.push_str(segment);
  }
  Site {
    ctx,
    declaration,
    containers,
    key,
  }
}

/// Whether the key of `site` keeps all its declarations as entries: a member of a JVM type, whose overloads are all in
/// the file of the type. A top-level function may have overloads in files the range doesn't change, so its key, as
/// every key of Rust and TypeScript, is only present or absent
fn has_entries(site: &Site<'_>) -> bool {
  site.ctx.language.family() == Family::Jvm && !site.containers.is_empty()
}

/// The declarations of `ctx` with the key of `site`
fn sites_of_key<'a>(ctx: &'a FileCtx<'a>, site: &Site<'_>) -> Vec<Site<'a>> {
  let Some(name) = ctx.summary.name_id(site.name()) else { return Vec::new() };
  ctx
    .summary
    .declarations
    .iter()
    .filter(|declaration| declaration.name == name)
    .map(|declaration| site_of(ctx, declaration))
    .filter(|other| other.key == site.key)
    .collect()
}

/// The entries of a member key after a commit that changes it, from its declarations before and after the commit; an
/// entry of the same identity on both sides (`same_entry`) keeps its origin, an entry `known` lacks was there before
/// the range, and a new one has the commit for its origin and every entry the commit removed for its predecessors.
/// `None` when the key keeps no entry (the removal drops it after the uses), or when nothing changed in a key the
/// index doesn't know
fn member_entries<'a>(known: Option<&[Entry<'a>]>, member: Member<'a>, commit: usize) -> Option<Vec<Entry<'a>>> {
  let mut unmatched: Vec<&Entry<'a>> = known.unwrap_or_default().iter().collect();
  let mut gone: Vec<Entry<'a>> = member
    .before
    .into_iter()
    .map(|site| {
      let origin = match unmatched.iter().position(|entry| same_entry(&entry.site, &site)) {
        Some(found) => Rc::clone(&unmatched.swap_remove(found).origin),
        None => Rc::new(Origin {
          commit: None,
          predecessors: Vec::new(),
        }),
      };
      Entry { site, origin }
    })
    .collect();
  let mut entries = Vec::new();
  let mut new = Vec::new();
  for site in member.after {
    match gone.iter().position(|entry| same_entry(&entry.site, &site)) {
      Some(found) => entries.push(Entry {
        site,
        origin: gone.swap_remove(found).origin,
      }),
      None => new.push(site),
    }
  }
  if (entries.is_empty() && new.is_empty()) || (known.is_none() && new.is_empty() && gone.is_empty()) {
    return None;
  }
  let origin = Rc::new(Origin {
    commit: Some(commit),
    predecessors: gone,
  });
  entries.extend(new.into_iter().map(|site| Entry { site, origin: Rc::clone(&origin) }));
  Some(entries)
}

/// Whether two declarations of one side are the same entry (`same_entry`): `foo(a.Date)` beside `foo(b.Date)`
fn has_twins(sites: &[Site<'_>]) -> bool {
  sites.iter().enumerate().any(|(index, a)| sites.iter().skip(index + 1).any(|b| same_entry(a, b)))
}

/// Whether two declarations of one key are the same entry: the same kind, extension receiver and parameter types
fn same_entry(a: &Site<'_>, b: &Site<'_>) -> bool {
  let same_type = |x: Option<&TypeRef>, y: Option<&TypeRef>| match (x, y) {
    (Some(x), Some(y)) => x.nullable == y.nullable && same_names(a.ctx.summary, x, b.ctx.summary, y),
    (x, y) => x.is_none() && y.is_none(),
  };
  a.declaration.kind == b.declaration.kind
    && receiver_of(a) == receiver_of(b)
    && match (&a.declaration.signature, &b.declaration.signature) {
      (Some(x), Some(y)) => x.parameters.len() == y.parameters.len() && x.parameters.iter().zip(&y.parameters).all(|(x, y)| same_type(x.type_ref.as_ref(), y.type_ref.as_ref())),
      (x, y) => x.is_none() && y.is_none(),
    }
}

/// The extension receiver of a declaration, its last name
fn receiver_of<'a>(site: &Site<'a>) -> Option<&'a str> {
  site.declaration.receiver.map(|receiver| site.ctx.summary.name(receiver))
}

/// Whether two types, each of its own file, have the same last names and array dimensions
fn same_names(a: &FileSummary, x: &TypeRef, b: &FileSummary, y: &TypeRef) -> bool {
  x.arrays == y.arrays && x.names.len() == y.names.len() && x.names.iter().zip(&y.names).all(|(x, y)| a.name(*x) == b.name(*y))
}

/// The commits a use needs of `entry`, which accepts it, into `origins` (`None`: the baseline): back along the forms
/// of the entry, the latest step whose forms before (its predecessors) all reject the use while the form after accepts
/// it; each predecessor that accepts it leads further back. `accepts` errs toward accepting, so a form it can't judge
/// takes the step further back, toward no edge
fn origins_of<'a>(entry: &Entry<'a>, accepts: &dyn Fn(&Site<'a>) -> bool, origins: &mut BTreeSet<Option<usize>>) {
  let mut stack = vec![entry];
  let mut seen: HashSet<*const Origin<'a>> = HashSet::new();
  while let Some(entry) = stack.pop() {
    if !seen.insert(Rc::as_ptr(&entry.origin)) {
      continue;
    }
    let Some(commit) = entry.origin.commit else {
      origins.insert(None);
      continue;
    };
    let before: Vec<&Entry<'a>> = entry.origin.predecessors.iter().filter(|predecessor| accepts(&predecessor.site)).collect();
    if before.is_empty() {
      origins.insert(Some(commit));
    } else {
      stack.extend(before);
    }
  }
}

/// Whether a key was there before the range: an entry of it, or a form an entry had before, is of the baseline
fn of_baseline(entries: &[Entry<'_>]) -> bool {
  let mut roots: BTreeSet<Option<usize>> = BTreeSet::new();
  for entry in entries {
    origins_of(entry, &|_| true, &mut roots);
  }
  roots.contains(&None)
}

/// The supertypes a type has without naming them
const ROOT_TYPES: [&[&str]; 4] = [&["Any"], &["kotlin", "Any"], &["Object"], &["java", "lang", "Object"]];

/// Whether a type of `summary` has a supertype other than `Any` or `Object`, whose members the range doesn't show
fn extends(summary: &FileSummary, declaration: &Declaration) -> bool {
  let root = |path: &[u32]| ROOT_TYPES.iter().any(|root| path.iter().map(|&segment| summary.name(segment)).eq(root.iter().copied()));
  declaration.supertypes.iter().any(|path| !root(path))
}

/// Whether the arguments of a call may tell the overloads of the member `site` apart: only where all of them are its
/// entries, so not of a type with a supertype other than `Any` or `Object`, which may inherit more that the range
/// doesn't show (`Base.buttons(Long)` takes `buttons(1)`), nor of a name every class has (`ROOT_MEMBERS`); and a bare
/// name of another file, which an import brings in, only where its file shows nothing that comes before the import
/// (`import_decides`)
fn calls_tell_apart(use_site: &UseSite<'_>, reference: &Reference, site: &Site<'_>, enclosed: bool) -> bool {
  has_entries(site)
    && !ROOT_MEMBERS.contains(&site.name())
    && !inherits(use_site, site)
    && (reference.qualifier.is_some() || use_site.path == site.ctx.path || import_decides(use_site.summary, reference, enclosed))
}

/// Whether a bare name that an import brings in (Java's `import static`, a Kotlin import of a member of an object) can
/// mean only what the import names among what its file shows. Not in a lambda, an anonymous function, an object
/// expression, an anonymous class or a local declaration (`enclosed`), whose receiver may take it first; no
/// declaration of its name in the file, which shadows the import (a member of a type around it, a function of the
/// file); no other import that may bring the name (`imports_of_function`), as Java weighs all the static imports of a
/// name together (`import static p.Util.add` shadows the `add` of `import static q.Other.*` only for the same
/// signature, JLS 6.4.1) and Kotlin falls back to a wildcard import and to its default imports; and no receiver
/// around it (`receivers`) with a supertype other than `Any` or `Object`, whose members it may mean (`add(1)` of
/// `import static p.Util.add` in a subclass of a `Base` with `add(int)` is that one), nor a Kotlin extension around
/// it, whose receiver's members come first
fn import_decides(summary: &FileSummary, reference: &Reference, enclosed: bool) -> bool {
  let name = summary.name(reference.name);
  if enclosed || summary.declares(name) || imports_of_function(summary, name) > 1 {
    return false;
  }
  let mut current = reference.container;
  while let Some(declaration) = current.and_then(|index| summary.declarations.get(index as usize)) {
    if declaration.receiver.is_some() {
      return false;
    }
    current = declaration.container;
  }
  !receivers(summary, reference).iter().any(|(_, declaration)| extends(summary, declaration))
}

/// Whether a bare name, `this.name` or `super.name` in the file of the member `site` can mean only a member of its type
/// among what the file shows. Not in a lambda, an anonymous function, an object expression, an anonymous class or a
/// local declaration (`enclosed`), whose receiver may take it first (`buildList { add(0, item) }`), nor in a Kotlin
/// member extension or an accessor of a member extension property, whose extension receiver comes before the type
/// (`fun MutableList<String>.fill() { add(0, "x") }`); the innermost type around it, an enum entry's body included, is
/// the type of `site`; and in Kotlin, whose call falls back to the receivers further out and then to the top level
/// when the members of the type reject it (`Outer.foo(Int)` takes `foo(1)` in an inner class whose `foo(String)`
/// rejects it), none of those receivers (`receivers`) has a supertype other than `Any` or `Object` or another function
/// or value of its name, nor has the top level of the file, nor does an import of the file or a default import
/// (`imports_of_function`: `import q.add`, a wildcard import, `kotlin.error`). Java looks no further than the
/// innermost class with a member of the name
fn means_its_type(summary: &FileSummary, reference: &Reference, enclosed: bool, site: &Site<'_>) -> bool {
  if enclosed {
    return false;
  }
  let Some((own, outer)) = site.containers.split_last() else { return false };
  let mut current = reference.container;
  let innermost = loop {
    let Some(declaration) = current.and_then(|index| summary.declarations.get(index as usize)) else {
      return false;
    };
    if matches!(declaration.kind, DeclKind::Type | DeclKind::EnumEntry) {
      break declaration;
    }
    if declaration.receiver.is_some() {
      return false;
    }
    current = declaration.container;
  };
  if innermost.kind != DeclKind::Type || summary.name(innermost.name) != own || containers_of(summary, innermost) != outer {
    return false;
  }
  if summary.language != Language::Kotlin {
    return true;
  }
  let receivers = receivers(summary, reference);
  // the receivers but the type itself, whose members are the entries
  let further = |index: u32| receivers.iter().skip(1).any(|(receiver, _)| *receiver == index);
  !receivers.iter().any(|(_, declaration)| extends(summary, declaration))
    && !summary.declarations.iter().any(|declaration| {
      matches!(declaration.kind, DeclKind::Function | DeclKind::Value) && summary.name(declaration.name) == site.name() && declaration.container.is_none_or(further)
    })
    && imports_of_function(summary, site.name()) == 0
}

/// How many imports of `summary` may bring a function `name` into it: each one that binds the name, each wildcard
/// import, which may hold one, and in Kotlin the default imports when the name is one of their functions
/// (`is_default_function`). A top-level function of the package in another file brings it too, which the range may not
/// show: a limit
fn imports_of_function(summary: &FileSummary, name: &str) -> usize {
  let imports = summary
    .imports
    .iter()
    .filter(|import| match import.target {
      ImportTarget::Name(imported) => summary.name(import.alias.unwrap_or(imported)) == name,
      ImportTarget::All => true,
      ImportTarget::Default | ImportTarget::Namespace => false,
    })
    .count();
  imports + usize::from(summary.language == Language::Kotlin && is_default_function(name))
}

/// The types whose members a bare name at `reference` may mean, with their indices: the types around it, the innermost
/// first, and in Kotlin the companion of each (a `Companion`: the summary doesn't tell a companion with a name of its
/// own from a nested object, which is no receiver)
fn receivers<'a>(summary: &'a FileSummary, reference: &Reference) -> Vec<(u32, &'a Declaration)> {
  let mut receivers: Vec<(u32, &Declaration)> = Vec::new();
  let mut current = reference.container;
  while let Some(index) = current {
    let Some(declaration) = summary.declarations.get(index as usize) else { break };
    if declaration.kind == DeclKind::Type {
      receivers.push((index, declaration));
    }
    current = declaration.container;
  }
  if summary.language == Language::Kotlin {
    let around: Vec<u32> = receivers.iter().map(|(index, _)| *index).collect();
    receivers.extend(
      summary
        .declarations
        .iter()
        .zip(0u32..)
        .filter(|(declaration, _)| {
          declaration.kind == DeclKind::Type && summary.name(declaration.name) == "Companion" && declaration.container.is_some_and(|it| around.contains(&it))
        })
        .map(|(declaration, index)| (index, declaration)),
    );
  }
  receivers
}

/// Whether the type that declares the member `site` has a supertype other than `Any` or `Object`: in the file of the
/// member, or in that file at the commit of `use_site`, which may have changed the supertypes since
fn inherits(use_site: &UseSite<'_>, site: &Site<'_>) -> bool {
  let declared = site.declaration.container.and_then(|index| site.ctx.summary.declarations.get(index as usize));
  if declared.is_some_and(|declaration| extends(site.ctx.summary, declaration)) {
    return true;
  }
  let Some((last, outer)) = site.containers.split_last().filter(|_| use_site.path == site.ctx.path) else {
    return false;
  };
  let summary = use_site.summary;
  summary
    .declarations
    .iter()
    .any(|declaration| summary.name(declaration.name) == last && containers_of(summary, declaration) == outer && extends(summary, declaration))
}

/// Whether a call with `arguments` may mean the function `site`, erring toward yes, so that a form it can't judge
/// accepts: the positional arguments fill the parameters in order, a vararg (the last) all of them left, the named
/// ones the parameters of their names, a last lambda the last parameter (a Kotlin trailing lambda) or its place in
/// order, and a parameter left unfilled must have a default or be a vararg; a literal must fit its parameter
/// (`fits_literal`). A spread, a named argument before a positional one, a vararg before the last parameter, and a
/// declaration without a signature (a type, a property, a function the parser didn't read) accept any call
fn accepts_call(use_summary: &FileSummary, arguments: &[(Option<u32>, Argument)], spread: bool, site: &Site<'_>) -> bool {
  let Some(signature) = &site.declaration.signature else { return true };
  let parameters = signature.parameters.as_slice();
  if spread || parameters.iter().rev().skip(1).any(|parameter| parameter.vararg) {
    return true;
  }
  let (listed, trailing) = match arguments.split_last() {
    Some(((None, Argument::Lambda), listed)) => (listed, true),
    _ => (arguments, false),
  };
  let (positional, named) = listed.split_at(listed.iter().take_while(|(name, _)| name.is_none()).count());
  if named.iter().any(|(name, _)| name.is_none()) {
    return true;
  }
  let fills = |positional: &[(Option<u32>, Argument)], trailing: bool| {
    let mut filled = vec![false; parameters.len()];
    let mut open = parameters;
    if trailing {
      let Some((_, before)) = parameters.split_last() else { return false };
      open = before;
      filled[before.len()] = true;
    }
    for (index, (_, argument)) in positional.iter().enumerate() {
      // past the parameters only a vararg, the last one, takes the arguments left
      let slot = match open.get(index) {
        Some(_) => index,
        None if open.last().is_some_and(|last| last.vararg) => open.len() - 1,
        None => return false,
      };
      if !fits_literal(*argument, &parameters[slot], site) {
        return false;
      }
      filled[slot] = true;
    }
    for (name, argument) in named.iter().filter_map(|(name, argument)| name.map(|name| (use_summary.name(name), *argument))) {
      let Some(slot) = parameters.iter().position(|parameter| site.ctx.summary.name(parameter.name) == name) else {
        return false;
      };
      if filled[slot] || !fits_literal(argument, &parameters[slot], site) {
        return false;
      }
      filled[slot] = true;
    }
    parameters.iter().zip(&filled).all(|(parameter, filled)| *filled || parameter.optional || parameter.vararg)
  };
  fills(positional, trailing) || (trailing && named.is_empty() && fills(arguments, false))
}

/// Whether an argument may be a value of the type of `parameter` as far as a literal tells: a text, a number or a
/// boolean is none of a type of another of these families (`literal_family`), nor of a class, an interface, an object
/// or an enum that the file of the function declares and doesn't shadow by an import of its name. Any other argument
/// fits, and so does a literal of a type of several names (type arguments, a function type), of an array, or of a type
/// parameter
fn fits_literal(argument: Argument, parameter: &Parameter, site: &Site<'_>) -> bool {
  if !matches!(argument, Argument::Text | Argument::Number | Argument::Boolean) {
    return true;
  }
  // a vararg takes its elements
  let Some(TypeRef { names, arrays, .. }) = &parameter.type_ref else { return true };
  let ([name], 0) = (names.as_slice(), arrays.saturating_sub(u8::from(parameter.vararg))) else {
    return true;
  };
  let summary = site.ctx.summary;
  let declared = summary
    .declarations
    .iter()
    .any(|declaration| declaration.kind == DeclKind::Type && declaration.name == *name)
    && !summary
      .imports
      .iter()
      .any(|import| matches!(import.target, ImportTarget::Name(imported) if import.alias.unwrap_or(imported) == *name));
  !declared && literal_family(summary.name(*name)).is_none_or(|family| family == argument)
}

/// The family of literals a JVM type takes: a text for `String` and `CharSequence`, a number for a Kotlin or Java
/// number type (a primitive, a box, `Number`; Java widens an `int` to a `double`), a boolean for `Boolean`; `None` for
/// every other type, which may take any literal (`Any`, `Object`, `Comparable`) or none
fn literal_family(name: &str) -> Option<Argument> {
  match name {
    "String" | "CharSequence" => Some(Argument::Text),
    "Int" | "Long" | "Short" | "Byte" | "Float" | "Double" | "Number" | "Integer" | "int" | "long" | "short" | "byte" | "float" | "double" => Some(Argument::Number),
    "Boolean" | "boolean" => Some(Argument::Boolean),
    _ => None,
  }
}

/// The declarations of `side` on its `ranges` (the changed lines of that side), and the members of a container whose
/// name changed there
fn changed_sites<'a>(side: &'a FileCtx<'a>, ranges: &[(u32, u32)]) -> Vec<Site<'a>> {
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
    .map(|declaration| site_of(side, declaration))
    .collect()
}

/// The declarations of a file other files can name, overloads included
fn declared_sites<'a>(ctx: &'a FileCtx<'a>) -> Vec<Site<'a>> {
  ctx
    .summary
    .declarations
    .iter()
    .filter(|declaration| ctx.summary.name(declaration.name).len() >= MIN_NAME_LENGTH)
    .map(|declaration| site_of(ctx, declaration))
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
  // an override is no use by its name and qualifier: its qualifier is a supertype of the class around it, which needs
  // the rules of overriding, in the same file too
  if reference.role == Role::Override {
    return resolve_override(use_site, reference, qualifier, site);
  }
  if use_site.path == site.ctx.path {
    // the extraction drops locals; a member of another container of the file is no use of a private one, nor is a
    // name qualified by another type
    if names_another_type(use_site.summary, qualifier, site) {
      return None;
    }
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

/// Whether a qualifier names a type other than the containers and the receiver of `site`: `Registry.register(id)` is
/// no use of a `register` of `Plugin` beside it. A type by its capital, the convention of every language here, unless
/// the file declares a value of that name (`val DEFAULT`, an `INSTANCE`, an enum constant); a value in small letters
/// (`manager.register`, of a type only a compiler knows), `this`, `Self` and `Companion` name none. A name in capitals
/// that the file only imports counts as a type, `JBUI` or a constant: an import doesn't say which it is
fn names_another_type(summary: &FileSummary, qualifier: Option<&[String]>, site: &Site<'_>) -> bool {
  let qualifier = qualifier.unwrap_or_default();
  let Some(last) = qualifier.iter().rfind(|segment| *segment != "Companion") else {
    return false;
  };
  let receiver = receiver_of(site);
  let own = |segment: &String| site.containers.contains(segment) || receiver == Some(segment.as_str());
  let value = summary
    .declarations
    .iter()
    .any(|declaration| matches!(declaration.kind, DeclKind::Value | DeclKind::Constant | DeclKind::EnumEntry) && summary.name(declaration.name) == last);
  last.starts_with(char::is_uppercase) && !value && last != "Self" && !qualifier.iter().any(own)
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

/// Names every class has from `Any` or `Object`: an override of one needs no commit, and a call of one may mean theirs
const ROOT_MEMBERS: [&str; 5] = ["equals", "hashCode", "toString", "clone", "finalize"];

/// The evidence that an `Override` reference overrides the member `site`. The member must be one a subtype overrides
/// (an instance function or property, `overridable`, of the same receiver and parameter types, `fits`), not one every
/// class has (`ROOT_MEMBERS`), and must override nothing itself, else the supertype's supertype has it already; its type
/// must be the one the reference's supertype path names: the same file, the full path, an import (an alias, a nested
/// type), or the same package in the same module without a type of that name in the file, never a wildcard import, and
/// no type of the name that a scope around the override may mean instead (`scope_may_shadow`). No evidence in a class
/// with several supertypes, whose member may come from any, nor for a Java method of a class with supertypes, which
/// may override one without `@Override`
fn resolve_override(use_site: &UseSite<'_>, reference: &Reference, qualifier: Option<&[String]>, site: &Site<'_>) -> Option<Evidence> {
  let summary = use_site.summary;
  let declaration = site.declaration;
  let Some(Shape::Override { function, receiver, parameters }) = reference.shape.and_then(|shape| summary.shapes.get(shape as usize)) else {
    return None;
  };
  let kind = if *function { DeclKind::Function } else { DeclKind::Value };
  if declaration.kind != kind
    || declaration.member != MemberKind::Instance
    || !declaration.overridable
    || declaration.overrides
    || ROOT_MEMBERS.contains(&site.name())
    || !reaches_module(use_site, site)
  {
    return None;
  }
  let container = declaration.container.and_then(|index| site.ctx.summary.declarations.get(index as usize));
  // the members of the supertypes of the member's type are unknown, and the override may mean one of them
  let inherits = container.is_some_and(|container| !container.supertypes.is_empty());
  if site.ctx.language == Language::Java && inherits {
    return None;
  }
  // one reference per supertype of the class around the override
  let supertypes = summary
    .references_to(summary.name(reference.name))
    .filter(|other| other.role == Role::Override && other.line == reference.line && other.container == reference.container && other.shape == reference.shape)
    .count();
  if supertypes > 1 || receiver.map(|it| summary.name(it)) != receiver_of(site) || !fits(use_site, parameters, site, inherits) {
    return None;
  }

  let path = qualifier?;
  let (head, rest) = path.split_first()?;
  let full = site.ctx.namespace.iter().chain(&site.containers).map(String::as_str);
  // the head through an import: `import p.Manager as M`, `import p.Outer.Inner`
  let imported = summary.imports.iter().zip(&use_site.import_paths).find_map(|(import, import_path)| match import.target {
    ImportTarget::Name(imported) if summary.name(import.alias.unwrap_or(imported)) == head => Some((import_path, summary.name(imported))),
    _ => None,
  });
  if let Some((import_path, imported)) = imported {
    let resolved = import_path.iter().map(String::as_str).chain([imported]).chain(rest.iter().map(String::as_str));
    return resolved.eq(full).then_some(Evidence::Override);
  }
  if !site.ctx.namespace.is_empty() && path.iter().map(String::as_str).eq(full) {
    return Some(Evidence::Override);
  }
  if path != site.containers.as_slice() || scope_may_shadow(use_site, reference, head, path) {
    return None;
  }
  if use_site.path == site.ctx.path {
    return Some(Evidence::Override);
  }
  let same_module = use_site.module_root.is_some() && use_site.module_root == site.ctx.module_root;
  (same_module && use_site.namespace == site.ctx.namespace && !summary.declares(head)).then_some(Evidence::Override)
}

/// Whether the parameter types of an override fit those of the member `site`. Where the member's type has no
/// supertypes, the override must mean one of its members, and each overload of it is an entry, so the match is
/// generous: a type parameter fits any type and two types of the same last names are the same, which only adds an
/// origin. Where it has (`exact`), a member of a supertype outside the range may be what the override means, so only
/// the same form counts: a signature, every type known, each name resolved by its file to the same type
fn fits(use_site: &UseSite<'_>, parameters: &[Option<TypeRef>], site: &Site<'_>, exact: bool) -> bool {
  let Some(signature) = &site.declaration.signature else {
    // a property has none, and a function's that the parser didn't find fits any unless the form must be exact
    return !exact || site.declaration.kind != DeclKind::Function;
  };
  // Kotlin's `?` only between two Kotlin files: a Java type has none, and a Kotlin override of it may add one
  let nullable = use_site.language == Language::Kotlin && site.ctx.language == Language::Kotlin;
  // each name of the override's type and of the member's type resolved by its own file
  let resolved = |x: &TypeRef, y: &TypeRef| {
    let (a, b) = (use_site.summary, site.ctx.summary);
    x.names.iter().zip(&y.names).all(|(x, y)| same_type(use_site, a.name(*x), site.ctx, b.name(*y)))
  };
  let fits = |shape: Option<&TypeRef>, parameter: Option<&TypeRef>| match (shape, parameter) {
    (Some(x), Some(y)) => (!nullable || x.nullable == y.nullable) && same_names(use_site.summary, x, site.ctx.summary, y) && (!exact || resolved(x, y)),
    _ => !exact,
  };
  signature.parameters.len() == parameters.len() && parameters.iter().zip(&signature.parameters).all(|(x, y)| fits(x.as_ref(), y.type_ref.as_ref()))
}

/// Whether the type name `a` of the file of `x` and `b` of the file of `y` name the same type: by an import or a type
/// of the file each, else by the package. Two names that neither file imports or declares are the same when they are
/// equal and in one package, whose type, a wildcard import or a default import each file then means alike, or when both
/// are of the common default imports (`is_default_type`: `String`, `int`); in two packages each may mean a type of its
/// own package or of its wildcard imports, even when both files have the same wildcard import
fn same_type(x: &FileCtx<'_>, a: &str, y: &FileCtx<'_>, b: &str) -> bool {
  let in_package = |ctx: &FileCtx<'_>, name: &str| ctx.namespace.iter().map(String::as_str).chain([name]).collect::<Vec<_>>().join(".");
  match (full_type_name(x, a), full_type_name(y, b)) {
    (Some(a), Some(b)) => a == b,
    (Some(a), None) => a == in_package(y, b),
    (None, Some(b)) => b == in_package(x, a),
    (None, None) => a == b && (x.namespace == y.namespace || (is_default_type(x.language, a) && is_default_type(y.language, b))),
  }
}

/// Whether a JVM file has the type `name` without an import, and a project rarely reuses its name, which a type of the
/// file's package or a wildcard import could shadow: a Java primitive, a common type of `java.lang` (a Kotlin file
/// has them too), or in Kotlin a common type or typealias of its default imports (`kotlin`, `kotlin.collections`,
/// `kotlin.text`, `kotlin.ranges`, `kotlin.sequences`). A default type left out (`Module`, `Process`, `Result`,
/// `Function`, which a project or a wildcard import often has of its own) is unknown, as the types of other packages
fn is_default_type(language: Language, name: &str) -> bool {
  const JAVA_LANG: &str = "Object String CharSequence Boolean Byte Character Short Integer Long Float Double Number Void Class Enum Comparable Iterable Runnable \
    Throwable Exception Error RuntimeException IllegalArgumentException IllegalStateException UnsupportedOperationException IndexOutOfBoundsException \
    NullPointerException ClassCastException ArithmeticException NumberFormatException InterruptedException CloneNotSupportedException \
    ReflectiveOperationException ClassNotFoundException NoSuchMethodException NoSuchFieldException SecurityException AssertionError OutOfMemoryError \
    StackOverflowError StringBuilder StringBuffer Appendable Readable AutoCloseable Cloneable Thread ThreadLocal ClassLoader StackTraceElement";
  const JAVA_PRIMITIVES: &str = "boolean byte char short int long float double";
  const KOTLIN: &str = "Any Unit Nothing Int Char UInt ULong UShort UByte Array BooleanArray ByteArray CharArray ShortArray IntArray LongArray FloatArray \
    DoubleArray Annotation Comparator Lazy Pair Triple NoSuchElementException ConcurrentModificationException Collection MutableCollection \
    MutableIterable List MutableList Set MutableSet Map MutableMap Iterator MutableIterator ListIterator MutableListIterator ArrayList HashMap HashSet \
    LinkedHashMap LinkedHashSet ArrayDeque RandomAccess Sequence Regex MatchResult IntRange LongRange CharRange ClosedRange";
  let among = |names: &str| names.split_whitespace().any(|it| it == name);
  among(JAVA_LANG)
    || match language {
      Language::Java => among(JAVA_PRIMITIVES),
      Language::Kotlin => among(KOTLIN),
      _ => false,
    }
}

/// Whether `name` is a common top-level function of Kotlin's default imports, which a bare call falls back to when the
/// members of its receivers reject it (`error("boom")` beside a member `error(code: Int)` is `kotlin.error`): of
/// `kotlin`, `kotlin.collections`, `kotlin.comparisons`, `kotlin.io`, `kotlin.sequences` and `kotlin.text`, an
/// extension of any receiver (`let`, `apply`) included, which an implicit receiver takes. A default function left out
/// is a limit: a call that it takes links to a new member overload of its name
fn is_default_function(name: &str) -> bool {
  const KOTLIN: &str = "error check checkNotNull require requireNotNull TODO repeat run with apply also let takeIf takeUnless runCatching lazy \
    lazyOf synchronized assert arrayOf arrayOfNulls emptyArray enumValues enumValueOf listOf listOfNotNull mutableListOf arrayListOf emptyList setOf \
    setOfNotNull mutableSetOf hashSetOf linkedSetOf sortedSetOf emptySet mapOf mutableMapOf hashMapOf linkedMapOf sortedMapOf emptyMap buildList \
    buildSet buildMap maxOf minOf compareBy compareByDescending compareValues compareValuesBy naturalOrder reverseOrder nullsFirst nullsLast print \
    println readLine readln readlnOrNull sequence sequenceOf emptySequence generateSequence iterator buildString";
  KOTLIN.split_whitespace().any(|it| it == name)
}

/// The full name of a type name of the file of `ctx` by an import that binds it or a type the file declares; `None`
/// when the package, a wildcard or a default import decides
fn full_type_name(ctx: &FileCtx<'_>, name: &str) -> Option<String> {
  let summary = ctx.summary;
  let imported = summary.imports.iter().zip(&ctx.import_paths).find_map(|(import, path)| match import.target {
    ImportTarget::Name(imported) if summary.name(import.alias.unwrap_or(imported)) == name => {
      Some(path.iter().map(String::as_str).chain([summary.name(imported)]).collect::<Vec<_>>().join("."))
    }
    _ => None,
  });
  imported.or_else(|| {
    let declared = summary
      .declarations
      .iter()
      .find(|declaration| matches!(declaration.kind, DeclKind::Type | DeclKind::TypeAlias) && summary.name(declaration.name) == name)?;
    Some(site_of(ctx, declared).key)
  })
}

/// Whether a type named `head` other than the one of the supertype path of an override may be what it means: one
/// nested in a declaration around the override (`Outer.Base` for a class inside `Outer`), or a member type that a Java
/// class around the overriding class inherits, which the file doesn't show. Kotlin also reaches the nested classes of
/// a supertype by their names; that is left out, since an object expression in a class with supertypes is everywhere
fn scope_may_shadow(use_site: &UseSite<'_>, reference: &Reference, head: &str, path: &[String]) -> bool {
  let summary = use_site.summary;
  // the declarations around the override, the innermost first
  let mut around: Vec<(u32, &Declaration)> = Vec::new();
  let mut current = reference.container;
  while let Some(index) = current {
    let Some(declaration) = summary.declarations.get(index as usize) else { break };
    around.push((index, declaration));
    current = declaration.container;
  }
  let nested = summary.declarations.iter().any(|declaration| {
    matches!(declaration.kind, DeclKind::Type | DeclKind::TypeAlias)
      && summary.name(declaration.name) == head
      && declaration.container.is_some_and(|container| around.iter().any(|(index, _)| *index == container))
  });
  // the overriding class itself, whose supertypes name the path, is no class around it
  let inherited = use_site.language == Language::Java
    && around.iter().enumerate().any(|(depth, (_, declaration))| {
      let owner = depth == 0 && declaration.supertypes.iter().any(|supertype| names(summary, supertype) == path);
      !owner && !declaration.supertypes.is_empty()
    });
  nested || inherited
}

/// Whether the JVM declaration `site` reaches the file of `use_site`: Kotlin `internal` reaches its module, Java's
/// package-private its package
fn reaches_module(use_site: &UseSite<'_>, site: &Site<'_>) -> bool {
  site.declaration.reach != Reach::Module
    || match site.ctx.language {
      Language::Kotlin => use_site.module_root.is_some() && use_site.module_root == site.ctx.module_root,
      _ => use_site.namespace == site.ctx.namespace,
    }
}

fn resolve_jvm(use_site: &UseSite<'_>, reference: &Reference, qualifier: Option<&[String]>, site: &Site<'_>) -> Option<Evidence> {
  let summary = use_site.summary;
  let name = site.name();
  if !reaches_module(use_site, site) {
    return None;
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
