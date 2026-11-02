//! The story of the commits of a review (spec/branch/review-tour.spec.md, "The Story of a Commit"): the body of each
//! commit once, read by the one grammar of a body (`sync_utils::commit_body`), with the code names it holds resolved to
//! the units and the files of the range, and the facts of each step a body does not tell. No git process and no query
//! of the index: the plan holds the units, the stops and the links already.
//!
//! - **The candidates** of a block that is no preformatted text: every span in backticks that names code (`name_like`:
//!   a path, a file name, or identifiers joined by `.`, `::` or `#`, a trailing `(…)` or `<…>` and a leading `@`
//!   aside), and a bare word outside backticks that is a compound name (`concerns::compound`: `toolWindowId`,
//!   `snake_case`, `v2`), an `Owner.member` with a capitalised owner, a path or a file name. A plain lowercase word is
//!   never one, in backticks neither (`changes`), unless it is called (`open()`); a span with a space, its arguments
//!   aside (`open(path, mode)`), is a command or an expression, no name.
//! - **A path** (`/` in it, or a file extension): the changed path itself, else the one path it ends on at a `/`, else
//!   the one changed file of that name; a `:line` after it aside. Two files of that name resolve nothing.
//! - **A declaration** by its segments (`.`, `::`, `#`; a leading `crate`, `self`, `super`, `Self` aside): the units of
//!   a declaration, and the declarations of a new or a deleted file by the summary the plan read (`spanned_declarations`:
//!   such a file is one unit whole; the line is that of the name, of the old side for a deleted file), whose name is the
//!   last segment, with an owner those whose key ends with every segment, else with
//!   all but the leading ones a key does not hold (the crate of a Rust path, whose key starts at its module), down to
//!   the owner and the name (JVM `Owner.member`, Rust `module::Type::member`, TypeScript `Type.member` or `Type#member`
//!   after its file path); one key resolves, its overloads one name; of two keys or more, the one among the stops of
//!   the commit's own steps, of two there the one that lands (not a removal), else nothing. Only in backticks: a
//!   capitalised name no declaration has is the file of the units it declares, when they are in one (a type whose
//!   members changed); a capitalised or compound one is the one changed file named for it, its case, `-` and `_` aside
//!   (`ReviewPrologue` of `review-prologue.ts`). A bare word resolves to a declaration or a path alone, so a product
//!   name in prose (`YouTrack`, `Node.js`) names no file.
//! - **The target** says the unit, its path, its side (the old one for a removal) and line, and the step whose stops
//!   hold it, a step of the commit first; a file its path and the step of its path. A bare candidate is kept only when
//!   it resolves, a span in backticks also when it resolves nothing (`target: None`), so `bd review-plan` shows what
//!   did not bind. Offsets are UTF-16 code units in the line of a paragraph or the item of a list, the backticks
//!   included.
//! - **Where a body is told** (`told_in`), only when the steps are the concerns of one commit: a labelled section (a
//!   label or a heading and its blocks up to the next one), else a block, goes to the step that holds the targets of
//!   most of its resolved mentions, the earliest of equal ones; the lead and a part without one stay before the steps
//!   (`None`). A story of several commits keeps the order of its author: no `told_in`.
//! - **The title of a step** of those concerns (`add_story_titles`): the label of the first labelled section told in
//!   it, a verification and a Look closely aside; the name of the concern stays its evidence.
//! - **The facts of a step**: the public declarations it adds (`ReviewStep::new_api`, the keys of `ApiChange` of kind
//!   `Added` its stops show, a new file's by its path) and the links between modules it adds (`module_links`, by the
//!   stop that holds the first use), each in one step, the first that holds it.

use crate::api::ApiChangeKind;
use crate::concerns::compound;
use crate::links::FileLink;
use crate::steps::{ReviewStep, StepStop, StepsInput, commit_title, split_message};
use crate::units::{ReviewUnit, UnitKind};
use domain_types::commit_body::{CommitBody, CommitBodyBlock, LabelKind};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use sync_utils::commit_body::commit_body;
use trial_check::symbol_dependencies::declaration_checks::spanned_declarations;

