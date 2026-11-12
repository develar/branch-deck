//! `accio` (github.com/JetBrains/accio), an index of the declarations of a repository, for a declaration and the usages
//! the files of a review lack (openspec/specs/commit/symbol-navigation/spec.md): `accio symbol <Name|Owner.name>` and
//! `accio references <name>`, with `--ref <revision>`, answer from the tree of a revision without a checkout. A review
//! plan asks about declarations by their place (openspec/specs/branch/review-plan/spec.md, "Fan-in";
//! openspec/specs/branch/review-tour/spec.md, "Tests That Reach It"): `accio references --at <path>:<line>[:<column>]` and
//! `accio tests --at … --tests <file>…`, every place of one plan in one process, which accio answers in parallel. It
//! reads what a change registers in the platform ("Registrations") with accio's port of the platform's reader:
//! `accio parse --descriptor --resolve --ref <revision>`, the changed plugin descriptors of one side in one process, the
//! paths on its standard input, each class a descriptor names resolved in the index at that revision. The diffs of a
//! subject ask whether the names it renamed are renamed at every use (openspec/specs/commit/quiet-changes/spec.md,
//! "Subject"): `accio renames --base <revision> --ref <revision>`, every declaration of the subject in one process, the
//! pairs of their places on its standard input; only an accio that says it answers them (`accio versions`,
//! `RENAMES_ANSWER`) is asked.
//!
//! - **One answer per target**: accio answers a JSON array of one entry for each name or place, in the order asked, so
//!   the names of one lookup go in one process (`symbols_any`: `Owner.name`, then the bare name). On the IntelliJ
//!   monorepo one `symbol` of both names takes 179 ms against 201 ms for the two queries one after the other (warm),
//!   and 265 ms against 388 ms at a revision 200 commits from the index (warmed, the median of 9).
//!
//! - **Never an index build**: every query runs with `--no-build`, so accio neither builds, waits for nor refreshes
//!   an index; without a usable index of the checkout it answers `available: false` with the reason in `note` ("no
//!   index of this repository", "accio is building the index of this repository", …), which the lookup answers as no
//!   index (`IndexUnavailable`): the review then looks only at its files, and the plan says why.
//! - **No deadline, a cancel**: one process per lookup, a null stdin, waited for as long as it runs: a warm query of the
//!   IntelliJ monorepo takes 0.03–0.4 s, one at a revision far from the commit of its index reads the changed files
//!   first (2.8 s for 535 of them), and a loaded machine stretches either, so a fixed bound failed answers the user
//!   still waited for. The caller cancels it (`AccioCli::search`): the process is killed within a poll interval and the
//!   lookup fails with `SearchCanceled`, which answers nothing. An index that fails fails no lookup: the review answers
//!   what it has, and says why the index did not (`Lenient`).
//! - **Found** on `PATH` (the login shell's for an app started from the Finder), else in `~/.local/bin`, `~/.cargo/bin`
//!   and Homebrew's directories.
//! - **Below the app ahead of a need**: the queries of a search the user waits for run at the priority of the app; those
//!   of a plan read ahead of its window (`AccioSearch::in_background`, the prefetch after a rewrite of
//!   openspec/specs/branch/review-plan/spec.md) at the lowest (`ChildPriority::Background`), so the index answers them with what
//!   the machine leaves over.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::SystemTime;
use sync_utils::child_process::{ChildPriority, ChildProcessError, cached_login_shell_path, find_executable, run_child_until_superseded, windowless_command_at};

/// The declarations a lookup answers at most
const LIMIT: &str = "20";
/// The seconds `accio surface` may spend on the new modules of a side: the review waits for it beside the usages and
/// the tests (opening the index comes on top, ~5 s)
const SURFACE_BUDGET: &str = "15";
/// The usages a lookup answers at most
const REFERENCES_LIMIT: &str = "100";
/// The revision of the answer of `accio renames` this reader was written for (`answers.renames` of `accio versions`).
/// accio raises it when a field of the answer changes its meaning or `complete` promises something else: an accio
/// of another revision, like one without the command, is not asked, since its answer would be read as a proof
pub const RENAMES_ANSWER: u32 = 1;
/// The tests a lookup answers at most, per declaration: the given tests of a change reach one declaration by the
/// hundred (97 tests of 7 files for one place of idea-1)
const TESTS_LIMIT: &str = "1000";

/// A declaration kind accio answers; an unrecognized kind remains a candidate rather than disappearing from navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HitKind {
  Class,
  Interface,
  Enum,
  Struct,
  Trait,
  Impl,
  Function,
  Method,
  Constructor,
  Field,
  Property,
  Constant,
  Variable,
  #[serde(rename = "type", alias = "typealias", alias = "type_alias")]
  TypeAlias,
  Module,
  Namespace,
  Macro,
  Object,
  EnumMember,
  Record,
  Delegate,
  Section,
  Key,
  ExtensionPoint,
  Extension,
  Action,
  #[serde(rename = "union")]
  Union,
  #[serde(rename = "annotation")]
  Annotation,
  #[serde(other)]
  Unknown,
}

impl HitKind {
  pub const fn name(self) -> &'static str {
    match self {
      Self::Class => "class",
      Self::Interface => "interface",
      Self::Enum => "enum",
      Self::Struct => "struct",
      Self::Trait => "trait",
      Self::Impl => "impl",
      Self::Function => "function",
      Self::Method => "method",
      Self::Constructor => "constructor",
      Self::Field => "field",
      Self::Property => "property",
      Self::Constant => "constant",
      Self::Variable => "variable",
      Self::TypeAlias => "type",
      Self::Module => "module",
      Self::Namespace => "namespace",
      Self::Macro => "macro",
      Self::Object => "object",
      Self::EnumMember => "enum_member",
      Self::Record => "record",
      Self::Delegate => "delegate",
      Self::Section => "section",
      Self::Key => "key",
      Self::ExtensionPoint => "extension_point",
      Self::Extension => "extension",
      Self::Action => "action",
      Self::Union => "union",
      Self::Annotation => "annotation",
      Self::Unknown => "unknown",
    }
  }
}

/// A declaration an index knows: a path of the repository, a line from 1, and the zero-based UTF-16 columns of its place
/// on that line
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SymbolHit {
  #[serde(rename = "file")]
  pub path: String,
  pub line: u32,
  /// Where the declaration is: its written name, or the token of a declaration that writes none (an anonymous class at
  /// `new`); absent from an accio older than 0.16, and for a name it found written nowhere before 0.21
  #[serde(default)]
  pub column: Option<u32>,
  /// Where the name written at `column` ends, `column` itself for a place that writes no name: what a review selects;
  /// absent from an accio older than 0.21
  #[serde(default)]
  pub end_column: Option<u32>,
  pub kind: HitKind,
  /// The declaration in the nine words a review shares (accio's `Kind::group`), the list `declarations::SymbolGroup`
  /// filters both sides of the boundary by; absent from an accio older than it, and from what is no code
  #[serde(default)]
  pub group: Option<String>,
  pub name: String,
  #[serde(default)]
  pub parent: Option<String>,
}

/// How an indexed usage accesses a name; unknown kinds display as `read`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
  Declaration,
  Import,
  Call,
  Write,
  Read,
  Argument,
  Type,
  Supertype,
  Override,
  Doc,
  String,
  #[serde(other)]
  Unknown,
}

