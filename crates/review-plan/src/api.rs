//! The API surface of a review (spec/branch/review-plan.spec.md, "API surface"): the public declarations the reviewed
//! commits add, remove or change in form, as the syntax summaries tell (`code_symbols`, through
//! `trial_check::symbol_dependencies::declaration_checks::changed_declarations`).
//!
//! - **Public**: the declaration and every one around it public (Java's `protected` too), none of them a function or a
//!   value; a change counts when the declaration is public before or after it, so a narrowed reach is listed.
//! - **Per key over the commits**, oldest first: the forms of a key in each file before the first commit that touches it
//!   there, against those after the last one; every rename carries the file's forms along (`ApiChange::file`, the path
//!   the facts of the files join on), and those before keep the path they were read at, so a removal names the file and the line
//!   before the commits. The commits need not be
//!   adjacent: with `--since-reviewed` they are the added and rewritten ones, each read against its own parent.
//! - **Any name**: a public declaration of one or two characters is listed too; only the checks of the sync skip them.
//! - **Added**: no form before; **removed**: no form after, or an overload gone while others stay; **changed**: the
//!   forms differ (`Form`: the header, the parameters, the reach, `open`, the constructors, what Java sees); one more
//!   overload is an addition. A change of the body alone, a move between files of one namespace, keep the forms. A
//!   member of a type added or removed with it is the type's entry.
//! - **No override**: a member that the summary marks as an override (Kotlin `override`, Java `@Override`) is no entry:
//!   its supertype declares the API. A change counts as one when it overrides before and after it.
//! - **The query of its usages** (`ApiChange::query`, `fan_in`): `Owner.name` for a member, by the type around it that
//!   is not a Kotlin `Companion` (`ProjectManager.getInstance`, not `Companion.getInstance`); `package.name` for a
//!   top-level declaration of a Kotlin or Java file in a package; else the bare name.
//! - **Shown** as the header with the parameters after the name (`fun changedCall(x:Int, y:Int) : Int`), a type by its
//!   constructors.

use code_symbols::language::{Family, language_of};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use trial_check::diff_scan::CommitDiff;
use trial_check::symbol_dependencies::declaration_checks::{DeclarationForm, Form, changed_declarations};
use trial_check::symbol_scan::SymbolInput;

/// What the reviewed commits do to a public declaration
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum ApiChangeKind {
  /// Gone, or one overload of it gone: a use of it may not compile
  Removed,
  /// Its form changed: a use may need to change
  Changed,
  /// New, or a new overload
  Added,
}

/// The part of the form of a declaration that changed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum FormChange {
  /// The tokens before the body: `suspend`, the return type, the receiver, the parameters outside the JVM
  Header,
  /// The parameters of a JVM function: names, types, defaults, varargs
  Parameters,
  /// Its own reach (`public`, `internal`, `private`)
  Reach,
  /// Whether a subtype may override it
  Open,
  /// The constructors of a type
  Constructors,
  /// What Java sees otherwise: static or not, `@JvmStatic`, `@JvmField`, `@JvmName`
  Jvm,
}

/// A usage of a declaration: a path at the head and a line from 1
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UsageSite {
  pub path: String,
  pub line: u32,
}

/// The usages of a declaration at the head of the range, from the index of the repository (`fan_in`): in the files the
/// reviewed commits change (updated in this change), and in the other files (left outside)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Usages {
  pub inside: u32,
  pub outside: u32,
  /// The first `fan_in::OUTSIDE_SITES` usages left outside, by path and line
  pub outside_sites: Vec<UsageSite>,
  /// False when the index cut its answer at its limit (100): there are more
  pub complete: bool,
  /// Every use is updated in this change: the count is `complete` and none is left outside. An unknown or incomplete
  /// count never is, also at 0
  pub updated: bool,
}

/// A public declaration the reviewed commits add, remove or change
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ApiChange {
  /// The file that declares it after the commits; of a removal, the file before the first commit that touched it, the
  /// old path when that commit renamed it
  pub path: String,
  /// The line of its name there, from 1
  pub line: u32,
  pub name: String,
  /// Namespace, containers and name: what identifies it across the commits
  pub key: String,
  pub kind: ApiChangeKind,
  /// The parts of its form that changed, of a change only
  pub changes: Vec<FormChange>,
  /// Its form before the commits, of a change or a removal: the header with the parameters after the name, or the
  /// constructors of a type
  pub before: Option<String>,
  /// Its form after the commits, of a change or an addition
  pub after: Option<String>,
  /// Of a removal or a change: its usages at the head (`fan_in`); `None` when not asked (a name of
  /// `fan_in::STOP_NAMES`, past `fan_in::MAX_USAGE_QUERIES`) or not answered (no accio, no index, a failed query)
  pub usages: Option<Usages>,
  /// The name its usages are asked by: `Owner.name`, `package.name` or the name
  #[serde(skip)]
  pub query: String,
  /// Of a member: the type of `query` (`Owner`)
  #[serde(skip)]
  pub owner: Option<String>,
  /// The path of its file after the commits, through every rename: the file whose `breaks` it counts in (`files`); differs
  /// from `path` for a removal in a file renamed since
  #[serde(skip)]
  pub file: String,
}