/// A commit of the range whose body has a block, its story once
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StoryCommit {
  pub id: String,
  /// Its title (`steps::commit_title`): its subject without the `(prefix)` and the issue key the brief names, as its
  /// author wrote it
  pub title: String,
  pub body: CommitBody,
  /// The code names of its blocks, in the order of the body
  pub mentions: Vec<Mention>,
  /// Of the steps of the concerns of one commit only, per block of `body`: the step (by its place in
  /// `ReviewSteps::steps`) whose opening tells it; `None` before the steps. Empty for a story of several commits
  pub told_in: Vec<Option<u32>>,
}

/// A code name in a block of a body
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct Mention {
  /// The block of `CommitBody::blocks` it is in
  pub block: u32,
  /// The line of a paragraph, the item of a list; 0 for a heading or a label
  pub item: u32,
  /// Its UTF-16 offsets in that line, the end excluded, the backticks included
  pub start: u32,
  pub end: u32,
  /// The name as the body writes it, without the backticks, a trailing `(…)` and `<…>`
  pub text: String,
  /// What it names; `None` when it resolves nothing, only of a span in backticks
  pub target: Option<MentionTarget>,
}

/// What a code name of a body names
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MentionTarget {
  /// A declaration of the range
  #[serde(rename_all = "camelCase")]
  Unit {
    /// `ReviewUnit::id`
    unit: String,
    /// The name of the declaration, its last segment (`allowed` of `Manager.allowed`): of a new or deleted file, whose
    /// unit is the file, the one name to select in it
    name: String,
    path: String,
    /// The side its line is of: the old one for a removed declaration
    side: MentionSide,
    /// The line of its name, from 1
    line: Option<u32>,
    /// The step whose stops hold it, by its place in `ReviewSteps::steps`: a step of the commit first
    step: Option<u32>,
  },
  /// A changed file
  #[serde(rename_all = "camelCase")]
  File {
    path: String,
    /// The step that shows the file, a step of the commit first
    step: Option<u32>,
  },
}

/// A side of the diff of a file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum MentionSide {
  Old,
  New,
}

/// The segments of a name of an owner that say nothing of the declaration
const PATH_KEYWORDS: [&str; 4] = ["crate", "self", "super", "Self"];
/// The longest file extension a path ends with
const MAX_EXTENSION: usize = 10;

/// A declaration of the range a name may resolve to: a unit of a declaration, or a declaration of a new file, whose
/// unit is the file
struct Declaration<'a> {
  unit: &'a ReviewUnit,
  name: String,
  /// Namespace, containers and name (`ReviewUnit::key`)
  key: String,
  /// The file it is in, after the commits; before them for a deleted file
  path: String,
  /// The line of its name, from 1
  line: Option<u32>,
  side: MentionSide,
}

impl Declaration<'_> {
  /// Its key cut at `.`: the namespace, the containers and the name
  fn segments(&self) -> Vec<&str> {
    self.key.split('.').collect()
  }
}

/// What resolves the names of a body: the declarations and the files of the range, and the steps that hold them
struct Resolver<'a> {
  declarations: Vec<Declaration<'a>>,
  paths: Vec<&'a str>,
  steps_of_unit: HashMap<&'a str, Vec<u32>>,
  steps_of_path: HashMap<&'a str, Vec<u32>>,
}

