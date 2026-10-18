//! The checks of a branch's tree by the syntax summaries (`dangling_references.rs` runs them beside its own): HEAD
//! compiles, and the virtual tree of a branch differs from HEAD's only in the paths of the other branches' commits, so a
//! use the branch's tree has there and HEAD's lacks, of a declaration the branch removes, changes or declares again, may
//! not compile. A child of `symbol_dependencies`, whose resolution (`resolve`) decides what a use means; decision 15 of
//! docs/design/symbol-dependencies.md.
//!
//! - **A removed member** (`Removed`): an instance or static member of a JVM type, an enum entry, that a commit of the
//!   branch removes with all its overloads, used where a use resolves to it: on a receiver of its type, by `this`, an
//!   override, an inherited call or its static path (decisions 3, 8, 12 and 13 of docs/design/symbol-dependencies.md),
//!   in its own file only inside its type. The lines of `dangling_references.rs` search the top-level declarations.
//! - **A changed declaration** (`Changed`): a commit of the branch changes the header of a function, a property or a
//!   constant (`Declaration::header`: `suspend`, the return type, the receiver), its parameters, its reach, whether a
//!   subtype may override it, or the constructors of a type, and keeps its name. A use counts where the old form takes
//!   it and no declaration of the key in the branch's tree does: one of the same header that the use resolves to by the
//!   same rules and whose parameters accept its call (`accepts_call`, the constructors for a call of a type). A change
//!   of the body alone changes nothing, and a use the branch's own commits wrote is written against the new form.
//! - **A duplicate declaration** (`Duplicate`): a commit of the branch adds a JVM type, or a Kotlin top-level function
//!   or property, whose key another file of the same package and module declares in the branch's tree but not in HEAD's
//!   (a move whose deletion is in another branch). Overloads are told apart by their parameter types.
//! - **Counts, not lines**, as in the line search: a path counts where the branch's tree has more such uses than HEAD's.

use crate::dangling_references::DanglingKind;
use crate::diff_scan::CommitDiff;
use crate::symbol_dependencies::{
  Accessor, Evidence, FileCtx, Site, accepts_call, accessor_names, bare, changed_lines, changed_sites, constructor_reaches, contains, has_entries, in_ranges, names, receiver_of,
  resolve, same_entry, same_names, site_of, with_constructors,
};
use crate::symbol_scan::SymbolInput;
use code_symbols::language::{Family, Language, language_of};
use code_symbols::summary::{DeclKind, FileSummary, ImportTarget, MemberKind, Reference, Role, Shape};
use std::borrow::Cow;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// The summaries and the changes of the commits ahead of the baseline, for the checks of the branches' trees
#[derive(Clone, Copy)]
pub struct DeclarationFacts<'a> {
  pub input: &'a SymbolInput,
  /// Oldest first, as the commits ahead of the baseline
  pub diffs: &'a [CommitDiff],
  /// The keys each commit declares anew, by commit id, shared by the checks of a session
  pub added: &'a AddedKeys,
}

/// Commit id -> the keys its added lines declare anew in a file (`added_keys`), computed once per commit: a commit id
/// names its changes, so the sessions of `with_rewrite` share it
#[derive(Debug, Default)]
pub struct AddedKeys(Mutex<HashMap<String, Arc<Vec<String>>>>);

/// The keys that the added lines of `diff` declare anew in a file
fn added_keys(input: &SymbolInput, diff: &CommitDiff) -> Vec<String> {
  let summary_of = |blob: Option<&String>| blob.and_then(|blob| input.summaries.get(blob)).map(AsRef::as_ref).filter(|summary| !summary.is_degraded());
  let mut keys = Vec::new();
  for change in &diff.files {
    let Some(after) = summary_of(change.new_blob.as_ref()) else { continue };
    let (added_lines, _) = changed_lines(change);
    if !after.declarations.iter().any(|declaration| in_ranges(&added_lines, declaration.name_line)) {
      continue;
    }
    let before = summary_of(change.old_blob.as_ref()).map(|summary| input.file_ctx(change.old_path.as_deref().unwrap_or(&change.path), summary));
    let after = input.file_ctx(&change.path, after);
    for declaration in after.summary.declarations.iter().filter(|declaration| in_ranges(&added_lines, declaration.name_line)) {
      let name = after.summary.name(declaration.name);
      let key = site_of(&after, declaration).key;
      let had = before.as_ref().is_some_and(|before| {
        before
          .summary
          .declarations
          .iter()
          .filter(|it| before.summary.name(it.name) == name)
          .any(|it| site_of(before, it).key == key)
      });
      if !had {
        keys.push(key);
      }
    }
  }
  keys
}

