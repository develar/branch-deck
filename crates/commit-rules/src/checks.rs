//! The checks of a Safe Push as `checks` of `.patronus/config.yaml` lists them (docs/design/git-operations.md,
//! "Landing footprint"): each with its `configurationId`, `name`, `optional` and the `executeOnChanges` rules of the paths
//! that start it. What the price of landing two pieces in one robot reads (`domain_policy::landing_plan`): the checks
//! the changes of each piece start.
//!
//! The rules of a check match a path as Patronus matches them (`WildcardPathMatcher`: the `IgnoreNode` of jgit, each
//! path a file, the full path): the last rule that matches decides, `!` negates; a rule without a slash but a trailing
//! one is a name and matches the last segment at any depth, any other is anchored at the root (a leading `/` dropped);
//! `**` as a whole segment is any number of segments (a trailing one at least one), `*`, `?` and `[…]` match within a
//! segment; a rule with a trailing `/` matches directories only, so no file. A check of a group (`group`, one check of
//! which Patronus runs) counts as the others: the price names what may run, as the monorepo's own test of the config
//! does (`UltimatePatronusConfigTest`).
//!
//! Only the `checks` block is parsed (the file of the IntelliJ monorepo has 1.7 MB of them), and it fails open: a file
//! without it or one that does not parse is the reason the checks are unknown, never an error of the caller.

use crate::{PATRONUS_FILE, top_level_block};
use serde::Deserialize;

/// The top-level key of [`PATRONUS_FILE`] that lists the checks
const CHECKS_KEY: &str = "checks";

/// A check of a Safe Push as the file lists it
#[derive(Debug, Clone)]
pub struct PatronusCheck {
  /// The build configuration of TeamCity it runs, unique among the checks
  pub configuration_id: String,
  /// Its name as Patronus shows it; the configuration id without one
  pub name: String,
  /// A failure of it doesn't fail the robot
  pub optional: bool,
  rules: Vec<ChangeRule>,
}

impl PatronusCheck {
  /// Whether a change of `path` (relative to the root, `/` separated) starts it: the last of its rules that matches
  /// decides
  pub fn runs_on(&self, path: &str) -> bool {
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    self.rules.iter().rev().find(|rule| rule.matches(&segments)).is_some_and(|rule| !rule.negated)
  }
}

/// The checks of a `.patronus/config.yaml`, in its order
#[derive(Debug, Clone, Default)]
pub struct PatronusChecks {
  pub checks: Vec<PatronusCheck>,
}

impl PatronusChecks {
  /// The checks a change of any of `paths` starts, in the order of the file
  pub fn started_by<'a, S: AsRef<str>>(&'a self, paths: &'a [S]) -> impl Iterator<Item = &'a PatronusCheck> + 'a {
    self.checks.iter().filter(|check| paths.iter().any(|path| check.runs_on(path.as_ref())))
  }
}

/// The `checks` of the text of a `.patronus/config.yaml`, or why they are unknown
pub fn parse_patronus_checks(text: &str) -> Result<PatronusChecks, String> {
  let block = top_level_block(text, CHECKS_KEY).ok_or_else(|| format!("{PATRONUS_FILE} has no {CHECKS_KEY}"))?;
  let document: ChecksDocument = serde_saphyr::from_str(block).map_err(|error| format!("{CHECKS_KEY} of {PATRONUS_FILE} does not parse: {error}"))?;
  let checks = document
    .checks
    .into_iter()
    .map(|check| PatronusCheck {
      name: check.name.unwrap_or_else(|| check.configuration_id.clone()),
      configuration_id: check.configuration_id,
      optional: check.optional,
      rules: check.execute_on_changes.iter().map(|rule| ChangeRule::new(rule)).collect(),
    })
    .collect();
  Ok(PatronusChecks { checks })
}

/// A rule of `executeOnChanges`
#[derive(Debug, Clone)]
struct ChangeRule {
  /// `!`: a path it matches doesn't start the check
  negated: bool,
  /// A trailing `/`: a directory only, never a changed file
  directory_only: bool,
  /// A name (no slash but a trailing one): it matches the last segment at any depth
  name_only: bool,
  /// The segments, anchored at the root but for a name
  segments: Vec<String>,
}

