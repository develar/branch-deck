//! The steps of a review (spec/branch/review-tour.spec.md): the story the review window tells first, each step a part
//! of the change with the words of its author, the diffs that carry it and the tests that name its code. No git
//! process: the plan read every commit of the range and its hunks already.
//!
//! - **Where a line comes from**: the lines of each file are carried through the hunks of the commits that change it,
//!   oldest first (`carry`), so each line at the head names the commit that wrote it last, and each line of the base
//!   the commit that removed it. A block of the range belongs to the commit of most of its lines.
//! - **Steps of the commits**, when the commits tell them: a commit with a body (`tells_a_story`) starts a step and owns
//!   the units most of whose lines it wrote; only what lands is shown, the units of the range. A commit whose changed
//!   code lines are mostly lines an earlier step head wrote follows that step up (a round of it), and so does a commit
//!   that owns no unit; a commit without a body owns nothing, its lines show where they landed, and the units only such
//!   commits wrote are the step "Changes without a story". A unit of a file of no language is cut by its blocks.
//! - **Steps of the concerns** when the commits tell no story: no body, or one commit, whose message is the brief. A
//!   long branch is told by its commits too: measured on idea-1, its concerns were no shorter a list, and they lost
//!   the words of the author (review-tour.spec.md, "Measurements").
//! - **A step whose lines a later commit rewrote whole** shows nothing of its own: it follows the step of that commit.
//! - **The order** of a step is that of the story, but a declaration the step adds or changes the form of comes first
//!   and the uses of it in the step right after it (`uses`); a mechanical part follows the step its concern is in.
//! - **The tests that name it**: per declaration of the code of a step, the tests of the range whose changed lines name
//!   it (a member with its owner named in the test's file), and those none names.
//! - **Coverage**: every block of the range in one stop, else in a mechanical unit (`StepCoverage`).

use crate::brief::ContractEntry;
use crate::concerns::{Concern, ConcernName, EvidenceKind, names_word};
use crate::files::{ChangedFile, blob_lines, lines_in, test_subject};
use crate::links::FileLink;
use crate::pair::{PairBlocks, block_of};
use crate::units::{OtherReason, ReviewUnit, UnitKind, UnitSlice};
use code_symbols::language::language_of;
use object_cache::commit_diff::FileChange;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::ops::Range;
use trial_check::diff_scan::CommitDiff;
use trial_check::symbol_scan::SymbolInput;

/// A declaration more stops of a step use than this is a hub: its uses keep the order of the story
const MAX_USES: usize = 6;
/// The shortest name of a declaration whose uses follow it, or that a test names
const MIN_NAME: usize = 3;
/// The fewest words of a body that tells a step
const MIN_BODY_WORDS: usize = 3;
/// The lines past the last one of a file a hunk names: the base goes on unchanged
const TAIL: u64 = u64::MAX / 4;

/// The steps of a review
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ReviewSteps {
  pub source: StepSource,
  /// In the order of the story: the commits that start them, oldest first; "Changes without a story" last
  pub steps: Vec<ReviewStep>,
  pub coverage: StepCoverage,
}

/// Where the steps come from
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StepSource {
  /// Each step starts at a commit whose body says what it does and why
  Commits,
  /// The concerns of the range are the steps: the commits tell no story
  Concerns { reason: ConcernsReason },
}

/// Why the concerns are the steps
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum ConcernsReason {
  /// No commit of the range has a body
  NoStory,
  /// The range is one commit: its message is the brief
  OneCommit,
}

/// A step of the story
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ReviewStep {
  /// The commit that starts it, the id of its concern, or `"no-story"`
  pub id: String,
  /// The commits of the step: the one that starts it first, then those that follow it up, then those of no story
  /// whose lines are mostly in it; of a step of a concern, the commits of its lines, the most lines first
  pub commits: Vec<StepCommit>,
  /// The concern a step of the concerns is
  pub concern: Option<String>,
  /// What the step shows, in its order
  pub stops: Vec<StepStop>,
  pub tests: StepTests,
}

/// A commit of a step
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StepCommit {
  pub id: String,
  pub subject: String,
  /// The message after its subject, trimmed; empty for a commit of no story
  pub body: String,
  pub role: CommitRole,
}

/// What a commit is to its step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum CommitRole {
  /// Its body tells the step
  Starts,
  /// A round of the step: it rewrites the code the commit that starts it wrote, or owns no change of its own
  FollowsUp,
  /// It has no body; its lines are in this step
  NoStory,
}

/// What a step shows of one unit in one file
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StepStop {
  /// `ReviewUnit::id`
  pub unit: String,
  pub path: String,
  /// The blocks of the unit's slice of `path` it shows, by their index; `None` for the whole slice
  pub blocks: Option<Vec<u32>>,
  /// The declaration of this step it uses, shown right after it
  pub uses: Option<String>,
  /// The other steps whose commits changed its lines, by their place in `ReviewSteps::steps`
  pub also: Vec<u32>,
  /// The concern of its unit, when the step holds parts of two concerns or more that evidence joined
  pub concern: Option<String>,
}