impl<'a> Resolver<'a> {
  fn new(input: &'a StepsInput<'_>, steps: &'a [ReviewStep]) -> Self {
    let mut declarations: Vec<Declaration<'a>> = input
      .units
      .iter()
      .filter_map(|unit| {
        Some(Declaration {
          unit,
          name: unit.name.clone()?,
          key: unit.key.clone()?,
          path: unit.slices.first()?.path.clone(),
          line: unit.line,
          side: if unit.kind == UnitKind::Removed { MentionSide::Old } else { MentionSide::New },
        })
      })
      .collect();
    // a new file and a deleted one are one unit whole: their declarations from the summary the plan read, of the side
    // the file is on
    for file in input.changed {
      let (blob, path, side) = match (file.before, file.after) {
        (None, Some(after)) => (after, file.path, MentionSide::New),
        (Some(before), None) => (before, file.first_path, MentionSide::Old),
        _ => continue,
      };
      let whole = |unit: &&ReviewUnit| {
        matches!(unit.kind, UnitKind::NewFile | UnitKind::DeletedFile | UnitKind::DeletedFiles { .. }) && unit.slices.iter().any(|slice| slice.path == file.path)
      };
      let (Some(symbols), Some(unit)) = (input.input, input.units.iter().find(whole)) else {
        continue;
      };
      for spanned in spanned_declarations(symbols, path, blob).map_or_default(|found| found.declarations) {
        declarations.push(Declaration {
          unit,
          name: spanned.form.name,
          key: spanned.form.key,
          path: file.path.to_string(),
          line: Some(spanned.form.line),
          side,
        });
      }
    }
    let mut steps_of_unit: HashMap<&str, Vec<u32>> = HashMap::new();
    let mut steps_of_path: HashMap<&str, Vec<u32>> = HashMap::new();
    for (place, step) in steps.iter().enumerate() {
      let place = u32::try_from(place).unwrap_or(u32::MAX);
      for stop in &step.stops {
        let of = steps_of_unit.entry(stop.unit.as_str()).or_default();
        if !of.contains(&place) {
          of.push(place);
        }
      }
      for path in &step.paths {
        steps_of_path.entry(path.as_str()).or_default().push(place);
      }
    }
    Resolver {
      declarations,
      paths: input.changed.iter().map(|file| file.path).collect(),
      steps_of_unit,
      steps_of_path,
    }
  }

  /// What `name` names, `own` the steps of its commit; a bare name (`quoted` false) only a declaration or a path: a
  /// product name in prose (`YouTrack`, `Node.js`) is no file it is named for
  fn resolve(&self, name: &str, own: &[u32], quoted: bool) -> Option<MentionTarget> {
    if path_like(name)
      && let Some(path) = self.path(name)
    {
      return Some(self.file(path, own));
    }
    let segments = segments(name)?;
    if let Some(declaration) = self.declaration(&segments, own) {
      let unit = declaration.unit;
      return Some(MentionTarget::Unit {
        unit: unit.id.clone(),
        name: declaration.name.clone(),
        path: declaration.path.clone(),
        side: declaration.side,
        line: declaration.line,
        step: pick(self.steps_of_unit.get(unit.id.as_str()), own),
      });
    }
    if !quoted {
      return None;
    }
    // a type whose members the range changes, all in one file: that file
    if let Some(path) = self.container(&segments) {
      return Some(self.file(path, own));
    }
    // a name a file is named for, its case, `-` and `_` aside (`useMeasuredHeights` of `use-measured-heights.ts`): the
    // one changed file of that stem
    let named_for = |name: &str| name.starts_with(char::is_uppercase) || compound(name);
    let mut named = self
      .paths
      .iter()
      .filter(|path| file_name(path).split_once('.').is_some_and(|(stem, _)| loose(stem) == loose(segments[0])));
    match (named.next(), named.next()) {
      (Some(path), None) if named_for(segments[0]) => Some(self.file(path, own)),
      _ => None,
    }
  }

  fn file(&self, path: &str, own: &[u32]) -> MentionTarget {
    MentionTarget::File {
      path: path.to_string(),
      step: pick(self.steps_of_path.get(path), own),
    }
  }

