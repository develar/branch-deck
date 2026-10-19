//! `accio` (github.com/JetBrains/accio), an index of the declarations of a repository, for a declaration the files of a
//! review lack (spec/commit/symbol-navigation.spec.md): `accio symbol <Name|Owner.name> --ref <revision>` answers from
//! the tree of a revision without a checkout.
//!
//! - **Never an index build**: a query in a repository without an index builds one (up to 90 s), so the index is
//!   looked for first, without a process: accio keeps it in `<home>/<the first 16 hex of the blob id of the root path>`
//!   (`index_dir` of its `index.rs`), `home` being `ACCIO_HOME`, `$XDG_CACHE_HOME/accio`, `%LOCALAPPDATA%\accio` on
//!   Windows, else `~/.cache/accio`. A changed layout reads as no index: the review then looks only at its files.
//! - **Bounded**: one process per lookup, a null stdin, killed after `TIMEOUT`.
//! - **Found** on `PATH` (the login shell's for an app started from the Finder), else in `~/.local/bin`, `~/.cargo/bin`
//!   and Homebrew's directories.

use anyhow::{Context, Result, bail};
use git_ops::commit_utils::blob_id;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;
use sync_utils::child_process::{cached_login_shell_path, find_executable, run_child};

/// The time a lookup may take: a warm query of the IntelliJ monorepo takes 0.2–0.4 s
const TIMEOUT: Duration = Duration::from_secs(3);
/// The declarations a lookup answers at most
const LIMIT: &str = "20";

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

/// The declarations of a repository by name: accio in the app, a fixture in the e2e server
pub trait SymbolIndex: Send + Sync {
  /// The declarations of `query` (`Name`, `Owner.name`) in the tree of `revision` (the working tree for `None`);
  /// `None` when the repository has no index
  fn symbols(&self, repository_root: &str, query: &str, revision: Option<&str>) -> Result<Option<Vec<SymbolHit>>>;
}

/// The `accio` executable of this machine
pub struct AccioCli {
  executable: PathBuf,
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

/// Where accio keeps the index of `root`, the absolute path of a repository
pub fn index_dir(root: &str) -> Option<PathBuf> {
  let home = std::env::var_os("ACCIO_HOME")
    .map(PathBuf::from)
    .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(|it| PathBuf::from(it).join("accio")))
    .or_else(|| {
      if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(|it| PathBuf::from(it).join("accio"))
      } else {
        None
      }
    })
    .or_else(|| {
      std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|it| PathBuf::from(it).join(".cache/accio"))
    })?;
  // the SHA-1 blob id of the path's bytes, as `git::blob_oid` of accio
  let key = blob_id(&"0".repeat(40), root.as_bytes());
  Some(home.join(&key[..16]))
}

#[derive(Deserialize)]
struct Answer {
  hits: Vec<SymbolHit>,
}

impl SymbolIndex for AccioCli {
  fn symbols(&self, repository_root: &str, query: &str, revision: Option<&str>) -> Result<Option<Vec<SymbolHit>>> {
    if !index_dir(repository_root).is_some_and(|dir| dir.join("meta.json").is_file()) {
      return Ok(None);
    }
    let mut command = Command::new(&self.executable);
    command.args(["symbol", "--format", "json", "--limit", LIMIT, "--project", repository_root]);
    if let Some(revision) = revision {
      command.args(["--ref", revision]);
    }
    command.arg("--").arg(query);
    let output = run_child(&mut command, None, TIMEOUT, &|| false).map_err(|error| anyhow::anyhow!("accio: {error:?}"))?;
    if !output.status.success() {
      bail!("{}", output.failure_message("accio"));
    }
    let answer: Answer = serde_json::from_slice(&output.stdout).context("the answer of accio")?;
    Ok(Some(answer.hits))
  }
}

/// The declarations of a fixture, for the e2e server: `<git dir>/branch-deck-accio.json`, a map from a query to its
/// hits; a repository without the file has no index
pub struct FixtureIndex;

impl SymbolIndex for FixtureIndex {
  fn symbols(&self, repository_root: &str, query: &str, _revision: Option<&str>) -> Result<Option<Vec<SymbolHit>>> {
    let path = Path::new(repository_root).join(".git/branch-deck-accio.json");
    let Ok(text) = std::fs::read_to_string(&path) else { return Ok(None) };
    let mut fixture: std::collections::HashMap<String, Vec<SymbolHit>> = serde_json::from_str(&text).with_context(|| format!("{}", path.display()))?;
    Ok(Some(fixture.remove(query).unwrap_or_default()))
  }
}