/// The tests of the range that name the code of a step
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StepTests {
  /// Its declarations that a test names, each with those tests
  pub named: Vec<NamedDeclaration>,
  /// Its declarations of a parsed language whose body or form changed, or that it adds, that no test of the range
  /// names; none of a test file, whose helpers a test runs. A name two of them share is led by its owner
  pub unnamed: Vec<String>,
  /// The tests the `[@test]` lines of the rules of a spec it adds or changes name, by the path they give: a scenario
  /// that names no declaration
  pub scenarios: Vec<String>,
}

/// A declaration of a step and the tests that name it
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct NamedDeclaration {
  pub name: String,
  pub tests: Vec<TestRef>,
}

/// A test of the range
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct TestRef {
  /// `ReviewUnit::id`
  pub unit: String,
  pub path: String,
  /// The name the test reads as (`test_name`)
  pub name: String,
}

/// Whether the steps show every block of the range once
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StepCoverage {
  /// The blocks of the range, a file without blocks one
  pub blocks: u32,
  pub in_stops: u32,
  /// In a mechanical unit no stop shows
  pub mechanical: u32,
  /// In no stop and no mechanical unit
  pub missing: u32,
  /// In two stops or more
  pub twice: u32,
}

/// What the steps read of the plan
pub(crate) struct StepsInput<'a> {
  /// The reviewed commits, oldest first: id and message
  pub commits: &'a [(String, String)],
  pub diffs: &'a [CommitDiff],
  pub changed: &'a [ChangedFile<'a>],
  /// By the place of each file of `changed`
  pub pairs: &'a [PairBlocks],
  pub units: &'a [ReviewUnit],
  pub concerns: &'a [Concern],
  pub input: Option<&'a SymbolInput>,
  pub texts: &'a HashMap<String, String>,
  pub links: &'a [FileLink],
  /// The prose of the brief: the tests the `[@test]` lines of a spec name prove the step that changes it
  pub contract: &'a [ContractEntry],
}

/// Whether a commit message tells a step: a body of `MIN_BODY_WORDS` words or more after its subject, and no
/// `fixup!`, `squash!` or `amend!` subject
pub(crate) fn tells_a_story(message: &str) -> bool {
  let (subject, body) = split_message(message);
  !["fixup!", "squash!", "amend!"].iter().any(|mark| subject.starts_with(mark))
    && body.split_whitespace().filter(|word| word.chars().any(char::is_alphabetic)).count() >= MIN_BODY_WORDS
}

/// The subject of a message and its body, trimmed
pub(crate) fn split_message(message: &str) -> (&str, &str) {
  let message = message.trim();
  message.split_once('\n').map_or((message, ""), |(subject, body)| (subject.trim(), body.trim()))
}

/// Where a line of a file is from: a line of the base by its place there, or the commit that wrote it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
  Base(u64),
  Commit(usize),
}

#[derive(Debug, Clone, Copy)]
struct Run {
  len: u64,
  origin: Origin,
}

/// The lines of one file carried through the commits that change it
#[derive(Default)]
struct Carried {
  /// The lines at the head, in runs, the base going on past them
  runs: Vec<Run>,
  /// The lines of the base a commit removed: (from, to, commit)
  removed: Vec<(u64, u64, usize)>,
}

impl Carried {
  /// The changed lines of `new` (at the head, `new_text`) and `old` (of the base, `old_text`) by the commit that wrote or
  /// removed them; a blank line tells no one's change
  fn count(&self, (old, old_text): (&Range<u32>, &[&str]), (new, new_text): (&Range<u32>, &[&str]), into: &mut BTreeMap<usize, u64>) {
    let blank = |text: &[&str], line: u32| text.get(line as usize).is_some_and(|line| line.trim().is_empty());
    for line in new.clone().filter(|&line| !blank(new_text, line)) {
      if let Some(commit) = self.writer_of(u64::from(line)) {
        *into.entry(commit).or_default() += 1;
      }
    }
    for line in old.clone().filter(|&line| !blank(old_text, line)) {
      let line = u64::from(line);
      if let Some(&(_, _, commit)) = self.removed.iter().find(|(from, to, _)| (*from..*to).contains(&line)) {
        *into.entry(commit).or_default() += 1;
      }
    }
  }

  /// The commit that wrote line `line` (from 0) at the head, `None` for a line of the base
  fn writer_of(&self, line: u64) -> Option<usize> {
    let mut at = 0;
    for run in &self.runs {
      if line < at + run.len {
        return match run.origin {
          Origin::Commit(commit) => Some(commit),
          Origin::Base(_) => None,
        };
      }
      at += run.len;
    }
    None
  }
}

/// What carrying the lines found across the files: per commit, its changed lines of code on the old side, and the
/// lines of each earlier commit it removed, of code and of any file
#[derive(Default)]
struct Rewrites {
  removed_code: HashMap<usize, u64>,
  /// (the commit that removed, the commit that wrote) -> lines of code
  of: HashMap<(usize, usize), u64>,
  /// (the commit that removed, the commit that wrote) -> lines of any file
  any: HashMap<(usize, usize), u64>,
}

