//! Which commits need which by the names they declare and use, from the syntax summaries of the files they change
//! (`symbol_scan`). The edges are of the class `Order` (docs/design/symbol-dependencies.md): a commit that uses a name
//! another commit adds can't be built without it, but the two may still be in two branches, one built after the other;
//! git's three-way merge can't refute such an edge, so it never joins two branches the way a line dependency does.
//!
//! - **The index is the range.** Only the declarations the commits ahead of the baseline add or change are looked up,
//!   so a repository of a million files costs only the blobs the commits change. A use of a name the range doesn't
//!   add or change is no edge.
//! - **Evidence, not names.** A use links to a declaration only with evidence that it means that one: an import of
//!   it, a qualified name, the same file (unless another type qualifies the name, or a value or an expression is the
//!   receiver of an extension's call), the same Rust module, a type of the same package in the same module, or an
//!   override in a subtype whose supertype resolves. A call of a function of the same package without an import, or a
//!   member of an unknown receiver, is no evidence: Kotlin's implicit receivers and extensions decide those, which only
//!   a compiler sees.
//! - **A key has entries.** The key of a JVM type or a member of one holds each overload and constructor with the
//!   step that made its form (`Entry`, `Origin`); a use needs the newest commit before its own that every entry it may
//!   mean needs (`steps_of`, `lineage`), told apart by its arguments where they can (`accepts_call`). A commit whose
//!   sides of a key aren't clean (a degraded side, a parse error in a declaration, twins, constructors that one side
//!   knows and the other doesn't) leaves it only present or absent, without its constructors. A top-level function,
//!   also one of the name of a type beside it (`KeyState`), and every key of Rust and TypeScript are only present or
//!   absent, per file. The rules and their reasons are decisions 7 to 11 of docs/design/symbol-dependencies.md.
//! - **Degraded files decide nothing**: a file with a twentieth of its bytes in parse errors makes no edge.

use crate::diff_scan::CommitDiff;
use crate::module_roots::ModuleRoots;
use crate::symbol_scan::{SymbolInput, TsAliases, join, normalize};
use code_symbols::language::{Family, Language};
use code_symbols::summary::{Argument, Constructor, DeclKind, Declaration, FileSummary, ImportTarget, MemberKind, Parameter, Reach, Reference, Role, Shape, Signature, TypeRef};
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
  /// A call of a member of the supertype in a subclass, by a bare name, `this` or `super` (`inherited`)
  Inherited,
  /// A use of a member on a receiver of its type: a local, a parameter or a property of that type, or the receiver of
  /// an extension function around a bare call (`through_receiver`)
  Receiver,
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
      Self::Inherited => "inherited",
      Self::Receiver => "receiver",
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
  /// One of the constructors of the type `declaration`: an entry of the type's key beside the type itself
  /// (`with_constructors`), which a call of the type means
  constructor: Option<&'a Constructor>,
  /// The names of the declarations it is a member of, the outermost first; for a Rust `impl` member its receiver
  containers: Vec<String>,
  /// Namespace, containers and name: what identifies it across the commits
  key: String,
  /// An older form of an entry that a use is judged by (`older_form`): its rejection makes an edge, so where the
  /// summary doesn't tell what the compiler lets through, it accepts (`private_scope`, `reaches_module`)
  older: bool,
}

impl Site<'_> {
  fn name(&self) -> &str {
    self.ctx.summary.name(self.declaration.name)
  }

  /// The parameters of a function, or of the constructor the site is
  fn signature(&self) -> Option<&Signature> {
    self.constructor.map_or(self.declaration.signature.as_ref(), |constructor| Some(&constructor.signature))
  }
}

/// One declaration of a key in the state of the range, an overload of its name or a constructor of its type, with the
/// step that made it what it is
#[derive(Clone)]
struct Entry<'a> {
  /// Its form now, in its file as the last commit that changed the key left it; of a key only present or absent, as
  /// the last commit that changed the file did (`follow_entries`), or the one before, which the parser could read
  site: Site<'a>,
  /// The file that declares it now: that of `site`, or one a commit renamed or moved it to without reading its
  /// declaration there, which the parser can't read or the commit doesn't change (`follow_entries`, the renames of
  /// `compute_symbol_dependencies`)
  file: &'a str,
  origin: Rc<Origin<'a>>,
}

impl<'a> Entry<'a> {
  /// An entry of `site`, in its file
  const fn new(site: Site<'a>, origin: Rc<Origin<'a>>) -> Self {
    Self {
      file: site.ctx.path,
      site,
      origin,
    }
  }
}

/// The step of the range that created an entry or changed its form
struct Origin<'a> {
  /// `None`: the entry was there before the range
  commit: Option<usize>,
  /// The entries of the key the commit removed, for a new entry: when it removed one and added one, that is this entry
  /// in its old form; for an entry whose form the commit changed (`same_form`), only its old form
  predecessors: Vec<Entry<'a>>,
}

/// The entries of a key the range added or changed so far, of two kinds that one key may hold together (a factory
/// `fun Foo(): Foo` beside `class Foo`). A use judges each kind as the entries of a key of its own
#[derive(Default)]
struct KeyState<'a> {
  /// Those of a JVM type, its constructors included, or of a member of one, all in the file of the type, with their
  /// forms (`has_entries`)
  entries: Vec<Entry<'a>>,
  /// Those of a key that is only present or absent, per file of the range that declares it, each with the commit that
  /// added the key there: a top-level function or property of the JVM, also beside a type of its name, and every key
  /// of Rust and TypeScript
  present: Vec<Entry<'a>>,
}

/// Name -> key -> its state
type Index<'a> = HashMap<String, BTreeMap<String, KeyState<'a>>>;

/// The arguments of a call that judge the entries it may mean, and whether one spreads an array
type CallArguments<'s> = Option<(&'s [(Option<u32>, Argument)], bool)>;

/// The declarations of a key of a type or a member (`has_entries`) on both sides of the files of a commit that change
/// it, a type's constructors included
#[derive(Default)]
struct Member<'a> {
  before: Vec<Site<'a>>,
  after: Vec<Site<'a>>,
  /// A side of one of those files is degraded
  degraded: bool,
}

/// The declarations from the one of index `start` outward, the innermost first, with their indices
fn outward(summary: &FileSummary, start: Option<u32>) -> impl Iterator<Item = (u32, &Declaration)> {
  let at = |index: u32| summary.declarations.get(index as usize).map(|declaration| (index, declaration));
  std::iter::successors(start.and_then(at), move |(_, declaration)| declaration.container.and_then(at))
}