/// A usage an index knows: a path, a line from 1 and a column in the units of the index (`Columns::hit` reads it), the
/// text of the line, how it uses the name, and what ties it to the declaration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReferenceHit {
  #[serde(rename = "file")]
  pub path: String,
  pub line: u32,
  pub column: u32,
  #[serde(default)]
  pub snippet: String,
  pub kind: Access,
  pub resolved: Resolved,
  /// The last line of a call, from 1
  #[serde(default)]
  pub end_line: Option<u32>,
  /// The arguments of a call; `None` where the grammar reads no argument list (a Rust macro) or for no call
  #[serde(default)]
  pub arguments: Option<Vec<CallArgument>>,
  /// Of a call tied by a type or an import: whether its arguments bind the parameters of the declaration at the
  /// revision of the answer
  #[serde(default)]
  pub fits: Option<bool>,
}

/// An entry of the `hits` of a `references` answer. An accio that answers lines (its decision
/// 2026-11-11-answer-lists-lines-of-marks) lists a line once, with each usage on it as a mark of the line; an older one
/// lists the usage itself. A review reads usages: a line answers one `ReferenceHit` for each of its marks (`usages`).
#[derive(Deserialize)]
#[serde(untagged)]
enum HitEntry {
  Line(LineHit),
  Usage(ReferenceHit),
}

/// A line of a `references` answer: its place and text, and the usages on it
#[derive(Deserialize)]
struct LineHit {
  #[serde(rename = "file")]
  path: String,
  line: u32,
  #[serde(default)]
  snippet: String,
  marks: Vec<LineMark>,
}

/// A usage on a `LineHit`: what a `ReferenceHit` says beyond its line
#[derive(Deserialize)]
struct LineMark {
  column: u32,
  kind: Access,
  resolved: Resolved,
  #[serde(default)]
  end_line: Option<u32>,
  #[serde(default)]
  arguments: Option<Vec<CallArgument>>,
  #[serde(default)]
  fits: Option<bool>,
}

/// The usages of the `hits` of a `references` answer, in its order: each usage of an older accio, each mark of a line
fn usages<'de, D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Vec<ReferenceHit>, D::Error> {
  let mut hits = Vec::new();
  for entry in Vec::<HitEntry>::deserialize(deserializer)? {
    match entry {
      HitEntry::Usage(hit) => hits.push(hit),
      HitEntry::Line(line) => hits.extend(line.marks.into_iter().map(|mark| ReferenceHit {
        path: line.path.clone(),
        line: line.line,
        column: mark.column,
        snippet: line.snippet.clone(),
        kind: mark.kind,
        resolved: mark.resolved,
        end_line: mark.end_line,
        arguments: mark.arguments,
        fits: mark.fits,
      })),
    }
  }
  Ok(hits)
}

/// What ties a usage to the declaration asked about
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Resolved {
  /// A type the index knows: the receiver, the argument or the type is the declaration's
  Typed,
  /// An import or a `use` of the declaration or of its module
  Import,
  /// The name alone: a receiver nothing typed, another language, prose or a namesake's
  Name,
}

/// An argument of a call, as accio reads it: by its name or its position, and the parameter it binds
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[expect(clippy::struct_excessive_bools, reason = "accio's JSON gives each flag of an argument as a field of its own")]
pub struct CallArgument {
  /// A named argument (Kotlin `name = …`, Python `name=…`)
  #[serde(default)]
  pub name: Option<String>,
  /// A positional argument, from 0
  #[serde(default)]
  pub position: Option<u32>,
  /// `*args`, `...rest`; `named` for `**kwargs`
  #[serde(default)]
  pub spread: bool,
  #[serde(default)]
  pub named: bool,
  /// A trailing lambda (Kotlin `f(x) { … }`)
  #[serde(default)]
  pub trailing_lambda: bool,
  /// The parameter it binds at the revision of the answer, of a call tied by a type or an import
  #[serde(default)]
  pub parameter: Option<String>,
  /// It binds no parameter: the argument that makes the call misfit
  #[serde(default)]
  pub unbound: bool,
}

/// A declaration by its place: the line and optional byte column of its name in `path` at `revision`, `None` for the revision the query
/// answers from (for a removed declaration, the parent of the commit that removed it)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclarationAt {
  pub revision: Option<String>,
  pub path: String,
  pub line: u32,
  /// The column of the declared name in the units the index takes (`Columns::at`), when the caller knows it
  pub column: Option<u32>,
}

/// How accio counts the column it takes in `--at` and answers in a `references` hit. An accio before 0.21 counts bytes
/// from 1 in both; from 0.21 (accio's decision 2026-11-09-one-column-unit) `--at` counts characters from 1 and a hit's
/// `column` UTF-16 units from 0, as a review does. Every column a review sends to the index or reads from it goes
/// through these two conversions, so no reader guesses a column from the words of a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Columns {
  Bytes,
  Characters,
}

impl Columns {
  /// The units of the accio whose `--version` printed `version` (`accio 0.21.0`, `accio 0.21.0 (source)`): `Bytes` for
  /// one before 0.21 or one it cannot read
  pub fn of_version(version: &str) -> Self {
    let number = version.split_whitespace().nth(1).unwrap_or_default();
    let mut parts = number.split('.').map(|it| it.parse::<u32>().unwrap_or(0));
    let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    if (major, minor) >= (0, 21) { Self::Characters } else { Self::Bytes }
  }

  /// The `--at` column of the zero-based UTF-16 `column` on line `line` (from 0) of `text`; `None` past the line's end
  /// or inside a character
  pub fn at(self, text: &str, line: u32, column: u32) -> Option<u32> {
    let (byte, character, _) = boundaries(line_of(text, line)?).find(|&(_, _, utf16)| utf16 == column)?;
    u32::try_from(1 + if self == Self::Bytes { byte } else { character }).ok()
  }

  /// The zero-based UTF-16 column of a `references` hit's `column` on line `line` (from 0) of `text`; `None` past the
  /// line's end or inside a character (a hit of another text)
  pub fn hit(self, text: &str, line: u32, column: u32) -> Option<u32> {
    let mut found = boundaries(line_of(text, line)?);
    match self {
      Self::Bytes => found.find(|&(byte, _, _)| byte + 1 == column as usize).map(|(_, _, utf16)| utf16),
      Self::Characters => found.find(|&(_, _, utf16)| utf16 == column).map(|(_, _, utf16)| utf16),
    }
  }
}

/// Line `line` (from 0) of `text`, without its line end
fn line_of(text: &str, line: u32) -> Option<&str> {
  text.split('\n').nth(line as usize).map(|it| it.trim_end_matches('\r'))
}

/// Each character boundary of `line`, its end included: the bytes, the characters and the UTF-16 units before it
fn boundaries(line: &str) -> impl Iterator<Item = (usize, usize, u32)> + '_ {
  let mut utf16 = 0u32;
  line.char_indices().map(Some).chain(std::iter::once(None)).enumerate().map(move |(character, it)| {
    let here = (it.map_or(line.len(), |(byte, _)| byte), character, utf16);
    utf16 += it.map_or(0, |(_, c)| u32::try_from(c.len_utf16()).unwrap_or(0));
    here
  })
}