/// Carries the lines of a file through its `changes`, each with the place of its commit, oldest first
fn carry(changes: &[(usize, &FileChange)], code: bool, rewrites: &mut Rewrites) -> Carried {
  let mut runs: VecDeque<Run> = VecDeque::from([Run {
    len: TAIL,
    origin: Origin::Base(0),
  }]);
  let mut removed = Vec::new();
  for &(commit, change) in changes {
    let mut next: Vec<Run> = Vec::new();
    let mut cursor = 0;
    for hunk in &change.hunks {
      let block = block_of(hunk);
      let (old, new) = (u64::from(block.old.start)..u64::from(block.old.end), u64::from(block.new.start)..u64::from(block.new.end));
      take(&mut runs, old.start.saturating_sub(cursor), |run| next.push(run));
      take(&mut runs, old.end - old.start, |run| match run.origin {
        Origin::Base(from) => removed.push((from, from + run.len, commit)),
        Origin::Commit(writer) => {
          *rewrites.any.entry((commit, writer)).or_default() += run.len;
          if code {
            *rewrites.of.entry((commit, writer)).or_default() += run.len;
          }
        }
      });
      if code {
        *rewrites.removed_code.entry(commit).or_default() += old.end - old.start;
      }
      if new.end > new.start {
        next.push(Run {
          len: new.end - new.start,
          origin: Origin::Commit(commit),
        });
      }
      cursor = old.end.max(cursor);
    }
    next.extend(runs);
    runs = next.into();
  }
  Carried { runs: runs.into(), removed }
}

/// Takes `count` lines from the front of `runs`, a run cut where they end
fn take(runs: &mut VecDeque<Run>, mut count: u64, mut each: impl FnMut(Run)) {
  while count > 0 {
    let Some(front) = runs.front_mut() else { return };
    if front.len <= count {
      count -= front.len;
      if let Some(run) = runs.pop_front() {
        each(run);
      }
    } else {
      each(Run { len: count, origin: front.origin });
      front.len -= count;
      if let Origin::Base(from) = front.origin {
        front.origin = Origin::Base(from + count);
      }
      count = 0;
    }
  }
}

/// A part of a unit a step may show: a unit in a file, or a block of a file of no language
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Part {
  /// The place of the unit in the story
  unit: usize,
  slice: usize,
  /// A block of a file of no language, `None` for the whole slice
  block: Option<u32>,
}

/// The steps of the review of `input` (`steps`)
pub(crate) fn review_steps(input: &StepsInput<'_>) -> ReviewSteps {
  let commit_of: HashMap<&str, usize> = input.commits.iter().enumerate().map(|(place, (id, _))| (id.as_str(), place)).collect();
  let mut rewrites = Rewrites::default();
  let carried: Vec<Carried> = input
    .changed
    .iter()
    .map(|file| {
      let changes: Vec<(usize, &FileChange)> = file
        .changes
        .iter()
        .filter_map(|change| {
          let commit = input.diffs.iter().find(|diff| diff.files.iter().any(|it| std::ptr::eq(it, *change)))?;
          Some((*commit_of.get(commit.id.as_str())?, *change))
        })
        .collect();
      carry(&changes, language_of(file.path).is_some(), &mut rewrites)
    })
    .collect();
  let file_of: HashMap<&str, usize> = input.changed.iter().enumerate().map(|(place, file)| (file.path, place)).collect();
  let parts = parts_with_lines(input, &file_of, &carried);
  let story: Vec<bool> = input.commits.iter().map(|(_, message)| tells_a_story(message)).collect();
  let source = if input.commits.len() == 1 {
    Some(ConcernsReason::OneCommit)
  } else if !story.contains(&true) {
    Some(ConcernsReason::NoStory)
  } else {
    None
  };
  let mut steps = match source {
    None => commit_steps(input, &parts, &story, &rewrites),
    Some(_) => concern_steps(input, &parts),
  };
  for step in &mut steps {
    order_stops(input, &file_of, step);
    step.tests = tests_of(input, &file_of, step);
  }
  mark_concerns(input, &mut steps);
  let coverage = coverage(input, &steps);
  ReviewSteps {
    source: source.map_or(StepSource::Commits, |reason| StepSource::Concerns { reason }),
    steps,
    coverage,
  }
}

/// Each part a step may show, with the changed lines of each commit in it: a slice of a mechanical unit too, which
/// follows the step that wrote it, folded at its end
fn parts_with_lines(input: &StepsInput<'_>, file_of: &HashMap<&str, usize>, carried: &[Carried]) -> Vec<(Part, BTreeMap<usize, u64>)> {
  let mut parts = Vec::new();
  for (place, unit) in input.units.iter().enumerate() {
    let per_block = unit.kind == (UnitKind::Other { reason: OtherReason::NoLanguage });
    for (slice_place, slice) in unit.slices.iter().enumerate() {
      let Some(&file) = file_of.get(slice.path.as_str()) else { continue };
      let changed = &input.changed[file];
      let old_text = blob_lines(
        input.texts,
        changed.before.or_else(|| changed.changes.first().and_then(|change| change.old_blob.as_deref())),
      );
      let new_text = blob_lines(input.texts, changed.after);
      let mut whole = BTreeMap::new();
      for block in &slice.blocks {
        let mut lines = BTreeMap::new();
        carried[file].count((&(block.old_start..block.old_end), &old_text), (&(block.new_start..block.new_end), &new_text), &mut lines);
        if per_block {
          parts.push((
            Part {
              unit: place,
              slice: slice_place,
              block: Some(block.index),
            },
            lines,
          ));
        } else {
          for (commit, count) in lines {
            *whole.entry(commit).or_default() += count;
          }
        }
      }
      if per_block && !slice.blocks.is_empty() {
        continue;
      }
      if whole.is_empty() {
        whole = file_lines(input, &carried[file], file, unit, slice);
      }
      parts.push((
        Part {
          unit: place,
          slice: slice_place,
          block: None,
        },
        whole,
      ));
    }
  }
  parts
}

