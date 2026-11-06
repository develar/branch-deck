//! `accio` (github.com/JetBrains/accio), an index of the declarations of a repository, for a declaration and the usages
//! the files of a review lack (spec/commit/symbol-navigation.spec.md): `accio symbol <Name|Owner.name>` and
//! `accio references <name>`, with `--ref <revision>`, answer from the tree of a revision without a checkout. A review
//! plan asks about declarations by their place (spec/branch/review-plan.spec.md, "Fan-in";
//! spec/branch/review-tour.spec.md, "Tests That Reach It"): `accio references --at <path>:<line>` and
//! `accio tests --at … --tests <file>…`, every place of one plan in one process, which accio answers in parallel.
//!
//! - **One answer per target**: accio answers a JSON array of one entry for each name or place, in the order asked.
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

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use sync_utils::child_process::{ChildProcessError, cached_login_shell_path, find_executable, run_child_until_superseded, windowless_command};

/// The declarations a lookup answers at most
const LIMIT: &str = "20";
/// The usages a lookup answers at most
const REFERENCES_LIMIT: &str = "100";
/// The tests a lookup answers at most, per declaration: the given tests of a change reach one declaration by the
/// hundred (97 tests of 7 files for one place of idea-1)
const TESTS_LIMIT: &str = "1000";

/// A declaration an index knows: a path of the repository and a line from 1
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SymbolHit {
  #[serde(rename = "file")]
  pub path: String,
  pub line: u32,
  pub kind: String,
  pub name: String,
  #[serde(default)]
  pub parent: Option<String>,
}

/// A usage an index knows: a path, a line and a column from 1, the text of the line, how it uses the name (`call`,
/// `write`, `argument`, `read`, `type`, `supertype`, `override`, `declaration`, `import`, `doc`, `string`, `config`) and
/// what ties it to the declaration
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReferenceHit {
  #[serde(rename = "file")]
  pub path: String,
  pub line: u32,
  pub column: u32,
  #[serde(default)]
  pub snippet: String,
  pub kind: String,
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

/// A declaration by its place: the line of its name in `path` at `revision`, `None` for the revision the query
/// answers from (for a removed declaration, the parent of the commit that removed it)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclarationAt {
  pub revision: Option<String>,
  pub path: String,
  pub line: u32,
}

impl DeclarationAt {
  /// The `--at` argument: `<path>:<line>`, `<revision>:<path>:<line>`
  pub fn argument(&self) -> String {
    match &self.revision {
      Some(revision) => format!("{revision}:{}:{}", self.path, self.line),
      None => format!("{}:{}", self.path, self.line),
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

/// The usages of a name an index answered
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct References {
  #[serde(default)]
  pub hits: Vec<ReferenceHit>,
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

/// The declarations and the usages of a repository by name: accio in the app, a fixture in the e2e server
pub trait SymbolIndex: Send + Sync {
  /// The declarations of `query` (`Name`, `Owner.name`) in the tree of `revision` (the working tree for `None`);
  /// `None` when the repository has no index
  fn symbols(&self, repository_root: &str, query: &str, revision: Option<&str>) -> Result<Option<Vec<SymbolHit>>>;

  /// The whole-word usages of `name` in the tree of `revision`; `None` when the repository has no index
  fn references(&self, repository_root: &str, name: &str, revision: Option<&str>) -> Result<Option<References>>;

  /// The usages in the tree of `revision` of each declaration of `declarations`, in their order; `None` when the
  /// repository has no index
  fn references_at(&self, repository_root: &str, declarations: &[DeclarationAt], revision: &str) -> Result<Option<Vec<References>>>;

  /// The tests of `given` (test files of the tree of `revision`) that reach each declaration of `declarations` through
  /// any number of calls, only by calls tied by a type or an import, in their order; `None` when the repository has no
  /// index
  fn tests(&self, repository_root: &str, declarations: &[DeclarationAt], given: &[String], revision: &str) -> Result<Option<Vec<TestReach>>>;
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
}

/// The lookups of one navigation: each query runs until it answers, and is killed once `canceled` turns true
pub struct AccioSearch<'a> {
  accio: &'a AccioCli,
  canceled: &'a (dyn Fn() -> bool + Sync),
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
          .map(|executable| Self { executable })
      })
      .as_ref()
  }

  /// The index of the repository for one navigation, which the caller cancels through `canceled`
  pub fn search<'a>(&'a self, canceled: &'a (dyn Fn() -> bool + Sync)) -> AccioSearch<'a> {
    AccioSearch { accio: self, canceled }
  }