  /// The changed path `text` names: itself, else the one it ends on at a `/`, else the one of its file name
  fn path(&self, text: &str) -> Option<&'a str> {
    let text = without_line(text.trim_start_matches("./").trim_start_matches('/'));
    if let Some(path) = self.paths.iter().find(|path| **path == text) {
      return Some(path);
    }
    let one = |mut found: std::vec::IntoIter<&'a str>| match (found.next(), found.next()) {
      (Some(path), None) => Some(path),
      _ => None,
    };
    if text.contains('/') {
      one(
        self
          .paths
          .iter()
          .copied()
          .filter(|path| path.strip_suffix(text).is_some_and(|head| head.ends_with('/')))
          .collect::<Vec<_>>()
          .into_iter(),
      )
    } else {
      one(self.paths.iter().copied().filter(|path| file_name(path) == text).collect::<Vec<_>>().into_iter())
    }
  }

  /// The one file of the units declared inside the type `segments` name (capitalised), its owners the segments of their
  /// keys before their own name
  fn container(&self, segments: &[&str]) -> Option<&'a str> {
    if !segments.last().is_some_and(|last| last.starts_with(char::is_uppercase)) {
      return None;
    }
    let inside = |declaration: &&Declaration<'a>| {
      let key = declaration.segments();
      let owners = &key[..key.len().saturating_sub(1)];
      owners.windows(segments.len()).any(|window| window == segments)
    };
    let mut paths = self.declarations.iter().filter(inside).map(|declaration| declaration.path.as_str());
    let first = paths.next()?;
    paths.all(|path| path == first).then(|| self.paths.iter().copied().find(|path| *path == first)).flatten()
  }

  /// The declaration `segments` name: by its name, its owners the end of its key; of two keys the one of a step of the
  /// commit, of two there the one that lands (a removal names what was), else none
  fn declaration(&self, segments: &[&str], own: &[u32]) -> Option<&Declaration<'a>> {
    let (&last, _) = segments.split_last()?;
    // the key ends with every segment, else without the leading ones it does not hold (a crate, a package), down to
    // the owner and the name
    let found: Vec<&Declaration<'a>> = (0..segments.len().saturating_sub(1).max(1))
      .map(|skip| {
        let wanted = &segments[skip..];
        self
          .declarations
          .iter()
          .filter(|declaration| declaration.name == last && (wanted.len() == 1 || declaration.segments().ends_with(wanted)))
          .collect::<Vec<_>>()
      })
      .find(|found| !found.is_empty())
      .unwrap_or_default();
    let in_own = |declaration: &&&Declaration<'a>| {
      self
        .steps_of_unit
        .get(declaration.unit.id.as_str())
        .is_some_and(|steps| steps.iter().any(|step| own.contains(step)))
    };
    let keys = |found: &[&&Declaration<'a>]| found.iter().map(|declaration| declaration.key.as_str()).collect::<BTreeSet<&str>>().len();
    let all: Vec<&&Declaration<'a>> = found.iter().collect();
    if keys(&all) == 1 {
      return found.iter().find(in_own).or_else(|| found.first()).copied();
    }
    let own_found: Vec<&&Declaration<'a>> = found.iter().filter(in_own).collect();
    if keys(&own_found) == 1 {
      return Some(own_found[0]);
    }
    let landing: Vec<&&Declaration<'a>> = own_found.into_iter().filter(|declaration| declaration.side == MentionSide::New).collect();
    (keys(&landing) == 1).then(|| *landing[0])
  }
}

/// The step of `steps` among `own`, else the first
fn pick(steps: Option<&Vec<u32>>, own: &[u32]) -> Option<u32> {
  let steps = steps?;
  steps.iter().find(|step| own.contains(step)).or_else(|| steps.first()).copied()
}

/// `name` in lowercase without `-` and `_`: a stem of a file and the name it is for compare so
fn loose(name: &str) -> String {
  name.chars().filter(|c| !matches!(c, '-' | '_')).flat_map(char::to_lowercase).collect()
}

/// The last part of a path
fn file_name(path: &str) -> &str {
  path.rsplit('/').next().unwrap_or(path)
}

/// A path without the `:171` or `:171-180` of a line after it
fn without_line(text: &str) -> &str {
  match text.rsplit_once(':') {
    Some((path, line)) if !line.is_empty() && line.chars().all(|c| c.is_ascii_digit() || c == '-') => path,
    _ => text,
  }
}

/// Whether `text` reads as a path: it holds a `/`, or ends in a file extension, lowercase letters and digits after its
/// last `.`, and no `::` or `#`
fn path_like(text: &str) -> bool {
  let text = without_line(text);
  if text.contains("::") || text.contains('#') || text.contains(char::is_whitespace) {
    return false;
  }
  if text.contains('/') {
    return !text.contains("://");
  }
  text.rsplit_once('.').is_some_and(|(stem, extension)| {
    !stem.is_empty()
      && (1..=MAX_EXTENSION).contains(&extension.len())
      && extension.starts_with(|c: char| c.is_ascii_lowercase())
      && extension.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
  })
}

