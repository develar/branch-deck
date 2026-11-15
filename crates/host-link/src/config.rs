//! `[host]` of `.branchdeck.toml` in the root of the shipped commit: what a repository asks of the host for each run
//! (openspec/specs/cli/host/spec.md, "The environment of a run and `[host]`"). The agent and the client parse it with this one
//! parser: the agent for the prepare steps, the environment and the outputs that it stages, the client for the outputs
//! that it takes back. Each end reads the file from the commit of the run, so a file beside the commit never counts. A
//! commit whose `.branchdeck.toml` has no `[host]` table has no prepare steps, no environment of its own and no
//! outputs.
//!
//! - `outputs`: the paths that come back after the run, for a client that asks for them.
//! - `heavy`: the commands that are heavy runs: the client sends such a run as heavy, and the agent runs the heavy runs
//!   of its host one at a time ([`RepoConfig::heavy_entry`]).
//! - `[host.env]`: the environment of the prepare steps and the command, on top of the login environment of the user.
//! - `[[host.prepare]]`: the steps that prepare the slot before the command, each one only when its inputs changed.
//!
//! An unknown key or a value of the wrong shape in `[host]` is a [`ConfigError`], so a typo never drops a step. An
//! unknown key says that it can be a key of a later Branch Deck, and how to update brd-host, since both ends refuse a
//! key that their build predates; the keys that a build knows are the fields of its serde types ([`fields_of`]). A key
//! that an older agent refuses names the feature of an agent that reads it ([`RepoConfig::needs_features`]), so the
//! client refuses before the start. The other tables of `.branchdeck.toml` are not the agent's, and they stay unread.

use crate::heavy::Ran;
use crate::proto::feature;
use serde::Deserialize;
use serde::de::Visitor;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path};

/// The file of Branch Deck in the root of the commit
pub const FILE: &str = ".branchdeck.toml";

/// The table of `FILE` for the remote runs
pub const TABLE: &str = "host";

/// The content of `[host]`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RepoConfig {
  /// Paths relative to the slot root: files or directories.
  pub outputs: Vec<String>,
  /// The patterns of the heavy commands, in the order of the file: `*` matches any characters, the rest is literal.
  /// `None` without the key, as a commit that an agent before heavy runs reads.
  pub heavy: Option<Vec<String>>,
  /// The variables in the order of their names.
  pub env: Vec<(String, EnvValue)>,
  pub prepare: Vec<Step>,
}

impl RepoConfig {
  /// The first pattern of `heavy` that matches `command`, `None` when the run is not heavy. A pattern matches the words
  /// of the command joined by one space, whole, as `heavy_line` writes them: `pnpm e2e*` matches
  /// `env DEBUG_E2E=1 pnpm e2e tests/e2e/x.spec.ts`, `pnpm -C web e2e` and `pnpm e2e-up`, `pnpm test` only `pnpm test`,
  /// `playwright test*` every run of `playwright test` (`npx playwright test x.spec.ts`, `pnpm exec playwright test`)
  pub fn heavy_entry(&self, command: &[String]) -> Option<&str> {
    let (line, written) = heavy_line(command);
    self
      .heavy
      .iter()
      .flatten()
      .find(|pattern| glob_matches(pattern, &line) || glob_matches(pattern, &written))
      .map(String::as_str)
  }

  /// Why a run of `command` is heavy by `heavy`, `None` when it is not: the first pattern that matches the command, or
  /// a command that the script of a shell it starts runs (`zsh -c '…; pnpm e2e …'`, a script file that `read` gives from
  /// the checkout), or a script file of a shell that `read` cannot give, which may run anything ([`crate::heavy`]).
  /// A table without `heavy` makes no run heavy
  pub fn heavy_reason(&self, command: &[String], read: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    self.heavy.as_ref()?;
    let mut unread = None;
    for ran in crate::heavy::commands_of(command, read) {
      match ran {
        Ran::Command { words, scripted } => {
          if let Some(pattern) = self.heavy_entry(&words) {
            let wrapped = if scripted { " in its script" } else { "" };
            return Some(format!("`{pattern}` of [host]{wrapped}"));
          }
        }
        Ran::Unread(path) => unread = unread.or(Some(path)),
      }
    }
    unread.map(|path| format!("its script `{path}`, which brd-host cannot read for `heavy` of [host]"))
  }

  /// The features of the agent that reads this table: an agent without one refuses every run of the commit with
  /// `bad_config`, as an unknown key, so the client refuses before the start. `heavy` needs `feature::HEAVY`, even empty
  pub fn needs_features(&self) -> Vec<&'static str> {
    self.heavy.as_ref().map(|_| feature::HEAVY).into_iter().collect()
  }
}