impl DeclarationAt {
  /// The `--at` argument: `<path>:<line>[:<column>]`, `<revision>:<path>:<line>[:<column>]`
  pub fn argument(&self) -> String {
    let place = self
      .column
      .map_or_else(|| format!("{}:{}", self.path, self.line), |column| format!("{}:{}:{column}", self.path, self.line));
    match &self.revision {
      Some(revision) => format!("{revision}:{place}"),
      None => place,
    }
  }
}

/// A given test that reaches a declaration through its callers (`accio tests --tests`)
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReachingTest {
  #[serde(rename = "file")]
  pub path: String,
  pub line: u32,
  /// `Owner.name` of the test
  pub declaration: String,
  /// The name the test reads as: a Kotlin name in backticks, the words of `snake_case` or `testCamelCase`
  pub reads_as: String,
  /// `junit5`, `rust`, `vitest`, …
  pub framework: String,
  /// The calls between the test and the declaration: 1 when the test calls it
  pub depth: u32,
  /// The callers on the way, from the test's callee to the declaration's caller
  #[serde(default)]
  pub via: Vec<TestStep>,
  /// What its way goes through
  pub class: TestClass,
  /// Its way goes through a caller so many call that accio walked it only for the given tests
  #[serde(default)]
  pub through_hub: bool,
}

/// What the way from a test to a declaration goes through, nearest first
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TestClass {
  /// The test calls it
  Direct,
  /// Only the code of tests is on the way: a fixture, a helper of the test
  TestCode,
  /// Product code is on the way: an end-to-end test
  Product,
}

/// A caller on the way from a test to a declaration
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TestStep {
  pub declaration: String,
  #[serde(rename = "file")]
  pub path: String,
  pub line: u32,
}

/// The given tests that reach one declaration
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct TestReach {
  #[serde(default)]
  pub tests: Vec<ReachingTest>,
  /// accio walked every way from the given tests to it: a given test it lists not is one that does not reach it
  #[serde(default)]
  pub complete: bool,
  /// accio found no declaration at the place asked (a line that names none, a declaration it does not index): no answer,
  /// never "no test"
  #[serde(default)]
  pub unavailable: bool,
}

/// The declarations of a module that something outside it uses (`accio surface`, accio#541), its answer for one
/// `--module`: what enters the module, a class a plugin descriptor registers first
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ModuleSurface {
  /// The module's directory as asked
  pub module: String,
  /// Every declaration was asked and no scan was cut (the budget, the index): what it lists is all that enters
  #[serde(default)]
  pub complete: bool,
  /// Why the answer is partial (`the budget of 15s ran out: …`) or empty (no usable index)
  #[serde(default)]
  pub note: Option<String>,
  #[serde(default)]
  pub declarations: Vec<SurfaceDeclaration>,
}

/// A declaration of a module used outside it (`ModuleSurface::declarations`)
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct SurfaceDeclaration {
  /// Its file at the revision asked, from the root of the repository
  pub file: String,
  /// The line of its name, from 1
  pub line: u32,
  /// accio's kind: `class`, `function`, …
  #[serde(default)]
  pub kind: String,
  pub name: String,
  /// Its qualified name; absent where the language qualifies none
  #[serde(default)]
  pub qualified: Option<String>,
  /// Every file with a resolved or descriptor use of it, its first such line, by path
  #[serde(default)]
  pub used_by: Vec<SurfaceSite>,
  /// Its uses, a registration first, at most `--limit` of them; one tied by the name alone is `resolved: "name"`
  #[serde(default)]
  pub uses: Vec<SurfaceUse>,
}

/// A file and a line of it, from 1
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct SurfaceSite {
  pub file: String,
  pub line: u32,
}

/// A use of a declaration of a module from outside it
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct SurfaceUse {
  pub file: String,
  pub line: u32,
  /// `call`, `type`, `import`, …, or `descriptor` for a class a plugin descriptor registers
  #[serde(default)]
  pub label: String,
  /// Of a `descriptor` use: the extension point (`com.intellij.applicationService`) and what the class is registered
  /// for (the interface of a service, the topic of a listener)
  #[serde(default)]
  pub point: Option<String>,
  #[serde(default)]
  pub of: Option<String>,
  /// How the use was tied to the declaration: `typed`, `import`, or `name` for a guess
  #[serde(default)]
  pub resolved: Option<String>,
  /// The use is in test code
  #[serde(default)]
  pub test: bool,
}

/// A plugin descriptor at a revision as the platform's reader reads it (`accio parse --descriptor --resolve`, one JSON
/// line per path, `plugin_xml::dump_line` of accio, whose fields are only ever added): what it registers, and each class
/// it names resolved in the index at that revision
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
  /// As asked, relative to the root of the repository
  pub path: String,
  /// The revision has no file at the path
  #[serde(default)]
  pub missing: bool,
  /// The name of its root element: `idea-plugin` for a descriptor; `None` for malformed XML
  #[serde(default)]
  pub root: Option<String>,
  /// The XML is malformed, or the platform's reader refuses the descriptor
  #[serde(default)]
  pub error: bool,
  #[serde(default)]
  pub id: Option<String>,
  #[serde(default)]
  pub content: Vec<DescriptorContentModule>,
  #[serde(default)]
  pub extension_points: Vec<DescriptorExtensionPoint>,
  #[serde(default)]
  pub services: Vec<DescriptorService>,
  #[serde(default)]
  pub listeners: Vec<DescriptorListener>,
  #[serde(default)]
  pub extensions: Vec<DescriptorExtension>,
  #[serde(default)]
  pub actions: Vec<DescriptorAction>,
  /// Every class it names by the field that names it, resolved at the revision; `None` without a usable index (`note`
  /// says why)
  #[serde(default)]
  pub classes: Option<Vec<DescriptorClass>>,
  /// Why `classes` is `None`, phrased as `IndexUnavailable`
  #[serde(default)]
  pub note: Option<String>,
}

impl Descriptor {
  /// It is a plugin descriptor the platform reads: there, its root `idea-plugin`, and read without an error
  pub fn is_read(&self) -> bool {
    !self.missing && !self.error && self.root.as_deref() == Some("idea-plugin")
  }
}

/// A content module of a plugin descriptor (`<content><module name loading>`)
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct DescriptorContentModule {
  pub name: String,
  /// `OPTIONAL`, `REQUIRED`, `EMBEDDED`, `ON_DEMAND`
  #[serde(default)]
  pub loading: String,
}

/// An extension point a descriptor declares, in its area (`app`, `project`, `module`)
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DescriptorExtensionPoint {
  pub area: String,
  #[serde(default)]
  pub name: Option<String>,
  #[serde(default)]
  pub qualified_name: Option<String>,
  #[serde(default)]
  pub interface: Option<String>,
  #[serde(default)]
  pub bean_class: Option<String>,
  #[serde(default)]
  pub dynamic: bool,
}

/// A service a descriptor registers, in its area (`app`, `project`, `module`)
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DescriptorService {
  pub area: String,
  #[serde(default)]
  pub interface: Option<String>,
  #[serde(default)]
  pub implementation: Option<String>,
  #[serde(default)]
  pub test: Option<String>,
  #[serde(default)]
  pub headless: Option<String>,
  #[serde(default)]
  pub schema_key: Option<String>,
  #[serde(default)]
  pub overrides: bool,
  #[serde(default)]
  pub open: bool,
  #[serde(default)]
  pub preload: Option<String>,
  #[serde(default)]
  pub client: Option<String>,
  #[serde(default)]
  pub os: Option<String>,
}