/// The identifiers `text` joins by `.`, `::` or `#`, a leading `crate`, `self`, `super` or `Self` aside; `None` when a
/// part is no identifier
fn segments(text: &str) -> Option<Vec<&str>> {
  let identifier = |part: &str| part.starts_with(|c: char| c.is_alphabetic() || c == '_') && part.chars().all(|c| c.is_alphanumeric() || c == '_');
  let parts: Vec<&str> = text.split("::").flat_map(|part| part.split(['.', '#'])).collect();
  if !parts.iter().all(|part| identifier(part)) {
    return None;
  }
  let start = parts.iter().take_while(|part| PATH_KEYWORDS.contains(part)).count();
  let parts = parts[start..].to_vec();
  (!parts.is_empty()).then_some(parts)
}

/// The name a span in backticks holds: without a trailing `(…)` or `<…>` and a leading `@`; `None` for a span that names
/// no code (a command, an expression, a flag) or is a plain lowercase word, which only its call (`open()`) makes a name
fn name_like(span: &str) -> Option<&str> {
  let mut name = span.trim();
  let mut call = false;
  // the arguments first, so a call written with them (`open(path, mode)`, `HashMap<K, V>`) is still a name
  for (open, close) in [('(', ')'), ('<', '>')] {
    if let Some(at) = opening_of_last(name, open, close) {
      call |= open == '(';
      name = &name[..at];
    }
  }
  if name.contains(char::is_whitespace) {
    return None;
  }
  let name = name.trim_start_matches('@');
  if path_like(name) {
    return Some(name);
  }
  let parts = segments(name)?;
  let plain = parts.len() == 1 && parts[0].chars().all(char::is_lowercase);
  (call || !plain).then_some(name)
}

/// Where the bracket `open` that matches the `close` that ends `text` is
fn opening_of_last(text: &str, open: char, close: char) -> Option<usize> {
  if !text.ends_with(close) {
    return None;
  }
  let mut depth = 0_usize;
  for (at, c) in text.char_indices().rev() {
    if c == close {
      depth += 1;
    } else if c == open {
      depth -= 1;
      if depth == 0 {
        return Some(at);
      }
    }
  }
  None
}

/// Whether a bare word may name code: a compound name, an `Owner.member` with a capitalised owner, a path
fn bare_candidate(word: &str) -> bool {
  if path_like(word) {
    return true;
  }
  segments(word).is_some_and(|parts| compound(word) || (parts.len() > 1 && parts[0].starts_with(|c: char| c.is_uppercase())))
}

/// A character of a bare word that may name code
fn word_char(c: char) -> bool {
  c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':' | '#')
}

/// The UTF-16 length of `text`
fn utf16(text: &str) -> u32 {
  u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}

/// The mentions of one line of a block (`block`, `item`): its spans in backticks, then its bare words outside them
fn line_mentions(resolver: &Resolver<'_>, line: &str, (block, item): (u32, u32), own: &[u32], into: &mut Vec<Mention>) {
  let mut found: Vec<Mention> = Vec::new();
  let mut push = |start: usize, end: usize, text: &str, target: Option<MentionTarget>| {
    found.push(Mention {
      block,
      item,
      start: utf16(&line[..start]),
      end: utf16(&line[..end]),
      text: text.to_string(),
      target,
    });
  };
  let mut at = 0;
  let mut outside: Vec<(usize, &str)> = Vec::new();
  while at < line.len() {
    let rest = &line[at..];
    let Some(open) = rest.find('`') else {
      outside.push((at, rest));
      break;
    };
    outside.push((at, &rest[..open]));
    let after = &rest[open + 1..];
    let Some(close) = after.find('`') else {
      outside.push((at + open, &rest[open..]));
      break;
    };
    let span = &after[..close];
    let (start, end) = (at + open, at + open + close + 2);
    if let Some(name) = name_like(span) {
      push(start, end, name, resolver.resolve(name, own, true));
    }
    at = end;
  }
  for (offset, text) in outside {
    let mut chars = text.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
      if !word_char(c) || (start > 0 && text[..start].chars().next_back().is_some_and(word_char)) {
        continue;
      }
      let mut end = start + c.len_utf8();
      while let Some(&(next, c)) = chars.peek() {
        if !word_char(c) {
          break;
        }
        end = next + c.len_utf8();
        chars.next();
      }
      let word = &text[start..end];
      let trimmed = word.trim_end_matches(['.', ':', '-', '/', '#']);
      let lead = trimmed.len() - trimmed.trim_start_matches(['.', ':', '-', '/', '#']).len();
      let word = &trimmed[lead..];
      if word.is_empty() || !bare_candidate(word) {
        continue;
      }
      if let Some(target) = resolver.resolve(word, own, false) {
        push(offset + start + lead, offset + start + lead + word.len(), word, Some(target));
      }
    }
  }
  found.sort_by_key(|mention| mention.start);
  into.extend(found);
}