/// The forms of one key in one file: before the first commit that touches it there, after the last one
struct KeyInFile {
  /// The path of the file before that first commit: the lines of `before` are of its content there
  before_path: String,
  before: Vec<DeclarationForm>,
  after: Vec<DeclarationForm>,
}

/// The API changes of `diffs` (oldest first), read with `input`, in the order `rank` sorts them
pub fn api_changes(input: &SymbolInput, diffs: &[CommitDiff]) -> Vec<ApiChange> {
  // key -> file -> forms
  let mut keys: BTreeMap<String, BTreeMap<String, KeyInFile>> = BTreeMap::new();
  for diff in diffs {
    // every rename carries the forms of its file along, also one that changes no declaration
    let renames: Vec<(&str, &str)> = diff
      .files
      .iter()
      .filter_map(|change| {
        change
          .old_path
          .as_deref()
          .filter(|old_path| *old_path != change.path)
          .map(|old_path| (old_path, change.path.as_str()))
      })
      .collect();
    if !renames.is_empty() {
      for files in keys.values_mut() {
        let moved: Vec<(&str, KeyInFile)> = renames.iter().filter_map(|(old_path, path)| files.remove(*old_path).map(|state| (*path, state))).collect();
        for (path, state) in moved {
          files.insert(path.to_string(), state);
        }
      }
    }
    for file in changed_declarations(input, diff) {
      let mut touched: BTreeMap<&str, (Vec<&DeclarationForm>, Vec<&DeclarationForm>)> = BTreeMap::new();
      for form in &file.before {
        touched.entry(&form.key).or_default().0.push(form);
      }
      for form in &file.after {
        touched.entry(&form.key).or_default().1.push(form);
      }
      for (key, (before, after)) in touched {
        let files = keys.entry(key.to_string()).or_default();
        let after: Vec<DeclarationForm> = after.into_iter().cloned().collect();
        match files.get_mut(&file.path) {
          Some(state) => state.after = after,
          None => {
            files.insert(
              file.path.clone(),
              KeyInFile {
                before_path: file.old_path.clone().unwrap_or_else(|| file.path.clone()),
                before: before.into_iter().cloned().collect(),
                after,
              },
            );
          }
        }
      }
    }
  }

  let mut changes: Vec<ApiChange> = keys.into_iter().filter_map(|(key, files)| change_of(&key, &files)).collect();
  // a member of a type added or removed with it
  let whole: HashSet<(ApiChangeKind, String)> = changes
    .iter()
    .filter(|change| change.kind != ApiChangeKind::Changed)
    .map(|change| (change.kind, change.key.clone()))
    .collect();
  changes.retain(|change| {
    let mut outer = change.key.as_str();
    while let Some((parent, _)) = outer.rsplit_once('.') {
      if change.kind != ApiChangeKind::Changed && whole.contains(&(change.kind, parent.to_string())) {
        return false;
      }
      outer = parent;
    }
    true
  });
  rank(&mut changes);
  changes
}

/// The form the card shows: the header with the parameters after the name, else the constructors of a type
fn shown(declaration: &DeclarationForm) -> Option<String> {
  let form = &declaration.form;
  let Some(header) = &form.header else { return form.constructors.clone() };
  let Some(parameters) = &form.parameters else { return Some(header.clone()) };
  let parameters = format!("({})", parameters.replace(',', ", "));
  let mut tokens: Vec<String> = header.split(' ').map(str::to_string).collect();
  match tokens.iter_mut().find(|token| **token == declaration.name) {
    Some(name) => name.push_str(&parameters),
    None => tokens.push(parameters),
  }
  Some(tokens.join(" "))
}

/// A form of a key: the path its line is of, the path of its file after the commits, the form
#[derive(Clone, Copy)]
struct Site<'a> {
  path: &'a str,
  file: &'a str,
  form: &'a DeclarationForm,
}

