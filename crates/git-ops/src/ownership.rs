//! The code owners of paths by the `OWNERSHIP` files of a tree, read-only: the files of every ancestor directory of
//! the paths from one `cat-file --batch`. An `OWNERSHIP` file is sections of `/pattern` lines closed by `owner 'Name'`
//! (no pattern is `/**`), `#` comments and blank lines; a pattern is relative to the directory of its file, with
//! gitignore semantics. The rules are ordered by the depth of their file, then by its path, then by their order in it;
//! the last rule that matches a path names its owner. Patronus resolves the owners the same way
//! (`OwnershipMappingBuilder`, `OwnershipMapping.getMatch`), so the owner of a path is the group whose review the
//! path needs, unless Patronus delegates it or `review-rules.yaml` names other reviewers.

use crate::cat_file_batch::parse_batch_output;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tracing::{instrument, warn};

/// The file name of the ownership rules of a directory
pub const OWNERSHIP_FILE_NAME: &str = "OWNERSHIP";

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetFileOwnersParams {
  pub repository_path: String,
  /// The tree whose `OWNERSHIP` files decide, the branch the changes are pushed into (e.g. `origin/master`)
  pub baseline_branch: String,
  pub paths: Vec<String>,
}

/// The owner of a path and the rule that names it
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FileOwner {
  pub path: String,
  pub owner: String,
  /// The pattern relative to the repository root, e.g. `plugins/air/**`
  pub rule: String,
  /// The `OWNERSHIP` file of the rule, e.g. `plugins/air/OWNERSHIP`
  pub source_file: String,
}

/// A rule of an `OWNERSHIP` file
#[derive(Debug, Clone, PartialEq, Eq)]
struct OwnershipRule {
  source_file: String,
  /// `<directory of the file>/<pattern>` without the leading `/`
  rule: String,
  pattern: Pattern,
  owner: String,
}

/// The rules of the `OWNERSHIP` files that can match some paths, in the order a lookup reverses
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnershipRules {
  rules: Vec<OwnershipRule>,
}

impl OwnershipRules {
  /// Parse `OWNERSHIP` files given as `(path of the file, its text)`; a file that doesn't parse is skipped with a warning
  pub fn from_files<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
    let mut files: Vec<(&str, &str)> = files.into_iter().collect();
    files.sort_by(|(a, _), (b, _)| depth(a).cmp(&depth(b)).then_with(|| a.cmp(b)));
    let mut rules = Vec::new();
    for (source_file, text) in files {
      match parse_ownership_file(source_file, text) {
        Ok(file_rules) => rules.extend(file_rules),
        Err(error) => warn!(source_file, %error, "skipping an OWNERSHIP file that does not parse"),
      }
    }
    Self { rules }
  }

  pub fn is_empty(&self) -> bool {
    self.rules.is_empty()
  }

  /// The owner of a repository-relative path: the last rule that matches it
  pub fn owner_of(&self, path: &str) -> Option<FileOwner> {
    let segments: Vec<&str> = path.split('/').collect();
    self.rules.iter().rev().find(|rule| rule.pattern.matches(&segments)).map(|rule| FileOwner {
      path: path.to_string(),
      owner: rule.owner.clone(),
      rule: rule.rule.clone(),
      source_file: rule.source_file.clone(),
    })
  }
}

/// The depth Patronus orders `OWNERSHIP` files by: the slashes of the file path
fn depth(source_file: &str) -> usize {
  source_file.bytes().filter(|&byte| byte == b'/').count()
}

fn parse_ownership_file(source_file: &str, text: &str) -> Result<Vec<OwnershipRule>> {
  let directory = source_file.rsplit_once('/').map_or("", |(directory, _)| directory);
  let mut rules = Vec::new();
  let mut patterns: Vec<&str> = Vec::new();
  for (index, line) in text.lines().enumerate() {
    if let Some(rest) = line.strip_prefix("owner ") {
      let owner = rest
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .filter(|owner| !owner.is_empty() && !owner.contains('\''))
        .ok_or_else(|| anyhow!("line {}: expected `owner 'Name'`: {line:?}", index + 1))?;
      if patterns.is_empty() {
        patterns.push("/**");
      }
      for pattern in patterns.drain(..) {
        let pattern = pattern.trim_start_matches('/');
        let rule = if directory.is_empty() { pattern.to_string() } else { format!("{directory}/{pattern}") };
        rules.push(OwnershipRule {
          source_file: source_file.to_string(),
          pattern: Pattern::new(&rule),
          rule,
          owner: owner.to_string(),
        });
      }
    } else if line.starts_with('/') {
      patterns.push(line);
    } else if !(line.starts_with('#') || line.trim().is_empty()) {
      return Err(anyhow!("line {}: a pattern starts with `/`: {line:?}", index + 1));
    }
  }
  if !patterns.is_empty() {
    return Err(anyhow!("the last patterns have no `owner` line"));
  }
  Ok(rules)
}

/// An anchored gitignore pattern, as jgit's `FastIgnoreRule.isMatch(path, false)` matches it: the pattern matches the
/// path or one of its parent directories; `**` as a whole segment is any number of segments (a trailing one at least
/// one), `*`, `?` and `[…]` match within a segment, a trailing `/` matches only a directory
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pattern {
  segments: Vec<String>,
  directory_only: bool,
}

impl Pattern {
  fn new(pattern: &str) -> Self {
    let directory_only = pattern.ends_with('/');
    let segments = pattern.trim_end_matches('/').split('/').filter(|segment| !segment.is_empty()).map(str::to_string).collect();
    Self { segments, directory_only }
  }