/// The mentions of `body`, `own` the steps of its commit
fn mentions_of(resolver: &Resolver<'_>, body: &CommitBody, own: &[u32]) -> Vec<Mention> {
  let mut mentions = Vec::new();
  for (place, block) in body.blocks.iter().enumerate() {
    let block_place = u32::try_from(place).unwrap_or(u32::MAX);
    let lines: Vec<&str> = match block {
      CommitBodyBlock::Paragraph { lines } => lines.iter().map(String::as_str).collect(),
      CommitBodyBlock::List { items, .. } => items.iter().map(String::as_str).collect(),
      CommitBodyBlock::Heading { text, .. } | CommitBodyBlock::Label { text, .. } => vec![text.as_str()],
      CommitBodyBlock::Preformatted { .. } | CommitBodyBlock::Rule => Vec::new(),
    };
    for (item, line) in lines.into_iter().enumerate() {
      line_mentions(resolver, line, (block_place, u32::try_from(item).unwrap_or(u32::MAX)), own, &mut mentions);
    }
  }
  mentions
}

/// The step that tells each block of a body of one commit told by its concerns (`StoryCommit::told_in`)
fn told_in(resolver: &Resolver<'_>, body: &CommitBody, mentions: &[Mention]) -> Vec<Option<u32>> {
  // the parts a step tells whole: a labelled section of the grammar (`CommitBody::parts`), else a block; the lead alone
  let mut parts: Vec<Vec<usize>> = Vec::new();
  for part in &body.parts {
    let range = part.start as usize..part.end as usize;
    if matches!(body.blocks[range.start], CommitBodyBlock::Label { .. } | CommitBodyBlock::Heading { .. }) {
      parts.push(range.collect());
    } else {
      parts.extend(range.map(|place| vec![place]));
    }
  }
  let mut told = vec![None; body.blocks.len()];
  for part in parts.iter().filter(|part| !(body.lead && part.contains(&0))) {
    let mut score: BTreeMap<u32, u32> = BTreeMap::new();
    for mention in mentions.iter().filter(|mention| part.contains(&(mention.block as usize))) {
      let held = match &mention.target {
        Some(MentionTarget::Unit { unit, .. }) => resolver.steps_of_unit.get(unit.as_str()),
        Some(MentionTarget::File { path, .. }) => resolver.steps_of_path.get(path.as_str()),
        None => None,
      };
      for &step in held.into_iter().flatten() {
        *score.entry(step).or_default() += 1;
      }
    }
    let best = score.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(step, _)| *step);
    for &place in part {
      told[place] = best;
    }
  }
  told
}