  /// accio at `executable`, for a test
  #[cfg(test)]
  pub(crate) const fn at(executable: PathBuf) -> Self {
    Self { executable }
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
  Name(&'a str),
  At(&'a [DeclarationAt]),
}

impl AccioSearch<'_> {
  /// The JSON answer of `accio <command> --no-build … <targets>` at `revision`, one entry per target in their order;
  /// `IndexUnavailable` when accio has no usable index of the repository
  fn query<T: serde::de::DeserializeOwned>(&self, command_name: &str, options: &[&str], repository_root: &str, targets: &Targets<'_>, revision: Option<&str>) -> Result<Vec<T>> {
    let mut command = windowless_command(&self.accio.executable);
    command.args([command_name, "--no-build", "--format", "json", "--project", repository_root]);
    command.args(options);
    if let Some(revision) = revision {
      command.args(["--ref", revision]);
    }
    let asked = match targets {
      Targets::Name(name) => {
        command.arg("--").arg(name);
        1
      }
      Targets::At(declarations) => {
        for declaration in *declarations {
          command.arg("--at").arg(declaration.argument());
        }
        declarations.len()
      }
    };
    let output = run_child_until_superseded(&mut command, None, self.canceled).map_err(|error| match error {
      ChildProcessError::Superseded => anyhow::Error::new(SearchCanceled),
      other => anyhow::anyhow!("{other:?}"),
    })?;
    if !output.status.success() {
      bail!("{}", output.failure_message("accio"));
    }
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).context("the answer of accio")?;
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
}

impl SymbolIndex for AccioSearch<'_> {
  fn symbols(&self, repository_root: &str, query: &str, revision: Option<&str>) -> Result<Option<Vec<SymbolHit>>> {
    let entries = self.query::<Answer>("symbol", &["--limit", LIMIT], repository_root, &Targets::Name(query), revision)?;
    Ok(Some(entries.into_iter().flat_map(|entry| entry.hits).collect()))
  }

  fn references(&self, repository_root: &str, name: &str, revision: Option<&str>) -> Result<Option<References>> {
    let entries = self.query::<References>("references", &["--limit", REFERENCES_LIMIT], repository_root, &Targets::Name(name), revision)?;
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
}

/// An index whose failures fail no lookup: a failed query answers nothing, and the first failure is kept for the answer;
/// an index accio cannot use answers as none, its reason kept apart (`IndexUnavailable`); only a canceled one fails the
/// lookup (`SearchCanceled`), which then answers nothing at all. Shared by the threads of parallel queries (the fan-in
/// of `review-plan`)
pub struct Lenient<'a> {
  index: &'a dyn SymbolIndex,
  error: std::sync::Mutex<Option<String>>,
  unavailable: std::sync::Mutex<Option<String>>,
}

impl<'a> Lenient<'a> {
  pub fn new(index: &'a dyn SymbolIndex) -> Self {
    Self {
      index,
      error: std::sync::Mutex::default(),
      unavailable: std::sync::Mutex::default(),
    }
  }

  pub(crate) fn symbols(&self, repository_root: &str, query: &str, revision: Option<&str>) -> Result<Vec<SymbolHit>> {
    Ok(self.answered(self.index.symbols(repository_root, query, revision))?.unwrap_or_default())
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

  /// The tests that reach each declaration, as `SymbolIndex::tests`: `None` when the repository has no index or the
  /// query failed (`error` says why)
  pub fn tests(&self, repository_root: &str, declarations: &[DeclarationAt], given: &[String], revision: &str) -> Result<Option<Vec<TestReach>>> {
    self.answered(self.index.tests(repository_root, declarations, given, revision))
  }

  /// Why accio has no usable index of the repository, as accio words it: none while no query was told so
  pub fn unavailable(&self) -> Option<String> {
    self.unavailable.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
  }

  /// Why the index did not answer, as the review says it: none when it answered every query
  pub fn error(self) -> Option<String> {
    self
      .error
      .into_inner()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .map(|error| format!("accio did not answer: {error}"))
  }

  fn answered<T>(&self, result: Result<Option<T>>) -> Result<Option<T>> {
    match result {
      Err(error) if error.is::<SearchCanceled>() => Err(error),
      Err(error) if error.is::<IndexUnavailable>() => {
        self
          .unavailable
          .lock()
          .unwrap_or_else(std::sync::PoisonError::into_inner)
          .get_or_insert_with(|| error.to_string());
        Ok(None)
      }
      Err(error) => {
        tracing::warn!(error = %format!("{error:#}"), "the index of the repository did not answer");
        self
          .error
          .lock()
          .unwrap_or_else(std::sync::PoisonError::into_inner)
          .get_or_insert_with(|| format!("{error:#}"));
        Ok(None)
      }
      Ok(answer) => Ok(answer),
    }
  }
}

/// The index of a fixture, for the e2e server: `<git dir>/branch-deck-accio.json`, the hits of each query of `symbol`
/// and of `references` (by a name, or by a place as `<path>:<line>`, whatever its revision), the answer of `tests` of
/// each place whatever the given tests, or the `error` every query fails with; a repository without the file has no
/// index
pub struct FixtureIndex;

#[derive(Deserialize)]
struct Fixture {
  #[serde(default)]
  symbol: std::collections::HashMap<String, Vec<SymbolHit>>,
  #[serde(default)]
  references: std::collections::HashMap<String, Vec<ReferenceHit>>,
  #[serde(default)]
  tests: std::collections::HashMap<String, TestReach>,
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
      complete: true,
      unavailable: false,
    }))
  }

  fn references_at(&self, repository_root: &str, declarations: &[DeclarationAt], _revision: &str) -> Result<Option<Vec<References>>> {
    Ok(Self::read(repository_root)?.map(|mut fixture| {
      declarations
        .iter()
        .map(|declaration| References {
          hits: fixture.references.remove(&fixture_place(declaration)).unwrap_or_default(),
          complete: true,
          unavailable: false,
        })
        .collect()
    }))
  }

  fn tests(&self, repository_root: &str, declarations: &[DeclarationAt], _given: &[String], _revision: &str) -> Result<Option<Vec<TestReach>>> {
    Ok(Self::read(repository_root)?.map(|mut fixture| {
      declarations
        .iter()
        .map(|declaration| fixture.tests.remove(&fixture_place(declaration)).unwrap_or_default())
        .collect()
    }))
  }
}

/// The key of a place in a fixture: `<path>:<line>`, whatever its revision
fn fixture_place(declaration: &DeclarationAt) -> String {
  format!("{}:{}", declaration.path, declaration.line)
}