/// The names of the declarations that `declaration` is a member of, the outermost first
fn containers_of(summary: &FileSummary, declaration: &Declaration) -> Vec<String> {
  let mut names: Vec<String> = outward(summary, declaration.container)
    .map(|(_, container)| summary.name(container.name).to_string())
    .collect();
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
    // the keys of the types and members it changes, with all their declarations on both sides
    let mut members: BTreeMap<String, Member<'_>> = BTreeMap::new();
    // the entries of keys only present or absent that move to a file of the commit (`follow_entries`)
    let mut follows: Vec<Follow<'_>> = Vec::new();
    for (change, before, after) in changed {
      let (added_lines, removed_lines) = changed_lines(change);
      match (before, after) {
        // the same namespace on both sides: a declaration added or removed lies on the changed lines of its side, so
        // only those are compared, by the declarations of the same key and kind on the other side
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
            member.before.extend(sites_of_key(before, site).into_iter().flat_map(with_constructors));
            member.after.extend(sites_of_key(after, site).into_iter().flat_map(with_constructors));
            member.degraded |= before.summary.is_degraded() || after.summary.is_degraded();
          }
          follows.extend(kept_entries(&index, before.path, after));
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
            for site in sites.into_iter().filter(has_entries).flat_map(with_constructors) {
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
    // a declaration that moved to another file of the same commit is neither added nor removed, and the entries of a
    // key only present or absent move with it
    let kind_of = |site: &Site<'_>| (site.key.clone(), has_entries(site));
    let added_keys: BTreeSet<(String, bool)> = added.iter().map(kind_of).collect();
    let removed_keys: BTreeSet<(String, bool)> = removed.iter().map(kind_of).collect();
    for (key, _) in added_keys.intersection(&removed_keys).filter(|(_, has_entries)| !has_entries) {
      let of_key = |site: &&Site<'_>| site.key == *key && !has_entries(site);
      follows.push(Follow {
        from: removed.iter().filter(of_key).map(|site| site.ctx.path).collect(),
        sites: added.iter().filter(of_key).cloned().collect(),
      });
    }
    added.retain(|site| !removed_keys.contains(&kind_of(site)));
    removed.retain(|site| !added_keys.contains(&kind_of(site)));
    for follow in follows {
      follow_entries(&mut index, follow);
    }
    // a key of a type or a member goes by its entries, but only on clean sides: a declaration the parser recovered
    // around may be misread, and two declarations of one side that are the same entry, or the constructors of a type
    // that one side knows and the other doesn't, can't be matched across the sides, so such a key, as every other, is
    // only present or absent; a removal stays one of the whole key. The entries the range gave it keep their forms
    // but those of its constructors, which may miss what the commit gave them: a call of a type needs the type then,
    // whose origin any constructor needs too, while an overload the commit adds needs no other
    members.retain(|key, member| {
      let clean = !member.degraded
        && member.before.iter().chain(&member.after).all(|site| !site.declaration.in_error)
        && !has_twins(&member.before)
        && !has_twins(&member.after)
        && knows_constructors(&member.before).is_none_or(|before| knows_constructors(&member.after).is_none_or(|after| before == after));
      if !clean
        && let Some(state) = member
          .before
          .iter()
          .chain(&member.after)
          .next()
          .and_then(|site| index.get_mut(site.name()))
          .and_then(|keys| keys.get_mut(key))
      {
        state.entries.retain(|entry| entry.site.constructor.is_none());
      }
      clean
    });
    // a function of the name of a type keeps its own entries beside those of the type
    added.retain(|site| !has_entries(site) || !members.contains_key(&site.key));

    // its own declarations first, so that its uses of them resolve to it
    let origin = Rc::new(Origin {
      commit: Some(position),
      predecessors: Vec::new(),
    });
    // a key of a type or a member comes here only when `members` dropped it, so a type is only present or absent,
    // without its constructors
    let mut fresh: BTreeMap<String, Vec<Entry<'_>>> = BTreeMap::new();
    for site in added.iter().filter(|site| is_clean_addition(site)) {
      fresh.entry(site.key.clone()).or_default().push(Entry::new(site.clone(), Rc::clone(&origin)));
    }
    for (key, entries) in fresh {
      let Some(name) = entries.first().map(|entry| entry.site.name().to_string()) else {
        continue;
      };
      // a key the range has in another file, an overload of a top-level function, keeps those entries; one of a file
      // the commit declares it in again is replaced, those of a type and those of the functions of its name each by
      // their own kind
      let state = index.entry(name).or_default().entry(key).or_default();
      let (entries, present): (Vec<Entry<'_>>, Vec<Entry<'_>>) = entries.into_iter().partition(|entry| has_entries(&entry.site));
      replace_by_file(&mut state.entries, entries);
      replace_by_file(&mut state.present, present);
    }
    for (key, member) in members {
      let Some(name) = member.before.iter().chain(&member.after).next().map(|site| site.name().to_string()) else {
        continue;
      };
      let known = index
        .get(&name)
        .and_then(|keys| keys.get(&key))
        .map(|state| state.entries.as_slice())
        .filter(|entries| !entries.is_empty());
      let Some(entries) = member_entries(known, member, position) else { continue };
      if !entries.is_empty() {
        index.entry(name).or_default().entry(key).or_default().entries = entries;
      } else if let Some(keys) = index.get_mut(&name)
        && let Some(state) = keys.get_mut(&key)
      {
        // gone from its file, before the uses of the commit; the key stays while a function of its name
        // (`fun Foo(): Foo`) has entries
        state.entries.clear();
        if state.present.is_empty() {
          keys.remove(&key);
        }
      }
    }

    // the uses on the added lines
    for (change, _, after) in changed {
      let Some(use_site) = after.as_ref().filter(|ctx| !ctx.summary.is_degraded()) else {
        continue;
      };
      let (added_lines, _) = changed_lines(change);
      for reference in use_site.summary.references.iter().filter(|reference| in_ranges(&added_lines, reference.line)) {
        let Some(candidates) = index.get(use_site.summary.name(reference.name)) else { continue };
        let Some((owner, entry, evidence)) = resolve_use(input, use_site, reference, candidates, position) else {
          continue;
        };
        edges.entry((position, owner)).or_default().push(SymbolLink {
          name: entry.site.name().to_string(),
          declared_in: entry.file.to_string(),
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
      // the whole key of a type or a member, the functions of its name aside; of a key only present or absent the
      // entries of this file, which follow their declarations (`follow_entries`), so its overloads elsewhere stay
      if let Some(keys) = index.get_mut(site.name())
        && let Some(state) = keys.get_mut(&site.key)
      {
        if has_entries(site) {
          state.entries.clear();
        } else {
          state.present.retain(|entry| entry.file != site.ctx.path);
        }
        if state.entries.is_empty() && state.present.is_empty() {
          keys.remove(&site.key);
        }
      }
    }

    // the entries still of a file the commit renames go with it in the form they had (a declaration the parser can't
    // read there, a member the commit doesn't change), so that a later commit on the file finds them (`kept_entries`)
    let renamed: HashMap<&str, &str> = changed
      .iter()
      .filter_map(|(change, _, _)| change.old_path.as_deref().filter(|old| *old != change.path).map(|old| (old, change.path.as_str())))
      .collect();
    if !renamed.is_empty() {
      for entry in index
        .values_mut()
        .flat_map(BTreeMap::values_mut)
        .flat_map(|state| state.entries.iter_mut().chain(&mut state.present))
      {
        if let Some(&file) = renamed.get(entry.file) {
          entry.file = file;
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

/// What `reference`, on an added line of `use_site` of the commit `position`, needs of the `candidates` of its name:
/// the commit, with the entry of the declaration it names and the evidence; `None` when it needs no older commit or is
/// ambiguous
fn resolve_use<'s>(
  input: &SymbolInput,
  use_site: &UseSite<'_>,
  reference: &Reference,
  candidates: &'s BTreeMap<String, KeyState<'s>>,
  position: usize,
) -> Option<(usize, &'s Entry<'s>, Evidence)> {
  // the names of the qualifier once, not once per candidate
  let qualifier = reference.qualifier.as_deref().map(|qualifier| names(use_site.summary, qualifier));
  let shape = reference.shape.and_then(|shape| use_site.summary.shapes.get(shape as usize));
  let unqualified = bare(qualifier.as_deref());
  // a call in a Kotlin lambda, an object expression, an anonymous class or a local declaration, whose receiver may be
  // what a bare name or `this` means
  let enclosed = matches!(shape, Some(Shape::Call { enclosed: true, .. }));
  let arguments: CallArguments<'_> = match shape {
    Some(Shape::Call { arguments, spread, .. }) => Some((arguments.as_slice(), *spread)),
    _ => None,
  };
  // an extension of the receiver's type that the use sees may take a call its member rejects: no receiver evidence
  let extension_may_take = std::cell::OnceCell::new();
  let extension_may_take = || {
    *extension_may_take.get_or_init(|| {
      receiver_name(use_site.summary, reference).is_some_and(|receiver| {
        candidates
          .values()
          .flat_map(|state| state.entries.iter().chain(&state.present))
          .any(|entry| sees_extension(use_site, &entry.site, receiver))
      })
    })
  };
  // per entry it may mean, the commits it needs of it (`lineage`)
  let mut alternatives: Vec<BTreeSet<Option<usize>>> = Vec::new();
  let mut first: Option<(&Entry<'_>, Evidence)> = None;
  for state in candidates.values() {
    // the arguments that tell the constructors of a type apart, and with them the functions of its name
    let constructor_call = arguments.filter(|_| {
      state.entries.iter().any(|entry| entry.site.constructor.is_some()) && state.entries.first().is_some_and(|entry| constructors_tell_apart(use_site, reference, &entry.site))
    });
    // the entries of a type or a member, and those of the functions of its name, each kind as a key of its own
    for (entries, functions) in [(state.entries.as_slice(), state.present.as_slice()), (state.present.as_slice(), &[][..])] {
      let Some(head) = entries.first().map(|entry| &entry.site) else { continue };
      // through a wildcard import, which a type of the name in the use's package may shadow: the type as it came
      let star = head.declaration.kind == DeclKind::Type && resolve(input, use_site, reference, qualifier.as_deref(), head) == Some(Evidence::StarImport);
      // a call of a type means one of its constructors, where its key has them (`with_constructors`)
      let constructs = arguments.is_some() && !star && entries.iter().any(|entry| entry.site.constructor.is_some());
      // the arguments that tell the overloads of a member apart
      let member_call = arguments.filter(|_| calls_tell_apart(use_site, reference, head, enclosed));
      // the functions of the type's name that the use sees and that accept the call, which the type stands for
      let stand_ins: Vec<&Entry<'_>> = functions
        .iter()
        .filter(|function| sees_function(input, use_site, reference, qualifier.as_deref(), &function.site))
        .filter(|function| constructor_call.is_none_or(|(arguments, spread)| accepts_call(use_site.summary, arguments, spread, &function.site)))
        .collect();
      // in the file of a key of the baseline only a use that can mean only a member of its type (`means_its_type`)
      let same_file = use_site.path != head.ctx.path || !of_baseline(entries) || (unqualified && means_its_type(use_site.summary, reference, enclosed, head));
      let accepts = |site: &Site<'_>, call: CallArguments<'_>| {
        if !takes(reference, constructs, !stand_ins.is_empty(), use_site.summary, constructor_call, site) {
          return None;
        }
        let evidence = resolve(input, use_site, reference, qualifier.as_deref(), site).filter(|it| *it != Evidence::Receiver || !extension_may_take())?;
        let fits = constructor_reaches(use_site, reference, site) && call.is_none_or(|(arguments, spread)| accepts_call(use_site.summary, arguments, spread, site));
        (fits && (same_file || evidence != Evidence::SameFile)).then_some(evidence)
      };
      // an older form (`older_form`) rejects only a use that nothing the range doesn't show may take instead; any other
      // use of a function or a constructor needs the key
      let before = |current: &Site<'_>, call: CallArguments<'_>, judged: bool, older: &Site<'_>| {
        star
          || (!judged && (older.constructor.is_some() || older.declaration.kind == DeclKind::Function))
          || older_form(older, current).is_none_or(|declaration| {
            let site = Site {
              declaration: &declaration,
              older: true,
              ..older.clone()
            };
            accepts(&site, call).is_some()
          })
      };
      let mut needs: Vec<BTreeSet<Option<usize>>> = Vec::new();
      let mut keyed: Vec<BTreeSet<Option<usize>>> = Vec::new();
      for entry in entries {
        // a constructor, and a function of the name of a type, by the arguments that tell its constructors apart
        let call = if entry.site.constructor.is_some() || !has_entries(&entry.site) {
          constructor_call
        } else {
          member_call
        };
        // an import of a member of a type whose overloads are all its entries compiles only when one is visible
        let judged = call.is_some() || matches!(shape, Some(Shape::Override { .. })) || (reference.role == Role::Import && all_entries(use_site, &entry.site));
        let Some(evidence) = accepts(&entry.site, call) else { continue };
        // a type that takes a call only for the functions of its name needs what one of those needs
        if constructs && entry.site.constructor.is_none() && !invokes(use_site.summary, constructor_call, &entry.site) {
          if let Some(function) = stand_ins.first() {
            first.get_or_insert((function, evidence));
          }
          needs.extend(stand_ins.iter().map(|function| lineage(function)));
          continue;
        }
        let mut steps = steps_of(entry, &|older| before(&entry.site, call, judged, older));
        // no origin: a bare name outside the type of a member its own commit adds means it only by an implicit
        // receiver, which the file doesn't show
        if evidence == Evidence::SameFile && reference.qualifier.is_none() && !contains(use_site.summary, reference, &entry.site.containers) {
          steps.retain(|step| step.origin.commit != Some(position));
        }
        if steps.is_empty() {
          continue;
        }
        first.get_or_insert((entry, evidence));
        let lineages = steps.into_iter().map(lineage);
        if judged || !has_entries(&entry.site) {
          needs.extend(lineages);
        } else {
          keyed.extend(lineages);
        }
      }
      // a use that tells no overloads apart needs the key: the oldest step of the entries that accept it
      needs.extend(keyed.into_iter().min_by_key(|lineage| lineage.last().copied()));
      alternatives.extend(needs);
    }
  }
  // the newest commit before its own that every entry it may mean needs: none (the baseline), or two entries that need
  // no commit in common, is no edge
  let common = alternatives.into_iter().reduce(|common, lineage| &common & &lineage)?;
  let &Some(owner) = common.range(..Some(position)).next_back()? else {
    return None;
  };
  let (entry, evidence) = first?;
  Some((owner, entry, evidence))
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
    constructor: None,
    containers,
    key,
    older: false,
  }
}

/// Whether the key of `site` keeps all its declarations as entries: a JVM type and a member of one, whose overloads
/// and constructors are all in the file of the type, as a package has one type of a name. A top-level function or
/// property may have overloads in files the range doesn't change, so its key, as every key of Rust and TypeScript, is
/// only present or absent, also beside a type of its name (`KeyState`)
fn has_entries(site: &Site<'_>) -> bool {
  site.ctx.language.family() == Family::Jvm && (!site.containers.is_empty() || site.declaration.kind == DeclKind::Type)
}

/// The entries of a declaration: itself, and for a class (`Declaration::constructors`) one per constructor beside it
fn with_constructors(site: Site<'_>) -> Vec<Site<'_>> {
  let mut sites: Vec<Site<'_>> = site
    .declaration
    .constructors
    .iter()
    .flatten()
    .map(|constructor| Site {
      constructor: Some(constructor),
      ..site.clone()
    })
    .collect();
  sites.insert(0, site);
  sites
}

/// Replaces the entries of `known` of each file of `entries` by those
fn replace_by_file<'a>(known: &mut Vec<Entry<'a>>, entries: Vec<Entry<'a>>) {
  known.retain(|entry| entries.iter().all(|new| new.file != entry.file));
  known.extend(entries);
}

/// The declarations of `ctx` with the key of `site` and of its kind (`has_entries`): a function of the name of a type
/// beside it (`fun Foo(): Foo`) is a key of its own (`KeyState`)
fn sites_of_key<'a>(ctx: &'a FileCtx<'a>, site: &Site<'_>) -> Vec<Site<'a>> {
  let Some(name) = ctx.summary.name_id(site.name()) else { return Vec::new() };
  ctx
    .summary
    .declarations
    .iter()
    .filter(|declaration| declaration.name == name)
    .map(|declaration| site_of(ctx, declaration))
    .filter(|other| other.key == site.key && has_entries(other) == has_entries(site))
    .collect()
}

/// Whether a declaration a commit adds may be an entry: not misread (a degraded file, a parse error around it) and
/// visible to other files (`is_exported`)
fn is_clean_addition(site: &Site<'_>) -> bool {
  !site.ctx.summary.is_degraded() && !site.declaration.in_error && is_exported(site.declaration, site.ctx.summary)
}

/// The entries of a key only present or absent that move to the declarations of their key in another version of their
/// file, or in another file (`follow_entries`)
struct Follow<'a> {
  /// The files the entries are of
  from: BTreeSet<&'a str>,
  /// The declarations of the key where they move to, also those the parser can't read (`is_clean_addition`)
  sites: Vec<Site<'a>>,
}

/// The entries of the keys only present or absent of the file `from` that its new version `after` keeps, a renamed
/// one included: they move to the declarations of their keys there, readable or not, whose signatures a call is
/// judged by
fn kept_entries<'a>(index: &Index<'a>, from: &'a str, after: &'a FileCtx<'a>) -> Vec<Follow<'a>> {
  let mut sites: BTreeMap<String, Vec<Site<'a>>> = BTreeMap::new();
  for declaration in &after.summary.declarations {
    let Some(keys) = index.get(after.summary.name(declaration.name)) else { continue };
    let site = site_of(after, declaration);
    let held = keys.get(&site.key).is_some_and(|state| state.present.iter().any(|entry| entry.file == from));
    if held && !has_entries(&site) {
      sites.entry(site.key.clone()).or_default().push(site);
    }
  }
  sites
    .into_values()
    .map(|sites| Follow {
      from: BTreeSet::from([from]),
      sites,
    })
    .collect()
}

/// Moves the entries of the files of `follow` to its declarations, each with its origin, as a key only present or
/// absent has no forms: to a declaration of its identity (`same_entry`), else to one of an identity none of them has,
/// in order (a changed signature). A declaration left over is as old as the oldest of them, as the key there is; an
/// entry left over keeps its form in each file of the declarations, as all do where the parser can't read those
/// (`is_clean_addition`): one may be its own, and two origins stay two, also when a commit moves the overloads of two
/// files into one
fn follow_entries<'a>(index: &mut Index<'a>, follow: Follow<'a>) {
  let Some(first) = follow.sites.first() else { return };
  let Some(state) = index.get_mut(first.name()).and_then(|keys| keys.get_mut(&first.key)) else {
    return;
  };
  let (mut moving, kept): (Vec<Entry<'a>>, Vec<Entry<'a>>) = std::mem::take(&mut state.present).into_iter().partition(|entry| follow.from.contains(entry.file));
  state.present = kept;
  let Some(oldest) = moving.iter().map(|entry| &entry.origin).min_by_key(|origin| origin.commit).map(Rc::clone) else {
    return;
  };
  let files: BTreeSet<&'a str> = follow.sites.iter().map(|site| site.ctx.path).collect();
  let mut unmatched: Vec<Site<'a>> = Vec::new();
  for site in follow.sites.into_iter().filter(is_clean_addition) {
    match moving.iter().position(|entry| same_entry(&entry.site, &site)) {
      Some(found) => state.present.push(Entry::new(site, moving.remove(found).origin)),
      None => unmatched.push(site),
    }
  }
  let mut left = moving.into_iter();
  for site in unmatched {
    let origin = left.next().map_or_else(|| Rc::clone(&oldest), |entry| entry.origin);
    state.present.push(Entry::new(site, origin));
  }
  for entry in left {
    state.present.extend(files.iter().map(|&file| Entry { file, ..entry.clone() }));
  }
}

/// The entries of a key of a type or a member after a commit that changes it, from its declarations before and after
/// the commit; an entry of the same identity (`same_entry`) and form (`same_form`) on both sides keeps its origin,
/// whatever its body became, and one of the same identity in a new form has the commit for its origin and its old form
/// for its predecessor; an entry `known` lacks was there before the range, but a constructor of a type the index
/// knows, which a commit gave it while the key was only present or absent (an unclean commit), is as old as its
/// type (`oldest_step`), which it needs. A new entry has the commit for its origin and every entry the commit removed
/// for its predecessors. Empty when the key keeps no entry, `None` when nothing changed in a key the index doesn't know
fn member_entries<'a>(known: Option<&[Entry<'a>]>, member: Member<'a>, commit: usize) -> Option<Vec<Entry<'a>>> {
  let mut unmatched: Vec<&Entry<'a>> = known.unwrap_or_default().iter().collect();
  let baseline = Rc::new(Origin {
    commit: None,
    predecessors: Vec::new(),
  });
  let created = known.and_then(oldest_step).map_or_else(|| Rc::clone(&baseline), |step| Rc::clone(&step.origin));
  let mut gone: Vec<Entry<'a>> = member
    .before
    .into_iter()
    .map(|site| {
      let origin = match unmatched.iter().position(|entry| same_entry(&entry.site, &site)) {
        Some(found) => Rc::clone(&unmatched.swap_remove(found).origin),
        None if site.constructor.is_some() => Rc::clone(&created),
        None => Rc::clone(&baseline),
      };
      Entry::new(site, origin)
    })
    .collect();
  let mut entries = Vec::new();
  let mut new = Vec::new();
  let mut reformed = false;
  for site in member.after {
    match gone.iter().position(|entry| same_entry(&entry.site, &site)) {
      Some(found) if same_form(&gone[found].site, &site) => entries.push(Entry::new(site, gone.swap_remove(found).origin)),
      Some(found) => {
        reformed = true;
        let origin = Rc::new(Origin {
          commit: Some(commit),
          predecessors: vec![gone.swap_remove(found)],
        });
        entries.push(Entry::new(site, origin));
      }
      None => new.push(site),
    }
  }
  if known.is_none() && new.is_empty() && gone.is_empty() && !reformed {
    return None;
  }
  let origin = Rc::new(Origin {
    commit: Some(commit),
    predecessors: gone,
  });
  entries.extend(new.into_iter().map(|site| Entry::new(site, Rc::clone(&origin))));
  Some(entries)
}

/// Whether the type among the declarations of one side knows its constructors (`Declaration::constructors`), which a
/// parse error in a class, one that may have swallowed a constructor, makes unknown; `None` without a type
fn knows_constructors(sites: &[Site<'_>]) -> Option<bool> {
  sites
    .iter()
    .find(|site| site.constructor.is_none() && site.declaration.kind == DeclKind::Type)
    .map(|site| site.declaration.constructors.is_some())
}

/// Whether two declarations of one side are the same entry (`same_entry`): `foo(a.Date)` beside `foo(b.Date)`
fn has_twins(sites: &[Site<'_>]) -> bool {
  sites.iter().enumerate().any(|(index, a)| sites.iter().skip(index + 1).any(|b| same_entry(a, b)))
}

/// Whether two declarations of one key are the same entry: the same kind, extension receiver and parameter types; a
/// constructor is no entry of the type itself
fn same_entry(a: &Site<'_>, b: &Site<'_>) -> bool {
  let same_type = |x: Option<&TypeRef>, y: Option<&TypeRef>| match (x, y) {
    (Some(x), Some(y)) => x.nullable == y.nullable && same_names(a.ctx.summary, x, b.ctx.summary, y),
    (x, y) => x.is_none() && y.is_none(),
  };
  a.declaration.kind == b.declaration.kind
    && a.constructor.is_some() == b.constructor.is_some()
    && receiver_of(a) == receiver_of(b)
    && same_parameters(a, b, |x, y| same_type(x.type_ref.as_ref(), y.type_ref.as_ref()))
}

/// Whether both have no signature, or signatures of as many parameters, each pair `same`
fn same_parameters(a: &Site<'_>, b: &Site<'_>, same: impl Fn(&Parameter, &Parameter) -> bool) -> bool {
  match (a.signature(), b.signature()) {
    (Some(x), Some(y)) => x.parameters.len() == y.parameters.len() && x.parameters.iter().zip(&y.parameters).all(|(x, y)| same(x, y)),
    (x, y) => x.is_none() && y.is_none(),
  }
}

/// Whether two declarations of one entry (`same_entry`) have the same form: the names of their parameters, which a
/// named argument needs, their defaults and varargs, which decide how many arguments a call may pass, their reach,
/// whether a subtype may override them and whether they are `@JvmStatic`, which a Java call by the class around a
/// companion needs (`static_paths`); a constructor's reach is its own and that of its type, which a call needs both of.
/// Whether a member is static, and whether it overrides, are no form: what they reject (`Owner.member` of an instance
/// member, an override of an override) may compile all the same
fn same_form(a: &Site<'_>, b: &Site<'_>) -> bool {
  let parameters = same_parameters(a, b, |x, y| {
    (a.ctx.summary.name(x.name), x.optional, x.vararg) == (b.ctx.summary.name(y.name), y.optional, y.vararg)
  });
  let reach = |site: &Site<'_>| (site.declaration.reach, site.constructor.map(|constructor| constructor.reach));
  let modality = |site: &Site<'_>| (site.declaration.overridable, site.declaration.jvm_static);
  parameters && reach(a) == reach(b) && modality(a) == modality(b)
}

/// The declaration that an older form `older` of the entry of `current` is judged as: the attributes of its form
/// (`same_form`) as they were, the rest as they are now, since what they reject may compile all the same: whether it is
/// static (`Outer.Inner` of an inner class names it as a type, which no qualified use of an instance member does) and
/// whether it overrides (an override of an override compiles without the member). `None`, which accepts, when the type
/// around it had supertypes then and has none now or the other way round, which an override is judged by
/// (`resolve_override`)
fn older_form(older: &Site<'_>, current: &Site<'_>) -> Option<Declaration> {
  (has_supertypes(older) == has_supertypes(current)).then(|| Declaration {
    member: current.declaration.member,
    overrides: current.declaration.overrides,
    ..older.declaration.clone()
  })
}

/// The declaration of the type that declares the member `site`, in the file of `site`
fn container_of<'a>(site: &Site<'a>) -> Option<&'a Declaration> {
  site.declaration.container.and_then(|index| site.ctx.summary.declarations.get(index as usize))
}

/// Whether the type that declares the member `site` names any supertype, `Any` or `Object` included
fn has_supertypes(site: &Site<'_>) -> bool {
  container_of(site).is_some_and(|container| !container.supertypes.is_empty())
}

/// The extension receiver of a declaration, its last name
fn receiver_of<'a>(site: &Site<'a>) -> Option<&'a str> {
  site.declaration.receiver.map(|receiver| site.ctx.summary.name(receiver))
}