/// A listener a descriptor registers on a topic, in its area (`app`, `project`)
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DescriptorListener {
  pub area: String,
  pub class: String,
  pub topic: String,
  #[serde(default)]
  pub test_mode: bool,
  #[serde(default)]
  pub headless_mode: bool,
  #[serde(default)]
  pub os: Option<String>,
}

/// An extension a descriptor registers on a point, by the point's qualified name
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DescriptorExtension {
  pub point: String,
  /// Its attribute `implementation` (or `implementationClass`)
  #[serde(default)]
  pub implementation: Option<String>,
  /// Its attribute `id`
  #[serde(default)]
  pub order_id: Option<String>,
  #[serde(default)]
  pub element: Option<DescriptorElement>,
}

/// An entry of the `<actions>` of a descriptor: an action, a group (whose element holds its nested actions and groups),
/// a reference
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct DescriptorAction {
  /// `action`, `group`, …
  pub kind: String,
  #[serde(default)]
  pub element: Option<DescriptorElement>,
}

/// An XML element as the reader keeps it: its attributes by name, its children in document order
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct DescriptorElement {
  #[serde(default)]
  pub name: String,
  #[serde(default)]
  pub attributes: std::collections::BTreeMap<String, String>,
  #[serde(default)]
  pub children: Vec<Self>,
}

/// A class a descriptor names, by the field that names it (`services.implementation`, `listeners.topic`, …), resolved
/// to its declaration at the revision of the answer
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct DescriptorClass {
  pub role: String,
  pub name: String,
  /// `exact`, `ambiguous` (`count` declarations), `none`
  pub resolved: String,
  #[serde(default)]
  pub count: Option<u32>,
  #[serde(default)]
  pub path: Option<String>,
  #[serde(default)]
  pub line: Option<u32>,
}

/// The usages an index answered, and its name-only candidates excluded by `--resolved`
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct References {
  /// The usages, also of an accio that lists them by line (`usages`)
  #[serde(default, deserialize_with = "usages")]
  pub hits: Vec<ReferenceHit>,
  /// Name-only hits excluded by `--resolved`
  #[serde(default)]
  pub by_name: usize,
  /// False when the limit cut the answer
  #[serde(default = "complete_by_default")]
  pub complete: bool,
  /// Of a query by place: accio found no declaration there, as `TestReach::unavailable`; never "no use"
  #[serde(default)]
  pub unavailable: bool,
}

const fn complete_by_default() -> bool {
  true
}

/// A declaration a change may have renamed, by the place of its name in the old tree and in the new one:
/// `<path>:<line>:<column>`, the path from the root of the repository, the line and the column from 1, the column in
/// characters, as `--at` takes a place (`Columns::Characters`)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RenamePair {
  pub old: String,
  pub new: String,
}

/// What accio says of a `RenamePair` between two trees (`accio renames`, answer revision `RENAMES_ANSWER`; the
/// contract is accio's decision 2026-11-11-rename-read-in-two-trees). Only the fields a diff decides by are read
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct RenameAnswer {
  /// The pair could not be asked: a place names no declaration, or reads as none. Never "no use"
  #[serde(default)]
  pub unavailable: bool,
  /// The two places are one declaration: the same kind, language, owners and signature apart from the name
  #[serde(default)]
  pub same: bool,
  /// Every candidate file of both trees was read, every use listed and tied by more than its name, and every
  /// relation of the declaration read: only then are the uses all there are, and no conflict "none"
  #[serde(default)]
  pub complete: bool,
  #[serde(default)]
  pub old: Option<RenamedDeclaration>,
  #[serde(default)]
  pub new: Option<RenamedDeclaration>,
  /// The uses of the old declaration in the old tree, and of the new one in the new tree; absent when the budget of
  /// the call ran out first
  #[serde(default)]
  pub base: Option<RenameUses>,
  #[serde(default)]
  pub head: Option<RenameUses>,
  /// What the rename changed beyond the name
  #[serde(default)]
  pub conflicts: Vec<RenameConflict>,
}

/// A declaration of a `RenameAnswer`
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct RenamedDeclaration {
  /// accio's name of its language: `kotlin`, `java`, …
  #[serde(default)]
  pub language: String,
}

/// The uses of a declaration in one tree
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct RenameUses {
  /// By file, line and column, the declaration itself among them; at most the limit of the call
  #[serde(default)]
  pub uses: Vec<RenameUse>,
  /// The uses found: more than `uses` holds when the limit cut them
  #[serde(default)]
  pub total: usize,
}

/// A use of a declaration: a path of its tree, a line from 1 and the zero-based UTF-16 columns of the word written
/// there, the two the same for a use that writes no word (a positional argument that sets a property, a constructor
/// delegation)
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RenameUse {
  #[serde(rename = "file")]
  pub path: String,
  pub line: u32,
  pub column: u32,
  pub end_column: u32,
  pub kind: Access,
  /// What ties it to the declaration: a use tied by its name alone proves nothing
  pub resolved: Resolved,
  /// The word written there when it is not the name of the declaration: an alias of an import, the accessor of a
  /// property in another language. A rename of the declaration asks nothing of such a word
  #[serde(default)]
  pub text: Option<String>,
}

/// What a rename changed beyond the name: `left` (a use the change did not rename, in a file it left alone),
/// `captured`, `overrides`, `overridden`, `overload`, `hides`, `hidden`, `namesake`
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct RenameConflict {
  pub kind: String,
}

/// The declarations and the usages of a repository by name: accio in the app, a fixture in the e2e server
pub trait SymbolIndex: Send + Sync {
  /// The declarations of `query` (`Name`, `Owner.name`) in the tree of `revision` (the working tree for `None`);
  /// `None` when the repository has no index
  fn symbols(&self, repository_root: &str, query: &str, revision: Option<&str>) -> Result<Option<Vec<SymbolHit>>>;

  /// The declarations of each name of `queries`, in their order, asked in one process; `None` when the repository has
  /// no index. An index that answers one name at a time asks one after the other
  fn symbols_any(&self, repository_root: &str, queries: &[&str], revision: Option<&str>) -> Result<Option<Vec<Vec<SymbolHit>>>> {
    let mut answers = Vec::with_capacity(queries.len());
    for query in queries {
      let Some(hits) = self.symbols(repository_root, query, revision)? else {
        return Ok(None);
      };
      answers.push(hits);
    }
    Ok(Some(answers))
  }

  /// How the index counts the columns it takes and answers (`Columns`); an index that does not say counts as an accio
  /// before 0.21
  fn columns(&self) -> Columns {
    Columns::Bytes
  }

  /// The whole-word usages of `name` in the tree of `revision`; `None` when the repository has no index
  fn references(&self, repository_root: &str, name: &str, revision: Option<&str>) -> Result<Option<References>>;

  /// The usages in the tree of `revision` of each declaration of `declarations`, in their order; `None` when the
  /// repository has no index
  fn references_at(&self, repository_root: &str, declarations: &[DeclarationAt], revision: &str) -> Result<Option<Vec<References>>>;