impl AddedKeys {
  fn of(&self, input: &SymbolInput, diff: &CommitDiff) -> Arc<Vec<String>> {
    let known = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&diff.id).cloned();
    known.unwrap_or_else(|| {
      let keys = Arc::new(added_keys(input, diff));
      self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(diff.id.clone(), Arc::clone(&keys));
      keys
    })
  }
}

/// How a use of an old form fits the forms of its key in the branch's tree
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fit {
  /// It may not compile, and the summary can tell: a call the new parameters reject, a suspend call outside a
  /// coroutine, a changed extension receiver, a removed declaration
  Breaks,
  /// It may not compile, and the summary can't tell: a changed return type or nullability, new type parameters, a
  /// suspend call in the lambda of an unknown function
  Unknown,
  Fits,
}

/// Tokens of a header whose addition breaks no use: a `var` for a `val`, `const`, `operator`, `infix`
const HARMLESS_ADDITIONS: [&str; 3] = ["const", "operator", "infix"];

/// How a use, in the coroutine context `coroutine` (`Reference::coroutine`), of the declaration `old` fits `new`, a
/// declaration of the same key that takes it by the rules of a use, by their headers (`Declaration::header`) as a
/// token diff: a changed extension receiver breaks it, an added `suspend` breaks it outside a coroutine, an explicit
/// type of a property for an inferred one, Java's `throws` and the additions of `HARMLESS_ADDITIONS` fit, any other
/// change may break it
fn header_fit(old: &Site<'_>, new: &Site<'_>, coroutine: Option<bool>) -> Fit {
  let (before, after) = (old.declaration.header.as_deref().unwrap_or_default(), new.declaration.header.as_deref().unwrap_or_default());
  if before == after {
    return Fit::Fits;
  }
  if receiver_of(old) != receiver_of(new) {
    return Fit::Breaks;
  }
  // a `var` for a `val` takes every read
  let widened;
  let after = if before.split(' ').any(|token| token == "val") && after.split(' ').any(|token| token == "var") {
    widened = after.split(' ').map(|token| if token == "var" { "val" } else { token }).collect::<Vec<_>>().join(" ");
    widened.as_str()
  } else {
    after
  };
  if before == after {
    return Fit::Fits;
  }
  // an explicit type for the one a property's initializer gives it: the header goes on with the type. A function
  // without one returns `Unit` unless its body is an expression, which the header doesn't show
  let value = matches!(old.declaration.kind, DeclKind::Value | DeclKind::Constant);
  if value && after.strip_prefix(before).is_some_and(|rest| rest.trim_start().starts_with(':')) {
    return Fit::Fits;
  }
  let throws = |header: &'_ str| -> String {
    if old.ctx.language == Language::Java {
      header.split_once(" throws ").map_or(header, |(head, _)| head).to_string()
    } else {
      header.to_string()
    }
  };
  let (before, after) = (throws(before), throws(after));
  let mut removed: Vec<&str> = before.split(' ').collect();
  let mut added: Vec<&str> = Vec::new();
  for token in after.split(' ') {
    match removed.iter().position(|it| *it == token) {
      Some(found) => {
        removed.swap_remove(found);
      }
      None => added.push(token),
    }
  }
  added.retain(|token| !HARMLESS_ADDITIONS.contains(token));
  // a suspend function that is no longer one takes every call
  removed.retain(|token| *token != "suspend");
  let suspend = added.iter().position(|it| *it == "suspend").map(|found| added.swap_remove(found)).is_some();
  match (suspend, removed.is_empty() && added.is_empty(), coroutine) {
    (false, true, _) | (true, true, Some(true)) => Fit::Fits,
    (true, _, Some(false)) => Fit::Breaks,
    _ => Fit::Unknown,
  }
}

