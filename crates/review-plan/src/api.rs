//! The API surface of a review (spec/branch/review-plan.spec.md, "API surface"): the public declarations the reviewed
//! commits add, remove or change in form, as the syntax summaries tell (`code_symbols`, through
//! `trial_check::symbol_dependencies::declaration_checks::changed_declarations`).
//!
//! - **Public**: the declaration and every one around it public (Java's `protected` too), none of them a function or a
//!   value; a change counts when the declaration is API or unstable API before or after it, so a narrowed reach is
//!   listed.
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
//! - **A widening is an addition** (`widens`, `ApiChange::widened`): a change of the reach alone to a wider one, of a
//!   declaration no subtype may override, breaks no use; it is new API, its form before kept. The reach of an
//!   overridable member stays a change (an override must not weaken the access of what it overrides), and so does an
//!   `open` member of a final Kotlin class, whose modality the summary does not read.
//! - **Surface** (`DeclarationForm::surface`, `surface_of`): an override (its supertype declares the API), test code (a
//!   test function, a type that declares one and every member of such a type) and an entry point (`main`) are no API,
//!   no entry of any kind; unstable API (`@ApiStatus.Internal`, `@ApiStatus.Experimental`, `@VisibleForTesting`,
//!   `@TestOnly`, on it or a type around it) is no new API: its addition is no entry, its removal and its change are,
//!   with their usages, as callers in other modules can break. A form both sides have that becomes API from unstable
//!   API or from no API (`@ApiStatus.Internal` removed) is an addition.
//! - **Its usages are asked by its place** (`ApiChange::at`, `fan_in`): the line of its name at the last commit that
//!   touched it, of a removal at the parent of the commit that first touched it; accio answers from the head.
//! - **Shown** as the header with the parameters after the name (`fun changedCall(x:Int, y:Int) : Int`), a type by its
//!   constructors, the reach as the language of the file writes it (`reach_word`).
//! - **The verdict** (`BreakVerdict`, `verdict_of`): whether a use left outside breaks, over every removed and changed
//!   entry.

use code_symbols::language::{Language, language_of};
use code_symbols::summary::Reach;
use file_diffs::accio::DeclarationAt;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use trial_check::diff_scan::CommitDiff;
use trial_check::symbol_dependencies::declaration_checks::{DeclarationForm, Form, ParameterForm, Surface, changed_declarations};
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
  /// New, or a new overload, or opened wider (`ApiChange::widened`)
  Added,
}

/// Whether a use left outside the change breaks, over the removed and changed entries of the API (`verdict_of`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum BreakVerdict {
  /// An entry has a use left outside that can break
  Breaks,
  /// Every removed and changed entry is `unbroken`, also when there is none: an addition and a widening break no use
  NoUseBreaks,
  /// None is known to break, and the uses of an entry are unknown, cut at the limit of the index or tied by the name alone
  Unknown,
  /// The index is asked: the plan with the usages follows
  Pending,
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