  /// The usages tied to each declaration by a type or import; name-only hits are counted, not returned. `None` uses the working tree.
  fn references_at_resolved(&self, repository_root: &str, declarations: &[DeclarationAt], revision: Option<&str>) -> Result<Option<Vec<References>>> {
    let Some(revision) = revision else { return Ok(None) };
    self.references_at(repository_root, declarations, revision)
  }

  /// The tests of `given` (test files of the tree of `revision`) that reach each declaration of `declarations` through
  /// any number of calls, only by calls tied by a type or an import, in their order; `None` when the repository has no
  /// index
  fn tests(&self, repository_root: &str, declarations: &[DeclarationAt], given: &[String], revision: &str) -> Result<Option<Vec<TestReach>>>;

  /// Each file of `paths` in the tree of `revision` read as a plugin descriptor, in their order, each class it names
  /// resolved at that revision, asked in one process; `None` when the index reads no descriptor
  fn descriptors(&self, _repository_root: &str, _paths: &[String], _revision: &str) -> Result<Option<Vec<Descriptor>>> {
    Ok(None)
  }

  /// What enters each of `modules` (their directories) at `revision`: its declarations something outside it uses, in
  /// the order of `modules`; `None` when the index answers none
  fn surface(&self, _repository_root: &str, _modules: &[String], _revision: &str) -> Result<Option<Vec<ModuleSurface>>> {
    Ok(None)
  }

  /// What each of `pairs` is between the tree of `base` and the tree of `head`, in their order, asked in one process:
  /// whether the two places are one declaration renamed, its uses in both trees and what the rename changed beyond
  /// the name; `None` when the index answers no renames (an accio of another answer revision, a fixture without them)
  fn renames(&self, _repository_root: &str, _pairs: &[RenamePair], _base: &str, _head: &str) -> Result<Option<Vec<RenameAnswer>>> {
    Ok(None)
  }
}

/// accio has no usable index of the repository (`--no-build` answered `available: false`): why, with a format mismatch
/// phrased as a version and an action. A lookup through `Lenient` answers it as no index, and keeps the reason
#[derive(Debug)]
pub struct IndexUnavailable(pub String);

impl std::fmt::Display for IndexUnavailable {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str(&self.0)
  }
}

impl std::error::Error for IndexUnavailable {}

/// The lookup was canceled (a newer navigation of the window, Escape, Cancel) and its query killed: it answers nothing
#[derive(Debug)]
pub struct SearchCanceled;

impl std::fmt::Display for SearchCanceled {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str("the search was canceled")
  }
}

impl std::error::Error for SearchCanceled {}

/// The `accio` executable of this machine
pub struct AccioCli {
  executable: PathBuf,
  /// Its `Columns`, from its `--version`
  columns: OfBuild<Columns>,
  /// The revision of its answer of `renames`, from its `versions`; `None` for an accio without it
  renames_answer: OfBuild<Option<u32>>,
}

/// What an executable says of itself, asked once of each build of it: again once its file is another one, by its
/// size and the time it was written. The app outlives an update of accio, and what the build before said (how it
/// counts columns, which revision of an answer it gives) is not what the new one does: an answer of another
/// revision read as the known one would be read as a proof. A file that cannot be read is asked every time
pub(crate) struct OfBuild<T>(Mutex<Option<((SystemTime, u64), T)>>);

impl<T: Copy> OfBuild<T> {
  pub(crate) const fn new() -> Self {
    Self(Mutex::new(None))
  }

  /// What the build of `executable` that is there now says: the known answer of it, else what `ask` answers
  pub(crate) fn of(&self, executable: &Path, ask: impl FnOnce() -> T) -> T {
    let build = std::fs::metadata(executable).ok().and_then(|file| Some((file.modified().ok()?, file.len())));
    let known = *self.0.lock().unwrap_or_else(PoisonError::into_inner);
    if let (Some(build), Some((of, said))) = (build, known)
      && of == build
    {
      return said;
    }
    // asked without the lock: two callers that ask at once get the same answer
    let said = ask();
    *self.0.lock().unwrap_or_else(PoisonError::into_inner) = build.map(|build| (build, said));
    said
  }
}

/// The lookups of one navigation: each query runs until it answers, at `priority`, and is killed once `canceled` turns
/// true
pub struct AccioSearch<'a> {
  accio: &'a AccioCli,
  canceled: &'a (dyn Fn() -> bool + Sync),
  priority: ChildPriority,
}

impl AccioCli {
  /// accio, if this machine has it; looked for once per process
  pub fn discover() -> Option<&'static Self> {
    static FOUND: OnceLock<Option<AccioCli>> = OnceLock::new();
    FOUND
      .get_or_init(|| {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let path = std::env::var_os("PATH");
        // an app started from the Finder has a short PATH: the login shell's has the user's directories
        find_accio(home.as_deref(), path.as_deref())
          .or_else(|| cached_login_shell_path().and_then(|path| find_executable("accio", path)))
          .map(Self::new)
      })
      .as_ref()
  }

  /// The index of the repository for one navigation, which the caller cancels through `canceled`; its queries run at
  /// the priority of the app
  pub fn search<'a>(&'a self, canceled: &'a (dyn Fn() -> bool + Sync)) -> AccioSearch<'a> {
    AccioSearch {
      accio: self,
      canceled,
      priority: ChildPriority::Normal,
    }
  }

  const fn new(executable: PathBuf) -> Self {
    Self {
      executable,
      columns: OfBuild::new(),
      renames_answer: OfBuild::new(),
    }
  }

  /// accio at `executable`, for a test
  #[cfg(test)]
  pub(crate) const fn at(executable: PathBuf) -> Self {
    Self::new(executable)
  }

  /// Where this accio is, for a test that runs it itself
  #[cfg(test)]
  pub(crate) fn executable(&self) -> &Path {
    &self.executable
  }

  /// Whether this accio answers `renames` as this reader reads it (`RENAMES_ANSWER`), from `accio versions`, asked
  /// once of each build of it (`OfBuild`): its `--version` does not say it (a build from source prints the version
  /// of the last release). An accio without the answer, one that fails and one of another revision do not
  pub(crate) fn answers_renames(&self) -> bool {
    let revision = self.renames_answer.of(&self.executable, || {
      let output = windowless_command_at(&self.executable, ChildPriority::Normal).arg("versions").output().ok()?;
      let versions: serde_json::Value = serde_json::from_slice(&output.stdout).ok().filter(|_| output.status.success())?;
      versions.get("answers")?.get("renames")?.as_u64().and_then(|revision| u32::try_from(revision).ok())
    });
    revision == Some(RENAMES_ANSWER)
  }

  /// How this accio counts columns (`Columns::of_version` of its `--version`), asked once of each build of it
  /// (`OfBuild`); `Bytes` when it answers none
  pub fn columns(&self) -> Columns {
    self.columns.of(&self.executable, || {
      let output = windowless_command_at(&self.executable, ChildPriority::Normal).arg("--version").output();
      output
        .ok()
        .filter(|it| it.status.success())
        .map_or(Columns::Bytes, |it| Columns::of_version(&String::from_utf8_lossy(&it.stdout)))
    })
  }
}