/// The shortest name searched, as in the line search: shorter ones are everywhere
const MIN_NAME_LENGTH: usize = 3;

/// A declaration that a commit of a branch removes, changes or adds, searched in the branch's tree
pub struct DeclarationSearch<'a> {
  pub kind: DanglingKind,
  pub name: &'a str,
  /// The file of the declaration after the commit (before it for a removal)
  pub file: &'a str,
  /// The commit of the branch
  pub commit: &'a str,
  /// The side the uses were written against: before the commit; after it for a duplicate, whose key it declares
  side: FileCtx<'a>,
  /// The declarations of the key on `side` the search is about: of a removal all, of a change those it changed, of a
  /// duplicate the one it added
  old: Vec<usize>,
  key: String,
  /// A change of the constructors of a type: only its calls count
  constructors: bool,
  /// The names a use of it is found by: its own, and those of its accessors in the other JVM language
  words: Vec<String>,
}

/// The declarations of the key of a search in the branch's version of its file, `DeclarationSearch::in_tree`
pub struct TreeForm<'a> {
  ctx: FileCtx<'a>,
  declarations: Vec<usize>,
}

/// The declarations of `ctx` by their key, of the kinds a search looks at, readable ones only; with `names`, only those
/// of these names
fn by_key<'a>(ctx: &'a FileCtx<'a>, names: Option<&HashSet<&str>>) -> BTreeMap<String, Vec<usize>> {
  let mut keys: BTreeMap<String, Vec<usize>> = BTreeMap::new();
  for (index, declaration) in ctx.summary.declarations.iter().enumerate() {
    let name = ctx.summary.name(declaration.name);
    if declaration.kind == DeclKind::Binding || declaration.in_error || name.len() < MIN_NAME_LENGTH || names.is_some_and(|names| !names.contains(name)) {
      continue;
    }
    keys.entry(site_of(ctx, declaration).key).or_default().push(index);
  }
  keys
}

/// Whether a commit changes the form of a declaration that keeps its identity, as a use may see it
fn changed_form(before: &Site<'_>, after: &Site<'_>) -> bool {
  let (a, b) = (before.declaration, after.declaration);
  // the names, defaults and varargs of the parameters, and their types with `?`, by which a literal fits
  let parameters = |site: &Site<'_>| {
    site.signature().map(|signature| {
      signature
        .parameters
        .iter()
        .map(|parameter| (site.ctx.summary.name(parameter.name).to_string(), parameter.optional, parameter.vararg))
        .collect::<Vec<_>>()
    })
  };
  let same_types = match (before.signature(), after.signature()) {
    (Some(x), Some(y)) => {
      x.parameters.len() == y.parameters.len()
        && x.parameters.iter().zip(&y.parameters).all(|(x, y)| match (&x.type_ref, &y.type_ref) {
          (Some(x), Some(y)) => x.nullable == y.nullable && same_names(before.ctx.summary, x, after.ctx.summary, y),
          (x, y) => x.is_none() && y.is_none(),
        })
    }
    (x, y) => x.is_none() && y.is_none(),
  };
  a.header != b.header
    || !same_types
    || parameters(before) != parameters(after)
    || (a.reach, a.member, a.overridable, a.jvm_static, a.jvm_field, a.jvm_name) != (b.reach, b.member, b.overridable, b.jvm_static, b.jvm_field, b.jvm_name)
}

/// The constructors of a type as a use of them sees them: the parameters and the reach of each, in order
fn constructor_forms(site: &Site<'_>) -> Option<Vec<String>> {
  let summary = site.ctx.summary;
  site.declaration.constructors.as_ref().map(|constructors| {
    let mut forms: Vec<String> = constructors
      .iter()
      .map(|constructor| {
        let parameters: Vec<String> = constructor
          .signature
          .parameters
          .iter()
          .map(|parameter| {
            let names: Vec<&str> = parameter
              .type_ref
              .iter()
              .flat_map(|type_ref| type_ref.names.iter().map(|name| summary.name(*name)))
              .collect();
            format!(
              "{}:{}{}{}",
              summary.name(parameter.name),
              names.join("/"),
              if parameter.optional { "=" } else { "" },
              if parameter.vararg { "*" } else { "" }
            )
          })
          .collect();
        format!("{:?}({})", constructor.reach, parameters.join(","))
      })
      .collect();
    forms.sort();
    forms
  })
}