/// The options of pnpm that name a value in the next word (`-C web`), before its script
const PNPM_VALUE_OPTIONS: [&str; 7] = ["-C", "--dir", "--filter", "-F", "--filter-prod", "--reporter", "--workspace-concurrency"];

/// The programs and the words that run a package binary for the next word: `npx playwright`, `pnpm exec playwright`
const RUNNERS: [&str; 6] = ["npx", "bunx", "yarn", "pnpm", "exec", "dlx"];

/// The words of `command` that `heavy` matches, joined by one space, by what it runs rather than how it is written:
/// without a leading `env` and the assignments after it; `pnpm` without its options before its script (`-C web`,
/// `--dir=web`, `--filter app`, `-r`), so `pnpm -C web e2e` is `pnpm e2e`; and a run of `playwright test` by any runner
/// (`npx`, `pnpm exec`, `pnpm`, a path of the binary) as `playwright test …`. Beside it the line as written, without
/// the `env` and the assignments alone, so a pattern written with its runner (`pnpm exec playwright test*`) matches too
fn heavy_line(command: &[String]) -> (String, String) {
  let mut words = command;
  if words.first().is_some_and(|word| word == "env") {
    words = &words[1..];
  }
  while words.first().is_some_and(|word| is_assignment(word)) {
    words = &words[1..];
  }
  let written = words.join(" ");
  let mut line: Vec<&str> = Vec::with_capacity(words.len());
  let mut rest = words.iter().map(String::as_str);
  if let Some(program) = rest.next() {
    line.push(program);
    if program == "pnpm" {
      // its options before its script, and before the script of its `run` or the binary of its `exec`
      while let Some(word) = rest.next() {
        if PNPM_VALUE_OPTIONS.contains(&word) {
          rest.next();
        } else if !word.starts_with('-') {
          line.push(word);
          if word != "run" && word != "exec" {
            break;
          }
        }
      }
    }
    line.extend(rest);
  }
  let playwright = line.iter().position(|word| word.rsplit('/').next() == Some("playwright"));
  if let Some(at) = playwright
    && line.get(at + 1) == Some(&"test")
    && line[..at].iter().all(|word| RUNNERS.contains(word))
  {
    return (std::iter::once("playwright").chain(line[at + 1..].iter().copied()).collect::<Vec<_>>().join(" "), written);
  }
  (line.join(" "), written)
}

/// `NAME=value`, with a name of letters, digits and `_` that does not start with a digit
pub(crate) fn is_assignment(word: &str) -> bool {
  word.split_once('=').is_some_and(|(name, _)| {
    name.chars().next().is_some_and(|first| first == '_' || first.is_ascii_alphabetic()) && name.chars().all(|char| char == '_' || char.is_ascii_alphanumeric())
  })
}

/// Whether `text` matches `pattern` whole, where each `*` matches any run of characters and the rest is literal
fn glob_matches(pattern: &str, text: &str) -> bool {
  let parts: Vec<&str> = pattern.split('*').collect();
  let (first, last) = (parts[0], parts[parts.len() - 1]);
  if parts.len() == 1 {
    return pattern == text;
  }
  if text.len() < first.len() + last.len() || !text.starts_with(first) || !text.ends_with(last) {
    return false;
  }
  let mut rest = &text[first.len()..text.len() - last.len()];
  for middle in &parts[1..parts.len() - 1] {
    match rest.find(middle) {
      Some(at) => rest = &rest[at + middle.len()..],
      None => return false,
    }
  }
  true
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

/// Why `[host]` does not parse. The message starts with the name of the file, and with the table when the table does
/// not parse: `.branchdeck.toml [host]: `
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
  heavy: Option<Vec<String>>,
  #[serde(default)]
  env: BTreeMap<String, toml::Value>,
  #[serde(default)]
  prepare: Vec<Step>,
}

/// Why the configuration of a commit is not there: the reader of the file of the commit failed, or the file does not
/// parse
#[derive(Debug)]
pub enum ReadError<E> {
  /// The reader of the file of the commit failed
  Read(E),
  /// The file of the commit does not parse
  Config(ConfigError),
}

/// The configuration of a commit, for the agent and the client alike. `read` answers the text of a file of the commit,
/// `None` when the commit lacks it: `[host]` of `.branchdeck.toml`, else the default
pub fn read_commit_config<E>(read: impl FnOnce(&str) -> Result<Option<String>, E>) -> Result<RepoConfig, ReadError<E>> {
  let text = read(FILE).map_err(ReadError::Read)?;
  let config = text.as_deref().map(parse_branchdeck).transpose().map_err(ReadError::Config)?.flatten();
  Ok(config.unwrap_or_default())
}