fn index_unavailable_note(note: &str, executable: &Path) -> String {
  let Some((_, versions)) = note.rsplit_once(" has format ") else {
    return note.to_string();
  };
  let Some((index, expected)) = versions.split_once(", expected ") else {
    return note.to_string();
  };
  let (Ok(index), Ok(expected)) = (index.parse::<u32>(), expected.parse::<u32>()) else {
    return note.to_string();
  };
  let (article, version, action) = match index.cmp(&expected) {
    std::cmp::Ordering::Greater => ("a", "newer", "update accio"),
    std::cmp::Ordering::Less => ("an", "older", "run accio index"),
    std::cmp::Ordering::Equal => return note.to_string(),
  };
  format!("the index of this repository is from {article} {version} accio than {}: {action}", executable.display())
}

/// accio on `path`, else in the directories it installs to
fn find_accio(home: Option<&Path>, path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
  if let Some(found) = path.and_then(|path| find_executable("accio", path)) {
    return Some(found);
  }
  let name = if cfg!(windows) { "accio.exe" } else { "accio" };
  let homebrew: &[&str] = if cfg!(windows) { &[] } else { &["/opt/homebrew/bin/accio", "/usr/local/bin/accio"] };
  home
    .into_iter()
    .flat_map(|home| [home.join(".local/bin").join(name), home.join(".cargo/bin").join(name)])
    .chain(homebrew.iter().map(PathBuf::from))
    .find(|candidate| candidate.is_file())
}

#[derive(Deserialize)]
struct Answer {
  hits: Vec<SymbolHit>,
}

/// The targets of a query: names after `--`, or declarations by `--at`
enum Targets<'a> {
  Names(&'a [&'a str]),
  At(&'a [DeclarationAt]),
}

impl AccioSearch<'_> {
  /// The same search, whose queries run below the app (`ChildPriority::Background`): no user waits for their answer yet
  #[must_use]
  pub const fn in_background(self) -> Self {
    Self {
      priority: ChildPriority::Background,
      ..self
    }
  }

  /// The JSON answer of `accio <command> --no-build … <targets>` at `revision`, one entry per target in their order;
  /// `IndexUnavailable` when accio has no usable index of the repository
  fn query<T: serde::de::DeserializeOwned>(&self, command_name: &str, options: &[&str], repository_root: &str, targets: &Targets<'_>, revision: Option<&str>) -> Result<Vec<T>> {
    let mut command = windowless_command_at(&self.accio.executable, self.priority);
    command.args([command_name, "--no-build", "--format", "json", "--project", repository_root]);
    command.args(options);
    if let Some(revision) = revision {
      command.args(["--ref", revision]);
    }
    let asked = match targets {
      Targets::Names(names) => {
        command.arg("--").args(*names);
        names.len()
      }
      Targets::At(declarations) => {
        for declaration in *declarations {
          command.arg("--at").arg(declaration.argument());
        }
        declarations.len()
      }
    };
    let stdout = self.run(&mut command, None)?;
    self.entries(&stdout, asked)
  }

  /// The entries of a JSON answer of accio to `asked` targets, one for each in their order; `IndexUnavailable` when an
  /// entry says accio has no usable index of the repository
  fn entries<T: serde::de::DeserializeOwned>(&self, stdout: &[u8], asked: usize) -> Result<Vec<T>> {
    let entries: Vec<serde_json::Value> = serde_json::from_slice(stdout).context("the answer of accio")?;
    if let Some(unavailable) = entries.iter().find(|entry| entry.get("available").and_then(serde_json::Value::as_bool) == Some(false)) {
      let note = unavailable
        .get("note")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("accio has no index of this repository");
      return Err(IndexUnavailable(index_unavailable_note(note, &self.accio.executable)).into());
    }
    if entries.len() != asked {
      bail!("accio answered {} entries for {asked} targets", entries.len());
    }
    entries.into_iter().map(|entry| serde_json::from_value(entry).context("the answer of accio")).collect()
  }

  /// The standard output of `command`, given `stdin`, run until it answers; `SearchCanceled` once the search is
  /// canceled, an error with what accio printed when it fails
  fn run(&self, command: &mut Command, stdin: Option<Vec<u8>>) -> Result<Vec<u8>> {
    let output = run_child_until_superseded(command, stdin, self.canceled).map_err(|error| match error {
      ChildProcessError::Superseded => anyhow::Error::new(SearchCanceled),
      other => anyhow::anyhow!("{other:?}"),
    })?;
    if !output.status.success() {
      bail!("{}", output.failure_message("accio"));
    }
    Ok(output.stdout)
  }
}

