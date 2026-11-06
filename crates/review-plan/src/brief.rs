//! The brief of a review (spec/branch/review-tour.spec.md, "Brief"): what the change does for its user in the words of
//! its author, before any diff. The issue its commits name, and the prose it adds or changes, by rank: the rules of the
//! specs (with the tests their `[@test]` lines name), the records of decisions, the guides of agents, the docs. No git
//! process: the texts of these files are in the memory of the plan.

use crate::files::{ChangedFile, blob_lines, lines_in};
use crate::prose::{ProseChange, ProseForm, ProseParagraph, apply_substitutions, paragraph_spans, paragraphs, prose_changes, utf16_slice};
use crate::units::{OtherReason, ReviewUnit, UnitKind};
use serde::Serialize;
use std::collections::HashMap;
use sync_utils::issue_pattern::leading_issue_keys;

/// The brief of a review
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ReviewBrief {
  /// The issue key the commits name in their subjects: one key, named by half of them at least and contradicted by none
  pub issue: Option<String>,
  /// The repeated replacements the changed paragraphs share, by the number of places
  pub substitutions: Vec<Substitution>,
  /// The prose the change adds, removes or changes, by the rank of its kind, then in the order of the story
  pub contract: Vec<ContractEntry>,
}

/// A repeated word replacement the paragraphs of the brief share
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct Substitution {
  pub from: String,
  pub to: String,
  pub places: u32,
  /// The distinct paths with a paragraph that has this replacement
  pub paths: Vec<String>,
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

/// A change of a file of prose: the paragraphs of one place of it, or the file added or deleted whole
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ContractEntry {
  pub kind: ContractKind,
  /// `ReviewUnit::id`, to open it
  pub unit: String,
  pub path: String,
  /// The blocks of the unit's slice it holds (the blocks whose paragraphs meet are one entry), none for a file added or
  /// deleted whole
  pub blocks: Vec<u32>,
  /// The heading of the section of Markdown above it (`#`), after the commits, before them for a removal alone
  pub heading: Option<String>,
  /// A spec the commits add: its `description:` and its rules
  pub new_spec: Option<NewSpec>,
  /// The commits delete the file
  pub deleted: bool,
  /// The whole paragraphs its blocks add, remove or change, in their order, without the markers of a list and the
  /// `[@test]` lines (`prose::prose_changes`): a paragraph wrapped anew is none
  pub paragraphs: Vec<ProseParagraph>,
  /// The tests the `[@test]` lines its blocks add name, by the path they give
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

/// The issue key the subjects of `messages` name (`ReviewBrief::issue`): of the keys a subject starts with
/// (`leading_issue_keys`: `IJAI-1184 IJAI-5640 …` names both), the one every subject that starts with a key names,
/// named by half of the subjects at least; the first such key of the first subject when several are
pub(crate) fn shared_issue<'a>(messages: impl Iterator<Item = &'a str>) -> Option<String> {
  let subjects: Vec<&str> = messages.map(|message| message.lines().next().unwrap_or_default()).collect();
  let keys: Vec<Vec<&str>> = subjects
    .iter()
    .map(|subject| leading_issue_keys(subject).into_iter().map(|(start, end)| &subject[start..end]).collect())
    .filter(|keys: &Vec<&str>| !keys.is_empty())
    .collect();
  let first = keys.first()?;
  first
    .iter()
    .find(|key| keys.iter().all(|named| named.contains(key)))
    .filter(|_| keys.len() * 2 >= subjects.len())
    .map(|key| (*key).to_string())
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
      let entry = |blocks: Vec<u32>| ContractEntry {
        kind,
        unit: unit.id.clone(),
        path: slice.path.clone(),
        blocks,
        heading: None,
        new_spec: None,
        deleted: false,
        paragraphs: Vec::new(),
        proven_by: Vec::new(),
      };
      match unit.kind {
        UnitKind::NewFile => {
          let mut added = entry(Vec::new());
          if kind == ContractKind::Spec {
            added.new_spec = Some(new_spec(&new));
          } else {
            let (texts, proven_by) = paragraphs(&new);
            added.paragraphs = prose_changes(&[], &texts[..texts.len().min(1)], &slice.path, |_| true);
            added.proven_by = proven_by;
          }
          contract.push(added);
        }
        UnitKind::DeletedFile => {
          let mut deleted = entry(Vec::new());
          deleted.deleted = true;
          contract.push(deleted);
        }
        _ => {
          // of a doc, only prose tells what changed: a line of a value (`dark: #88aaff`) says nothing beside the
          // heading of its section, which names the change then
          let doc = kind == ContractKind::Doc;
          for (old_range, new_range, blocks) in paragraph_spans(&old, &new, &slice.blocks) {
            let mut changed = entry(blocks.clone());
            let (before, _) = paragraphs(&old[old_range.clone()]);
            let (after, _) = paragraphs(&new[new_range.clone()]);
            changed.heading = if !markdown {
              None
            } else if !new_range.is_empty() {
              heading_above(&new, u32::try_from(new_range.start).unwrap_or(u32::MAX))
            } else {
              heading_above(&old, u32::try_from(old_range.start).unwrap_or(u32::MAX))
            };
            changed.paragraphs = prose_changes(&before, &after, &slice.path, |paragraph| {
              matches!(&paragraph.form, ProseForm::Code { .. }) || !doc || reads_as_prose(&paragraph.text)
            });
            // the tests of the `[@test]` lines the blocks add, not of the ones the widened paragraphs kept
            changed.proven_by = slice
              .blocks
              .iter()
              .filter(|it| blocks.contains(&it.index))
              .flat_map(|it| paragraphs(&lines_in(&new, it.new_start..it.new_end)).1)
              .collect();
            if !changed.paragraphs.is_empty() || !changed.proven_by.is_empty() || (doc && changed.heading.is_some()) {
              contract.push(changed);
            }
          }
        }
      }
    }
  }
  // by rank; the order of the units, the story, within one
  contract.sort_by_key(|entry| entry.kind);
  let substitutions = repeated_substitutions(&mut contract);
  ReviewBrief {
    issue: shared_issue(messages),
    substitutions,
    contract,
  }
}