/// The searches of the commits `own` of a branch (oldest first): what each removes, changes or adds. `members` are the
/// commits of the branch's tree, whose additions of a name make its removal a move
pub fn searches_of<'a>(facts: DeclarationFacts<'a>, own: &[&'a str], members: &HashSet<&str>) -> Vec<DeclarationSearch<'a>> {
  let input = facts.input;
  let summary_of = |blob: Option<&String>| blob.and_then(|blob| input.summaries.get(blob)).map(AsRef::as_ref).filter(|summary| !summary.is_degraded());
  let diff_of = |id: &str| facts.diffs.iter().find(|diff| diff.id == id);
  // the keys that the commits of the tree declare anew in a file, on their added lines: a member removed here and
  // added there with its type moved
  let mut added: HashSet<String> = HashSet::new();
  for diff in facts.diffs.iter().filter(|diff| members.contains(diff.id.as_str())) {
    added.extend(facts.added.of(input, diff).iter().cloned());
  }

  let mut searches: BTreeMap<(DanglingKind, String), DeclarationSearch<'a>> = BTreeMap::new();
  for &commit in own {
    let Some(diff) = diff_of(commit) else { continue };
    for change in &diff.files {
      let old_path = change.old_path.as_deref().unwrap_or(&change.path);
      let before = summary_of(change.old_blob.as_ref()).map(|summary| input.file_ctx(old_path, summary));
      let after = summary_of(change.new_blob.as_ref()).map(|summary| input.file_ctx(&change.path, summary));
      let (Some(before), Some(after)) = (before, after) else {
        // a new file adds every key it declares; a deleted one removes its types too, which the lines search
        if let Some(after) = summary_of(change.new_blob.as_ref()).filter(|_| change.old_blob.is_none()) {
          let ctx = input.file_ctx(&change.path, after);
          for (key, indices) in by_key(&ctx, None) {
            push_duplicates(&mut searches, &ctx, commit, &key, &indices);
          }
        }
        continue;
      };
      // the keys of the declarations on the changed lines, a member of a renamed type included (`changed_sites`)
      let (added_lines, removed_lines) = changed_lines(change);
      let mut changed: HashSet<&str> = HashSet::new();
      for (side, ranges) in [(&after, &added_lines), (&before, &removed_lines)] {
        let summary = side.summary;
        changed.extend(changed_sites(side, ranges).iter().map(|site| summary.name(site.declaration.name)));
      }
      let (before_keys, after_keys) = (by_key(&before, Some(&changed)), by_key(&after, Some(&changed)));
      for (key, old) in &before_keys {
        let Some(new) = after_keys.get(key) else {
          // a member gone with all its overloads, unless another file of the tree declares its name now
          let removable = old.iter().all(|&index| {
            let declaration = &before.summary.declarations[index];
            declaration.member != MemberKind::TopLevel
              && matches!(declaration.kind, DeclKind::Function | DeclKind::Value | DeclKind::Constant | DeclKind::EnumEntry)
              && before.language.family() == Family::Jvm
          });
          // a member moved to another type of the file (pulled up) keeps its uses
          let name = before.summary.name(before.summary.declarations[old[0]].name);
          if removable && !added.contains(key) && !after.summary.declares(name) && before.namespace == after.namespace {
            insert(
              &mut searches,
              DanglingKind::Removed,
              key,
              before.clone(),
              old.clone(),
              name,
              change.path.as_str(),
              commit,
              false,
            );
          }
          continue;
        };
        // pairs of one identity, then the one left of each side
        let old_sites: Vec<Site<'_>> = old.iter().map(|&index| site_of(&before, &before.summary.declarations[index])).collect();
        let new_sites: Vec<Site<'_>> = new.iter().map(|&index| site_of(&after, &after.summary.declarations[index])).collect();
        let mut unmatched_new: Vec<usize> = (0..new_sites.len()).collect();
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        let mut unmatched_old: Vec<usize> = Vec::new();
        for (index, old_site) in old_sites.iter().enumerate() {
          match unmatched_new.iter().position(|&candidate| same_entry(old_site, &new_sites[candidate])) {
            Some(found) => pairs.push((index, unmatched_new.swap_remove(found))),
            None => unmatched_old.push(index),
          }
        }
        if let ([old_index], [new_index]) = (unmatched_old.as_slice(), unmatched_new.as_slice()) {
          pairs.push((*old_index, *new_index));
          unmatched_new.clear();
        }
        let mut changed: Vec<usize> = pairs
          .iter()
          .filter(|(old_index, new_index)| {
            let (old_site, new_site) = (&old_sites[*old_index], &new_sites[*new_index]);
            match old_site.declaration.kind {
              DeclKind::Function | DeclKind::Value | DeclKind::Constant => changed_form(old_site, new_site),
              DeclKind::Type => constructor_forms(old_site).zip(constructor_forms(new_site)).is_some_and(|(a, b)| a != b),
              _ => false,
            }
          })
          .map(|(old_index, _)| old[*old_index])
          .collect();
        // an overload removed while another of the key stays: its uses are judged by those left
        changed.extend(
          unmatched_old
            .iter()
            .filter(|index| !pairs.iter().any(|(paired, _)| paired == *index))
            .map(|&index| old[index]),
        );
        // a form only the JVM languages judge: the others keep the parameters in the header, with their defaults
        if let Some(&first) = changed.first().filter(|_| before.language.family() == Family::Jvm) {
          let declaration = &before.summary.declarations[first];
          let name = before.summary.name(declaration.name);
          insert(
            &mut searches,
            DanglingKind::Changed,
            key,
            before.clone(),
            changed.clone(),
            name,
            change.path.as_str(),
            commit,
            declaration.kind == DeclKind::Type,
          );
        }
        // an overload of a new identity is a duplicate only of the same identity elsewhere
        if after.namespace == before.namespace {
          let fresh: Vec<usize> = unmatched_new.iter().map(|&index| new[index]).collect();
          if !fresh.is_empty() {
            push_duplicates(&mut searches, &after, commit, key, &fresh);
          }
        }
      }
      for (key, indices) in &after_keys {
        if !before_keys.contains_key(key) {
          push_duplicates(&mut searches, &after, commit, key, indices);
        }
      }
    }
  }
  searches.into_values().collect()
}

/// A search of a key: of a change or an addition the first commit's, of a removal the last one's
#[expect(clippy::too_many_arguments, reason = "the fields of a search")]
fn insert<'a>(
  searches: &mut BTreeMap<(DanglingKind, String), DeclarationSearch<'a>>,
  kind: DanglingKind,
  key: &str,
  side: FileCtx<'a>,
  old: Vec<usize>,
  name: &'a str,
  file: &'a str,
  commit: &'a str,
  constructors: bool,
) {
  let search = DeclarationSearch {
    words: words_of(kind, name, side.language),
    kind,
    name,
    file,
    commit,
    side,
    old,
    key: key.to_string(),
    constructors,
  };
  // a change from the form before the first commit that makes it, a removal by the last
  match searches.entry((kind, key.to_string())) {
    Entry::Occupied(mut known) if kind == DanglingKind::Removed => {
      known.insert(search);
    }
    Entry::Occupied(_) => {}
    Entry::Vacant(vacant) => {
      vacant.insert(search);
    }
  }
}