/// The story of each commit of `input` whose body has a block, oldest first; `one_commit` when the steps are the
/// concerns of one commit, whose blocks then go to the steps that tell them
pub(crate) fn story_commits(input: &StepsInput<'_>, steps: &[ReviewStep], one_commit: bool) -> Vec<StoryCommit> {
  let resolver = Resolver::new(input, steps);
  input
    .commits
    .iter()
    .filter_map(|(id, message)| {
      let (subject, body) = split_message(message);
      let body = commit_body(body);
      if body.blocks.is_empty() {
        return None;
      }
      let own: Vec<u32> = steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step.commits.iter().any(|commit| commit.id == *id))
        .filter_map(|(place, _)| u32::try_from(place).ok())
        .collect();
      let mentions = mentions_of(&resolver, &body, &own);
      let told_in = if one_commit { told_in(&resolver, &body, &mentions) } else { Vec::new() };
      Some(StoryCommit {
        id: id.clone(),
        title: commit_title(subject, input.issue),
        body,
        mentions,
        told_in,
      })
    })
    .collect()
}

/// The title of each step of the concerns of one commit that tells a labelled section of its body (`ReviewStep::title`):
/// the label (or the heading) of the first one it tells, a section of verification or a Look closely aside, so a step
/// reads as the author named its task ("The splash") and its identifier stays the evidence that joined it
pub(crate) fn add_story_titles(steps: &mut [ReviewStep], commits: &[StoryCommit]) {
  let [commit] = commits else { return };
  for part in &commit.body.parts {
    let start = part.start as usize;
    let (CommitBodyBlock::Label {
      text: title,
      kind: LabelKind::Plain,
      ..
    }
    | CommitBodyBlock::Heading { text: title, .. }) = &commit.body.blocks[start]
    else {
      continue;
    };
    let Some(step) = commit.told_in.get(start).copied().flatten().and_then(|step| steps.get_mut(step as usize)) else {
      continue;
    };
    if step.concern.is_some() && step.title.is_none() {
      step.title = Some(title.clone());
    }
  }
}

/// Whether `stop` of a step shows line `line` (from 1) of its file after the commits: a slice taken whole, or a block of
/// it whose new side holds the line
fn shows_line(unit_of: &HashMap<&str, &ReviewUnit>, stop: &StepStop, line: u32) -> bool {
  let Some(unit) = unit_of.get(stop.unit.as_str()) else { return false };
  unit.slices.iter().filter(|slice| slice.path == stop.path).any(|slice| {
    slice.whole && slice.blocks.is_empty()
      || slice
        .blocks
        .iter()
        .filter(|block| stop.blocks.as_ref().is_none_or(|blocks| blocks.contains(&block.index)))
        .any(|block| block.new_start < line && line <= block.new_end.max(block.new_start + 1))
  })
}

/// The new API and the new links between modules of each step (`ReviewStep::new_api`, `module_links`): each in the
/// first step whose stops show it
pub(crate) fn add_step_facts(input: &StepsInput<'_>, steps: &mut [ReviewStep]) {
  let unit_of: HashMap<&str, &ReviewUnit> = input.units.iter().map(|unit| (unit.id.as_str(), unit)).collect();
  for change in input.api.iter().filter(|change| change.kind == ApiChangeKind::Added) {
    let shows = |step: &ReviewStep| {
      step.stops.iter().any(|stop| {
        stop.path == change.file
          && unit_of.get(stop.unit.as_str()).is_some_and(|unit| {
            unit.key.as_deref() == Some(change.key.as_str()) || matches!(unit.kind, UnitKind::NewFile) || unit.slices.iter().any(|slice| slice.path == stop.path && slice.whole)
          })
      })
    };
    if let Some(step) = steps.iter_mut().find(|step| shows(step))
      && !step.new_api.contains(&change.key)
    {
      step.new_api.push(change.key.clone());
    }
  }
  for (place, link) in input.links.iter().enumerate().filter(|(_, link)| link.new_between_modules) {
    let Ok(place) = u32::try_from(place) else { continue };
    let first = first_use(link);
    let at = steps
      .iter()
      .position(|step| {
        step
          .stops
          .iter()
          .any(|stop| stop.path == link.from && first.is_some_and(|line| shows_line(&unit_of, stop, line)))
      })
      .or_else(|| steps.iter().position(|step| step.paths.contains(&link.from)));
    if let Some(at) = at {
      steps[at].module_links.push(place);
    }
  }
}

/// The line of the first use of a link in the file that uses
fn first_use(link: &FileLink) -> Option<u32> {
  link.symbols.iter().map(|symbol| symbol.line).min()
}