impl SymbolIndex for AccioSearch<'_> {
  fn columns(&self) -> Columns {
    self.accio.columns()
  }

  fn symbols(&self, repository_root: &str, query: &str, revision: Option<&str>) -> Result<Option<Vec<SymbolHit>>> {
    let entries = self.query::<Answer>("symbol", &["--limit", LIMIT], repository_root, &Targets::Names(&[query]), revision)?;
    Ok(Some(entries.into_iter().flat_map(|entry| entry.hits).collect()))
  }

  /// Every name of one lookup in one process: accio answers an entry per name, in the order asked
  fn symbols_any(&self, repository_root: &str, queries: &[&str], revision: Option<&str>) -> Result<Option<Vec<Vec<SymbolHit>>>> {
    if queries.is_empty() {
      return Ok(Some(Vec::new()));
    }
    let entries = self.query::<Answer>("symbol", &["--limit", LIMIT], repository_root, &Targets::Names(queries), revision)?;
    Ok(Some(entries.into_iter().map(|entry| entry.hits).collect()))
  }

  fn references(&self, repository_root: &str, name: &str, revision: Option<&str>) -> Result<Option<References>> {
    let entries = self.query::<References>("references", &["--limit", REFERENCES_LIMIT], repository_root, &Targets::Names(&[name]), revision)?;
    Ok(entries.into_iter().next())
  }

  fn references_at(&self, repository_root: &str, declarations: &[DeclarationAt], revision: &str) -> Result<Option<Vec<References>>> {
    if declarations.is_empty() {
      return Ok(Some(Vec::new()));
    }
    self
      .query("references", &["--limit", REFERENCES_LIMIT], repository_root, &Targets::At(declarations), Some(revision))
      .map(Some)
  }

  fn references_at_resolved(&self, repository_root: &str, declarations: &[DeclarationAt], revision: Option<&str>) -> Result<Option<Vec<References>>> {
    if declarations.is_empty() {
      return Ok(Some(Vec::new()));
    }
    self
      .query(
        "references",
        &["--limit", REFERENCES_LIMIT, "--resolved"],
        repository_root,
        &Targets::At(declarations),
        revision,
      )
      .map(Some)
  }

  fn tests(&self, repository_root: &str, declarations: &[DeclarationAt], given: &[String], revision: &str) -> Result<Option<Vec<TestReach>>> {
    if declarations.is_empty() {
      return Ok(Some(Vec::new()));
    }
    let mut options = vec!["--limit", TESTS_LIMIT, "--resolved"];
    for test in given {
      options.extend(["--tests", test.as_str()]);
    }
    self.query("tests", &options, repository_root, &Targets::At(declarations), Some(revision)).map(Some)
  }

  /// `accio parse --descriptor --resolve --ref <revision>`, the paths on its standard input: one line per path in their
  /// order, a path the revision lacks `missing`; without a usable index each line has no `classes` and the reason in
  /// `note`, which a format mismatch phrases as `IndexUnavailable` does
  fn descriptors(&self, repository_root: &str, paths: &[String], revision: &str) -> Result<Option<Vec<Descriptor>>> {
    if paths.is_empty() {
      return Ok(Some(Vec::new()));
    }
    let mut command = windowless_command_at(&self.accio.executable, self.priority);
    // `--root`: the paths are of the repository, whatever the directory the app runs in
    command.args(["parse", "--descriptor", "--resolve", "--no-build", "--project", repository_root, "--root", repository_root]);
    command.args(["--ref", revision, "--files-from", "-"]);
    let listed = paths.iter().fold(String::new(), |mut listed, path| {
      listed.push_str(path);
      listed.push('\n');
      listed
    });
    let stdout = self.run(&mut command, Some(listed.into_bytes()))?;
    let mut descriptors = stdout
      .split(|byte| *byte == b'\n')
      .filter(|line| !line.is_empty())
      .map(|line| serde_json::from_slice::<Descriptor>(line).context("the answer of accio"))
      .collect::<Result<Vec<_>>>()?;
    if descriptors.len() != paths.len() {
      bail!("accio answered {} descriptors for {} paths", descriptors.len(), paths.len());
    }
    for note in descriptors.iter_mut().filter_map(|descriptor| descriptor.note.as_mut()) {
      *note = index_unavailable_note(note, &self.accio.executable);
    }
    Ok(Some(descriptors))
  }

  fn surface(&self, repository_root: &str, modules: &[String], revision: &str) -> Result<Option<Vec<ModuleSurface>>> {
    if modules.is_empty() {
      return Ok(Some(Vec::new()));
    }
    let mut command = windowless_command_at(&self.accio.executable, self.priority);
    command.args([
      "surface",
      "--no-build",
      "--project",
      repository_root,
      "--ref",
      revision,
      "--budget",
      SURFACE_BUDGET,
      "--format",
      "json",
    ]);
    for module in modules {
      command.args(["--module", module]);
    }
    let stdout = self.run(&mut command, None)?;
    let mut surfaces: Vec<ModuleSurface> = serde_json::from_slice(&stdout).context("the answer of accio")?;
    if surfaces.len() != modules.len() {
      bail!("accio answered {} modules for {}", surfaces.len(), modules.len());
    }
    for note in surfaces.iter_mut().filter_map(|surface| surface.note.as_mut()) {
      *note = index_unavailable_note(note, &self.accio.executable);
    }
    Ok(Some(surfaces))
  }

  /// `accio renames --no-build --base <base> --ref <head> --pairs-from -`, a line `OLD<TAB>NEW` per pair on its
  /// standard input: every pair of a subject in one process, since a relative that another pair of the call renames
  /// along is no conflict. An accio that does not answer them as this reader reads them is not started
  fn renames(&self, repository_root: &str, pairs: &[RenamePair], base: &str, head: &str) -> Result<Option<Vec<RenameAnswer>>> {
    if pairs.is_empty() {
      return Ok(Some(Vec::new()));
    }
    if !self.accio.answers_renames() {
      return Ok(None);
    }
    let mut command = windowless_command_at(&self.accio.executable, self.priority);
    command.args(["renames", "--no-build", "--format", "json", "--project", repository_root]);
    command.args(["--base", base, "--ref", head, "--pairs-from", "-"]);
    let listed = pairs.iter().fold(String::new(), |mut listed, pair| {
      listed.push_str(&pair.old);
      listed.push('\t');
      listed.push_str(&pair.new);
      listed.push('\n');
      listed
    });
    let stdout = self.run(&mut command, Some(listed.into_bytes()))?;
    self.entries(&stdout, pairs.len()).map(Some)
  }
}

/// An index whose failures fail no lookup: a failed query answers nothing, and the first failure is kept for the answer;
/// an index accio cannot use answers as none, its reason kept apart (`IndexUnavailable`); only a canceled one fails the
/// lookup (`SearchCanceled`), which then answers nothing at all. Shared by the threads of parallel queries (the fan-in
/// of `review-plan`)
pub struct Lenient<'a> {
  index: &'a dyn SymbolIndex,
  error: Mutex<Option<String>>,
  unavailable: Mutex<Option<String>>,
}

impl<'a> Lenient<'a> {
  pub fn new(index: &'a dyn SymbolIndex) -> Self {
    Self {
      index,
      error: Mutex::default(),
      unavailable: Mutex::default(),
    }
  }

  /// The declarations of each name, in their order, as `SymbolIndex::symbols_any`: none where the index did not answer
  pub(crate) fn symbols_any(&self, repository_root: &str, queries: &[&str], revision: Option<&str>) -> Result<Vec<Vec<SymbolHit>>> {
    let answers = self.answered(self.index.symbols_any(repository_root, queries, revision))?;
    Ok(answers.unwrap_or_else(|| vec![Vec::new(); queries.len()]))
  }

  pub(crate) fn references(&self, repository_root: &str, name: &str, revision: Option<&str>) -> Result<References> {
    Ok(self.indexed_references(repository_root, name, revision)?.unwrap_or_default())
  }

  /// The usages of `name`, as `SymbolIndex::references`: `None` when the repository has no index or the query failed
  /// (`error` says why)
  pub fn indexed_references(&self, repository_root: &str, name: &str, revision: Option<&str>) -> Result<Option<References>> {
    self.answered(self.index.references(repository_root, name, revision))
  }

  /// The usages of each declaration, as `SymbolIndex::references_at`: `None` when the repository has no index or the
  /// query failed (`error` says why)
  pub fn references_at(&self, repository_root: &str, declarations: &[DeclarationAt], revision: &str) -> Result<Option<Vec<References>>> {
    self.answered(self.index.references_at(repository_root, declarations, revision))
  }

  /// The resolved usages at each declaration, with name-only hits counted; `None` when the repository has no usable index or the query failed
  pub fn references_at_resolved(&self, repository_root: &str, declarations: &[DeclarationAt], revision: Option<&str>) -> Result<Option<Vec<References>>> {
    self.answered(self.index.references_at_resolved(repository_root, declarations, revision))
  }

  /// The tests that reach each declaration, as `SymbolIndex::tests`: `None` when the repository has no index or the
  /// query failed (`error` says why)
  pub fn tests(&self, repository_root: &str, declarations: &[DeclarationAt], given: &[String], revision: &str) -> Result<Option<Vec<TestReach>>> {
    self.answered(self.index.tests(repository_root, declarations, given, revision))
  }

  /// Each descriptor of `paths` at `revision`, as `SymbolIndex::descriptors`: `None` when the index reads none or the
  /// query failed (`error` says why)
  pub fn descriptors(&self, repository_root: &str, paths: &[String], revision: &str) -> Result<Option<Vec<Descriptor>>> {
    self.answered(self.index.descriptors(repository_root, paths, revision))
  }

  pub fn surface(&self, repository_root: &str, modules: &[String], revision: &str) -> Result<Option<Vec<ModuleSurface>>> {
    self.answered(self.index.surface(repository_root, modules, revision))
  }