/// What the commits did to `key`, by its forms in every file they touch it in; `None` when nothing public changed
fn change_of(key: &str, files: &BTreeMap<String, KeyInFile>) -> Option<ApiChange> {
  let before: Vec<Site> = files
    .iter()
    .flat_map(|(file, state)| {
      state.before.iter().map(move |form| Site {
        path: &state.before_path,
        file,
        form,
      })
    })
    .collect();
  let after: Vec<Site> = files
    .iter()
    .flat_map(|(file, state)| state.after.iter().map(move |form| Site { path: file, file, form }))
    .collect();
  // the forms of each side the other side lacks, one for one
  let mut unmatched_after: Vec<Site> = after.clone();
  let mut unmatched_before: Vec<Site> = Vec::new();
  for site in &before {
    match unmatched_after.iter().position(|other| other.form.form == site.form.form) {
      Some(found) => {
        unmatched_after.remove(found);
      }
      None => unmatched_before.push(*site),
    }
  }
  let entry = |kind: ApiChangeKind, site: Site| {
    let (query, owner) = query_of(site);
    ApiChange {
      path: site.path.to_string(),
      line: site.form.line,
      name: site.form.name.clone(),
      key: key.to_string(),
      kind,
      changes: Vec::new(),
      before: None,
      after: None,
      usages: None,
      query,
      owner,
      file: site.file.to_string(),
    }
  };
  match (first_public(&unmatched_before), first_public(&unmatched_after)) {
    (None, None) => None,
    (Some(old), None) if unmatched_after.is_empty() && old.form.overrides => None,
    (None, Some(new)) if unmatched_before.is_empty() && new.form.overrides => None,
    (Some(old), None) if unmatched_after.is_empty() => Some(ApiChange {
      before: shown(old.form),
      ..entry(ApiChangeKind::Removed, old)
    }),
    (None, Some(new)) if unmatched_before.is_empty() => Some(ApiChange {
      after: shown(new.form),
      ..entry(ApiChangeKind::Added, new)
    }),
    (old, new) => {
      // a public form changed, or a form changed its reach to or from public
      let old = old.or_else(|| unmatched_before.first().copied())?;
      let new = new.or_else(|| unmatched_after.first().copied())?;
      if old.form.overrides && new.form.overrides {
        return None;
      }
      Some(ApiChange {
        changes: form_changes(&old.form.form, &new.form.form),
        before: shown(old.form),
        after: shown(new.form),
        ..entry(ApiChangeKind::Changed, new)
      })
    }
  }
}

/// The name the usages of the declaration of `site` are asked by, and its owner: `Owner.name` for a member (the type
/// around it that is not a Kotlin `Companion`), `package.name` for a top-level declaration of a JVM file in a package,
/// else its name
fn query_of(site: Site) -> (String, Option<String>) {
  let declaration = site.form;
  let name = declaration.name.as_str();
  if !declaration.containers.is_empty() {
    return match declaration.containers.iter().rev().find(|container| *container != "Companion") {
      Some(owner) => (format!("{owner}.{name}"), Some(owner.clone())),
      None => (name.to_string(), None),
    };
  }
  let jvm = language_of(site.path).is_some_and(|language| language.family() == Family::Jvm);
  if jvm && !declaration.namespace.is_empty() {
    return (format!("{}.{name}", declaration.namespace.join(".")), None);
  }
  (name.to_string(), None)
}

/// The first public one of `sites`
fn first_public<'a>(sites: &[Site<'a>]) -> Option<Site<'a>> {
  sites.iter().find(|site| site.form.public).copied()
}

/// The parts in which two forms of one declaration differ
fn form_changes(before: &Form, after: &Form) -> Vec<FormChange> {
  [
    (before.header != after.header, FormChange::Header),
    (before.parameters != after.parameters, FormChange::Parameters),
    (before.reach != after.reach, FormChange::Reach),
    (before.overridable != after.overridable, FormChange::Open),
    (before.constructors != after.constructors, FormChange::Constructors),
    (before.jvm != after.jvm, FormChange::Jvm),
  ]
  .into_iter()
  .filter_map(|(differs, change)| differs.then_some(change))
  .collect()
}

/// The order of the card: the most usages left outside first, then the most updated in this change, a change without
/// an answer after those with one; then removals, changes and additions; then by file and line
pub fn rank(changes: &mut [ApiChange]) {
  changes.sort_by(|a, b| {
    let usages = |change: &ApiChange| change.usages.as_ref().map(|usages| (usages.outside, usages.inside));
    usages(b)
      .cmp(&usages(a))
      .then(a.kind.cmp(&b.kind))
      .then_with(|| a.path.cmp(&b.path))
      .then(a.line.cmp(&b.line))
  });
}