/// Whether two types, each of its own file, have the same last names and array dimensions
fn same_names(a: &FileSummary, x: &TypeRef, b: &FileSummary, y: &TypeRef) -> bool {
  x.arrays == y.arrays && x.names.len() == y.names.len() && x.names.iter().zip(&y.names).all(|(x, y)| a.name(*x) == b.name(*y))
}

/// The steps a use may need of `entry`, which accepts it, each as the entry in the form that step gave it: back along
/// the forms of the entry, the latest step whose forms before (its predecessors) all reject the use while the form
/// after accepts it, or the baseline; each predecessor that accepts it leads further back, to a step the use may need
/// instead. `accepts` errs toward accepting, so a form it can't judge takes the step further back, toward no edge
fn steps_of<'e, 'a>(entry: &'e Entry<'a>, accepts: &dyn Fn(&Site<'a>) -> bool) -> Vec<&'e Entry<'a>> {
  let mut stack = vec![entry];
  let mut seen: HashSet<*const Origin<'a>> = HashSet::new();
  let mut steps = Vec::new();
  while let Some(entry) = stack.pop() {
    if !seen.insert(Rc::as_ptr(&entry.origin)) {
      continue;
    }
    let before: Vec<&Entry<'a>> = match entry.origin.commit {
      Some(_) => entry.origin.predecessors.iter().filter(|predecessor| accepts(&predecessor.site)).collect(),
      None => Vec::new(),
    };
    if before.is_empty() {
      steps.push(entry);
    } else {
      stack.extend(before);
    }
  }
  steps
}

