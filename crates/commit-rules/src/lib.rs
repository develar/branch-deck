//! The commit rules of a repository (openspec/specs/cli/pick/spec.md, "The commit rules"): `[commit]` of `.branchdeck.toml` names
//! them, and a pick checks the message of each commit it lands by them, before anything lands. No git, no network.
//!
//! ```toml
//! [commit]
//! check = "patronus"                  # the commitMessageFormat of .patronus/config.yaml
//! # check = ["(feat|fix|docs)\\(.+\\): .+"]  # or regexes of its own, one of which must match
//! ```
//!
//! The owner of `.branchdeck.toml` wrote `[commit]`, so a bad value there is an error of the file
//! ([`parse_commit_check`]): a typo must not drop the check without a word. `.patronus/config.yaml` belongs to another
//! team, and Patronus checks the push anyway, so its content fails open ([`load_rules`]): a missing file or block, a
//! block that does not parse, or a rule that does not compile skips the check, and the reason says why.
//!
//! A rule matches the whole message, as `Regex.matches` of Kotlin with `DOT_MATCHES_ALL` and `MULTILINE` does in the
//! safe-push plugin: each one is compiled as `(?ms)\A(?:<rule>)\z`, so `(?i)WIP.*` refuses `WIP: x`, never a body
//! that has "swipe". The message is the one that lands on the baseline: the caller strips the `(branch)` prefix, and
//! [`MessageRules::check`] strips the leading `fixup! `, `squash! ` and `amend! `, which `brd fold` removes before a push.

use anyhow::{Context as _, Result, anyhow, bail};
use fancy_regex::Regex;
use glob::{MatchOptions, Pattern};
use serde::Deserialize;
use std::path::Path;

/// The file of Branch Deck at the root of a checkout
pub const BRANCH_DECK_FILE: &str = ".branchdeck.toml";
/// The configuration of Patronus at the root of a checkout
pub const PATRONUS_FILE: &str = ".patronus/config.yaml";
/// The top-level key of [`PATRONUS_FILE`] that holds the rules of a message
const FORMAT_KEY: &str = "commitMessageFormat";
/// The value of `check` that names the rules of Patronus
const PATRONUS: &str = "patronus";
/// Git's autosquash markers, which `brd fold` removes before a push
const MARKERS: [&str; 3] = ["fixup! ", "squash! ", "amend! "];

/// Which rules `[commit] check` names
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitCheck {
  /// `check = "patronus"`: the `commitMessageFormat` of `.patronus/config.yaml`
  Patronus,
  /// `check = ["<regex>", ...]`: one of them must match; a violation is STRICT
  Rules(Vec<String>),
}

/// `[commit] check` of `.branchdeck.toml` in `checkout`: `None` without the file, the table or the key. A file that does
/// not parse, an unknown key of `[commit]` or a bad value is an error that names the file
pub fn read_commit_check(checkout: &Path) -> Result<Option<CommitCheck>> {
  let path = checkout.join(BRANCH_DECK_FILE);
  let named = || format!("Cannot read {}", path.display());
  let text = match std::fs::read_to_string(&path) {
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    read => read.with_context(named)?,
  };
  parse_commit_check(&text).with_context(named)
}

/// `[commit] check` of the text of a `.branchdeck.toml`. The other tables stay unread: their readers check them
pub fn parse_commit_check(text: &str) -> Result<Option<CommitCheck>> {
  let mut document: toml::Table = text.parse()?;
  let Some(commit) = document.remove("commit") else { return Ok(None) };
  let toml::Value::Table(mut commit) = commit else { bail!("`commit` is not a table") };
  if let Some(key) = commit.keys().find(|key| *key != "check") {
    bail!("unknown key commit.{key}; [commit] has only `check`");
  }
  match commit.remove("check") {
    None => Ok(None),
    Some(toml::Value::String(name)) if name == PATRONUS => Ok(Some(CommitCheck::Patronus)),
    Some(toml::Value::Array(values)) => {
      let rules = values
        .into_iter()
        .map(|value| match value {
          toml::Value::String(rule) => compile(&rule)
            .map(|_| rule)
            .map_err(|error| anyhow!("`commit.check` has a regex that does not compile: {error}")),
          other => Err(anyhow!("`commit.check` has a {}, not a regex", other.type_str())),
        })
        .collect::<Result<Vec<String>>>()?;
      if rules.is_empty() {
        bail!("`commit.check` names no regex");
      }
      Ok(Some(CommitCheck::Rules(rules)))
    }
    Some(other) => bail!("`commit.check` is {other}; it is \"{PATRONUS}\" or an array of regexes"),
  }
}