/// The duplicate searches of the declarations `indices` of a key that `ctx`, the side after a commit, adds: a top-level
/// JVM type, a Kotlin top-level function or property, each identity once
fn push_duplicates<'a>(searches: &mut BTreeMap<(DanglingKind, String), DeclarationSearch<'a>>, ctx: &FileCtx<'a>, commit: &'a str, key: &str, indices: &[usize]) {
  let summary = ctx.summary;
  for &index in indices {
    let declaration = &summary.declarations[index];
    // a nested type is a duplicate only beside its outer one
    let fits = declaration.member == MemberKind::TopLevel
      && match declaration.kind {
        DeclKind::Type => true,
        DeclKind::Function | DeclKind::Value => ctx.language == Language::Kotlin,
        _ => false,
      };
    if !fits || ctx.language.family() != Family::Jvm || ctx.module_root.is_none() {
      continue;
    }
    let name = summary.name(declaration.name);
    // a key of overloads: one search per identity, by the parameter types in the key
    let identity = declaration.signature.as_ref().map_or_else(String::new, |signature| {
      let types: Vec<String> = signature
        .parameters
        .iter()
        .map(|parameter| {
          parameter
            .type_ref
            .as_ref()
            .map_or_else(|| "_".to_string(), |it| it.names.iter().map(|name| summary.name(*name)).collect::<Vec<_>>().join("/"))
        })
        .collect();
      format!("({})", types.join(","))
    });
    insert(
      searches,
      DanglingKind::Duplicate,
      &format!("{key}{identity}"),
      ctx.clone(),
      vec![index],
      name,
      ctx.path,
      commit,
      false,
    );
  }
}