/// The usages of a declaration at the head of the range, from the index of the repository (`fan_in`): on the lines the
/// reviewed commits changed (updated in this change), and on the other lines (left outside)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Usages {
  pub inside: u32,
  pub outside: u32,
  /// The first `fan_in::OUTSIDE_SITES` usages left outside, by path and line
  pub outside_sites: Vec<UsageSite>,
  /// The calls left outside that still bind the parameters they bound before the commits: no break
  pub still_bound: u32,
  /// The hits in code left outside that only the name ties to the declaration (a receiver nothing typed, another
  /// language, a namesake's): unknown, never counted as a use
  pub by_name: u32,
  /// False when the index cut its answer at its limit (100): there are more
  pub complete: bool,
  /// No use left outside can break: the count is `complete`, none is left outside that breaks, and none is tied by the
  /// name alone. An unknown or incomplete count never is, also at 0
  pub unbroken: bool,
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
  /// An addition that opens the reach of a declaration no subtype may override wider (`widens`): its form is the same
  /// but for the reach, `before` holds the one it had, and no use of it can break
  pub widened: bool,
  /// The parts of its form that changed, of a change only
  pub changes: Vec<FormChange>,
  /// Its form before the commits, of a change, a removal or a widening: the header with the parameters after the name,
  /// or the constructors of a type
  pub before: Option<String>,
  /// Its form after the commits, of a change or an addition
  pub after: Option<String>,
  /// Of a removal or a change: its usages at the head (`fan_in`); `None` when not answered (no accio, no index, a
  /// failed query, no declaration accio finds at its place)
  pub usages: Option<Usages>,
  /// The place its usages are asked by: the line of its name at the last commit that touched it, of a removal at the
  /// parent of the commit that first touched it; asked at the head
  #[serde(skip)]
  pub at: DeclarationAt,
  /// Of a change or a removal: the parameter lists a call bound before the commits, one per constructor of a type
  #[serde(skip)]
  pub signatures_before: Vec<Vec<ParameterForm>>,
  /// Of a change: the parameter lists a call binds after the commits
  #[serde(skip)]
  pub signatures_after: Vec<Vec<ParameterForm>>,
  /// The path of its file after the commits, through every rename: the file whose `breaks` it counts in (`files`); differs
  /// from `path` for a removal in a file renamed since
  #[serde(skip)]
  pub file: String,
}