  fn matches(&self, path: &[&str]) -> bool {
    if self.segments.is_empty() {
      return false;
    }
    // a prefix shorter than the path is a directory of it
    (1..=path.len()).any(|end| (end < path.len() || !self.directory_only) && match_segments(&self.segments, &path[..end]))
  }
}

fn match_segments(pattern: &[String], path: &[&str]) -> bool {
  match pattern.split_first() {
    None => path.is_empty(),
    Some((first, rest)) if first == "**" => {
      // a trailing `**` is everything inside, not the directory itself
      let min = usize::from(rest.is_empty());
      (min..=path.len()).any(|skip| match_segments(rest, &path[skip..]))
    }
    Some((first, rest)) => path
      .split_first()
      .is_some_and(|(segment, path)| match_glob(first.as_bytes(), segment.as_bytes()) && match_segments(rest, path)),
  }
}

/// A wildmatch of one segment: `*`, `?`, `[abc]`, `[a-z]`, `[!a]` (`[^a]`) and `\` escapes; `**` inside a segment is `*`
fn match_glob(pattern: &[u8], text: &[u8]) -> bool {
  match pattern.split_first() {
    None => text.is_empty(),
    Some((b'*', rest)) => (0..=text.len()).any(|skip| match_glob(rest, &text[skip..])),
    Some((b'?', rest)) => !text.is_empty() && match_glob(rest, &text[1..]),
    Some((b'[', rest)) => match (text.split_first(), match_class(rest)) {
      (Some((&byte, text)), Some((class, rest))) => class.contains(byte) && match_glob(rest, text),
      // an unclosed `[` is a literal
      (Some((&byte, text)), None) => byte == b'[' && match_glob(rest, text),
      (None, _) => false,
    },
    Some((b'\\', rest)) if !rest.is_empty() => text.first() == rest.first() && match_glob(&rest[1..], &text[1..]),
    Some((&byte, rest)) => text.first() == Some(&byte) && match_glob(rest, &text[1..]),
  }
}

struct ByteClass<'a> {
  negated: bool,
  items: &'a [u8],
}

impl ByteClass<'_> {
  fn contains(&self, byte: u8) -> bool {
    let mut items = self.items;
    let mut found = false;
    while let Some((&first, rest)) = items.split_first() {
      let (first, rest) = if first == b'\\' && !rest.is_empty() { (rest[0], &rest[1..]) } else { (first, rest) };
      if let [b'-', last, tail @ ..] = rest {
        found |= (first..=*last).contains(&byte);
        items = tail;
      } else {
        found |= first == byte;
        items = rest;
      }
    }
    found != self.negated
  }
}

/// The class after `[` and the pattern after its `]`; None when it is not closed
fn match_class(pattern: &[u8]) -> Option<(ByteClass<'_>, &[u8])> {
  let (negated, body) = match pattern.first() {
    Some(b'!' | b'^') => (true, &pattern[1..]),
    _ => (false, pattern),
  };
  // a `]` right after `[` (or `[!`) is a member
  let mut index = usize::from(body.first() == Some(&b']'));
  while index < body.len() {
    match body[index] {
      b'\\' => index += 2,
      b']' => return Some((ByteClass { negated, items: &body[..index] }, &body[index + 1..])),
      _ => index += 1,
    }
  }
  None
}

/// The directories whose `OWNERSHIP` files can name the owners of the paths: the root and every parent directory
fn ownership_directories<'a>(paths: impl IntoIterator<Item = &'a str>) -> BTreeSet<&'a str> {
  let mut directories = BTreeSet::new();
  for path in paths {
    directories.insert("");
    let mut end = 0;
    while let Some(slash) = path[end..].find('/') {
      end += slash;
      directories.insert(&path[..end]);
      end += 1;
    }
  }
  directories
}

/// The rules of the `OWNERSHIP` files of the tree `revision` that can match the paths, from one `cat-file --batch`
#[instrument(skip(git, paths), fields(paths = paths.len()))]
pub fn read_ownership(git: &GitCommandExecutor, repository_path: &str, revision: &str, paths: &[String]) -> Result<OwnershipRules> {
  if paths.is_empty() {
    return Ok(OwnershipRules::default());
  }
  let files: Vec<String> = ownership_directories(paths.iter().map(String::as_str))
    .into_iter()
    .map(|directory| {
      if directory.is_empty() {
        OWNERSHIP_FILE_NAME.to_string()
      } else {
        format!("{directory}/{OWNERSHIP_FILE_NAME}")
      }
    })
    .collect();
  let input = files.iter().fold(String::new(), |mut input, file| {
    input.push_str(revision);
    input.push(':');
    input.push_str(file);
    input.push('\0');
    input
  });
  let output = git.execute_command_with_input_raw(&["cat-file", "--batch", "--buffer", "-Z"], repository_path, &input)?;
  let objects = parse_batch_output(&output, files.len())?;
  let texts: Vec<(&str, String)> = files
    .iter()
    .zip(objects)
    .filter_map(|(file, object)| {
      object
        .filter(|object| object.kind == "blob")
        .map(|object| (file.as_str(), String::from_utf8_lossy(object.content).into_owned()))
    })
    .collect();
  Ok(OwnershipRules::from_files(texts.iter().map(|(file, text)| (*file, text.as_str()))))
}

/// The owners of the paths that have one, in the order of the paths
pub fn read_file_owners(git: &GitCommandExecutor, repository_path: &str, revision: &str, paths: &[String]) -> Result<Vec<FileOwner>> {
  let rules = read_ownership(git, repository_path, revision, paths)?;
  Ok(paths.iter().filter_map(|path| rules.owner_of(path)).collect())
}