  /// Why accio has no usable index of the repository, as accio words it: none while no query was told so
  pub fn unavailable(&self) -> Option<String> {
    self.unavailable.lock().unwrap_or_else(PoisonError::into_inner).clone()
  }

  /// Why the index did not answer, as the review says it: none when it answered every query
  pub fn error(self) -> Option<String> {
    self
      .error
      .into_inner()
      .unwrap_or_else(PoisonError::into_inner)
      .map(|error| format!("accio did not answer: {error}"))
  }

  fn answered<T>(&self, result: Result<Option<T>>) -> Result<Option<T>> {
    match result {
      Err(error) if error.is::<SearchCanceled>() => Err(error),
      Err(error) if error.is::<IndexUnavailable>() => {
        self.unavailable.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(|| error.to_string());
        Ok(None)
      }
      Err(error) => {
        tracing::warn!(error = %format!("{error:#}"), "the index of the repository did not answer");
        self.error.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(|| format!("{error:#}"));
        Ok(None)
      }
      Ok(answer) => Ok(answer),
    }
  }
}

/// The index of a fixture, for the e2e server: `<git dir>/branch-deck-accio.json`, the hits of each query of `symbol`
/// and of `references` (by a name, or by a place as `<path>:<line>`, whatever its revision), the answer of `tests` of
/// each place whatever the given tests, the line of `parse --descriptor` of each `<full commit id>:<path>` (a path it
/// lacks is `missing`), the answer of `surface` of each `<full commit id>:<module>` (a module it lacks unknown, not
/// `complete`; none without any), the answer of `renames` of each pair by the place of its old declaration (a pair
/// it lacks `unavailable`; none without any), or the `error` every query fails with; a repository without the file
/// has no index
pub struct FixtureIndex;

#[derive(Deserialize)]
struct Fixture {
  #[serde(default)]
  symbol: std::collections::HashMap<String, Vec<SymbolHit>>,
  #[serde(default)]
  references: std::collections::HashMap<String, Vec<ReferenceHit>>,
  #[serde(default)]
  tests: std::collections::HashMap<String, TestReach>,
  #[serde(default)]
  descriptors: std::collections::HashMap<String, Descriptor>,
  /// What enters a module, by `"<full commit id>:<module directory>"`
  #[serde(default)]
  surface: std::collections::HashMap<String, ModuleSurface>,
  /// The answer of `renames` of a pair, by the place of its old declaration (`<path>:<line>:<column>`), whatever its
  /// trees
  #[serde(default)]
  renames: std::collections::HashMap<String, RenameAnswer>,
  error: Option<String>,
}

impl FixtureIndex {
  fn read(repository_root: &str) -> Result<Option<Fixture>> {
    let path = Path::new(repository_root).join(".git/branch-deck-accio.json");
    let Ok(text) = std::fs::read_to_string(&path) else { return Ok(None) };
    let fixture: Fixture = serde_json::from_str(&text).with_context(|| format!("{}", path.display()))?;
    if let Some(error) = fixture.error {
      bail!("{error}");
    }
    Ok(Some(fixture))
  }
}

impl SymbolIndex for FixtureIndex {
  fn symbols(&self, repository_root: &str, query: &str, _revision: Option<&str>) -> Result<Option<Vec<SymbolHit>>> {
    Ok(Self::read(repository_root)?.map(|mut fixture| fixture.symbol.remove(query).unwrap_or_default()))
  }

  fn references(&self, repository_root: &str, name: &str, _revision: Option<&str>) -> Result<Option<References>> {
    Ok(Self::read(repository_root)?.map(|mut fixture| References {
      hits: fixture.references.remove(name).unwrap_or_default(),
      by_name: 0,
      complete: true,
      unavailable: false,
    }))
  }

  fn references_at(&self, repository_root: &str, declarations: &[DeclarationAt], _revision: &str) -> Result<Option<Vec<References>>> {
    Ok(Self::read(repository_root)?.map(|mut fixture| declarations.iter().map(|declaration| fixture_references(&mut fixture, declaration, false)).collect()))
  }

  fn references_at_resolved(&self, repository_root: &str, declarations: &[DeclarationAt], _revision: Option<&str>) -> Result<Option<Vec<References>>> {
    Ok(Self::read(repository_root)?.map(|mut fixture| declarations.iter().map(|declaration| fixture_references(&mut fixture, declaration, true)).collect()))
  }

  fn tests(&self, repository_root: &str, declarations: &[DeclarationAt], _given: &[String], _revision: &str) -> Result<Option<Vec<TestReach>>> {
    Ok(Self::read(repository_root)?.map(|mut fixture| {
      declarations
        .iter()
        .map(|declaration| fixture.tests.remove(&fixture_place(declaration)).unwrap_or_default())
        .collect()
    }))
  }

  fn descriptors(&self, repository_root: &str, paths: &[String], revision: &str) -> Result<Option<Vec<Descriptor>>> {
    Ok(Self::read(repository_root)?.map(|mut fixture| {
      paths
        .iter()
        .map(|path| {
          fixture.descriptors.remove(&format!("{revision}:{path}")).unwrap_or_else(|| Descriptor {
            path: path.clone(),
            missing: true,
            ..Descriptor::default()
          })
        })
        .collect()
    }))
  }
  fn surface(&self, repository_root: &str, modules: &[String], revision: &str) -> Result<Option<Vec<ModuleSurface>>> {
    // a fixture that names no surface answers none, as an accio without the command
    Ok(Self::read(repository_root)?.filter(|fixture| !fixture.surface.is_empty()).map(|mut fixture| {
      modules
        .iter()
        .map(|module| {
          // a module the fixture does not name: an answer that knows nothing of it, not that nothing enters it
          fixture.surface.remove(&format!("{revision}:{module}")).unwrap_or_else(|| ModuleSurface {
            module: module.clone(),
            ..ModuleSurface::default()
          })
        })
        .collect()
    }))
  }

  fn renames(&self, repository_root: &str, pairs: &[RenamePair], _base: &str, _head: &str) -> Result<Option<Vec<RenameAnswer>>> {
    // a fixture that names no rename answers none, as an accio without the command
    Ok(Self::read(repository_root)?.filter(|fixture| !fixture.renames.is_empty()).map(|mut fixture| {
      pairs
        .iter()
        .map(|pair| {
          fixture.renames.remove(&pair.old).unwrap_or_else(|| RenameAnswer {
            unavailable: true,
            ..RenameAnswer::default()
          })
        })
        .collect()
    }))
  }
}

/// The key of a place in a fixture: `<path>:<line>`, whatever its revision
fn fixture_place(declaration: &DeclarationAt) -> String {
  declaration.column.map_or_else(
    || format!("{}:{}", declaration.path, declaration.line),
    |column| format!("{}:{}:{column}", declaration.path, declaration.line),
  )
}

fn fixture_references(fixture: &mut Fixture, declaration: &DeclarationAt, resolved: bool) -> References {
  let mut hits = fixture.references.remove(&fixture_place(declaration)).unwrap_or_default();
  let by_name = if resolved {
    let count = hits.iter().filter(|hit| hit.resolved == Resolved::Name).count();
    hits.retain(|hit| hit.resolved != Resolved::Name);
    count
  } else {
    0
  };
  References {
    hits,
    by_name,
    complete: true,
    unavailable: false,
  }
}