/// How much a violation of the rules counts, as Patronus words it: `IGNORE`, `WARNING`, `STRICT`; the order is the
/// strictness
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
  /// The violation is no matter: the commit changes only paths that the rules leave out (`/fleet/**`)
  Ignore,
  /// The commit lands, with a warning
  Warning,
  /// Nothing lands
  Strict,
}

impl Severity {
  /// The word of Patronus: `STRICT`, `WARNING`, `IGNORE`
  fn parse(word: &str) -> Option<Self> {
    match word {
      "STRICT" => Some(Self::Strict),
      "WARNING" => Some(Self::Warning),
      "IGNORE" => Some(Self::Ignore),
      _ => None,
    }
  }
}

/// A rule as the file has it and compiled
#[derive(Debug, Clone)]
struct Rule {
  regex: Regex,
}

/// A rule of severity: it applies to a commit when every path the commit changes matches one of `paths`
#[derive(Debug, Clone)]
struct SeverityRule {
  paths: Vec<Pattern>,
  severity: Severity,
}

/// The rules of a message, compiled: one of `rules` must match (none: any message), none of `negative` may
#[derive(Debug, Clone)]
pub struct MessageRules {
  rules: Vec<Rule>,
  negative: Vec<Rule>,
  severity_default: Severity,
  severity_rules: Vec<SeverityRule>,
  /// What a message must look like (`hintMessage`), for the refusal
  pub hint: Option<String>,
  /// Where the form is documented (`documentationLink`)
  pub documentation: Option<String>,
}

impl MessageRules {
  /// How much a violation counts in a commit that changes `paths` (relative to the root, `/` separated): the strictest
  /// rule of severity that applies to every path, else the default. A commit without changed paths takes the default
  pub fn severity(&self, paths: &[String]) -> Severity {
    let options = MatchOptions {
      case_sensitive: true,
      require_literal_separator: true,
      require_literal_leading_dot: false,
    };
    let rooted: Vec<String> = paths.iter().map(|path| format!("/{}", path.trim_start_matches('/'))).collect();
    self
      .severity_rules
      .iter()
      .filter(|rule| !rooted.is_empty() && rooted.iter().all(|path| rule.paths.iter().any(|pattern| pattern.matches_with(path, options))))
      .map(|rule| rule.severity)
      .max()
      .unwrap_or(self.severity_default)
  }

  /// Whether the rules accept `message`, after its leading autosquash markers and the trailing whitespace. A rule
  /// that fails at run time (the backtrack limit) counts as the answer that accepts: the check fails open
  pub fn accepts(&self, message: &str) -> bool {
    let mut text = message.trim_end();
    while let Some(rest) = MARKERS.iter().find_map(|marker| text.strip_prefix(marker)) {
      text = rest.trim_start();
    }
    let matched = |rule: &Rule, failed: bool| rule.regex.is_match(text).unwrap_or(failed);
    (self.rules.is_empty() || self.rules.iter().any(|rule| matched(rule, true))) && !self.negative.iter().any(|rule| matched(rule, false))
  }

  /// The severity of `message` of a commit that changes `paths`: `None` when the rules accept it or the paths make it
  /// `IGNORE`
  pub fn check(&self, message: &str, paths: &[String]) -> Option<Severity> {
    if self.accepts(message) {
      return None;
    }
    Some(self.severity(paths)).filter(|severity| *severity != Severity::Ignore)
  }
}

/// The rules of `check` for `checkout`, or why the check is skipped: the content of `.patronus/config.yaml` fails open
pub fn load_rules(checkout: &Path, check: &CommitCheck) -> Result<MessageRules, String> {
  match check {
    CommitCheck::Rules(rules) => Ok(MessageRules {
      rules: rules.iter().map(|rule| compile(rule).map(|regex| Rule { regex })).collect::<Result<_, _>>()?,
      negative: Vec::new(),
      severity_default: Severity::Strict,
      severity_rules: Vec::new(),
      hint: Some(format!(
        "A message must match one of the regexes of `[commit] check` of {BRANCH_DECK_FILE}: {}",
        rules.join(", ")
      )),
      documentation: None,
    }),
    CommitCheck::Patronus => {
      let path = checkout.join(PATRONUS_FILE);
      let text = std::fs::read_to_string(&path).map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
      parse_patronus_rules(&text)
    }
  }
}