impl ChangeRule {
  fn new(rule: &str) -> Self {
    let (negated, rule) = rule.strip_prefix('!').map_or((false, rule), |rest| (true, rest));
    let directory_only = rule.ends_with('/');
    let rule = rule.trim_end_matches('/');
    let name_only = !rule.contains('/');
    let segments = rule.trim_start_matches('/').split('/').filter(|segment| !segment.is_empty()).map(str::to_string).collect();
    Self {
      negated,
      directory_only,
      name_only,
      segments,
    }
  }

  /// Whether it matches the path of a file, given by its segments
  fn matches(&self, path: &[&str]) -> bool {
    if self.directory_only || self.segments.is_empty() {
      return false;
    }
    if self.name_only {
      return path.last().is_some_and(|name| wildmatch(self.segments[0].as_bytes(), name.as_bytes()));
    }
    segments_match(&self.segments, path)
  }
}

/// The segments of an anchored rule against all of a path; `**` is any number of segments, a trailing one at least one
fn segments_match(rule: &[String], path: &[&str]) -> bool {
  let Some((head, tail)) = rule.split_first() else {
    return path.is_empty();
  };
  if head == "**" {
    let least = usize::from(tail.is_empty());
    return (least..=path.len()).any(|skipped| segments_match(tail, &path[skipped..]));
  }
  match path.split_first() {
    Some((segment, rest)) => wildmatch(head.as_bytes(), segment.as_bytes()) && segments_match(tail, rest),
    None => false,
  }
}

/// One segment of a rule against one of a path: `*` any run (`**` inside a segment too), `?` one byte, `[…]` a class
/// (`!` or `^` negates it, `a-z` a range), `\` escapes; an unclosed `[` is itself
fn wildmatch(rule: &[u8], text: &[u8]) -> bool {
  let Some((&first, rule_rest)) = rule.split_first() else {
    return text.is_empty();
  };
  match first {
    b'*' => {
      let rest = &rule_rest[rule_rest.iter().take_while(|&&byte| byte == b'*').count()..];
      (0..=text.len()).any(|at| wildmatch(rest, &text[at..]))
    }
    b'?' => text.split_first().is_some_and(|(_, text)| wildmatch(rule_rest, text)),
    b'[' => match (class_end(rule_rest), text.split_first()) {
      (Some(end), Some((&byte, text))) => in_class(&rule_rest[..end], byte) && wildmatch(&rule_rest[end + 1..], text),
      (None, Some((&byte, text))) => byte == b'[' && wildmatch(rule_rest, text),
      (_, None) => false,
    },
    b'\\' if !rule_rest.is_empty() => text.first() == Some(&rule_rest[0]) && wildmatch(&rule_rest[1..], &text[1..]),
    literal => text.split_first().is_some_and(|(&byte, text)| byte == literal && wildmatch(rule_rest, text)),
  }
}

/// Where the class after a `[` closes (the index of its `]`); a `]` first in it is a member
fn class_end(class: &[u8]) -> Option<usize> {
  let start = usize::from(matches!(class.first(), Some(b'!' | b'^')));
  let mut index = start + usize::from(class.get(start) == Some(&b']'));
  while let Some(&byte) = class.get(index) {
    match byte {
      b'\\' => index += 2,
      b']' => return Some(index),
      _ => index += 1,
    }
  }
  None
}

/// Whether `byte` is in the class between `[` and `]`
fn in_class(class: &[u8], byte: u8) -> bool {
  let (negated, mut members) = match class.split_first() {
    Some((b'!' | b'^', rest)) => (true, rest),
    _ => (false, class),
  };
  let mut found = false;
  while let Some((&low, rest)) = members.split_first() {
    let (low, rest) = match (low, rest) {
      (b'\\', [escaped, rest @ ..]) => (*escaped, rest),
      _ => (low, rest),
    };
    if let [b'-', high, tail @ ..] = rest {
      found |= (low..=*high).contains(&byte);
      members = tail;
    } else {
      found |= low == byte;
      members = rest;
    }
  }
  found != negated
}

#[derive(Debug, Deserialize)]
struct ChecksDocument {
  #[serde(default)]
  checks: Vec<RawCheck>,
}

/// A check as Patronus writes it; the keys this reader doesn't need stay unread
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCheck {
  configuration_id: String,
  name: Option<String>,
  #[serde(default)]
  optional: bool,
  /// Every change without one, as the DSL of Patronus has it (`TeamCityCheckConfig`)
  #[serde(default = "every_change")]
  execute_on_changes: Vec<String>,
}

fn every_change() -> Vec<String> {
  vec!["*".to_string()]
}

#[cfg(test)]
#[path = "checks_test.rs"]
mod checks_test;