/// Names repeated word replacements once, then takes their pairs out of each paragraph diff
fn repeated_substitutions(contract: &mut [ContractEntry]) -> Vec<Substitution> {
  let mut indexes: HashMap<(String, String), usize> = HashMap::new();
  let mut substitutions = Vec::new();
  for entry in contract.iter() {
    for paragraph in &entry.paragraphs {
      let ProseChange::Changed { inserted, deleted, .. } = &paragraph.change else {
        continue;
      };
      for inserted_range in inserted {
        let same_position: Vec<&_> = deleted.iter().filter(|deletion| deletion.at == inserted_range.start).collect();
        let same_inserted_position = inserted.iter().filter(|range| range.start == inserted_range.start).count();
        if same_position.len() != 1 || same_inserted_position != 1 {
          continue;
        }
        let deletion = same_position[0];
        let Some(to) = utf16_slice(&paragraph.text, *inserted_range) else {
          continue;
        };
        if !is_substitution_word(&deletion.text) || !is_substitution_word(to) {
          continue;
        }
        let pair = (deletion.text.clone(), to.to_string());
        let index = if let Some(index) = indexes.get(&pair) {
          *index
        } else {
          let index = substitutions.len();
          indexes.insert(pair.clone(), index);
          substitutions.push(Substitution {
            from: pair.0,
            to: pair.1,
            places: 0,
            paths: Vec::new(),
          });
          index
        };
        let substitution = &mut substitutions[index];
        substitution.places = substitution.places.saturating_add(1);
        if !substitution.paths.iter().any(|path| path == &entry.path) {
          substitution.paths.push(entry.path.clone());
        }
      }
    }
  }
  substitutions.retain(|substitution| substitution.places >= 3);
  substitutions.sort_by(|left, right| right.places.cmp(&left.places).then_with(|| left.from.cmp(&right.from)).then_with(|| left.to.cmp(&right.to)));
  for entry in contract {
    apply_substitutions(&mut entry.paragraphs, &substitutions);
  }
  substitutions
}

/// A replacement's words are non-whitespace tokens between 2 and 40 characters
fn is_substitution_word(text: &str) -> bool {
  let length = text.chars().count();
  (2..=40).contains(&length) && !text.chars().any(char::is_whitespace)
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

/// Whether a paragraph reads as prose: three words or more of letters alone, its punctuation aside, that are no key
/// (`primary:`) and no value right after one, so a line of values (`primary: #3366ff, secondary: #ff8800`, `color:
/// red; background: blue`) is none
fn reads_as_prose(paragraph: &str) -> bool {
  let mut words = 0;
  let mut after_key = false;
  for token in paragraph.split_whitespace() {
    let key = token.len() > 1 && token.ends_with(':');
    let word = token.trim_matches(|c: char| !c.is_alphanumeric());
    if !key && !after_key && !word.is_empty() && word.chars().all(char::is_alphabetic) {
      words += 1;
    }
    after_key = key;
  }
  words >= 3
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