/// The commit of a slice without changed lines of its own: a file added, deleted or without text, by the commit that
/// added, deleted or last changed it; a declaration whose block another unit holds, by the commit that wrote its name
fn file_lines(input: &StepsInput<'_>, carried: &Carried, file: usize, unit: &ReviewUnit, slice: &UnitSlice) -> BTreeMap<usize, u64> {
  let changed = &input.changed[file];
  let place = |change: &FileChange| {
    input
      .diffs
      .iter()
      .position(|diff| diff.files.iter().any(|it| std::ptr::eq(it, change)))
      .and_then(|diff| input.commits.iter().position(|(id, _)| *id == input.diffs[diff].id))
  };
  let writer = match unit.kind {
    UnitKind::Added | UnitKind::Behaviour | UnitKind::Shape { .. } if !slice.whole => unit.line.and_then(|line| carried.writer_of(u64::from(line.saturating_sub(1)))),
    UnitKind::NewFile => changed.changes.iter().find(|change| change.status == 'A').copied().and_then(place),
    UnitKind::DeletedFile | UnitKind::DeletedFiles { .. } => changed.changes.iter().rev().find(|change| change.status == 'D').copied().and_then(place),
    _ => None,
  }
  .or_else(|| changed.changes.last().copied().and_then(place));
  writer.into_iter().map(|commit| (commit, 1)).collect()
}

/// The steps of the commits that tell them (`steps`)
fn commit_steps(input: &StepsInput<'_>, parts: &[(Part, BTreeMap<usize, u64>)], story: &[bool], rewrites: &Rewrites) -> Vec<ReviewStep> {
  // the owner of a part: the commit of a story with the most of its lines, the later of equal ones, which shaped the
  // text at the head
  let owner_of = |lines: &BTreeMap<usize, u64>| -> Option<usize> {
    lines
      .iter()
      .filter(|(commit, count)| story[**commit] && **count > 0)
      .max_by(|a, b| a.1.cmp(b.1).then(a.0.cmp(b.0)))
      .map(|(commit, _)| *commit)
  };
  let owners: Vec<Option<usize>> = parts.iter().map(|(_, lines)| owner_of(lines)).collect();
  let mechanical = |part: &Part| matches!(input.units[part.unit].kind, UnitKind::Mechanical { .. });
  // a mechanical change alone is no step of its own
  let owns = |commit: usize| parts.iter().zip(&owners).any(|((part, _), owner)| *owner == Some(commit) && !mechanical(part));
  // the head of the step of each commit of a story: itself, or the head it follows up
  let mut head_of: Vec<Option<usize>> = vec![None; input.commits.len()];
  for commit in (0..input.commits.len()).filter(|&commit| story[commit]) {
    let removed = rewrites.removed_code.get(&commit).copied().unwrap_or(0);
    let rewritten = (0..commit)
      .filter(|&earlier| head_of[earlier] == Some(earlier))
      .map(|earlier| (earlier, rewrites.of.get(&(commit, earlier)).copied().unwrap_or(0)))
      .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
      .filter(|(_, lines)| removed > 0 && lines * 2 >= removed);
    head_of[commit] = Some(match rewritten {
      Some((earlier, _)) => earlier,
      None if !owns(commit) => {
        // no unit of its own: the step that holds most of its lines, else its own step without a stop
        let mut held: BTreeMap<usize, u64> = BTreeMap::new();
        for ((_, lines), owner) in parts.iter().zip(&owners) {
          if let (Some(owner), Some(count)) = (owner, lines.get(&commit)) {
            *held.entry(*owner).or_default() += count;
          }
        }
        held
          .into_iter()
          .filter(|(owner, _)| head_of[*owner] == Some(*owner))
          .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
          .map_or(commit, |(owner, _)| owner)
      }
      None => commit,
    });
  }
  // a head with no line in any part: a later commit rewrote all it wrote, it follows the step of the one that
  // rewrote the most of it
  for commit in 0..input.commits.len() {
    if head_of[commit] != Some(commit) || parts.iter().any(|(_, lines)| lines.contains_key(&commit)) {
      continue;
    }
    let rewriter = (commit + 1..input.commits.len())
      .filter_map(|later| Some((head_of[later]?, rewrites.any.get(&(later, commit)).copied().unwrap_or(0))))
      .filter(|(head, lines)| *lines > 0 && head_of[*head] == Some(*head) && *head != commit)
      .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)));
    if let Some((head, _)) = rewriter {
      head_of[commit] = Some(head);
    }
  }
  // a commit that follows one up which follows another up now follows the last one: the chains end at a head
  for commit in 0..input.commits.len() {
    let mut head = head_of[commit];
    for _ in 0..input.commits.len() {
      match head {
        Some(next) if head_of[next] != Some(next) => head = head_of[next],
        _ => break,
      }
    }
    head_of[commit] = head;
  }
  let heads: Vec<usize> = (0..input.commits.len()).filter(|&commit| head_of[commit] == Some(commit)).collect();
  let step_of_head: HashMap<usize, usize> = heads.iter().enumerate().map(|(step, &head)| (head, step)).collect();
  let no_story = heads.len();
  let step_of_part: Vec<usize> = owners
    .iter()
    .map(|owner| owner.and_then(|owner| head_of[owner]).and_then(|head| step_of_head.get(&head).copied()).unwrap_or(no_story))
    .collect();
  // a commit of no story belongs to the step that holds most of its lines
  let step_of_commit: Vec<usize> = (0..input.commits.len())
    .map(|commit| {
      if let Some(&step) = head_of[commit].and_then(|head| step_of_head.get(&head)) {
        return step;
      }
      let mut held: BTreeMap<usize, u64> = BTreeMap::new();
      for ((_, lines), &step) in parts.iter().zip(&step_of_part) {
        if let Some(count) = lines.get(&commit) {
          *held.entry(step).or_default() += count;
        }
      }
      held.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))).map_or(no_story, |(step, _)| step)
    })
    .collect();
  let mut steps: Vec<ReviewStep> = heads
    .iter()
    .map(|&head| ReviewStep {
      id: input.commits[head].0.clone(),
      commits: Vec::new(),
      concern: None,
      stops: Vec::new(),
      tests: StepTests::default(),
    })
    .collect();
  steps.push(ReviewStep {
    id: "no-story".to_string(),
    commits: Vec::new(),
    concern: None,
    stops: Vec::new(),
    tests: StepTests::default(),
  });
  for (commit, &step) in step_of_commit.iter().enumerate() {
    let role = match head_of[commit] {
      Some(head) if head == commit => CommitRole::Starts,
      Some(_) => CommitRole::FollowsUp,
      None => CommitRole::NoStory,
    };
    steps[step].commits.push(step_commit(input, commit, role));
  }
  // the commit that starts a step first, then those that follow it up, then those of no story, each in their order
  for step in &mut steps {
    step.commits.sort_by_key(|commit| match commit.role {
      CommitRole::Starts => 0,
      CommitRole::FollowsUp => 1,
      CommitRole::NoStory => 2,
    });
  }
  for ((part, lines), &step) in parts.iter().zip(&step_of_part) {
    let also: BTreeSet<u32> = lines
      .keys()
      .map(|&commit| step_of_commit[commit])
      .filter(|&other| other != step)
      .filter_map(|other| u32::try_from(other).ok())
      .collect();
    steps[step].stops.push(stop_of(input, part, also.into_iter().collect()));
  }
  if steps.last().is_some_and(|last| last.stops.is_empty() && last.commits.is_empty()) {
    steps.pop();
  }
  steps
}