/// The commits a step needs (`None`: the baseline): its own, and back along the forms of its entry those of the steps
/// whose form it changed, to the one that created the entry. A new form changes a declaration that a commit before it
/// made, so a use that two entries accept, one in a form a later commit gave it, needs the commit both come from
fn lineage(step: &Entry<'_>) -> BTreeSet<Option<usize>> {
  let mut commits = BTreeSet::new();
  let mut origin = &step.origin;
  loop {
    commits.insert(origin.commit);
    match origin.predecessors.as_slice() {
      [only] => origin = &only.origin,
      _ => return commits,
    }
  }
}

/// The step that created a key: back along the forms of all its entries, one of the baseline if any, else the one of
/// the oldest commit
fn oldest_step<'e, 'a>(entries: &'e [Entry<'a>]) -> Option<&'e Entry<'a>> {
  entries.iter().flat_map(|entry| steps_of(entry, &|_| true)).min_by_key(|step| step.origin.commit)
}

/// Whether a key was there before the range: an entry of it, or a form an entry had before, is of the baseline
fn of_baseline(entries: &[Entry<'_>]) -> bool {
  oldest_step(entries).is_some_and(|step| step.origin.commit.is_none())
}

/// The supertypes a type has without naming them
const ROOT_TYPES: [&[&str]; 4] = [&["Any"], &["kotlin", "Any"], &["Object"], &["java", "lang", "Object"]];

/// Whether a type of `summary` has a supertype other than `Any` or `Object`, whose members the range doesn't show
fn extends(summary: &FileSummary, declaration: &Declaration) -> bool {
  let root = |path: &[u32]| ROOT_TYPES.iter().any(|root| path.iter().map(|&segment| summary.name(segment)).eq(root.iter().copied()));
  declaration.supertypes.iter().any(|path| !root(path))
}

/// Whether the entries of the member `site` are all the declarations of its name on its type: not of a type with a
/// supertype other than `Any` or `Object`, which may inherit more that the range doesn't show (`Base.buttons(Long)`
/// takes `buttons(1)`), nor of a name every class has (`ROOT_MEMBERS`)
fn all_entries(use_site: &UseSite<'_>, site: &Site<'_>) -> bool {
  has_entries(site) && !ROOT_MEMBERS.contains(&site.name()) && !inherits_unseen(use_site, site)
}

/// Whether the arguments of a call may tell the overloads of the member `site` apart: only where all of them are its
/// entries (`all_entries`), and of a bare name of another file only where its file shows nothing that comes before
/// the import that brings it in (`import_decides`) or before the supertype it inherits it from (`inherited`)
fn calls_tell_apart(use_site: &UseSite<'_>, reference: &Reference, site: &Site<'_>, enclosed: bool) -> bool {
  all_entries(use_site, site)
    && (reference.qualifier.is_some()
      || use_site.path == site.ctx.path
      || import_decides(use_site.summary, reference, enclosed)
      || inherited(use_site, reference, None, site)
      || through_receiver(use_site, reference, None, site))
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
  if outward(summary, reference.container).any(|(_, declaration)| declaration.receiver.is_some()) {
    return false;
  }
  !receivers(summary, reference).iter().any(|(_, declaration)| extends(summary, declaration))
}

/// Whether the arguments of a call of the type `site` tell its constructors apart. Constructors are not inherited, so
/// no supertype declares more of them, and Java's `new` means one of the type its name resolves to; a Kotlin call falls
/// back from them to the functions of the name (a constructor invocation of a supertype doesn't, but the summary
/// doesn't tell one from a call). So a bare name must mean only the type among what its file shows
/// (`names_only_its_type`), in Kotlin brought in by at most one import from another package
fn constructors_tell_apart(use_site: &UseSite<'_>, reference: &Reference, site: &Site<'_>) -> bool {
  let summary = use_site.summary;
  reference.qualifier.is_some()
    || names_only_its_type(
      summary,
      summary.name(reference.name),
      use_site.path == site.ctx.path,
      usize::from(use_site.namespace != site.ctx.namespace),
    )
}

/// Whether a bare name of a type can mean only that type among what the file of `summary` shows: no declaration of the
/// name but the type itself (`own`, in the type's file), which a nested type of the name, and in Kotlin a function, a
/// factory `fun Foo(): Foo` included, or a property would be, and in Kotlin no import that may bring a function of the
/// name but the `imports` that bring the type (`imports_of_function`: an import of the name, a wildcard import). A
/// member of a receiver of the name, an inherited nested type or a function of the receiver's, is left out, as a type
/// and a function are rarely named alike (a function of a type's name is its factory), and so is a function of the name
/// in a file of the package that the range doesn't change (one that it adds and that accepts the call lets the type
/// take it, `takes`): limits
fn names_only_its_type(summary: &FileSummary, name: &str, own: bool, imports: usize) -> bool {
  let declared = summary.name_id(name).map_or(0, |id| {
    summary
      .declarations
      .iter()
      .filter(|declaration| declaration.name == id && declaration.kind != DeclKind::Binding)
      .count()
  });
  declared == usize::from(own) && (summary.language != Language::Kotlin || imports_of_function(summary, name) <= imports)
}

/// Whether a use may mean the entry `site` by what it is: a call of a type with constructors among the entries of its
/// kind (`constructs`; none while its key is only present or absent) means one of them, or a function of its name that
/// the range adds and that accepts the call (`stands_in`), as no evidence links such a function from another file of
/// the package, or an `invoke` of its companion that accepts it (`invokes`), for which the type itself stands; any other
/// use of a type means the type itself; a JVM type position (`: Foo`, an annotation) means no function or value of its
/// name
fn takes(reference: &Reference, constructs: bool, stands_in: bool, use_summary: &FileSummary, call: CallArguments<'_>, site: &Site<'_>) -> bool {
  let declaration = site.declaration;
  if reference.role == Role::Type && site.ctx.language.family() == Family::Jvm && matches!(declaration.kind, DeclKind::Function | DeclKind::Value | DeclKind::Constant) {
    return false;
  }
  match site.constructor {
    Some(_) => constructs,
    None => !constructs || stands_in || invokes(use_summary, call, site),
  }
}

/// Whether a Kotlin call of a type may mean the function `site` of its name, which the type then stands for: one that
/// the use sees by the evidence of a function (an import, its qualified name, its file), or, as no evidence links a
/// function from another file of the package, one of the package in the module of the use that its reach lets in. A
/// private one of another file, one of another module or language, and any for a Java `new` are none
fn sees_function(input: &SymbolInput, use_site: &UseSite<'_>, reference: &Reference, qualifier: Option<&[String]>, site: &Site<'_>) -> bool {
  use_site.language == Language::Kotlin
    && site.ctx.language == Language::Kotlin
    && (resolve(input, use_site, reference, qualifier, site).is_some()
      || (!matches!(site.declaration.reach, Reach::File | Reach::Container)
        && reaches_module(use_site, site, site.declaration.reach)
        && use_site.namespace == site.ctx.namespace
        && use_site.module_root.is_some()
        && use_site.module_root == site.ctx.module_root))
}

/// Whether a Kotlin call of the type `site` may mean an `invoke` of its companion instead of a constructor: its file
/// declares a function `invoke` in a type nested in a type of its name, or an extension of a `Companion`
/// (`operator fun Foo.Companion.invoke()`), that accepts the arguments that tell the constructors apart (`call`). One of
/// another file is a limit, and so is the commit it comes from: the type stands for it with its own steps
fn invokes(use_summary: &FileSummary, call: CallArguments<'_>, site: &Site<'_>) -> bool {
  let summary = site.ctx.summary;
  let container = |declaration: &Declaration| declaration.container.and_then(|index| summary.declarations.get(index as usize));
  site.ctx.language == Language::Kotlin
    && summary.name_id("invoke").is_some_and(|invoke| {
      summary.declarations.iter().any(|declaration| {
        declaration.name == invoke
          && declaration.kind == DeclKind::Function
          && (declaration.receiver.is_some_and(|receiver| summary.name(receiver) == "Companion")
            || container(declaration).and_then(container).is_some_and(|outer| summary.name(outer.name) == site.name()))
          && call.is_none_or(|(arguments, spread)| accepts_call(use_summary, arguments, spread, &site_of(site.ctx, declaration)))
      })
    })
}

/// Whether the constructor of `site`, if it is one, reaches the use, as a member of its class: a private one its class
/// in Kotlin and the top-level class around it in Java, an `internal` or package-private one by `reaches_module`. An
/// `internal` one of a file or a use whose module no build file tells reaches it, as its type does: the type stands for
/// no call, so the rule of a member, which rejects the use then, would lose the edge of the type
fn constructor_reaches(use_site: &UseSite<'_>, reference: &Reference, site: &Site<'_>) -> bool {
  let Some(constructor) = site.constructor else { return true };
  match constructor.reach {
    Reach::File | Reach::Container => {
      let class: Vec<String> = site.containers.iter().cloned().chain([site.name().to_string()]).collect();
      let scope = if site.ctx.language == Language::Java { &class[..1] } else { class.as_slice() };
      use_site.path == site.ctx.path && contains(use_site.summary, reference, scope)
    }
    Reach::Module if site.ctx.language == Language::Kotlin && (use_site.module_root.is_none() || site.ctx.module_root.is_none()) => true,
    reach => reaches_module(use_site, site, reach),
  }
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
/// innermost class with a member of the name. A top-level type `site` is meant by a bare name anywhere in its file that
/// nothing else of the file may take (`names_only_its_type`)
fn means_its_type(summary: &FileSummary, reference: &Reference, enclosed: bool, site: &Site<'_>) -> bool {
  if enclosed {
    return false;
  }
  let Some((own, outer)) = site.containers.split_last() else {
    // a top-level type, by a bare name that nothing else of the file may take
    return site.declaration.kind == DeclKind::Type && reference.qualifier.is_none() && names_only_its_type(summary, site.name(), true, 0);
  };
  // the innermost type around it, unless an extension receiver comes first
  let Some((_, innermost)) = outward(summary, reference.container)
    .find(|(_, declaration)| matches!(declaration.kind, DeclKind::Type | DeclKind::EnumEntry) || declaration.receiver.is_some())
    .filter(|(_, declaration)| matches!(declaration.kind, DeclKind::Type | DeclKind::EnumEntry))
  else {
    return false;
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
  let mut receivers: Vec<(u32, &Declaration)> = outward(summary, reference.container)
    .filter(|(_, declaration)| declaration.kind == DeclKind::Type)
    .collect();
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

/// Whether the type that declares the member `site` has a supertype other than `Any` or `Object`, whose members the
/// range doesn't show: in the file of the member, or in that file at the commit of `use_site`, which may have changed
/// the supertypes since
fn inherits_unseen(use_site: &UseSite<'_>, site: &Site<'_>) -> bool {
  if container_of(site).is_some_and(|declaration| extends(site.ctx.summary, declaration)) {
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
  let Some(signature) = site.signature() else { return true };
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
  let renamed_container = |declaration: &Declaration| outward(side.summary, declaration.container).any(|(_, container)| on_changed_lines(container.name_line, container.name_line));
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
    // the extraction drops locals; a private member is used only in the containers that see it (`private_scope`), and
    // neither a name qualified by another type nor an extension called on anything but `this` is a use of it
    if names_another_type(use_site.summary, qualifier, site) || on_unknown_receiver(qualifier, site) || of_another_receiver(use_site.summary, reference, site) {
      return None;
    }
    return match site.declaration.reach {
      Reach::Container => contains(use_site.summary, reference, private_scope(site, qualifier)).then_some(Evidence::SameFile),
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

/// Whether a use is on a receiver of a type the summary knows (`Reference::receiver_type`) other than the containers
/// and the extension receiver of `site` and a type of the file that extends them: `migration.awaitMigrated()` on a
/// `LegacySettingsMigration` is no use of an `awaitMigrated` of the file's own class, `s.foo()` on a `Sub : Base` of
/// the file is one of its `Base.foo`
fn of_another_receiver(summary: &FileSummary, reference: &Reference, site: &Site<'_>) -> bool {
  let Some(last) = reference.receiver_type.as_deref().and_then(<[u32]>::last).map(|it| summary.name(*it)) else {
    return false;
  };
  let own = |name: &str| site.containers.last().is_some_and(|container| container == name) || receiver_of(site) == Some(name);
  // a subtype of the file whose one supertype names the member's type is a receiver of that type too
  let subtype = || {
    summary.declarations.iter().any(|declaration| {
      declaration.kind == DeclKind::Type
        && summary.name(declaration.name) == last
        && declaration.supertypes.iter().any(|supertype| supertype.last().is_some_and(|it| own(summary.name(*it))))
    })
  };
  !own(last) && !subtype()
}

/// Whether a use with `qualifier` calls the JVM extension `site` (a declaration with an extension receiver) on anything
/// but a bare name, `this`, `super` (`bare`) or its receiver's type (`Scope::openGlobalPrompt`): an expression, a local
/// or another name, as `prompt.openGlobalPrompt()` on a `val prompt = Driver(…)` beside a `fun Scope.openGlobalPrompt()`.
/// A member of the receiver's type wins over an extension in Kotlin, and only a compiler knows that type
fn on_unknown_receiver(qualifier: Option<&[String]>, site: &Site<'_>) -> bool {
  let names_receiver = qualifier.and_then(<[String]>::last).is_some_and(|last| receiver_of(site) == Some(last.as_str()));
  site.ctx.language.family() == Family::Jvm && site.declaration.receiver.is_some() && !bare(qualifier) && !names_receiver
}

/// Whether a use with `qualifier` is one by a bare name, `this.name` or `super.name`: of a member of the type around it
fn bare(qualifier: Option<&[String]>) -> bool {
  qualifier.is_none_or(|qualifier| matches!(qualifier, [only] if only == "this" || only == "super"))
}

/// The containers whose bodies see a private member `site` by a use with `qualifier`: in Java the whole top-level
/// class, nested classes included, for a qualified use (`Inner.helper()`, `inner.helper()`), but a bare name outside
/// the member's own type means a member of the type around it, an inherited one or a static import, never the private
/// member of a nested class; in Kotlin its own, but the class around a companion (`private const val TAG` of a
/// companion is the class's). An older form's (`Site::older`) rejection makes an edge, so it must not be stricter than
/// the compiler: in Java it reaches the whole top-level class for a bare name too, and since the summary doesn't tell a
/// Kotlin companion with a name of its own from a nested object or class, whose private members the class around
/// doesn't see, in Kotlin the class around any nested type that isn't inner
fn private_scope<'s>(site: &'s Site<'_>, qualifier: Option<&[String]>) -> &'s [String] {
  let containers = site.containers.as_slice();
  if site.ctx.language == Language::Java {
    return if site.older || !bare(qualifier) {
      &containers[..containers.len().min(1)]
    } else {
      containers
    };
  }
  // a companion by its name, and for an older form any nested type that isn't inner
  let companion = |last: &str| last == "Companion" || (site.older && container_of(site).is_some_and(|container| container.member == MemberKind::Static));
  match containers.split_last() {
    Some((last, outer)) if !outer.is_empty() && companion(last) => outer,
    _ => containers,
  }
}

/// Whether `reference` is inside a declaration of `containers` (the innermost container chain of its file)
fn contains(summary: &FileSummary, reference: &Reference, containers: &[String]) -> bool {
  let Some(last) = containers.last() else { return true };
  outward(summary, reference.container).any(|(_, declaration)| summary.name(declaration.name) == last)
}

/// The names of `segments` in `summary`
fn names(summary: &FileSummary, segments: &[u32]) -> Vec<String> {
  segments.iter().map(|&segment| summary.name(segment).to_string()).collect()
}

/// Names every class has from `Any` or `Object`: an override of one needs no commit, and a call of one may mean theirs
const ROOT_MEMBERS: [&str; 5] = ["equals", "hashCode", "toString", "clone", "finalize"];

/// The evidence that an `Override` reference overrides the member `site`. The member must be one a subtype overrides
/// (an instance function or property, `overridable`, of the same receiver and parameter types, `override_fits`), not
/// one every class has (`ROOT_MEMBERS`), and must override nothing itself, else the supertype's supertype has it
/// already; its type must be the one the reference's supertype path names (`supertype_resolves`). No evidence in a
/// class with several supertypes, whose member may come from any, nor for an override of a Java method whose class has
/// supertypes: that method may itself override one without `@Override`
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
    || !reaches_module(use_site, site, declaration.reach)
  {
    return None;
  }
  // the members of the supertypes of the member's type are unknown, and the override may mean one of them
  let inherits = has_supertypes(site);
  if site.ctx.language == Language::Java && inherits {
    return None;
  }
  // one reference per supertype of the class around the override
  let supertypes = summary
    .references_to(summary.name(reference.name))
    .filter(|other| other.role == Role::Override && other.line == reference.line && other.container == reference.container && other.shape == reference.shape)
    .count();
  if supertypes > 1 || receiver.map(|it| summary.name(it)) != receiver_of(site) || !override_fits(use_site, parameters, site, inherits) {
    return None;
  }

  supertype_resolves(use_site, reference, qualifier?, site).then_some(Evidence::Override)
}

/// Whether the supertype `path` of a class around `reference` names the type that declares the member `site`: the
/// same file, the full path, an import (an alias, a nested type), or the same package in the same module without a
/// type of that name in the file, never a wildcard import, and no type of the name that a scope around the reference
/// may mean instead (`scope_may_shadow`)
fn supertype_resolves(use_site: &UseSite<'_>, reference: &Reference, path: &[String], site: &Site<'_>) -> bool {
  let Some((head, rest)) = path.split_first() else { return false };
  // a type of the name nested in a declaration around the reference wins over an import
  if nested_shadows(use_site, reference, head) {
    return false;
  }
  let full = site.ctx.namespace.iter().chain(&site.containers).map(String::as_str);
  // the head through an import: `import p.Manager as M`, `import p.Outer.Inner`
  if let Some((import_path, imported)) = import_binding(use_site, head) {
    let resolved = import_path.iter().map(String::as_str).chain([imported]).chain(rest.iter().map(String::as_str));
    return resolved.eq(full);
  }
  if !site.ctx.namespace.is_empty() && path.iter().map(String::as_str).eq(full) {
    return true;
  }
  if path != site.containers.as_slice() || scope_may_shadow(use_site, reference, head, path) {
    return false;
  }
  if use_site.path == site.ctx.path {
    return true;
  }
  let same_module = use_site.module_root.is_some() && use_site.module_root == site.ctx.module_root;
  same_module && use_site.namespace == site.ctx.namespace && !use_site.summary.declares(head)
}

/// Whether a call in a subclass means the member `site` of its supertype, which the subclass inherits (`Inherited`): a
/// call (not enclosed: a lambda, an object expression, an anonymous class or a local declaration has a receiver of its
/// own) by a bare name, `this.name` or `super.name`, in a class with exactly one supertype, which is not `Any` or
/// `Object` and resolves to the type of the member (`supertype_resolves`), not inside a member extension, whose
/// receiver comes first. Nothing else of the file may take the name: no declaration of it (a member of the class, of a
/// class or companion around it, a function of the file), no import that may bring it (`imports_of_function`; Java's
/// inherited method shadows a static import, but the call compiled with the import before the member came), and no
/// receiver further out with a supertype of its own (a class around it, a companion), whose inherited member the call
/// compiled with before (JLS 15.12.1; Kotlin falls back to it when the members of the class reject the call). The member
/// must be an instance function that overrides nothing (the member it overrides compiles the call without the commit),
/// no extension (a member extension of the supertype takes no bare call of a plain member), not one every class has
/// (`ROOT_MEMBERS`), and of a type without supertypes: a supertype's overload the range doesn't show may take the call
/// (`Disposable.fresh(Int)` takes `fresh(1)`), and a Java method may override without `@Override`. A member of a
/// supertype of the supertype resolves to no path of the class: no evidence
fn inherited(use_site: &UseSite<'_>, reference: &Reference, qualifier: Option<&[String]>, site: &Site<'_>) -> bool {
  let summary = use_site.summary;
  let declaration = site.declaration;
  let not_enclosed = matches!(
    reference.shape.and_then(|shape| summary.shapes.get(shape as usize)),
    Some(Shape::Call { enclosed: false, .. })
  );
  if !not_enclosed
    || !bare(qualifier)
    || declaration.kind != DeclKind::Function
    || declaration.member != MemberKind::Instance
    || declaration.overrides
    || declaration.receiver.is_some()
    || ROOT_MEMBERS.contains(&site.name())
    || has_supertypes(site)
  {
    return false;
  }
  let name = site.name();
  if summary.declares(name) || imports_of_function(summary, name) > 0 {
    return false;
  }
  // the innermost type around the call, unless an extension receiver comes first
  let Some((index, class)) = outward(summary, reference.container).find(|(_, declaration)| declaration.kind == DeclKind::Type || declaration.receiver.is_some()) else {
    return false;
  };
  if class.kind != DeclKind::Type {
    return false;
  }
  let [supertype] = class.supertypes.as_slice() else { return false };
  if !extends(summary, class) || !supertype_resolves(use_site, reference, &names(summary, supertype), site) {
    return false;
  }
  !receivers(summary, reference)
    .iter()
    .any(|(receiver, declaration)| *receiver != index && extends(summary, declaration))
}

/// Whether the parameter types of an override fit those of the member `site`. Where the member's type has no
/// supertypes, the override must mean one of its members, and each overload of it is an entry, so the match is
/// generous: a type parameter fits any type and two types of the same last names are the same, which only adds an
/// origin. Where it has (`exact`), a member of a supertype outside the range may be what the override means, so only
/// the same form counts: a signature, every type known, each name resolved by its file to the same type
fn override_fits(use_site: &UseSite<'_>, parameters: &[Option<TypeRef>], site: &Site<'_>, exact: bool) -> bool {
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
  let fits_parameter = |shape: Option<&TypeRef>, parameter: Option<&TypeRef>| match (shape, parameter) {
    (Some(x), Some(y)) => (!nullable || x.nullable == y.nullable) && same_names(use_site.summary, x, site.ctx.summary, y) && (!exact || resolved(x, y)),
    _ => !exact,
  };
  signature.parameters.len() == parameters.len() && parameters.iter().zip(&signature.parameters).all(|(x, y)| fits_parameter(x.as_ref(), y.type_ref.as_ref()))
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
  let imported = import_binding(ctx, name).map(|(path, imported)| path.iter().map(String::as_str).chain([imported]).collect::<Vec<_>>().join("."));
  imported.or_else(|| {
    let declared = summary
      .declarations
      .iter()
      .find(|declaration| matches!(declaration.kind, DeclKind::Type | DeclKind::TypeAlias) && summary.name(declaration.name) == name)?;
    Some(site_of(ctx, declared).key)
  })
}

/// The import of the file of `ctx` that binds `name` (by its alias, else by its last name): its path and the name it
/// imports
fn import_binding<'c>(ctx: &'c FileCtx<'_>, name: &str) -> Option<(&'c [String], &'c str)> {
  let summary = ctx.summary;
  summary.imports.iter().zip(&ctx.import_paths).find_map(|(import, path)| match import.target {
    ImportTarget::Name(imported) if summary.name(import.alias.unwrap_or(imported)) == name => Some((path.as_slice(), summary.name(imported))),
    _ => None,
  })
}

/// Whether a type named `head` other than the one of the supertype path of an override or an inherited call may be what
/// it means: one nested in a declaration around the reference (`Outer.Base` for a class inside `Outer`), or a member
/// type that a Java class around the overriding or calling class inherits, which the file doesn't show. Kotlin also
/// reaches the nested classes of a supertype by their names; that is left out, since an object expression in a class
/// with supertypes is everywhere
fn scope_may_shadow(use_site: &UseSite<'_>, reference: &Reference, head: &str, path: &[String]) -> bool {
  let summary = use_site.summary;
  // the declarations around the override, the innermost first
  let around: Vec<(u32, &Declaration)> = outward(summary, reference.container).collect();
  let nested = nested_shadows(use_site, reference, head);
  // the class of the override or the call itself, the innermost one, whose supertypes name the path, is no class
  // around it
  let class = around.iter().position(|(_, declaration)| declaration.kind == DeclKind::Type);
  let inherited = use_site.language == Language::Java
    && around.iter().enumerate().any(|(depth, (_, declaration))| {
      let owner = Some(depth) == class && declaration.supertypes.iter().any(|supertype| names(summary, supertype) == path);
      !owner && !declaration.supertypes.is_empty()
    });
  nested || inherited
}

/// Whether a type named `head` is nested in a declaration around `reference` (`Outer.Base` for a class inside `Outer`)
fn nested_shadows(use_site: &UseSite<'_>, reference: &Reference, head: &str) -> bool {
  let summary = use_site.summary;
  let around: Vec<u32> = outward(summary, reference.container).map(|(index, _)| index).collect();
  summary.declarations.iter().any(|declaration| {
    matches!(declaration.kind, DeclKind::Type | DeclKind::TypeAlias)
      && summary.name(declaration.name) == head
      && declaration.container.is_some_and(|container| around.contains(&container))
  })
}

/// Whether the JVM declaration `site` of `reach` (its own, or its constructor's) reaches the file of `use_site`: Kotlin
/// `internal` reaches its module, Java's package-private its package. A file whose module no build file tells is of no
/// module, but an older form (`Site::older`), whose rejection makes an edge, reaches it, as the compiler may see one
/// module. Java sees an `internal` Kotlin type and its constructors from anywhere, as the JVM has them public and,
/// unlike a member's, their names aren't mangled
fn reaches_module(use_site: &UseSite<'_>, site: &Site<'_>, reach: Reach) -> bool {
  reach != Reach::Module
    || match site.ctx.language {
      Language::Kotlin => match (use_site.module_root, site.ctx.module_root) {
        _ if use_site.language == Language::Java && site.declaration.kind == DeclKind::Type => true,
        (Some(used), Some(declared)) => used == declared,
        _ => site.older,
      },
      _ => use_site.namespace == site.ctx.namespace,
    }
}

fn resolve_jvm(use_site: &UseSite<'_>, reference: &Reference, qualifier: Option<&[String]>, site: &Site<'_>) -> Option<Evidence> {
  let summary = use_site.summary;
  let name = site.name();
  if !reaches_module(use_site, site, site.declaration.reach) {
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
      let paths = static_paths(use_site, site);
      let in_namespace = |path: &[&str]| {
        site
          .ctx
          .namespace
          .iter()
          .cloned()
          .chain(path.iter().map(|segment| (*segment).to_string()))
          .collect::<Vec<_>>()
      };
      let Some(qualifier) = qualifier.filter(|_| !site.containers.is_empty()) else {
        // an import of the member by one of its paths: Java's `import static a.B.member`, Kotlin's `import a.Obj.member`
        // and `import a.Owner.Companion.member`
        return (reference.qualifier.is_none() && paths.iter().any(|path| imports_of(&in_namespace(path), name))).then_some(Evidence::Import);
      };
      // `a.b.Owner.member`, or `Owner.member` and `Outer.Nested.member` with the head resolved: by an import that
      // binds it (an alias, a nested type), else by the same package without a declaration of its name in the file
      let (head, rest) = qualifier.split_first()?;
      let import = import_binding(use_site, head);
      let same_package = use_site.namespace == site.ctx.namespace && !summary.declares(head);
      let names = |path: &[&str]| {
        let full = || site.ctx.namespace.iter().map(String::as_str).chain(path.iter().copied());
        qualifier.iter().map(String::as_str).eq(full())
          || match import {
            Some((import_path, imported)) => import_path.iter().map(String::as_str).chain([imported]).chain(rest.iter().map(String::as_str)).eq(full()),
            None => same_package && qualifier.iter().map(String::as_str).eq(path.iter().copied()),
          }
      };
      paths.iter().any(|path| names(path)).then_some(Evidence::Qualified)
    }
    MemberKind::Instance => {
      if inherited(use_site, reference, qualifier, site) {
        return Some(Evidence::Inherited);
      }
      through_receiver(use_site, reference, qualifier, site).then_some(Evidence::Receiver)
    }
  }
}

/// Whether a use of the instance member `site` on a receiver means it (`Receiver`): one whose type the summary knows
/// (`Reference::receiver_type`: a local, a parameter or a property of a declared or constructed type), or the
/// extension receiver of the function around a bare call (`implicit_receiver`), that type resolving to the type of
/// the member as an override's supertype does (`supertype_resolves`). The member must override nothing (the member it
/// overrides compiles the use without the commit) and not be one every class has (`ROOT_MEMBERS`); a Java one only of
/// a type without supertypes, which may override without `@Override`. A type whose file declares a function that a
/// call of its name may mean instead of a constructor (an `invoke` of its companion, a factory of its name) may give a
/// receiver its initializer constructs another type (`Reference::receiver_constructed`): none. A member of a type with
/// supertypes is judged by its key only (`all_entries`), and then a function that a Kotlin import may bring (an
/// extension of the name) or an overload of a supertype takes a call the member rejects: none with such an import, a
/// declaration of the name in the file, or a call that the entry doesn't accept (`accepts_call`). A member that the type inherits is no member of it:
/// no evidence. A Kotlin extension of the type is no member: an extension on a receiver links only by the rules of a
/// top-level function
fn through_receiver(use_site: &UseSite<'_>, reference: &Reference, qualifier: Option<&[String]>, site: &Site<'_>) -> bool {
  let summary = use_site.summary;
  let declaration = site.declaration;
  if !matches!(reference.role, Role::Call | Role::Value)
    || !matches!(declaration.kind, DeclKind::Function | DeclKind::Value)
    || declaration.member != MemberKind::Instance
    || declaration.overrides
    || ROOT_MEMBERS.contains(&site.name())
    || (site.ctx.language == Language::Java && has_supertypes(site))
    || (reference.receiver_constructed && constructed_otherwise(site))
  {
    return false;
  }
  let path = match &reference.receiver_type {
    Some(path) => names(summary, path),
    None => match implicit_receiver(use_site, reference, qualifier, site) {
      Some(path) => path,
      None => return false,
    },
  };
  // judged by its key, the call must still fit an entry of it: an overload of a supertype may take one none accepts
  let key_only = inherits_unseen(use_site, site);
  let fits = || match reference.shape.and_then(|shape| summary.shapes.get(shape as usize)) {
    Some(Shape::Call { arguments, spread, .. }) => accepts_call(summary, arguments, *spread, site),
    _ => true,
  };
  if key_only && (summary.declares(site.name()) || imports_of_function(summary, site.name()) > 0 || !fits()) {
    return false;
  }
  supertype_resolves(use_site, reference, &path, site)
}

/// The extension receiver of the function or property around a bare call or `this.name()` (`[Scope]` of a
/// `fun Scope.body()`, by its last name), the innermost receiver: not in an enclosed call (a lambda's receiver may
/// take it), not in a member of a class (an inherited call, `inherited`), where nothing else of the file may take the
/// name (a declaration of it, an import that may bring it), no receiver further out has supertypes (the class around a
/// member extension, a companion) and the type of the member has none, whose members would take a call its own
/// overloads reject
fn implicit_receiver(use_site: &UseSite<'_>, reference: &Reference, qualifier: Option<&[String]>, site: &Site<'_>) -> Option<Vec<String>> {
  let summary = use_site.summary;
  let not_enclosed = matches!(
    reference.shape.and_then(|shape| summary.shapes.get(shape as usize)),
    Some(Shape::Call { enclosed: false, .. })
  );
  let this = qualifier.is_none_or(|qualifier| matches!(qualifier, [only] if only == "this"));
  if !not_enclosed || !this || summary.declares(site.name()) || imports_of_function(summary, site.name()) > 0 {
    return None;
  }
  let (_, around) = outward(summary, reference.container).find(|(_, declaration)| declaration.kind == DeclKind::Type || declaration.receiver.is_some())?;
  if around.kind == DeclKind::Type || container_of(site).is_some_and(|container| extends(site.ctx.summary, container)) {
    return None;
  }
  if receivers(summary, reference).iter().any(|(_, declaration)| extends(summary, declaration)) {
    return None;
  }
  Some(vec![summary.name(around.receiver?).to_string()])
}

/// The last name of the type of the receiver of `reference`: its `receiver_type`, else the extension receiver of the
/// function or property around it
fn receiver_name<'a>(summary: &'a FileSummary, reference: &Reference) -> Option<&'a str> {
  if let Some(last) = reference.receiver_type.as_deref().and_then(<[u32]>::last) {
    return Some(summary.name(*last));
  }
  let (_, around) = outward(summary, reference.container).find(|(_, declaration)| declaration.kind == DeclKind::Type || declaration.receiver.is_some())?;
  around.receiver.map(|receiver| summary.name(receiver))
}

/// Whether `site` is a JVM extension of the type `receiver` (by its last name) that the file of `use_site` sees without
/// a qualifier: of its package, or through an import of its name or a wildcard import of its package. A member of the
/// type wins over it only for a call the member accepts, which the summary judges generously
fn sees_extension(use_site: &UseSite<'_>, site: &Site<'_>, receiver: &str) -> bool {
  let summary = use_site.summary;
  site.ctx.language.family() == Family::Jvm
    && site.declaration.member == MemberKind::TopLevel
    && receiver_of(site) == Some(receiver)
    && (use_site.namespace == site.ctx.namespace
      || summary.imports.iter().zip(&use_site.import_paths).any(|(import, path)| {
        *path == site.ctx.namespace
          && match import.target {
            ImportTarget::Name(imported) => summary.name(imported) == site.name(),
            ImportTarget::All => true,
            ImportTarget::Default | ImportTarget::Namespace => false,
          }
      }))
}

/// Whether the file of the member `site` declares a function that a call of its type's name may mean instead of a
/// constructor: an `invoke` of a companion of the type, or a top-level function of the type's name (a factory, which
/// may return another type)
fn constructed_otherwise(site: &Site<'_>) -> bool {
  let summary = site.ctx.summary;
  let Some(index) = site.declaration.container else { return false };
  let Some(owner) = container_of(site) else { return false };
  summary.declarations.iter().any(|declaration| {
    declaration.kind == DeclKind::Function
      && match declaration.container {
        None => declaration.name == owner.name,
        Some(container) => {
          summary.name(declaration.name) == "invoke"
            && summary
              .declarations
              .get(container as usize)
              .is_some_and(|companion| summary.name(companion.name) == "Companion" && companion.container == Some(index))
        }
      }
  })
}

/// The containers by which a use from the file of `use_site` may name the type of the static member `site`, the
/// outermost first: its own (`Owner.Companion.create()`, `Obj.create()`); for a member of a Kotlin companion without a
/// name of its own the class around it too, from Kotlin for a function or a property (`Owner.create()`; a type nested
/// in the companion is no classifier of the class), from Java only for a `@JvmStatic` function or a `const val`
/// (`Owner.CONSTANT`), which the JVM has on that class; and from Java the `INSTANCE` of a Kotlin object for a function
/// of it (`Obj.INSTANCE.create()`; Java reads a property through its getter, another name), which the object itself
/// names only when it is `@JvmStatic`: a function or property of a top-level object (`Obj.create()`). The summary
/// doesn't tell a companion with a name of its own (`companion object Factory`) from a nested object, whose members
/// the class around it doesn't have, so it is named only by its own path (`Owner.Factory.create()`, which Java has
/// for a companion without `@JvmStatic` too)
fn static_paths<'s>(use_site: &UseSite<'_>, site: &'s Site<'_>) -> Vec<Vec<&'s str>> {
  let containers: Vec<&str> = site.containers.iter().map(String::as_str).collect();
  if site.ctx.language != Language::Kotlin {
    return vec![containers];
  }
  let declaration = site.declaration;
  let java = use_site.language == Language::Java;
  let member = matches!(declaration.kind, DeclKind::Function | DeclKind::Value);
  let on_class = (declaration.kind == DeclKind::Function && declaration.jvm_static) || declaration.kind == DeclKind::Constant;
  let mut paths = Vec::new();
  match containers.split_last() {
    Some((&"Companion", outer)) if !outer.is_empty() => {
      paths.push(containers.clone());
      if (!java && declaration.kind != DeclKind::Type) || (java && on_class) {
        paths.push(outer.to_vec());
      }
    }
    Some((_, outer)) if java && member => {
      if !outer.is_empty() || on_class {
        paths.push(containers.clone());
      }
      if declaration.kind == DeclKind::Function {
        paths.push([containers.as_slice(), &["INSTANCE"]].concat());
      }
    }
    _ => paths.push(containers),
  }
  paths
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
