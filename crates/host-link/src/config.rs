//! `[host]` of `.branchdeck.toml` in the root of the shipped commit: what a repository asks of the host for each run
//! (spec/cli/host.spec.md, "The environment of a run and `[host]`"). The agent and the client parse it with this one
//! parser: the agent for the prepare steps, the environment and the outputs that it stages, the client for the outputs
//! that it takes back. Each end reads the file from the commit of the run, so a file beside the commit never counts. A
//! commit whose `.branchdeck.toml` has no `[host]` table gets its `.yonder.toml`, which has the same keys at its top
//! level, until the repositories moved (`read_commit_config`). A commit without both has no prepare steps, no environment of its
//! own and no outputs.
//!
//! - `outputs`: the paths that come back after the run, for a client that asks for them.
//! - `[host.env]`: the environment of the prepare steps and the command, on top of the login environment of the user.
//! - `[[host.prepare]]`: the steps that prepare the slot before the command, each one only when its inputs changed.
//!
//! An unknown key or a value of the wrong shape in `[host]` is a [`ConfigError`], so a typo never drops a step. The
//! other tables of `.branchdeck.toml` are not the agent's, and they stay unread.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path};

/// The file of Branch Deck in the root of the commit
pub const FILE: &str = ".branchdeck.toml";

/// The table of `FILE` for the remote runs
pub const TABLE: &str = "host";

/// The file of yonder in the root of the commit, read when `FILE` has no `TABLE`
pub const YONDER_FILE: &str = ".yonder.toml";

/// The content of `[host]`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RepoConfig {
  /// Paths relative to the slot root: files or directories.
  pub outputs: Vec<String>,
  /// The variables in the order of their names.
  pub env: Vec<(String, EnvValue)>,
  pub prepare: Vec<Step>,
}

/// One prepare step.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
  /// The name in the log. The command line when the file names none.
  #[serde(default)]
  pub name: String,
  /// The program and its arguments, run in the slot root.
  pub run: Vec<String>,
  /// Tracked paths whose content keys the step: files or directories at the commit of the run.
  #[serde(default)]
  pub inputs: Vec<String>,
  /// Paths that the step makes. A missing one runs the step again.
  #[serde(default)]
  pub creates: Vec<String>,
}

/// The value of one variable of `[env]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnvValue {
  /// `NAME = "text"`: the text as it is.
  Text(String),
  /// `NAME = { prepend = [...], append = [...] }`: entries joined with `:` before and after the inherited value. A
  /// leading `~/` of an entry is the home directory, and a relative entry is below the slot root.
  List { prepend: Vec<String>, append: Vec<String> },
  /// `NAME = { port = 3100 }`: a free TCP port, the base plus the number of the slot or the next free one.
  Port(u16),
  /// `NAME = { unset = true }`: the variable is removed.
  Unset,
}

/// Why `[host]` does not parse. The message starts with the name of the file and the table: `.branchdeck.toml
/// [host]: ` or `.yonder.toml: `
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ConfigError {
  message: String,
}

impl ConfigError {
  fn new(place: &str, message: impl fmt::Display) -> Self {
    Self {
      message: format!("{place}: {message}"),
    }
  }
}

/// The table form of an `[env]` value, before the check of its shape.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvTable {
  prepend: Option<Vec<String>>,
  append: Option<Vec<String>>,
  port: Option<u16>,
  unset: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
  #[serde(default)]
  outputs: Vec<String>,
  #[serde(default)]
  env: BTreeMap<String, toml::Value>,
  #[serde(default)]
  prepare: Vec<Step>,
}

/// Why the configuration of a commit is not there: the reader of a file of the commit failed, or a file does not parse
#[derive(Debug)]
pub enum ReadError<E> {
  /// The reader of a file of the commit failed
  Read(E),
  /// A file of the commit does not parse
  Config(ConfigError),
}

/// The configuration of a commit, for the agent and the client alike. `read` answers the text of a file of the commit,
/// `None` when the commit lacks it: `[host]` of `.branchdeck.toml`, else `.yonder.toml`, else the default. A
/// `.branchdeck.toml` with `[host]` wins, and `read` never gets `.yonder.toml` then
pub fn read_commit_config<E>(mut read: impl FnMut(&str) -> Result<Option<String>, E>) -> Result<RepoConfig, ReadError<E>> {
  let branchdeck = read(FILE).map_err(ReadError::Read)?;
  if let Some(config) = branchdeck.as_deref().map(parse_branchdeck).transpose().map_err(ReadError::Config)?.flatten() {
    return Ok(config);
  }
  let yonder = read(YONDER_FILE).map_err(ReadError::Read)?;
  yonder.as_deref().map_or_else(|| Ok(RepoConfig::default()), parse_yonder).map_err(ReadError::Config)
}