/// A commit of a step, its message cut into its subject and its body
fn step_commit(input: &StepsInput<'_>, commit: usize, role: CommitRole) -> StepCommit {
  let (id, message) = &input.commits[commit];
  let (subject, body) = split_message(message);
  StepCommit {
    id: id.clone(),
    subject: subject.to_string(),
    body: if role == CommitRole::NoStory { String::new() } else { body.to_string() },
    role,
  }
}

/// The stop of a part
fn stop_of(input: &StepsInput<'_>, part: &Part, also: Vec<u32>) -> StepStop {
  let unit = &input.units[part.unit];
  StepStop {
    unit: unit.id.clone(),
    path: unit.slices[part.slice].path.clone(),
    blocks: part.block.map(|block| vec![block]),
    uses: None,
    also,
    concern: None,
  }
}

/// The steps of the concerns (`steps`): one per concern, the rest last, its mechanical parts too; the commits of each
/// by its lines
fn concern_steps(input: &StepsInput<'_>, parts: &[(Part, BTreeMap<usize, u64>)]) -> Vec<ReviewStep> {
  let lines_of: HashMap<(&str, &str), &BTreeMap<usize, u64>> = parts
    .iter()
    .map(|(part, lines)| {
      let unit = &input.units[part.unit];
      ((unit.id.as_str(), unit.slices[part.slice].path.as_str()), lines)
    })
    .collect();
  input
    .concerns
    .iter()
    .map(|concern| {
      let mut held: BTreeMap<usize, u64> = BTreeMap::new();
      for part in &concern.parts {
        for (&commit, &count) in lines_of.get(&(part.unit.as_str(), part.path.as_str())).copied().into_iter().flatten() {
          *held.entry(commit).or_default() += count;
        }
      }
      let mut commits: Vec<(usize, u64)> = held.into_iter().collect();
      commits.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
      ReviewStep {
        id: concern.id.clone(),
        commits: commits
          .into_iter()
          .map(|(commit, _)| {
            let role = if tells_a_story(&input.commits[commit].1) {
              CommitRole::Starts
            } else {
              CommitRole::NoStory
            };
            step_commit(input, commit, role)
          })
          .collect(),
        concern: Some(concern.id.clone()),
        stops: concern
          .parts
          .iter()
          .map(|part| StepStop {
            unit: part.unit.clone(),
            path: part.path.clone(),
            blocks: part.blocks.clone(),
            uses: None,
            also: Vec::new(),
            concern: None,
          })
          .collect(),
        tests: StepTests::default(),
      }
    })
    .collect()
}

