//! The brief of a review (spec/branch/review-tour.spec.md, "Brief"): what the change does for its user in the words of
//! its author, before any diff. The issue its commits name, and the prose it adds or changes, by rank: the rules of the
//! specs (with the tests their `[@test]` lines name), the records of decisions, the guides of agents, the docs. No git
//! process: the texts of these files are in the memory of the plan.

use crate::files::{ChangedFile, blob_lines, lines_in};
use crate::units::{OtherReason, ReviewUnit, UnitKind};
use serde::Serialize;
use std::collections::HashMap;
use sync_utils::issue_pattern::find_issue_number;

/// The brief of a review
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ReviewBrief {
  /// The issue key the commits name in their subjects: one key, named by half of them at least and contradicted by none
  pub issue: Option<String>,
  /// The prose the change adds, removes or changes, by the rank of its kind, then in the order of the story
  pub contract: Vec<ContractEntry>,
}

/// What a file of prose is to the change
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum ContractKind {
  /// A spec: the contract of a feature (`*.spec.md`, a file below `spec/` or `specs/`)
  Spec,
  /// A record of a decision (below `decisions/` or `adr/`)
  Decision,
  /// A guide of agents (`AGENTS.md`, `CLAUDE.md`, a file below `.ai/`)
  Guide,
  /// A doc (below `docs/`, a README)
  Doc,
}

/// A change of a file of prose: one block of it, or the file added or deleted whole
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ContractEntry {
  pub kind: ContractKind,
  /// `ReviewUnit::id`, to open it
  pub unit: String,
  pub path: String,
  /// The block of the unit's slice it is, `None` for a file added or deleted whole
  pub block: Option<u32>,
  /// The heading of the section of Markdown above it (`#`), after the commits, before them for a removal alone
  pub heading: Option<String>,
  /// A spec the commits add: its `description:` and its rules
  pub new_spec: Option<NewSpec>,
  /// The commits delete the file
  pub deleted: bool,
  /// Its paragraphs after the commits that the block adds or changes, each its lines joined, without the markers of
  /// a list and the `[@test]` lines
  pub added: Vec<String>,
  /// Its paragraphs before the commits that the block removes or changes
  pub removed: Vec<String>,
  /// The tests the `[@test]` lines it adds name, by the path they give
  pub proven_by: Vec<String>,
}

/// A spec the commits add
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct NewSpec {
  /// `description:` of its front matter
  pub description: Option<String>,
  /// The items of its "Requirements" section
  pub rules: u32,
}

/// The issue key the subjects of `messages` name (`ReviewBrief::issue`)
pub(crate) fn shared_issue<'a>(messages: impl Iterator<Item = &'a str>) -> Option<String> {
  let (mut count, mut keyed, mut key): (usize, usize, Option<&str>) = (0, 0, None);
  for message in messages {
    count += 1;
    let Some(found) = find_issue_number(message.lines().next().unwrap_or_default()) else {
      continue;
    };
    if key.is_some_and(|key| key != found) {
      return None;
    }
    key = Some(found);
    keyed += 1;
  }
  key.filter(|_| keyed * 2 >= count).map(str::to_string)
}

/// The kind of the file of prose at `path`, `None` for another file
pub(crate) fn contract_kind(path: &str) -> Option<ContractKind> {
  let lower = path.to_ascii_lowercase();
  let (directories, name) = lower.rsplit_once('/').map_or(("", lower.as_str()), |(directories, name)| (directories, name));
  let extension = name.rsplit_once('.').map_or("", |(_, extension)| extension);
  if !matches!(extension, "md" | "markdown" | "txt" | "rst" | "adoc") {
    return None;
  }
  let under = |names: &[&str]| directories.split('/').any(|directory| names.contains(&directory));
  if name.ends_with(".spec.md") || under(&["spec", "specs"]) {
    Some(ContractKind::Spec)
  } else if under(&["decisions", "adr", "adrs"]) {
    Some(ContractKind::Decision)
  } else if matches!(name, "agents.md" | "claude.md") || under(&[".ai"]) {
    Some(ContractKind::Guide)
  } else if under(&["docs", "doc"]) || name.starts_with("readme.") {
    Some(ContractKind::Doc)
  } else {
    None
  }
}