/// `[host]` of the text of `.branchdeck.toml`, or `None` when it has no such table. The other tables are not read
pub fn parse_branchdeck(text: &str) -> Result<Option<RepoConfig>, ConfigError> {
  let place = format!("{FILE} [{TABLE}]");
  let mut document: toml::Table = toml::from_str(text).map_err(|error| ConfigError::new(FILE, error.message()))?;
  match document.remove(TABLE) {
    None => Ok(None),
    Some(table @ toml::Value::Table(_)) => {
      let raw: RawConfig = table.try_into().map_err(|error: toml::de::Error| ConfigError::new(&place, error.message()))?;
      check(raw).map(Some).map_err(|message| ConfigError::new(&place, message))
    }
    Some(other) => Err(ConfigError::new(FILE, format!("`{TABLE}` is a {}, not a table", other.type_str()))),
  }
}

/// The text of `.yonder.toml`: the keys of `[host]` at its top level
pub fn parse_yonder(text: &str) -> Result<RepoConfig, ConfigError> {
  let raw: RawConfig = toml::from_str(text).map_err(|error| ConfigError::new(YONDER_FILE, error.message()))?;
  check(raw).map_err(|message| ConfigError::new(YONDER_FILE, message))
}

/// Checks the paths, the names of the variables and the steps
fn check(raw: RawConfig) -> Result<RepoConfig, String> {
  for output in &raw.outputs {
    check_path("outputs", output)?;
  }
  let mut env = Vec::with_capacity(raw.env.len());
  for (name, value) in raw.env {
    if name.is_empty() || name.contains(['=', '\0']) {
      return Err(format!("{name:?} cannot name a variable"));
    }
    let value = env_value(&name, value)?;
    env.push((name, value));
  }
  let mut prepare = raw.prepare;
  for step in &mut prepare {
    if step.run.first().is_none_or(String::is_empty) {
      return Err("a prepare step has no command in `run`".to_owned());
    }
    if step.name.is_empty() {
      step.name = step.run.join(" ");
    }
    for path in step.inputs.iter().chain(&step.creates) {
      check_path(&format!("the prepare step {:?}", step.name), path)?;
    }
  }
  Ok(RepoConfig {
    outputs: raw.outputs,
    env,
    prepare,
  })
}

/// Whether `path` is a relative path that stays inside its base: no root, no `..`, no `.`.
pub fn is_inner_path(path: &str) -> bool {
  !path.is_empty() && Path::new(path).components().all(|component| matches!(component, Component::Normal(_)))
}

fn check_path(what: &str, path: &str) -> Result<(), String> {
  if is_inner_path(path) {
    Ok(())
  } else {
    Err(format!("{what} names {path:?}, which is not a path inside the slot"))
  }
}

fn env_value(name: &str, value: toml::Value) -> Result<EnvValue, String> {
  let table = match value {
    toml::Value::String(text) => return Ok(EnvValue::Text(text)),
    toml::Value::Table(table) => table,
    other => return Err(format!("env.{name} is a {}, not a string or a table", other.type_str())),
  };
  let table: EnvTable = toml::Value::Table(table)
    .try_into()
    .map_err(|error: toml::de::Error| format!("env.{name}: {}", error.message()))?;
  match table {
    EnvTable {
      prepend,
      append,
      port: None,
      unset: None,
    } if prepend.is_some() || append.is_some() => Ok(EnvValue::List {
      prepend: prepend.unwrap_or_default(),
      append: append.unwrap_or_default(),
    }),
    EnvTable {
      prepend: None,
      append: None,
      port: Some(port),
      unset: None,
    } => Ok(EnvValue::Port(port)),
    EnvTable {
      prepend: None,
      append: None,
      port: None,
      unset: Some(true),
    } => Ok(EnvValue::Unset),
    _ => Err(format!("env.{name} takes one of: `prepend` and `append`, `port`, or `unset = true`")),
  }
}

#[cfg(test)]
#[path = "config_test.rs"]
mod config_test;