/// The changed lines of a stop: removed, added
fn stop_lines<'a>(input: &StepsInput<'a>, file_of: &HashMap<&str, usize>, unit: &ReviewUnit, stop: &StepStop) -> (Vec<&'a str>, Vec<&'a str>) {
  let (mut removed, mut added) = (Vec::new(), Vec::new());
  let Some(&file) = file_of.get(stop.path.as_str()) else { return (removed, added) };
  let changed = &input.changed[file];
  let old = blob_lines(input.texts, changed.before.or_else(|| changed.changes.last().and_then(|change| change.old_blob.as_deref())));
  let new = blob_lines(input.texts, changed.after);
  for slice in unit.slices.iter().filter(|slice| slice.path == stop.path) {
    if slice.blocks.is_empty() {
      // a file added or deleted whole
      if changed.before.is_none() {
        added.extend(&new);
      }
      if changed.after.is_none() {
        removed.extend(&old);
      }
    }
    for block in slice.blocks.iter().filter(|block| stop.blocks.as_ref().is_none_or(|blocks| blocks.contains(&block.index))) {
      removed.extend(lines_in(&old, block.old_start..block.old_end));
      added.extend(lines_in(&new, block.new_start..block.new_end));
    }
  }
  (removed, added)
}

/// The stops of a step in the order of the story, a part of a no-language file by its block; a declaration the step
/// adds or changes the form of, then its uses in the step (`StepStop::uses`), unless more than `MAX_USES` stops use it
fn order_stops(input: &StepsInput<'_>, file_of: &HashMap<&str, usize>, step: &mut ReviewStep) {
  let place_of: HashMap<&str, usize> = input.units.iter().enumerate().map(|(place, unit)| (unit.id.as_str(), place)).collect();
  let unit_of = |stop: &StepStop| place_of.get(stop.unit.as_str()).map(|&place| &input.units[place]);
  let mechanical = |stop: &StepStop| unit_of(stop).is_some_and(|unit| matches!(unit.kind, UnitKind::Mechanical { .. }));
  // the story's order, the mechanical parts last
  step.stops.sort_by_key(|stop| {
    (
      mechanical(stop),
      place_of.get(stop.unit.as_str()).copied().unwrap_or(usize::MAX),
      stop.blocks.as_ref().and_then(|blocks| blocks.first().copied()),
    )
  });
  let linked: HashSet<(&str, &str, &str)> = input
    .links
    .iter()
    .flat_map(|link| link.symbols.iter().map(move |symbol| (link.from.as_str(), link.to.as_str(), symbol.name.as_str())))
    .collect();
  let lines: Vec<Vec<&str>> = step
    .stops
    .iter()
    .map(|stop| {
      unit_of(stop).map_or_else(Vec::new, |unit| {
        let (removed, added) = stop_lines(input, file_of, unit, stop);
        removed.into_iter().chain(added).collect()
      })
    })
    .collect();
  let stops = std::mem::take(&mut step.stops);
  let mut placed = vec![false; stops.len()];
  let mut ordered: Vec<StepStop> = Vec::with_capacity(stops.len());
  for at in 0..stops.len() {
    if placed[at] {
      continue;
    }
    placed[at] = true;
    ordered.push(stops[at].clone());
    let Some(unit) = unit_of(&stops[at]) else { continue };
    let decision = matches!(unit.kind, UnitKind::Shape { .. } | UnitKind::Added) && !mechanical(&stops[at]);
    let Some(name) = unit.name.as_deref().filter(|name| decision && name.len() >= MIN_NAME) else {
      continue;
    };
    let uses: Vec<usize> = (0..stops.len())
      .filter(|&other| {
        !placed[other]
          && stops[other].unit != stops[at].unit
          && !mechanical(&stops[other])
          && (stops[other].path == stops[at].path || linked.contains(&(stops[other].path.as_str(), stops[at].path.as_str(), name)))
          && lines[other].iter().any(|line| names_word(line, name))
      })
      .collect();
    if uses.len() > MAX_USES {
      continue;
    }
    for other in uses {
      placed[other] = true;
      let mut stop = stops[other].clone();
      stop.uses = Some(name.to_string());
      ordered.push(stop);
    }
  }
  step.stops = ordered;
}

/// The lines of the declaration of `unit` after the commits, from 1, both included, when it is a function a test
/// framework runs, by its summary
fn test_span(input: &StepsInput<'_>, file_of: &HashMap<&str, usize>, unit: &ReviewUnit) -> Option<(u32, u32)> {
  let (Some(symbols), Some(name), Some(line), [slice]) = (input.input, unit.name.as_deref(), unit.line, unit.slices.as_slice()) else {
    return None;
  };
  let blob = file_of.get(slice.path.as_str()).and_then(|&file| input.changed[file].after)?;
  let summary = symbols.summaries.get(blob)?;
  summary
    .declarations
    .iter()
    .find(|declaration| declaration.test && declaration.name_line == line && summary.name(declaration.name) == name)
    .map(|declaration| (declaration.start_line, declaration.end_line))
}