/// The forms of one key in one file: before the first commit that touches it there, after the last one
struct KeyInFile {
  /// The path of the file before that first commit: the lines of `before` are of its content there
  before_path: String,
  /// The revision `before` is read at: the parent of that first commit
  before_revision: String,
  before: Vec<DeclarationForm>,
  /// The path of the file at the last commit that touches the key there, and that commit: the lines of `after` are of
  /// its content there; a later rename that changes no declaration moves the file, not them
  after_path: String,
  after_revision: String,
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
          Some(state) => {
            state.after = after;
            state.after_path.clone_from(&file.path);
            state.after_revision.clone_from(&diff.id);
          }
          None => {
            files.insert(
              file.path.clone(),
              KeyInFile {
                before_path: file.old_path.clone().unwrap_or_else(|| file.path.clone()),
                before_revision: format!("{}^", diff.id),
                before: before.into_iter().cloned().collect(),
                after_path: file.path.clone(),
                after_revision: diff.id.clone(),
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

/// The form the card shows, as code reads: its reach as the language of its file writes it (`reach_word`), the header
/// with the parameters after the name, else the constructors of a type, a line each (`shown_constructors`).
/// `public_word`: the reach of the change differs (`reach_differs`), so a public one is written too. Only the display:
/// the forms Rust compares stay as `Form` holds them
fn shown(site: &Site<'_>, public_word: bool) -> Option<String> {
  let declaration = site.form;
  let form = &declaration.form;
  let language = language_of(site.read_path);
  let Some(header) = &form.header else {
    return form
      .constructors
      .as_deref()
      .map(|constructors| shown_constructors(&declaration.name, constructors, form.reach, language, public_word));
  };
  let reach = reach_word(language, form.reach, declaration.containers.is_empty(), public_word);
  let mut tokens: Vec<String> = std::iter::once(reach).chain(header.split(' ')).map(str::to_string).collect();
  if let Some(parameters) = &form.parameters {
    let parameters = format!("({})", shown_parameters(parameters));
    match tokens.iter_mut().find(|token| **token == declaration.name) {
      Some(name) => name.push_str(&parameters),
      None => tokens.push(parameters),
    }
  }
  Some(as_written(tokens.iter().map(String::as_str)))
}

/// The modifier that writes `reach` in `language` (of a declaration at the top level of its file, or of a member): Kotlin
/// `internal`, `private`; Java `private`, its package none; Rust `pub(crate)`, `pub(super)`, private none; TypeScript
/// `private` of a member, a top-level one not exported none; C# `internal`, `private`, `file`. A public reach only with
/// `public_word` (Java and C# `public`, Rust `pub`, a top-level TypeScript `export`; Kotlin none): written in every form,
/// it could be wrong, as the summary holds a `protected` member of Kotlin, Java and TypeScript as public and a Rust
/// trait has public members without `pub`
const fn reach_word(language: Option<Language>, reach: Reach, top_level: bool, public_word: bool) -> &'static str {
  match (language, reach) {
    (Some(Language::Java | Language::CSharp), Reach::Public) if public_word => "public",
    (Some(Language::Rust), Reach::Public) if public_word => "pub",
    (Some(Language::TypeScript), Reach::Public) if public_word && top_level => "export",
    (Some(Language::Kotlin | Language::CSharp), Reach::Module) => "internal",
    (Some(Language::Kotlin | Language::Java | Language::CSharp | Language::TypeScript), Reach::Container) | (Some(Language::Kotlin | Language::Java), Reach::File) => "private",
    (Some(Language::Rust), Reach::Module) => "pub(crate)",
    (Some(Language::Rust), Reach::Parent) => "pub(super)",
    (Some(Language::CSharp), Reach::File) => "file",
    _ => "",
  }
}

/// The constructors of the type `name` (`Form::constructors`: `Reach(name:Type,…)`, `;` apart) as calls of it read, a
/// line each: `Event(text: String, styles, start: Long = …)`, each after the reach a call of it has, the narrower of its
/// own and that of its type (`type_reach`), as `language` writes it (`reach_word`: `private Event()`, `internal
/// Event()`), never the name of a `Reach`
pub(crate) fn shown_constructors(name: &str, constructors: &str, type_reach: Reach, language: Option<Language>, public_word: bool) -> String {
  constructors
    .split(';')
    .filter(|constructor| !constructor.is_empty())
    .map(|constructor| {
      let (reach, parameters) = constructor.split_once('(').unwrap_or((constructor, ""));
      let reach = reach_named(reach).map_or(type_reach, |reach| if reach.wider_than(type_reach) { type_reach } else { reach });
      let visibility = reach_word(language, reach, false, public_word);
      let parameters = shown_parameters(parameters.strip_suffix(')').unwrap_or(parameters));
      if visibility.is_empty() {
        format!("{name}({parameters})")
      } else {
        format!("{visibility} {name}({parameters})")
      }
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// The reach a constructor of `Form::constructors` names (its `Debug` name)
fn reach_named(name: &str) -> Option<Reach> {
  [Reach::Public, Reach::Module, Reach::Parent, Reach::File, Reach::Container]
    .into_iter()
    .find(|reach| format!("{reach:?}") == name)
}

/// Whether two forms of a change differ in a reach: their own, or one of the constructors of a type
fn reach_differs(before: &Form, after: &Form) -> bool {
  before.reach != after.reach || constructor_reaches(before) != constructor_reaches(after)
}

/// The reaches of the constructors of a type (`Form::constructors`), sorted
fn constructor_reaches(form: &Form) -> Vec<&str> {
  let mut reaches: Vec<&str> = form
    .constructors
    .as_deref()
    .unwrap_or_default()
    .split(';')
    .filter(|constructor| !constructor.is_empty())
    .map(|constructor| constructor.split_once('(').map_or(constructor, |(reach, _)| reach))
    .collect();
  reaches.sort_unstable();
  reaches
}

/// A change that only opens the reach of a declaration wider, of one no subtype may override: no use of it can break.
/// An override must not weaken the access of what it overrides, so the reach of an overridable member stays a change
fn widens(changes: &[FormChange], before: &Form, after: &Form) -> bool {
  changes == [FormChange::Reach] && !before.overridable && after.reach.wider_than(before.reach)
}

/// A parameter list as a call sees it (`name:Type?`, `,` apart, `=` for a default, `*` for a vararg) as code reads:
/// `name: Type?`, `= …` for a default, `vararg` before a vararg. A type of several names (`List/Window`: the last name
/// of each type in it, its structure gone) shows no type: `List<Window>`, `String.(Int) -> Unit` cannot be told from it
pub(crate) fn shown_parameters(parameters: &str) -> String {
  parameters
    .split(',')
    .filter(|parameter| !parameter.is_empty())
    .map(|parameter| {
      let (parameter, vararg) = parameter.strip_suffix('*').map_or((parameter, false), |it| (it, true));
      let (parameter, optional) = parameter.strip_suffix('=').map_or((parameter, false), |it| (it, true));
      let (name, type_text) = parameter.split_once(':').unwrap_or((parameter, ""));
      let mut shown = String::new();
      if vararg {
        shown.push_str("vararg ");
      }
      shown.push_str(name);
      if !type_text.is_empty() && !type_text.contains('/') {
        shown.push_str(": ");
        shown.push_str(type_text);
      }
      if optional {
        shown.push_str(" = …");
      }
      shown
    })
    .collect::<Vec<_>>()
    .join(", ")
}

/// The tokens of a form as code is written: no space before `>`, `,`, `)`, `.`, `?`, `:` and `[`, none after `<`, `(`,
/// `.` and `[`, so a header reads `fun hooks(): List<String>`, never `fun hooks() : List < String >`. A `<` is glued only
/// to the name of a type or a function before it (`List<T>`, `first<T>`): after a keyword or a modifier it opens the
/// type parameters of the declaration (`fun <T> String.trim2(): T?`, `static <T> List<T> of()`). A `(` is glued to a
/// name and to the `>` of type parameters before it, the parameters a header holds (Rust `fn open() -> u8`, `fn
/// first<T>(x: T)`), never to `suspend` (`suspend (Int) -> Unit`) or an arrow (`-> (u8, u8)`)
pub(crate) fn as_written<'a>(tokens: impl Iterator<Item = &'a str>) -> String {
  let mut text = String::new();
  let mut previous = "";
  let named = |previous: &str| previous.ends_with(|c: char| c.is_alphanumeric() || c == '_');
  for token in tokens.filter(|token| !token.is_empty()) {
    let glued_before = if token.starts_with('<') {
      named(previous) && !DECLARATION_WORDS.contains(&previous)
    } else if token.starts_with('(') {
      (named(previous) && previous != "suspend") || (previous.ends_with('>') && previous != "->" && previous != "=>")
    } else {
      token.starts_with(['>', ',', ')', '.', '?', ':', '[']) && token != "->" && token != "=>"
    };
    let glued_after = text.ends_with(['<', '(', '.', '[']);
    if !text.is_empty() && !glued_before && !glued_after {
      text.push(' ');
    }
    text.push_str(token);
    previous = token;
  }
  text
}

/// The keywords and modifiers of a header that the type parameters of its declaration may follow (`fun <T>`, `public
/// static <T>`): a `<` after them is no argument of a type
const DECLARATION_WORDS: &[&str] = &[
  "abstract",
  "actual",
  "class",
  "data",
  "default",
  "expect",
  "external",
  "final",
  "fun",
  "infix",
  "inline",
  "interface",
  "internal",
  "native",
  "open",
  "operator",
  "override",
  "private",
  "protected",
  "public",
  "sealed",
  "static",
  "strictfp",
  "suspend",
  "synchronized",
  "tailrec",
  "typealias",
  "val",
  "var",
];

/// A form of a key: the path its line is of, the path and the revision it was read at, the path of its file after the
/// commits, the form
#[derive(Clone, Copy)]
struct Site<'a> {
  path: &'a str,
  read_path: &'a str,
  revision: &'a str,
  file: &'a str,
  form: &'a DeclarationForm,
}

/// Whether a declaration of the surface `before` the commits and `after` them has its entry in the API of a review, or
/// would have it by a change of its form (`change_of`): an addition when it is API after them, any other change when
/// it is API or unstable API on a side; a form both sides have that becomes API (a promotion) is API on its new side
pub(crate) fn listed(before: Option<Surface>, after: Option<Surface>) -> bool {
  match (before, after) {
    (None, after) => after == Some(Surface::Api),
    (Some(before), after) => before != Surface::None || after.is_some_and(|after| after != Surface::None),
  }
}

/// What the commits did to `key`, by its forms in every file they touch it in and their `Surface`; `None` when nothing
/// of the API changed
fn change_of(key: &str, files: &BTreeMap<String, KeyInFile>) -> Option<ApiChange> {
  let before: Vec<Site> = files
    .iter()
    .flat_map(|(file, state)| {
      state.before.iter().map(move |form| Site {
        path: &state.before_path,
        read_path: &state.before_path,
        revision: &state.before_revision,
        file,
        form,
      })
    })
    .collect();
  let after: Vec<Site> = files
    .iter()
    .flat_map(|(file, state)| {
      state.after.iter().map(move |form| Site {
        path: file,
        read_path: &state.after_path,
        revision: &state.after_revision,
        file,
        form,
      })
    })
    .collect();
  // the forms of each side the other side lacks, one for one; of a form both sides have, the first one that becomes API
  let mut unmatched_after: Vec<Site> = after.clone();
  let mut unmatched_before: Vec<Site> = Vec::new();
  let mut promoted: Option<Site> = None;
  for site in &before {
    match unmatched_after.iter().position(|other| other.form.form == site.form.form) {
      Some(found) => {
        let new = unmatched_after.remove(found);
        if promoted.is_none() && site.form.surface != Surface::Api && new.form.surface == Surface::Api {
          promoted = Some(new);
        }
      }
      None => unmatched_before.push(*site),
    }
  }
  let entry = |kind: ApiChangeKind, site: Site| ApiChange {
    path: site.path.to_string(),
    line: site.form.line,
    name: site.form.name.clone(),
    key: key.to_string(),
    kind,
    widened: false,
    changes: Vec::new(),
    before: None,
    after: None,
    usages: None,
    at: DeclarationAt {
      revision: Some(site.revision.to_string()),
      path: site.read_path.to_string(),
      line: site.form.line,
      column: None,
    },
    signatures_before: Vec::new(),
    signatures_after: Vec::new(),
    file: site.file.to_string(),
  };
  let added = |new: Site| ApiChange {
    after: shown(&new, false),
    ..entry(ApiChangeKind::Added, new)
  };
  let listed = [Surface::Api, Surface::Unstable];
  match (unmatched_before.is_empty(), unmatched_after.is_empty()) {
    (true, true) => promoted.map(added),
    // a form gone: unstable API too, its callers can break
    (false, true) => first_of(&unmatched_before, &listed)
      .map(|old| ApiChange {
        before: shown(&old, false),
        signatures_before: old.form.signatures.clone(),
        ..entry(ApiChangeKind::Removed, old)
      })
      .or_else(|| promoted.map(added)),
    // a new form: API only, unstable API and test code are no new API
    (true, false) => first_of(&unmatched_after, &[Surface::Api]).or(promoted).map(added),
    (false, false) => {
      // a form changed on a side that is API or unstable API, or a reach to or from it
      let (old, new) = (first_of(&unmatched_before, &listed), first_of(&unmatched_after, &listed));
      if old.is_none() && new.is_none() {
        return promoted.map(added);
      }
      let old = old.unwrap_or(unmatched_before[0]);
      let new = new.unwrap_or(unmatched_after[0]);
      let changes = form_changes(&old.form.form, &new.form.form);
      if widens(&changes, &old.form.form, &new.form.form) {
        // new API with the form it had; opened to unstable API it is none, as an addition of it is no entry
        return if new.form.surface == Surface::Api {
          Some(ApiChange {
            widened: true,
            before: shown(&old, true),
            after: shown(&new, true),
            ..entry(ApiChangeKind::Added, new)
          })
        } else {
          promoted.map(added)
        };
      }
      let public_word = reach_differs(&old.form.form, &new.form.form);
      Some(ApiChange {
        changes,
        before: shown(&old, public_word),
        signatures_before: old.form.signatures.clone(),
        signatures_after: new.form.signatures.clone(),
        after: shown(&new, public_word),
        ..entry(ApiChangeKind::Changed, new)
      })
    }
  }
}

/// The first one of `sites` of one of `surfaces`
fn first_of<'a>(sites: &[Site<'a>], surfaces: &[Surface]) -> Option<Site<'a>> {
  sites.iter().find(|site| surfaces.contains(&site.form.surface)).copied()
}

/// The parts in which two forms of one declaration differ
pub(crate) fn form_changes(before: &Form, after: &Form) -> Vec<FormChange> {
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