/// The `commitMessageFormat` of the text of a `.patronus/config.yaml`, or why the check is skipped. Only that top-level
/// block is parsed: the file of the IntelliJ monorepo has 750 KB of other checks
pub fn parse_patronus_rules(text: &str) -> Result<MessageRules, String> {
  let block = format_block(text).ok_or_else(|| format!("{PATRONUS_FILE} has no {FORMAT_KEY}"))?;
  let document: FormatDocument = serde_saphyr::from_str(block).map_err(|error| format!("{FORMAT_KEY} of {PATRONUS_FILE} does not parse: {error}"))?;
  let format = document.format.ok_or_else(|| format!("{PATRONUS_FILE} has no {FORMAT_KEY}"))?;
  let compiled = |rules: &[RawRule]| {
    rules
      .iter()
      .map(|rule| compile(&rule.regex).map(|regex| Rule { regex }))
      .collect::<Result<Vec<Rule>, String>>()
      .map_err(|error| format!("a rule of {FORMAT_KEY} of {PATRONUS_FILE} does not compile: {error}"))
  };
  let severity = |word: &str| Severity::parse(word).ok_or_else(|| format!("{FORMAT_KEY} of {PATRONUS_FILE} has the unknown severity {word:?}"));
  let mut severity_default = format.severity_default.as_deref().map_or(Ok(Severity::Strict), severity)?;
  let mut severity_rules = Vec::with_capacity(format.severity_rules.len());
  for rule in &format.severity_rules {
    let paths = rule
      .paths
      .iter()
      .map(|path| Pattern::new(path).map_err(|error| format!("the path {path:?} of {FORMAT_KEY} of {PATRONUS_FILE} is no glob: {error}")))
      .collect::<Result<_, _>>()?;
    severity_rules.push(SeverityRule {
      paths,
      severity: severity(&rule.severity)?,
    });
  }
  if format.warning_only {
    severity_default = severity_default.min(Severity::Warning);
    for rule in &mut severity_rules {
      rule.severity = rule.severity.min(Severity::Warning);
    }
  }
  Ok(MessageRules {
    rules: compiled(&format.rules)?,
    negative: compiled(&format.negative_rules)?,
    severity_default,
    severity_rules,
    hint: format.hint_message,
    documentation: format.documentation_link,
  })
}

/// The top-level block `commitMessageFormat:` of a YAML text: its key line and every line after it up to the next
/// top-level key or document marker. Blank lines, comments, indented lines and the `- ` items of a block sequence at the
/// left edge belong to it
fn format_block(text: &str) -> Option<&str> {
  let mut start = None;
  let mut offset = 0;
  for line in text.split_inclusive('\n') {
    let at = offset;
    offset += line.len();
    match start {
      None if line.strip_prefix(FORMAT_KEY).is_some_and(|rest| rest.trim_start().starts_with(':')) => start = Some(at),
      Some(start) if ends_block(line) => return Some(&text[start..at]),
      _ => {}
    }
  }
  start.map(|start| &text[start..])
}

/// Whether `line` starts the next top-level entry of a YAML text: a key or a document marker at the left edge
fn ends_block(line: &str) -> bool {
  let Some(first) = line.chars().next() else { return false };
  if line.starts_with("---") || line.starts_with("...") {
    return true;
  }
  !(first.is_whitespace() || first == '#' || first == '-')
}

/// A rule as `regex` of the file, compiled to match the whole message
fn compile(rule: &str) -> Result<Regex, String> {
  Regex::new(&format!(r"(?ms)\A(?:{rule})\z")).map_err(|error| format!("{rule:?}: {error}"))
}

#[derive(Debug, Deserialize)]
struct FormatDocument {
  #[serde(rename = "commitMessageFormat")]
  format: Option<RawFormat>,
}

/// `commitMessageFormat` as Patronus writes it; a key this version does not know stays unread
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFormat {
  #[serde(default)]
  rules: Vec<RawRule>,
  #[serde(default)]
  negative_rules: Vec<RawRule>,
  documentation_link: Option<String>,
  hint_message: Option<String>,
  severity_default: Option<String>,
  #[serde(default)]
  severity_rules: Vec<RawSeverityRule>,
  /// Every violation is a warning at most
  #[serde(default)]
  warning_only: bool,
}

#[derive(Debug, Deserialize)]
struct RawRule {
  regex: String,
}

#[derive(Debug, Deserialize)]
struct RawSeverityRule {
  paths: Vec<String>,
  severity: String,
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod lib_test;