/// The lines a test adds or changes: the lines of the blocks of its file, after the commits, within its declaration
/// (a block may hold two tests, and the unit of one holds it), without the names in backticks: the words of a Kotlin
/// test's name are no use of a declaration
fn test_lines(input: &StepsInput<'_>, file_of: &HashMap<&str, usize>, path: &str, (start, end): (u32, u32)) -> Vec<String> {
  let Some(&file) = file_of.get(path) else { return Vec::new() };
  let new = blob_lines(input.texts, input.changed[file].after);
  let span = start.saturating_sub(1)..end;
  let lines = match &input.pairs[file] {
    PairBlocks::Added => lines_in(&new, span),
    PairBlocks::Blocks(blocks) => blocks
      .iter()
      .flat_map(|block| lines_in(&new, block.new.start.max(span.start)..block.new.end.min(span.end)))
      .collect(),
    _ => Vec::new(),
  };
  lines.into_iter().map(|line| line.split('`').step_by(2).collect::<Vec<_>>().join(" ")).collect()
}

/// The name a test reads as: a Kotlin name in backticks without them, the words of `snake_case`, of `testCamelCase`
/// without its `test`
pub(crate) fn test_name(name: &str) -> String {
  if let Some(sentence) = name.strip_prefix('`').and_then(|rest| rest.strip_suffix('`')) {
    return sentence.to_string();
  }
  if name.contains('_') {
    return name.trim_start_matches("test_").split('_').filter(|word| !word.is_empty()).collect::<Vec<_>>().join(" ");
  }
  let camel = name.strip_prefix("test").filter(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase())).unwrap_or(name);
  let mut words = String::new();
  for (place, c) in camel.chars().enumerate() {
    if c.is_ascii_uppercase() && place > 0 {
      words.push(' ');
    }
    words.push(c.to_ascii_lowercase());
  }
  words
}

/// The tests of the range that name each declaration of the code of `step` (`StepTests`)
fn tests_of(input: &StepsInput<'_>, file_of: &HashMap<&str, usize>, step: &ReviewStep) -> StepTests {
  let tests: Vec<(&ReviewUnit, Vec<String>, &str)> = input
    .units
    .iter()
    .filter(|unit| matches!(unit.kind, UnitKind::Added | UnitKind::Behaviour | UnitKind::Shape { .. }))
    .filter_map(|unit| Some((unit, test_span(input, file_of, unit)?)))
    .map(|(unit, span)| {
      let path = unit.slices[0].path.as_str();
      let added = test_lines(input, file_of, path, span);
      let text = file_of
        .get(path)
        .and_then(|&file| input.changed[file].after)
        .and_then(|blob| input.texts.get(blob))
        .map_or("", String::as_str);
      (unit, added, text)
    })
    .collect();
  let place_of: HashMap<&str, &ReviewUnit> = input.units.iter().map(|unit| (unit.id.as_str(), unit)).collect();
  let mut seen: HashSet<&str> = HashSet::new();
  let mut named = Vec::new();
  let mut unnamed = Vec::new();
  for stop in &step.stops {
    let Some(unit) = place_of.get(stop.unit.as_str()) else { continue };
    let code = matches!(unit.kind, UnitKind::Added | UnitKind::Behaviour | UnitKind::Shape { .. })
      && language_of(&stop.path).is_some()
      && test_subject(&stop.path).is_none()
      && test_span(input, file_of, unit).is_none();
    let Some(name) = unit.name.as_deref().filter(|name| code && name.len() >= MIN_NAME) else {
      continue;
    };
    if !seen.insert(unit.id.as_str()) {
      continue;
    }
    // a member: its owner, a type, named in the file of the test too
    let owner = unit
      .key
      .as_deref()
      .and_then(|key| key.rsplit('.').nth(1))
      .filter(|owner| owner.starts_with(|c: char| c.is_ascii_uppercase()));
    let naming: Vec<TestRef> = tests
      .iter()
      .filter(|(test, added, text)| test.id != unit.id && added.iter().any(|line| names_word(line, name)) && owner.is_none_or(|owner| names_word(text, owner)))
      .map(|(test, ..)| TestRef {
        unit: test.id.clone(),
        path: test.slices[0].path.clone(),
        name: test_name(test.name.as_deref().unwrap_or_default()),
      })
      .collect();
    let shown = owner.map_or_else(|| name.to_string(), |owner| format!("{owner}.{name}"));
    if naming.is_empty() {
      unnamed.push((name.to_string(), shown));
    } else {
      named.push((
        shown,
        NamedDeclaration {
          name: name.to_string(),
          tests: naming,
        },
      ));
    }
  }
  // a name two declarations share is led by its owner
  let mut count: HashMap<String, usize> = HashMap::new();
  for name in unnamed.iter().map(|(name, _)| name).chain(named.iter().map(|(_, named)| &named.name)) {
    *count.entry(name.clone()).or_default() += 1;
  }
  let qualified = |name: String, shown: String| if count.get(&name).copied().unwrap_or(0) > 1 { shown } else { name };
  let scenarios = input
    .contract
    .iter()
    .filter(|entry| {
      step
        .stops
        .iter()
        .any(|stop| stop.unit == entry.unit && stop.path == entry.path && entry.block.is_none_or(|block| stop.blocks.as_ref().is_none_or(|blocks| blocks.contains(&block))))
    })
    .flat_map(|entry| entry.proven_by.iter().cloned())
    .collect::<BTreeSet<String>>()
    .into_iter()
    .collect();
  StepTests {
    named: named
      .into_iter()
      .map(|(shown, mut named)| {
        named.name = qualified(named.name.clone(), shown);
        named
      })
      .collect(),
    unnamed: unnamed.into_iter().map(|(name, shown)| qualified(name, shown)).collect(),
    scenarios,
  }
}