/// The names a use of a declaration is found by: its own, and those of its accessors in the other JVM language
fn words_of(kind: DanglingKind, name: &str, language: Language) -> Vec<String> {
  let mut words = vec![name.to_string()];
  if kind != DanglingKind::Duplicate && language.family() == Family::Jvm {
    let mut chars = name.chars();
    let capital: String = chars.next().map_or_default(|first| first.to_uppercase().chain(chars).collect());
    words.extend([format!("get{capital}"), format!("set{capital}")]);
    if let Some(rest) = name.strip_prefix("get") {
      let mut chars = rest.chars();
      words.extend(chars.next().map(|first| first.to_lowercase().chain(chars).collect::<String>()));
    }
  }
  words
}

/// Whether an import of `summary` brings `name` in, by its name or an alias
fn imports_name(summary: &FileSummary, name: &str) -> bool {
  summary.imports.iter().any(|import| {
    let target = match import.target {
      ImportTarget::Name(target) => Some(target),
      _ => None,
    };
    import.alias.or(target).is_some_and(|bound| summary.name(bound) == name)
  })
}

/// The context of a use that names a declaration by `accessor`: `use_site` itself without one
fn named_by<'c, 'u>(use_site: &'c FileCtx<'u>, accessor: Option<Accessor>) -> Cow<'c, FileCtx<'u>> {
  match accessor {
    Some(_) => Cow::Owned(FileCtx { accessor, ..use_site.clone() }),
    None => Cow::Borrowed(use_site),
  }
}