/// The brief of a review: the issue of `messages`, the prose `units` change in `changed` (texts in `texts`)
pub(crate) fn review_brief<'a>(messages: impl Iterator<Item = &'a str>, units: &[ReviewUnit], changed: &[ChangedFile<'_>], texts: &HashMap<String, String>) -> ReviewBrief {
  let file_of: HashMap<&str, &ChangedFile<'_>> = changed.iter().map(|file| (file.path, file)).collect();
  let mut contract: Vec<ContractEntry> = Vec::new();
  for unit in units {
    let prose_unit = matches!(unit.kind, UnitKind::Other { reason: OtherReason::NoLanguage } | UnitKind::NewFile | UnitKind::DeletedFile);
    for slice in unit.slices.iter().filter(|_| prose_unit) {
      let (Some(kind), Some(file)) = (contract_kind(&slice.path), file_of.get(slice.path.as_str())) else {
        continue;
      };
      let old = blob_lines(texts, file.before);
      let new = blob_lines(texts, file.after);
      // a heading of Markdown: a line of `#` in a text file is a comment
      let markdown = matches!(
        slice.path.rsplit_once('.').map(|(_, extension)| extension.to_ascii_lowercase()).as_deref(),
        Some("md" | "markdown")
      );
      let entry = |block: Option<u32>| ContractEntry {
        kind,
        unit: unit.id.clone(),
        path: slice.path.clone(),
        block,
        heading: None,
        new_spec: None,
        deleted: false,
        added: Vec::new(),
        removed: Vec::new(),
        proven_by: Vec::new(),
      };
      match unit.kind {
        UnitKind::NewFile => {
          let mut added = entry(None);
          if kind == ContractKind::Spec {
            added.new_spec = Some(new_spec(&new));
          } else {
            let (paragraphs, proven_by) = paragraphs(&new);
            added.added = paragraphs.into_iter().take(1).collect();
            added.proven_by = proven_by;
          }
          contract.push(added);
        }
        UnitKind::DeletedFile => {
          let mut deleted = entry(None);
          deleted.deleted = true;
          contract.push(deleted);
        }
        _ => {
          for block in &slice.blocks {
            let mut changed = entry(Some(block.index));
            let (added, proven_by) = paragraphs(&lines_in(&new, block.new_start..block.new_end));
            let (removed, _) = paragraphs(&lines_in(&old, block.old_start..block.old_end));
            changed.heading = if !markdown {
              None
            } else if block.new_end > block.new_start {
              heading_above(&new, block.new_start)
            } else {
              heading_above(&old, block.old_start)
            };
            changed.added = added;
            changed.removed = removed;
            changed.proven_by = proven_by;
            if !changed.added.is_empty() || !changed.removed.is_empty() || !changed.proven_by.is_empty() {
              contract.push(changed);
            }
          }
        }
      }
    }
  }
  // by rank; the order of the units, the story, within one
  contract.sort_by_key(|entry| entry.kind);
  ReviewBrief {
    issue: shared_issue(messages),
    contract,
  }
}

/// The nearest heading (`#`) above line `line` (from 0) of `lines`, without its marks
fn heading_above(lines: &[&str], line: u32) -> Option<String> {
  lines
    .get(..(line as usize).min(lines.len()))?
    .iter()
    .rev()
    .find(|text| text.starts_with('#'))
    .map(|text| text.trim_start_matches('#').trim().to_string())
}

/// The paragraphs of `lines`: a blank line, a heading and the start of an item of a list end one; each its lines
/// joined by a space, without the marker of its item; the `[@test]` lines apart, as the paths they name
fn paragraphs(lines: &[&str]) -> (Vec<String>, Vec<String>) {
  let (mut paragraphs, mut proven_by) = (Vec::new(), Vec::new());
  let mut current = String::new();
  let mut flush = |current: &mut String| {
    if !current.is_empty() {
      paragraphs.push(std::mem::take(current));
    }
  };
  for line in lines {
    let trimmed = line.trim();
    if let Some(test) = trimmed.strip_prefix("[@test]") {
      if let Some(path) = test.split_whitespace().next() {
        proven_by.push(path.trim_start_matches("../").to_string());
      }
      continue;
    }
    let item = ["- ", "* ", "+ "].iter().find_map(|marker| trimmed.strip_prefix(marker)).or_else(|| {
      let digits = trimmed.find(|c: char| !c.is_ascii_digit()).unwrap_or(0);
      (digits > 0).then(|| trimmed[digits..].strip_prefix(". ")).flatten()
    });
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("```") || item.is_some() {
      flush(&mut current);
    }
    let text = item.unwrap_or(trimmed);
    if text.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("```") {
      continue;
    }
    if !current.is_empty() {
      current.push(' ');
    }
    current.push_str(text);
  }
  flush(&mut current);
  (paragraphs, proven_by)
}

/// The description and the count of the rules of a new spec: the `description:` of its front matter, the items of its
/// "Requirements" section (a line `- ` at its start, below `## Requirements` up to the next `## `)
fn new_spec(lines: &[&str]) -> NewSpec {
  let front: Vec<&str> = if lines.first() == Some(&"---") {
    lines.iter().skip(1).take_while(|line| **line != "---").copied().collect()
  } else {
    Vec::new()
  };
  let description = front
    .iter()
    .find_map(|line| line.strip_prefix("description:"))
    .map(|text| text.trim().trim_matches('"').to_string());
  let mut inside = false;
  let mut rules = 0;
  for line in lines {
    if let Some(heading) = line.strip_prefix("## ") {
      inside = heading.trim().eq_ignore_ascii_case("requirements");
    } else if inside && line.starts_with("- ") {
      rules += 1;
    }
  }
  NewSpec { description, rules }
}