/// Marks each stop of a step with the concern of its unit when the step holds parts of two concerns or more that
/// `uses` or a group joined
fn mark_concerns(input: &StepsInput<'_>, steps: &mut [ReviewStep]) {
  let joined: HashSet<&str> = input
    .concerns
    .iter()
    .filter(|concern| concern.name != ConcernName::Rest && concern.evidence.iter().any(|evidence| matches!(evidence.kind, EvidenceKind::Uses | EvidenceKind::Group)))
    .map(|concern| concern.id.as_str())
    .collect();
  let concern_of: HashMap<(&str, &str), &str> = input
    .concerns
    .iter()
    .flat_map(|concern| concern.parts.iter().map(move |part| ((part.unit.as_str(), part.path.as_str()), concern.id.as_str())))
    .collect();
  for step in steps.iter_mut().filter(|step| step.concern.is_none()) {
    let found: BTreeSet<&str> = step
      .stops
      .iter()
      .filter_map(|stop| concern_of.get(&(stop.unit.as_str(), stop.path.as_str())).copied())
      .filter(|concern| joined.contains(concern))
      .collect();
    if found.len() < 2 {
      continue;
    }
    for stop in &mut step.stops {
      stop.concern = concern_of.get(&(stop.unit.as_str(), stop.path.as_str())).map(|concern| (*concern).to_string());
    }
  }
}

/// Whether the steps show every block of the range once (`StepCoverage`)
fn coverage(input: &StepsInput<'_>, steps: &[ReviewStep]) -> StepCoverage {
  let unit_of: HashMap<&str, &ReviewUnit> = input.units.iter().map(|unit| (unit.id.as_str(), unit)).collect();
  // the blocks of each file of the pair, none for a file of no blocks
  let count_of: HashMap<&str, u32> = input
    .changed
    .iter()
    .zip(input.pairs)
    .map(|(file, pair)| {
      (
        file.path,
        if let PairBlocks::Blocks(blocks) = pair {
          u32::try_from(blocks.len()).unwrap_or(u32::MAX)
        } else {
          0
        },
      )
    })
    .collect();
  // (path, block; u32::MAX for a file without blocks) -> the stops that show it
  let mut shown: HashMap<(&str, u32), u32> = HashMap::new();
  let held = |unit: &ReviewUnit, path: &str, only: Option<&Vec<u32>>| -> Vec<u32> {
    unit
      .slices
      .iter()
      .filter(|slice| slice.path == path)
      .flat_map(|slice| match (slice.blocks.is_empty(), count_of.get(path).copied().unwrap_or(0)) {
        // a slice taken whole that lists no block holds every block of its file
        (true, 0) => vec![u32::MAX],
        (true, count) if slice.whole => (0..count).collect(),
        (true, _) => Vec::new(),
        (false, _) => slice
          .blocks
          .iter()
          .map(|block| block.index)
          .filter(|index| only.is_none_or(|only| only.contains(index)))
          .collect(),
      })
      .collect()
  };
  for stop in steps.iter().flat_map(|step| &step.stops) {
    let Some(unit) = unit_of.get(stop.unit.as_str()) else { continue };
    for block in held(unit, &stop.path, stop.blocks.as_ref()) {
      *shown.entry((stop.path.as_str(), block)).or_default() += 1;
    }
  }
  let mut folded: HashSet<(&str, u32)> = HashSet::new();
  for unit in input.units.iter().filter(|unit| matches!(unit.kind, UnitKind::Mechanical { .. })) {
    for slice in &unit.slices {
      for block in held(unit, &slice.path, None) {
        folded.insert((slice.path.as_str(), block));
      }
    }
  }
  let mut coverage = StepCoverage::default();
  for (file, pair) in input.changed.iter().zip(input.pairs) {
    let blocks: Vec<u32> = match pair {
      // changed and changed back: no change of the pair
      PairBlocks::Blocks(blocks) => (0..u32::try_from(blocks.len()).unwrap_or(u32::MAX)).collect(),
      _ => vec![u32::MAX],
    };
    for block in blocks {
      coverage.blocks += 1;
      // a file of no blocks (its line ends, its type, no text) is shown by the stops of any slice of it
      let count = if block == u32::MAX {
        shown.iter().filter(|((path, _), _)| *path == file.path).map(|(_, count)| *count).max().unwrap_or(0)
      } else {
        shown.get(&(file.path, block)).copied().unwrap_or(0)
      };
      let folded_file = block == u32::MAX && folded.iter().any(|(path, _)| *path == file.path);
      match count {
        0 if folded_file => coverage.mechanical += 1,
        0 if folded.contains(&(file.path, block)) => coverage.mechanical += 1,
        0 => coverage.missing += 1,
        1 => coverage.in_stops += 1,
        _ => coverage.twice += 1,
      }
    }
  }
  coverage
}