/// `[host]` of the text of `.branchdeck.toml`, or `None` when it has no such table. The other tables are not read
pub fn parse_branchdeck(text: &str) -> Result<Option<RepoConfig>, ConfigError> {
  let place = format!("{FILE} [{TABLE}]");
  let mut document: toml::Table = toml::from_str(text).map_err(|error| ConfigError::new(FILE, error.message()))?;
  match document.remove(TABLE) {
    None => Ok(None),
    Some(toml::Value::Table(table)) => {
      check_keys(&table).map_err(|message| ConfigError::new(&place, message))?;
      let raw: RawConfig = toml::Value::Table(table)
        .try_into()
        .map_err(|error: toml::de::Error| ConfigError::new(&place, error.message()))?;
      check(raw).map(Some).map_err(|message| ConfigError::new(&place, message))
    }
    Some(other) => Err(ConfigError::new(FILE, format!("`{TABLE}` is a {}, not a table", other.type_str()))),
  }
}

/// Checks that `[host]`, each table of `[host.env]` and each step of `[[host.prepare]]` have only the keys of this build.
/// An unknown key can be a typo, or a key of a later Branch Deck: the message says both, and how to update brd-host
fn check_keys(host: &toml::Table) -> Result<(), String> {
  unknown_key(host, fields_of::<RawConfig>(), "")?;
  if let Some(toml::Value::Table(env)) = host.get("env") {
    for (name, value) in env {
      if let toml::Value::Table(table) = value {
        unknown_key(table, fields_of::<EnvTable>(), &format!("env.{name}."))?;
      }
    }
  }
  if let Some(toml::Value::Array(steps)) = host.get("prepare") {
    for step in steps {
      if let toml::Value::Table(table) = step {
        unknown_key(table, fields_of::<Step>(), "prepare.")?;
      }
    }
  }
  Ok(())
}

/// The fields of the struct `T` as its derive of `Deserialize` names them to a deserializer: the keys of its table that
/// this build knows, so the check of the keys and serde never differ
fn fields_of<'de, T: Deserialize<'de>>() -> &'static [&'static str] {
  let mut fields: &'static [&'static str] = &[];
  // the derive hands the fields to `deserialize_struct`, which keeps them and ends the deserialization
  let _ = T::deserialize(FieldsOf(&mut fields));
  fields
}

/// A deserializer that only keeps the fields of the struct that asks it for one
struct FieldsOf<'a>(&'a mut &'static [&'static str]);

/// The end of a deserialization by `FieldsOf`
#[derive(Debug)]
struct Kept;

impl fmt::Display for Kept {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str("the fields of the struct are kept")
  }
}

impl std::error::Error for Kept {}

impl serde::de::Error for Kept {
  fn custom<T: fmt::Display>(_: T) -> Self {
    Self
  }
}

impl<'de> serde::Deserializer<'de> for FieldsOf<'_> {
  type Error = Kept;

  fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Kept> {
    Err(Kept)
  }

  fn deserialize_struct<V: Visitor<'de>>(self, _: &'static str, fields: &'static [&'static str], _: V) -> Result<V::Value, Kept> {
    *self.0 = fields;
    Err(Kept)
  }

  serde::forward_to_deserialize_any! {
    bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf option unit unit_struct
    newtype_struct seq tuple tuple_struct map enum identifier ignored_any
  }
}

fn unknown_key(table: &toml::Table, known: &[&str], within: &str) -> Result<(), String> {
  match table.keys().find(|key| !known.contains(&key.as_str())) {
    None => Ok(()),
    Some(key) => Err(format!(
      "unknown key `{within}{key}`: a typo, or a key of a later Branch Deck than this brd-host, which knows {}; `brd install` updates brd-host on this Mac, and `brd-host upgrade <alias>` the agent of a host",
      known.iter().map(|known| format!("`{known}`")).collect::<Vec<_>>().join(", ")
    )),
  }
}

/// Checks the paths, the patterns of `heavy`, the names of the variables and the steps
fn check(raw: RawConfig) -> Result<RepoConfig, String> {
  for output in &raw.outputs {
    check_path("outputs", output)?;
  }
  if raw.heavy.iter().flatten().any(|pattern| pattern.trim().is_empty()) {
    return Err("`heavy` has an empty pattern".to_owned());
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
    heavy: raw.heavy,
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