impl DeclarationSearch<'_> {
  /// The names a use of it is found by in a text: its own, and those of its accessors in the other JVM language
  pub fn words(&self) -> &[String] {
    &self.words
  }

  /// Whether a file of `summary` may use it, by the names it has; `None`, a summary not known yet, may
  pub fn may_use(&self, summary: Option<&FileSummary>) -> bool {
    summary.is_none_or(|summary| self.words.iter().any(|word| summary.name_id(word).is_some()))
  }

  /// Whether the uses in `path` are searched: a file of the declaration's language family; for a duplicate one of its
  /// module, which another declaration of its key must be in
  pub fn searches(&self, input: &SymbolInput, path: &str) -> bool {
    language_of(path).is_some_and(|language| language.family() == self.side.language.family())
      && (self.kind != DanglingKind::Duplicate || input.roots.module_of(path) == self.side.module_root)
  }

  fn old_sites(&self) -> Vec<Site<'_>> {
    self.old.iter().map(|&index| site_of(&self.side, &self.side.summary.declarations[index])).collect()
  }

  /// The declarations of its key in `current`, the branch's version of its file (`path`): `None` when the search ends
  /// there, a member declared again, a changed declaration removed later, an added one gone
  pub fn in_tree<'s>(&self, input: &'s SymbolInput, path: &'s str, current: &'s FileSummary) -> Option<TreeForm<'s>> {
    let ctx = input.file_ctx(path, current);
    let key = self.key.split_once('(').map_or(self.key.as_str(), |(key, _)| key);
    let declarations: Vec<usize> = current
      .declarations
      .iter()
      .enumerate()
      .filter(|(_, declaration)| current.name(declaration.name) == self.name)
      .filter(|(_, declaration)| site_of(&ctx, declaration).key == key)
      .map(|(index, _)| index)
      .collect();
    let kept = match self.kind {
      DanglingKind::Removed => declarations.is_empty(),
      DanglingKind::Changed | DanglingKind::Duplicate => !declarations.is_empty(),
    };
    kept.then_some(TreeForm { ctx, declarations })
  }

  /// The lines of `summary`, the file `path`, that the search counts: the uses that may not compile in the branch's
  /// tree (`form`, of `in_tree`), or for a duplicate the declarations of its key
  pub fn uses(&self, input: &SymbolInput, form: &TreeForm<'_>, path: &str, summary: &FileSummary) -> Vec<(u32, Fit)> {
    let use_site = input.file_ctx(path, summary);
    match self.kind {
      DanglingKind::Duplicate => self.declarations_of_key(&use_site).into_iter().map(|line| (line, Fit::Breaks)).collect(),
      DanglingKind::Removed => self.resolved(input, &use_site).into_iter().map(|(reference, ..)| (reference.line, Fit::Breaks)).collect(),
      DanglingKind::Changed => self
        .resolved(input, &use_site)
        .into_iter()
        .map(|(reference, accessor, old)| (reference.line, self.fit(input, form, &use_site, reference, accessor, &old)))
        .filter(|(_, fit)| *fit != Fit::Fits)
        .collect(),
    }
  }

  /// The declarations of a duplicate's key in `ctx`: the same key and identity, of the same package and module
  fn declarations_of_key(&self, ctx: &FileCtx<'_>) -> Vec<u32> {
    if ctx.module_root != self.side.module_root || ctx.namespace != self.side.namespace {
      return Vec::new();
    }
    let added = self.old_sites();
    let Some(added) = added.first() else { return Vec::new() };
    ctx
      .summary
      .declarations
      .iter()
      .filter(|declaration| ctx.summary.name(declaration.name) == self.name && !declaration.in_error)
      .map(|declaration| site_of(ctx, declaration))
      .filter(|site| site.key == added.key && has_entries(site) == has_entries(added) && same_entry(site, added))
      .map(|site| site.declaration.name_line)
      .collect()
  }

  /// The references of `use_site` that mean one of the old declarations, with the accessor they name it by, and that
  /// the old form takes: an import compiles with any form, and a call the old form rejects is one the summary misjudges
  fn resolved<'u>(&self, input: &SymbolInput, use_site: &FileCtx<'u>) -> Vec<(&'u Reference, Option<Accessor>, Site<'_>)> {
    let summary = use_site.summary;
    let old = self.old_sites();
    let mut found = Vec::new();
    // its own name, and in the other JVM language the names of its accessors
    let words = self.words();
    let other_language = use_site.language != self.side.language;
    for (word, reference) in words
      .iter()
      .enumerate()
      .filter(|(index, _)| *index == 0 || other_language)
      .flat_map(|(index, word)| summary.references_to(word).map(move |reference| (index, reference)))
    {
      let own = word == 0;
      let accessors: Vec<Accessor> = if own {
        Vec::new()
      } else {
        accessor_names(use_site, reference)
          .into_iter()
          .filter(|(name, _)| name == self.name)
          .map(|(_, accessor)| accessor)
          .collect()
      };
      if !own && accessors.is_empty() {
        continue;
      }
      if reference.role == Role::Import || (self.constructors && reference.role != Role::Call) {
        continue;
      }
      // a removed member whose name the file declares or imports otherwise may be meant there: a member extension
      // moved to the top level, which the file imports now
      if self.kind == DanglingKind::Removed && (summary.declares(self.name) || imports_name(summary, self.name)) {
        continue;
      }
      let accessor = accessors.first().copied();
      let ctx = &*named_by(use_site, accessor);
      let qualifier = reference.qualifier.as_deref().map(|qualifier| names(summary, qualifier));
      let means = old.iter().find(|site| {
        let Some(evidence) = resolve(input, ctx, reference, qualifier.as_deref(), site) else {
          return false;
        };
        // in its own file a member is meant inside its type, or on a receiver of its type
        if evidence == Evidence::SameFile && site.declaration.member == MemberKind::Instance && (!bare(qualifier.as_deref()) || !contains(summary, reference, &site.containers)) {
          return false;
        }
        self.calls(ctx, reference, site)
      });
      if let Some(site) = means {
        found.push((reference, accessor, site.clone()));
      }
    }
    found
  }

  /// Whether the call `reference` (any other use) fits `site`: its parameters accept the arguments, a constructor of
  /// the type for a change of the constructors
  fn calls(&self, use_site: &FileCtx<'_>, reference: &Reference, site: &Site<'_>) -> bool {
    let Some(Shape::Call { arguments, spread, .. }) = reference.shape.and_then(|shape| use_site.summary.shapes.get(shape as usize)) else {
      return !self.constructors;
    };
    if self.constructors {
      return with_constructors(site.clone())
        .iter()
        .filter(|site| site.constructor.is_some())
        .any(|constructor| accepts_call(use_site.summary, arguments, *spread, constructor) && constructor_reaches(use_site, reference, constructor));
    }
    accepts_call(use_site.summary, arguments, *spread, site)
  }

  /// How a use of the old declaration `old` fits the declarations of the key in the branch's tree, the best of them: one
  /// that takes it by the same rules and accepts its call fits as far as its header does (`header_fit`); none breaks it
  fn fit(&self, input: &SymbolInput, form: &TreeForm<'_>, use_site: &FileCtx<'_>, reference: &Reference, accessor: Option<Accessor>, old: &Site<'_>) -> Fit {
    let ctx = &*named_by(use_site, accessor);
    let qualifier = reference.qualifier.as_deref().map(|qualifier| names(use_site.summary, qualifier));
    form
      .declarations
      .iter()
      .map(|&index| site_of(&form.ctx, &form.ctx.summary.declarations[index]))
      .filter(|site| resolve(input, ctx, reference, qualifier.as_deref(), site).is_some() && self.calls(ctx, reference, site))
      .map(|site| if self.constructors { Fit::Fits } else { header_fit(old, &site, reference.coroutine) })
      .max()
      .unwrap_or(Fit::Breaks)
  }

  /// Whether a commit's change of a file, `before` to `after` (each a path and its summary), removes uses the search
  /// counts: it is the change the branch lacks
  pub fn removes_use(&self, input: &SymbolInput, form: &TreeForm<'_>, before: (&str, &FileSummary), after: Option<(&str, &FileSummary)>) -> bool {
    let count = |(path, summary): (&str, &FileSummary)| self.uses(input, form, path, summary).len();
    count(before) > after.map_or(0, count)
  }

  /// Whether a commit's change of a file adds a use of a changed declaration that only the new form takes: the change
  /// needs the branch too
  pub fn needs_new_form(&self, input: &SymbolInput, form: &TreeForm<'_>, before: Option<(&str, &FileSummary)>, after: (&str, &FileSummary)) -> bool {
    if self.kind != DanglingKind::Changed {
      return false;
    }
    let count = |(path, summary): (&str, &FileSummary)| {
      let use_site = input.file_ctx(path, summary);
      let qualifier = |reference: &Reference| reference.qualifier.as_deref().map(|qualifier| names(summary, qualifier));
      summary
        .references_to(self.name)
        .filter(|reference| reference.role != Role::Import)
        .filter(|reference| {
          let new = form.declarations.iter().any(|&index| {
            let site = site_of(&form.ctx, &form.ctx.summary.declarations[index]);
            resolve(input, &use_site, reference, qualifier(reference).as_deref(), &site).is_some() && self.calls(&use_site, reference, &site)
          });
          new
            && !self
              .old_sites()
              .iter()
              .any(|site| resolve(input, &use_site, reference, qualifier(reference).as_deref(), site).is_some() && self.calls(&use_site, reference, site))
        })
        .count()
    };
    count(after) > before.map_or(0, count)
  }
}

#[cfg(test)]
#[path = "declaration_checks_test.rs"]
mod declaration_checks_test;
