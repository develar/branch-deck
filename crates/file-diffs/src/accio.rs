//! `accio` (github.com/JetBrains/accio), an index of the declarations of a repository, for a declaration and the usages
//! the files of a review lack (spec/commit/symbol-navigation.spec.md): `accio symbol <Name|Owner.name>` and
//! `accio references <name>`, with `--ref <revision>`, answer from the tree of a revision without a checkout.
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

/// A usage an index knows: a path, a line and a column from 1, the text of the line, and how it uses the name
/// (`call`, `write`, `argument`, `read`, `type`, `supertype`, `declaration`, `import`, `doc`, `string`)
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReferenceHit {
  #[serde(rename = "file")]
  pub path: String,
  pub line: u32,
  pub column: u32,
  #[serde(default)]
  pub snippet: String,
  pub kind: String,
}

/// The usages of a name an index answered
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct References {
  pub hits: Vec<ReferenceHit>,
  /// False when the limit cut the answer
  #[serde(default = "complete_by_default")]
  pub complete: bool,
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
}

/// accio has no usable index of the repository (`--no-build` answered `available: false`): why, as accio words it. A
/// lookup through `Lenient` answers it as no index, and keeps the reason
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

impl AccioSearch<'_> {
  /// The JSON answer of `accio <command> --no-build … -- <query>` at `revision`; `IndexUnavailable` when accio has no
  /// usable index of the repository
  fn query<T: serde::de::DeserializeOwned>(&self, command_name: &str, limit: &str, repository_root: &str, query: &str, revision: Option<&str>) -> Result<T> {
    let mut command = windowless_command(&self.accio.executable);
    command.args([command_name, "--no-build", "--format", "json", "--limit", limit, "--project", repository_root]);
    if let Some(revision) = revision {
      command.args(["--ref", revision]);
    }
    command.arg("--").arg(query);
    let output = run_child_until_superseded(&mut command, None, self.canceled).map_err(|error| match error {
      ChildProcessError::Superseded => anyhow::Error::new(SearchCanceled),
      other => anyhow::anyhow!("{other:?}"),
    })?;
    if !output.status.success() {
      bail!("{}", output.failure_message("accio"));
    }
    let answer: serde_json::Value = serde_json::from_slice(&output.stdout).context("the answer of accio")?;
    if answer.get("available").and_then(serde_json::Value::as_bool) == Some(false) {
      let note = answer.get("note").and_then(serde_json::Value::as_str).unwrap_or("accio has no index of this repository");
      return Err(IndexUnavailable(note.to_string()).into());
    }
    serde_json::from_value(answer).context("the answer of accio")
  }
}

impl SymbolIndex for AccioSearch<'_> {
  fn symbols(&self, repository_root: &str, query: &str, revision: Option<&str>) -> Result<Option<Vec<SymbolHit>>> {
    Ok(Some(self.query::<Answer>("symbol", LIMIT, repository_root, query, revision)?.hits))
  }

  fn references(&self, repository_root: &str, name: &str, revision: Option<&str>) -> Result<Option<References>> {
    self.query("references", REFERENCES_LIMIT, repository_root, name, revision).map(Some)
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
/// and of `references`, or the `error` every query fails with; a repository without the file has no index
pub struct FixtureIndex;

#[derive(Deserialize)]
struct Fixture {
  #[serde(default)]
  symbol: std::collections::HashMap<String, Vec<SymbolHit>>,
  #[serde(default)]
  references: std::collections::HashMap<String, Vec<ReferenceHit>>,
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
    }))
  }
}
