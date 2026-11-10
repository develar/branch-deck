//! The steps of a review (openspec/specs/branch/review-tour/spec.md): the story the review window tells first, each step a part
//! of the change with the words of its author, the diffs that carry it and the tests that name its code. No git
//! process: the plan read every commit of the range and its hunks already.
//!
//! - **Where a line comes from**: the lines of each file are carried through the hunks of the commits that change it,
//!   oldest first (`carry`), so each line at the head names the commit that wrote it last, and each line of the base
//!   the commit that removed it. A block of the range belongs to the commit of most of its lines.
//! - **Steps of the commits**, when the commits tell them: a commit with a body (`tells_a_story`) starts a step and owns
//!   the units most of whose lines it wrote; only what lands is shown, the units of the range. A commit whose changed
//!   code lines are mostly lines an earlier step head wrote follows that step up (a round of it) only when it owns no
//!   non-mechanical unit that step never changed. Its body can still tell a story of its own
//!   (`tells_its_own_story`: a lead and a labelled part, or more than `OWN_STORY_LINES` lines, its verification sections
//!   counting for nothing), and so does a commit that owns no unit; a commit without a body owns nothing, its lines show where they landed, and the units only such
//!   commits wrote are the step "Changes without a story". A unit of a file of no language is cut by its blocks.
//! - **Steps of the concerns** when the commits tell no story: no body, or one commit, whose message is the brief. A
//!   long branch is told by its commits too: measured on idea-1, its concerns were no shorter a list, and they lost
//!   the words of the author (openspec/specs/branch/review-tour/spec.md, "Measurements").
//! - **A step whose lines a later commit rewrote whole** shows nothing of its own: it follows the step of that commit,
//!   and names the later commit that took out the most of what it wrote (`taken_out_by`).
//! - **The order** of a step is that of the story, but a declaration the step adds or changes the form of comes first
//!   and the uses of it in the step right after it (`uses`); a mechanical part follows the step its concern is in.
//! - **The tests that reach it** (`add_tests`, after the plan): per declaration of the code of a step (no test code: a
//!   test, a fixture of its class or module, `ReviewUnit::test`; of a new file each declaration at its top), the tests of
//!   the change that accio finds among its callers at any depth by calls a type or an import ties (`accio tests --at …
//!   --tests <file>…`, every declaration of the plan and every test file of the change in one process). The step lists
//!   them by their file (a test of a new file is one too), each with what its way goes through, and a declaration none
//!   reaches only when accio walked every way (`complete`): an answer it cut names nothing, unknown, never "no test".
//! - **A declaration without a block of its own** (of one side, whose lines a block of another unit holds) is no stop:
//!   it joins the stop that shows the block of the line of its name (`StepStop::joined`, with the step it is of), and a
//!   step whose unit another step's stop shows names it among its facts (`ReviewStep::shown_elsewhere`) and lists its
//!   file, whose main row is elsewhere; a step left with no stop tells its story and shows no card. One whose line no
//!   stop shows, or only a group's, stays a stop of its own; one of two stops (a part of two concerns) joins once.
//! - **Coverage**: every block of the range in one stop, else in a mechanical unit (`StepCoverage`).

use crate::api::ApiChange;
use crate::brief::ContractEntry;
use crate::concerns::{Concern, names_word};
use crate::fan_in::ask_index;
use crate::files::{ChangedFile, blob_lines, lines_in, test_subject};
use crate::groups::{GroupKind, StepGroup, cut_groups};
use crate::interdiff::MessageChange;
use crate::links::FileLink;
use crate::message_steps::message_steps;
use crate::modules::Modules;
use crate::pair::{PairBlocks, block_of};
use crate::story::{StoryCommit, StoryRef, add_step_facts, add_story_marks, add_story_titles, story_commits};
use crate::units::{OtherReason, ReviewUnit, UnitBlock, UnitKind, UnitSlice};
use anyhow::Result;
use code_symbols::language::language_of;
use code_symbols::summary::{DeclKind, FileSummary};
use domain_types::commit_body::{CommitBodyBlock, LabelKind};
use file_diffs::accio::{DeclarationAt, ReachingTest, SymbolIndex, TestClass, TestReach};
use object_cache::commit_diff::FileChange;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::ops::Range;
use sync_utils::commit_body::{commit_body, story_lines};
use sync_utils::commit_subject::stripped_subject;
use sync_utils::issue_pattern::{leading_issue_keys, parse_semantic_prefix};
use trial_check::diff_scan::CommitDiff;
use trial_check::symbol_scan::SymbolInput;

/// A declaration more stops of a step use than this is a hub: its uses keep the order of the story
const MAX_USES: usize = 6;
/// The shortest name of a declaration whose uses follow it
const MIN_NAME: usize = 3;
/// The fewest words of a body that tells a step
const MIN_BODY_WORDS: usize = 3;
/// The lines past which a body tells a story of its own (`tells_its_own_story`)
const OWN_STORY_LINES: u32 = 5;
/// The lines past the last one of a file a hunk names: the base goes on unchanged
const TAIL: u64 = u64::MAX / 4;

/// The steps of a review
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ReviewSteps {
  pub source: StepSource,
  /// Whether the story tells the range in steps: not when the steps are one concern, which covers the whole range and
  /// would only repeat the header; the range then reads as without a plan: its whole body, no list of steps, no
  /// opening, no place in the step bar
  pub told: bool,
  /// In the order of the story: the commits that start them, oldest first; "Changes without a story" last
  pub steps: Vec<ReviewStep>,
  pub coverage: StepCoverage,
  /// Every commit of the range whose body has a block, once, oldest first: its body by the grammar of a body, its code
  /// names resolved to the units and the files (`story`)
  pub commits: Vec<StoryCommit>,
  /// The tests of the range: the lines of each at the head and its unit, for `add_tests`
  #[serde(skip)]
  pub range_tests: Vec<RangeTest>,
}

/// A test the range adds or changes: its path at the head, its lines from 1 (both included) and its unit
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeTest {
  pub path: String,
  pub lines: (u32, u32),
  /// `ReviewUnit::id`
  pub unit: String,
}

/// A declaration of the code of a step, as `add_tests` asks about it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepCode {
  /// Its name, led by its owner when another declaration of the step has the same name
  pub name: String,
  /// The line of its name at the head
  pub at: DeclarationAt,
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
  /// The range is one commit whose body tells its parts by the code they name: each part is a step, with the concerns
  /// it names (`message_steps`); a concern no part names a step of its own after them
  Message,
}

/// What titles a step (`ReviewStep::title`)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum TitleSource {
  /// The subject of the commit that starts it
  Commit,
  /// The label or the heading of the section of the body it tells, which its opening names once, as its title
  Label,
  /// The first sentence of the paragraph it tells, its backticks aside: its opening tells the paragraph whole, so it
  /// shows no title of its own
  Sentence,
  /// The first code name of the paragraph it tells that resolves: an identifier, set as code
  Name,
  /// No title: the page names it by the evidence of its concern
  #[default]
  Evidence,
}

/// A title as the review shows it (`ReviewStep::title`, `StepCommit::title`, `StoryCommit::title`): of a commit its
/// conventional prefix apart (`cleanup platform:`, `fix(vm):`, with its colon), which every place that names it sets
/// in the code font and dimmed, as the main window sets a subject, and the rest; a label, a sentence or a name has none
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ReviewTitle {
  pub prefix: Option<String>,
  pub text: String,
}

impl ReviewTitle {
  /// A title of no prefix: a label, a sentence, a name
  pub(crate) fn plain(text: impl Into<String>) -> Self {
    Self { prefix: None, text: text.into() }
  }

  /// The conventional prefix of `title` apart (`sync_utils::issue_pattern::parse_semantic_prefix`), when text follows it
  fn of_subject(title: &str) -> Self {
    match parse_semantic_prefix(title).filter(|parsed| !parsed.rest.is_empty()) {
      Some(parsed) => Self {
        prefix: Some(title[..title.len() - parsed.rest.len()].trim_end().to_string()),
        text: parsed.rest.to_string(),
      },
      None => Self::plain(title),
    }
  }
}

/// The title as one line, its prefix before the rest: what a screen reader and `brd` read
impl std::fmt::Display for ReviewTitle {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match &self.prefix {
      Some(prefix) => write!(f, "{prefix} {}", self.text),
      None => f.write_str(&self.text),
    }
  }
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
  /// The files of its stops, each once, in the order of its stops, then the files of its units another step's stop
  /// shows (`shown_elsewhere`), whose main row is that step's
  pub paths: Vec<String>,
  /// Its files whose main row, in the tree of the review, is another step's: of the steps of a file, the one whose
  /// stops change the most lines of it holds its row, the first of equal ones
  pub elsewhere: Vec<String>,
  pub tests: StepTests,
  /// The other steps whose code the tests of its units reach (`add_tests`), by their place in `ReviewSteps::steps`:
  /// "Proves step 2"
  pub proves: Vec<u32>,
  /// The public declarations it adds or opens wider, by the `key` of their entry of `ReviewPlan::api` (kind `added`, a
  /// `widened` one too), each in the first step whose stops show it; one a stop of another step shows beside its own
  /// (`StepStop::joined`) in the step that wrote it
  pub new_api: Vec<String>,
  /// Of a large step: its stops cut into the parts it reads as (`groups::cut_groups`), each a run of `stops`: the
  /// contract, a group per module or per folder, the mechanical stops; empty for a step read as one list
  pub groups: Vec<StepGroup>,
  /// The new uses of a module by another that it adds: the links between their files the commits add
  /// (`new_between_modules`), each in the step whose stop shows its first use, by the pair of modules, in the order of
  /// their first link
  pub module_edges: Vec<ModuleEdge>,
  /// Its title wherever the review names it: of a step of the commits, the title of the commit that starts it
  /// (`commit_title`); of a step of the concerns of one commit, the label of the first labelled section of its body the
  /// step tells (`story::add_story_titles`), the name of its concern staying the evidence that joined it. `None` for the
  /// changes without a story and a concern no section names, which the page names by its evidence
  pub title: Option<ReviewTitle>,
  /// What gives it its title
  pub title_source: TitleSource,
  /// Of a step of the commits none of whose lines lands: the later commit that took out the most of what it wrote, by
  /// its id; `None` for a step that shows something
  pub taken_out_by: Option<String>,
  /// Its units another step's stop shows (`StepStop::joined` there), each with that step: no stop of its own shows them,
  /// so its facts name them ("Removes hooks, shown in step 2")
  pub shown_elsewhere: Vec<ShownElsewhere>,
  /// The declarations of its code, which `add_tests` asks the tests of
  #[serde(skip)]
  pub code: Vec<StepCode>,
}

impl ReviewStep {
  /// A step of `id` that shows nothing yet
  pub(crate) fn new(id: String) -> Self {
    Self {
      id,
      commits: Vec::new(),
      concern: None,
      stops: Vec::new(),
      paths: Vec::new(),
      elsewhere: Vec::new(),
      tests: StepTests::default(),
      proves: Vec::new(),
      new_api: Vec::new(),
      groups: Vec::new(),
      module_edges: Vec::new(),
      title: None,
      title_source: TitleSource::Evidence,
      taken_out_by: None,
      shown_elsewhere: Vec::new(),
      code: Vec::new(),
    }
  }
}

/// A module of the review that another newly uses in a step (`ReviewStep::module_edges`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ModuleEdge {
  /// The module that uses and the one it uses, by their place in `ReviewPlan::modules`
  pub from: u32,
  pub to: u32,
  /// The links of the step between their files, by their place in `ReviewPlan::links`
  pub links: Vec<u32>,
}

/// A unit of a step that a stop of another step shows (`ReviewStep::shown_elsewhere`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ShownElsewhere {
  /// `ReviewUnit::id`
  pub unit: String,
  /// The step whose stop shows it, by its place in `ReviewSteps::steps`
  pub step: u32,
}

/// A commit of a step; its body is its `StoryCommit` of `ReviewSteps::commits`
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StepCommit {
  pub id: String,
  /// Its title (`commit_title`): its subject without the `(prefix)` that groups it and the issue key the brief names,
  /// in the case its author wrote it
  pub title: ReviewTitle,
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
  /// The units it shows beside its own: each declaration without a block of its own whose name is on a line of a block
  /// it shows, with the step it is of, another step's too (its heading says so where the stop shows it)
  pub joined: Vec<JoinedUnit>,
  /// The lines of the told bodies that name it (`story::add_story_marks`): the header of its section marks it
  pub marks: Vec<StoryRef>,
}

impl StepStop {
  /// The stop of `unit` in `path` that shows `blocks` (`None`: the whole slice), alone
  pub(crate) const fn new(unit: String, path: String, blocks: Option<Vec<u32>>) -> Self {
    Self {
      unit,
      path,
      blocks,
      uses: None,
      also: Vec::new(),
      joined: Vec::new(),
      marks: Vec::new(),
    }
  }
}

/// A declaration without a block of its own that a stop shows (`StepStop::joined`): of one side, its lines in a block
/// of the stop's unit
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct JoinedUnit {
  /// `ReviewUnit::id`
  pub unit: String,
  /// The block of the stop's slice that holds the line of its name, by its index
  pub block: u32,
  /// The step it is of, by its place in `ReviewSteps::steps`: the step that wrote it, which may be another than the
  /// stop's
  pub step: u32,
}

/// The tests that reach the code of a step
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StepTests {
  /// accio answered which tests reach its code (`add_tests`); without the answer the rest is empty, and the plan's
  /// `index_note` says why
  pub answered: bool,
  /// The tests of the change (`StepTest::unit`) that reach its code, by their file, each file and each test once, in the
  /// order of its code
  pub proof: Vec<ProofFile>,
  /// How many of its declarations a test of the change reaches
  pub reached: u32,
  /// Its declarations of a parsed language whose body or form changed, or that it adds, that no test of the change
  /// reaches, accio having walked every way from them (`complete`); none of a test file, whose helpers a test runs. A
  /// name two of them share is led by its owner
  pub unreached: Vec<UnreachedDeclaration>,
  /// The tests the `[@test]` lines of the rules of a spec it adds or changes name, by the path they give: a scenario
  /// that names no declaration
  pub scenarios: Vec<String>,
}

/// A file of tests of the change that reach the code of a step, with those tests
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProofFile {
  pub path: String,
  /// In the order accio answered them
  pub tests: Vec<StepTest>,
}

/// A declaration of a step no test reaches
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UnreachedDeclaration {
  pub name: String,
  /// The file of its name at the head
  pub path: String,
  /// The line of its name at the head, from 1
  pub line: u32,
}

/// A test that reaches the code of a step
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StepTest {
  /// `ReviewUnit::id` of a test the range adds or changes (of a new file, the unit of the file); `None` for one the
  /// change did not touch
  pub unit: Option<String>,
  pub path: String,
  /// The line of its name at the head, from 1
  pub line: u32,
  /// The name the test reads as
  pub name: String,
  /// The fewest calls between the test and the code of the step: 1 when the test calls it
  pub depth: u32,
  /// What its nearest way to the code of the step goes through
  pub way: TestWay,
  /// The declarations of the step it reaches, by their names as the step leads them, in the order of its code
  pub reaches: Vec<String>,
}

/// What the way from a test to the code of a step goes through, nearest first (accio's `class`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum TestWay {
  /// The test calls it
  Calls,
  /// Only the code of tests is on the way: a fixture, a helper of the test
  TestCode,
  /// Product code is on the way: an end-to-end test
  EndToEnd,
}

impl From<TestClass> for TestWay {
  fn from(class: TestClass) -> Self {
    match class {
      TestClass::Direct => Self::Calls,
      TestClass::TestCode => Self::TestCode,
      TestClass::Product => Self::EndToEnd,
    }
  }
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
  /// How the message of a commit differs from its reviewed version, by its id (`StoryCommit::change`)
  pub message_changes: &'a HashMap<String, MessageChange>,
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
  /// The API of the plan, before its usages: the new API of each step (`story::add_step_facts`)
  pub api: &'a [ApiChange],
  /// The issue key the brief names (`ReviewBrief::issue`): the title of a commit leaves it out (`commit_title`)
  pub issue: Option<&'a str>,
  /// The module of each changed file, for the new uses between modules of each step (`ReviewStep::module_edges`)
  pub modules: &'a Modules,
}

/// Whether a commit message tells a step: a body of `MIN_BODY_WORDS` words or more after its subject, its trailers
/// (`Co-Authored-By: …`) left out, and no `fixup!`, `squash!` or `amend!` subject
pub(crate) fn tells_a_story(message: &str) -> bool {
  let (subject, body) = split_message(message);
  !["fixup!", "squash!", "amend!"].iter().any(|mark| subject.starts_with(mark)) && body_words(body) >= MIN_BODY_WORDS
}

/// Whether a commit tells a story of its own, never a round of another step however much of that step's code it
/// rewrites: its body opens with a lead and has a labelled part, or its story reads in more than `OWN_STORY_LINES`
/// lines. A verification section (`Tests:`, `Evidence:`) and a Look closely are no part of a story here, and the lines
/// of a verification section count for nothing (`story_lines`): every round ends with how it was checked
fn tells_its_own_story(message: &str) -> bool {
  let body = commit_body(split_message(message).1);
  let labelled = body
    .blocks
    .iter()
    .any(|block| matches!(block, CommitBodyBlock::Label { kind: LabelKind::Plain, .. } | CommitBodyBlock::Heading { .. }));
  (body.lead && labelled) || story_lines(&body) > OWN_STORY_LINES
}

/// The words of `body` with a letter, the trailers left out
fn body_words(body: &str) -> usize {
  let words = |text: &str| text.split_whitespace().filter(|word| word.chars().any(char::is_alphabetic)).count();
  commit_body(body)
    .blocks
    .iter()
    .map(|block| match block {
      CommitBodyBlock::Paragraph { lines } => lines.iter().map(|line| words(line)).sum(),
      CommitBodyBlock::List { items, .. } => items.iter().map(|item| words(item)).sum(),
      CommitBodyBlock::Table { header, rows, .. } => header.iter().chain(rows.iter().flatten()).map(|cell| words(cell)).sum(),
      CommitBodyBlock::Preformatted { text } | CommitBodyBlock::Heading { text, .. } | CommitBodyBlock::Label { text, .. } => words(text),
      CommitBodyBlock::Rule => 0,
    })
    .sum()
}

/// The title of a commit wherever the review names it (the row of its step, the opening, the contents, a card): its
/// subject without the `(prefix)` that groups it and, before what is left, the issue key the brief names already, with
/// a `:`, `,` or dash after it, also from among the keys the subject starts with (`IJAI-1184 IJAI-5640 Air …` keeps
/// `IJAI-1184`); in the case its author wrote it, never capitalised. A subject of the key alone keeps it. Its
/// conventional prefix stands apart (`ReviewTitle::prefix`)
pub(crate) fn commit_title(subject: &str, issue: Option<&str>) -> ReviewTitle {
  ReviewTitle::of_subject(&commit_title_text(subject, issue))
}

fn commit_title_text(subject: &str, issue: Option<&str>) -> String {
  let stripped = stripped_subject(subject);
  let subject = stripped.trim();
  // the issue among the keys the subject starts with, not the first of them: it goes with the space after it
  let keys = leading_issue_keys(subject);
  if let Some(issue) = issue
    && let Some(&(start, end)) = keys.iter().skip(1).find(|(start, end)| &subject[*start..*end] == issue)
  {
    let after = subject[end..].trim_start_matches([' ', ',']);
    return format!("{}{after}", &subject[..start]);
  }
  let rest = issue
    .and_then(|issue| subject.strip_prefix(issue))
    .filter(|rest| rest.is_empty() || rest.starts_with([' ', ':', ',', '-', '\u{2013}', '\u{2014}']))
    .map(|rest| rest.trim_start_matches([' ', ':', ',', '-', '\u{2013}', '\u{2014}']));
  match rest {
    Some(rest) if !rest.is_empty() => rest.to_string(),
    _ => subject.to_string(),
  }
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
      if let Some(commit) = self.remover_of(u64::from(line)) {
        *into.entry(commit).or_default() += 1;
      }
    }
  }

  /// The commit that removed line `line` (from 0) of the base, `None` for a line that stays
  fn remover_of(&self, line: u64) -> Option<usize> {
    self.removed.iter().find(|(from, to, _)| (*from..*to).contains(&line)).map(|(_, _, commit)| *commit)
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
  // one commit whose body tells its parts by the code they name: the parts are the steps
  let message = (source == Some(ConcernsReason::OneCommit)).then(|| message_steps(input)).flatten();
  let mut steps = match (&message, source) {
    (Some(message), _) => message.steps.clone(),
    (None, None) => commit_steps(input, &parts, &story, &rewrites),
    (None, Some(_)) => concern_steps(input, &parts),
  };
  for step in &mut steps {
    order_stops(input, &file_of, step);
    (step.code, step.tests.scenarios) = code_of(input, &file_of, step);
  }
  join_units(input.units, &mut steps);
  for step in &mut steps {
    cut_groups(input, step);
  }
  mark_paths(input, &mut steps);
  add_step_facts(input, &mut steps);
  let coverage = coverage(input, &steps);
  let told = source.is_none() || message.is_some() || input.concerns.len() > 1;
  let telling = message.as_ref().map(|message| (message.told.as_slice(), &message.titled));
  let mut commits = story_commits(input, &steps, source == Some(ConcernsReason::OneCommit), told, telling);
  add_story_titles(&mut steps, &mut commits);
  add_story_marks(&mut steps, &commits);
  if source.is_none() {
    for step in steps.iter_mut().filter(|step| step.concern.is_none()) {
      step.title = step.commits.iter().find(|commit| commit.role == CommitRole::Starts).map(|commit| commit.title.clone());
      step.title_source = if step.title.is_some() { TitleSource::Commit } else { TitleSource::Evidence };
    }
  }
  ReviewSteps {
    source: match (&message, source) {
      (Some(_), _) => StepSource::Message,
      (None, None) => StepSource::Commits,
      (None, Some(reason)) => StepSource::Concerns { reason },
    },
    told,
    steps,
    coverage,
    commits,
    range_tests: range_tests(input, &file_of),
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
/// added, deleted or last changed it; a declaration whose block another unit holds, by the commit that wrote its name,
/// a removed one by the commit that removed it
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
    UnitKind::Removed if !slice.whole => unit.line.and_then(|line| carried.remover_of(u64::from(line.saturating_sub(1)))),
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
  // a non-mechanical unit of its own: one the candidate step never changed
  let owns_apart_from = |commit: usize, earlier: usize, head_of: &[Option<usize>]| {
    parts
      .iter()
      .zip(&owners)
      .any(|((part, lines), owner)| *owner == Some(commit) && !mechanical(part) && !lines.keys().any(|&other| other < commit && head_of[other] == Some(earlier)))
  };
  // the head of the step of each commit of a story: itself, or the head it follows up
  let mut head_of: Vec<Option<usize>> = vec![None; input.commits.len()];
  for commit in (0..input.commits.len()).filter(|&commit| story[commit]) {
    let removed = rewrites.removed_code.get(&commit).copied().unwrap_or(0);
    // a story of its own starts its step, though it rewrites the code of another
    let rewritten = (0..commit)
      .filter(|&earlier| head_of[earlier] == Some(earlier))
      .map(|earlier| (earlier, rewrites.of.get(&(commit, earlier)).copied().unwrap_or(0)))
      .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
      .filter(|(earlier, lines)| removed > 0 && lines * 2 >= removed && !tells_its_own_story(&input.commits[commit].1) && !owns_apart_from(commit, *earlier, &head_of));
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
    .map(|&head| input.commits[head].0.clone())
    .chain(std::iter::once("no-story".to_string()))
    .map(ReviewStep::new)
    .collect();
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
  // a step none of whose lines lands names the later commit that took out the most of what it wrote
  for (step, &head) in steps.iter_mut().zip(&heads).filter(|(step, _)| step.stops.is_empty()) {
    step.taken_out_by = (head + 1..input.commits.len())
      .map(|later| (later, rewrites.any.get(&(later, head)).copied().unwrap_or(0)))
      .filter(|(_, lines)| *lines > 0)
      .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
      .map(|(later, _)| input.commits[later].0.clone());
  }
  if steps.last().is_some_and(|last| last.stops.is_empty() && last.commits.is_empty()) {
    steps.pop();
  }
  steps
}

/// A commit of a step: its id and its subject
pub(crate) fn step_commit(input: &StepsInput<'_>, commit: usize, role: CommitRole) -> StepCommit {
  let (id, message) = &input.commits[commit];
  let (subject, _) = split_message(message);
  StepCommit {
    id: id.clone(),
    title: commit_title(subject, input.issue),
    role,
  }
}

/// The stop of a part
fn stop_of(input: &StepsInput<'_>, part: &Part, also: Vec<u32>) -> StepStop {
  let unit = &input.units[part.unit];
  StepStop {
    also,
    ..StepStop::new(unit.id.clone(), unit.slices[part.slice].path.clone(), part.block.map(|block| vec![block]))
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
          .map(|part| StepStop::new(part.unit.clone(), part.path.clone(), part.blocks.clone()))
          .collect(),
        ..ReviewStep::new(concern.id.clone())
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

/// The summary of the file of `path` after the commits
fn summary_after<'a>(input: &StepsInput<'a>, file_of: &HashMap<&str, usize>, path: &str) -> Option<&'a FileSummary> {
  let blob = file_of.get(path).and_then(|&file| input.changed[file].after)?;
  input.input?.summaries.get(blob).map(AsRef::as_ref)
}

/// The tests of the range: each unit of a test function, with its lines at the head (`test_span`), and each test
/// function of a file the range adds, its unit the file's
fn range_tests(input: &StepsInput<'_>, file_of: &HashMap<&str, usize>) -> Vec<RangeTest> {
  let mut tests = Vec::new();
  for unit in input.units {
    match unit.kind {
      UnitKind::Added | UnitKind::Behaviour | UnitKind::Shape { .. } => {
        if let Some(lines) = test_span(input, file_of, unit) {
          tests.push(RangeTest {
            path: unit.slices[0].path.clone(),
            lines,
            unit: unit.id.clone(),
          });
        }
      }
      UnitKind::NewFile => {
        let Some(path) = unit.slices.first().map(|slice| slice.path.as_str()) else { continue };
        let Some(summary) = summary_after(input, file_of, path) else { continue };
        tests.extend(summary.declarations.iter().filter(|declaration| declaration.test).map(|declaration| RangeTest {
          path: path.to_string(),
          lines: (declaration.start_line, declaration.end_line),
          unit: unit.id.clone(),
        }));
      }
      _ => {}
    }
  }
  tests
}

/// The declarations of the code of `step` (`StepCode`), and the tests the `[@test]` lines of the rules of a spec it
/// adds or changes name
fn code_of(input: &StepsInput<'_>, file_of: &HashMap<&str, usize>, step: &ReviewStep) -> (Vec<StepCode>, Vec<String>) {
  let place_of: HashMap<&str, &ReviewUnit> = input.units.iter().map(|unit| (unit.id.as_str(), unit)).collect();
  let mut seen: HashSet<&str> = HashSet::new();
  // name, the name led by its owner, its place
  let mut code: Vec<(&str, String, DeclarationAt)> = Vec::new();
  for stop in &step.stops {
    let Some(unit) = place_of.get(stop.unit.as_str()) else { continue };
    let source = language_of(&stop.path).is_some() && test_subject(&stop.path).is_none() && !unit.test;
    // a new file: each declaration at its top, as a new type of a changed file is one unit with its members
    if unit.kind == UnitKind::NewFile {
      let Some(summary) = summary_after(input, file_of, &stop.path).filter(|_| source && seen.insert(unit.id.as_str())) else {
        continue;
      };
      for declaration in summary.declarations.iter().filter(|it| it.container.is_none() && !it.test && it.kind != DeclKind::Binding) {
        let name = summary.name(declaration.name);
        let at = DeclarationAt {
          revision: None,
          path: stop.path.clone(),
          line: declaration.name_line,
          column: None,
        };
        code.push((name, name.to_string(), at));
      }
      continue;
    }
    let is_code = matches!(unit.kind, UnitKind::Added | UnitKind::Behaviour | UnitKind::Shape { .. }) && source;
    let (Some(name), Some(line)) = (unit.name.as_deref(), unit.line) else { continue };
    if !is_code || !seen.insert(unit.id.as_str()) {
      continue;
    }
    let owner = unit
      .key
      .as_deref()
      .and_then(|key| key.rsplit('.').nth(1))
      .filter(|owner| owner.starts_with(|c: char| c.is_ascii_uppercase()));
    let shown = owner.map_or_else(|| name.to_string(), |owner| format!("{owner}.{name}"));
    let at = DeclarationAt {
      revision: None,
      path: stop.path.clone(),
      line,
      column: None,
    };
    code.push((name, shown, at));
  }
  // a name two declarations share is led by its owner
  let mut count: HashMap<&str, usize> = HashMap::new();
  for (name, ..) in &code {
    *count.entry(name).or_default() += 1;
  }
  let code = code
    .into_iter()
    .map(|(name, shown, at)| StepCode {
      name: if count[name] > 1 { shown } else { name.to_string() },
      at,
    })
    .collect();
  let scenarios = input
    .contract
    .iter()
    .filter(|entry| {
      step.stops.iter().any(|stop| {
        stop.unit == entry.unit
          && stop.path == entry.path
          && (entry.blocks.is_empty() || stop.blocks.as_ref().is_none_or(|blocks| entry.blocks.iter().any(|block| blocks.contains(block))))
      })
    })
    .flat_map(|entry| entry.proven_by.iter().cloned())
    .collect::<BTreeSet<String>>()
    .into_iter()
    .collect();
  (code, scenarios)
}

/// The unit of `test` when the range adds or changes it: the test of the range whose lines hold its line (`range_tests`)
fn range_unit(test: &ReachingTest, range_tests: &[RangeTest]) -> Option<String> {
  range_tests
    .iter()
    .find(|it| it.path == test.path && (it.lines.0..=it.lines.1).contains(&test.line))
    .map(|it| it.unit.clone())
}

/// Adds `test`, which reaches `name` of the step, to `tests`: once by its place, its fewest calls and its nearest way,
/// each name once
fn add_test(tests: &mut Vec<StepTest>, test: &ReachingTest, unit: String, name: &str) {
  if let Some(known) = tests.iter_mut().find(|it| it.path == test.path && it.line == test.line) {
    known.depth = known.depth.min(test.depth);
    known.way = known.way.min(test.class.into());
    if !known.reaches.iter().any(|it| it == name) {
      known.reaches.push(name.to_string());
    }
    return;
  }
  tests.push(StepTest {
    unit: Some(unit),
    path: test.path.clone(),
    line: test.line,
    name: test.reads_as.clone(),
    depth: test.depth,
    way: test.class.into(),
    reaches: vec![name.to_string()],
  });
}

/// Add the tests of the change that reach the code of each step at `head`, at any depth by calls a type or an import
/// ties, asked by their places in one query with every test file of the range (`StepTests`), and to each step the other
/// steps its tests prove (`ReviewStep::proves`); the answer is why the steps have none, `None` when the index answered or
/// nothing was asked. With an index, a range without tests asks nothing: no test of the change reaches its code. Fails
/// when the index was canceled
pub fn add_tests(index: Option<&dyn SymbolIndex>, repository_path: &str, head: &str, steps: &mut ReviewSteps) -> Result<Option<String>> {
  let places: Vec<DeclarationAt> = steps
    .steps
    .iter()
    .flat_map(|step| step.code.iter().map(|code| code.at.clone()))
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect();
  if places.is_empty() {
    return Ok(None);
  }
  let given: Vec<String> = steps.range_tests.iter().map(|test| test.path.clone()).collect::<BTreeSet<_>>().into_iter().collect();
  let answers = if given.is_empty() && index.is_some() {
    // no test of the change: none reaches any of its code, whatever the index would answer
    vec![
      TestReach {
        complete: true,
        ..TestReach::default()
      };
      places.len()
    ]
  } else {
    match ask_index(index, |lenient| lenient.tests(repository_path, &places, &given, head))? {
      Ok(answers) => answers,
      Err(note) => return Ok(Some(note)),
    }
  };
  let reach_of: HashMap<&DeclarationAt, _> = places.iter().zip(&answers).collect();
  let ReviewSteps { steps, range_tests, .. } = steps;
  for step in steps.iter_mut() {
    let ReviewStep { code, tests, groups, stops, .. } = step;
    tests.answered = true;
    // the group of each file of a large step: of its first stop outside the mechanical group
    let mut group_of: HashMap<&str, usize> = HashMap::new();
    for (place, group) in groups.iter_mut().enumerate() {
      group.tested.answered = true;
      if group.kind != GroupKind::Mechanical {
        for stop in &stops[group.start as usize..group.end as usize] {
          group_of.entry(stop.path.as_str()).or_insert(place);
        }
      }
    }
    for code in code.iter() {
      // a place accio found no declaration at is neither reached nor unreached: it answered nothing of it
      let Some(reach) = reach_of.get(&code.at).filter(|reach| !reach.unavailable) else {
        continue;
      };
      // a test of a given file the change did not touch is no test of the change
      let of_change: Vec<(&ReachingTest, String)> = reach.tests.iter().filter_map(|test| range_unit(test, range_tests).map(|unit| (test, unit))).collect();
      record_tests(tests, code, reach.complete, &of_change);
      if let Some(&place) = group_of.get(code.at.path.as_str()) {
        record_tests(&mut groups[place].tested, code, reach.complete, &of_change);
      }
    }
  }
  add_proves(steps);
  Ok(None)
}

/// What accio answered of `code` into `tests` of a step or of a group of it: its tests of the change (`of_change`), else
/// that none reaches it when accio walked every way (`complete`)
fn record_tests(tests: &mut StepTests, code: &StepCode, complete: bool, of_change: &[(&ReachingTest, String)]) {
  if of_change.is_empty() {
    if complete {
      tests.unreached.push(UnreachedDeclaration {
        name: code.name.clone(),
        path: code.at.path.clone(),
        line: code.at.line,
      });
    }
    return;
  }
  tests.reached += 1;
  for (test, unit) in of_change {
    let file = tests.proof.iter().position(|file| file.path == test.path).unwrap_or_else(|| {
      tests.proof.push(ProofFile {
        path: test.path.clone(),
        tests: Vec::new(),
      });
      tests.proof.len() - 1
    });
    add_test(&mut tests.proof[file].tests, test, unit.clone(), &code.name);
  }
}

/// To each step, the other steps whose proof holds a test of one of its units (`ReviewStep::proves`)
fn add_proves(steps: &mut [ReviewStep]) {
  let step_of: HashMap<String, usize> = steps
    .iter()
    .enumerate()
    .flat_map(|(place, step)| step.stops.iter().map(move |stop| (stop.unit.clone(), place)))
    .collect();
  let mut proves: Vec<BTreeSet<u32>> = vec![BTreeSet::new(); steps.len()];
  for (place, step) in steps.iter().enumerate() {
    for test in step.tests.proof.iter().flat_map(|file| &file.tests) {
      let Some(&of) = test.unit.as_ref().and_then(|unit| step_of.get(unit)) else { continue };
      if of != place {
        proves[of].insert(u32::try_from(place).unwrap_or(u32::MAX));
      }
    }
  }
  for (step, proves) in steps.iter_mut().zip(proves) {
    step.proves = proves.into_iter().collect();
  }
}

/// The blocks of `unit` that `stop` shows: of its slice of the path of the stop, those it names, all without
fn shown_blocks<'a>(unit: &'a ReviewUnit, stop: &'a StepStop) -> impl Iterator<Item = &'a UnitBlock> + 'a {
  unit
    .slices
    .iter()
    .filter(|slice| slice.path == stop.path)
    .flat_map(|slice| &slice.blocks)
    .filter(|block| stop.blocks.as_ref().is_none_or(|blocks| blocks.contains(&block.index)))
}

/// The block of `stop` that holds line `line` (from 1) of its file, of the old side for `old`, by its index
fn block_at(unit_of: &HashMap<&str, &ReviewUnit>, stop: &StepStop, line: u32, old: bool) -> Option<u32> {
  let unit = unit_of.get(stop.unit.as_str())?;
  let at = line.checked_sub(1)?;
  shown_blocks(unit, stop)
    .find(|block| {
      if old {
        block.old_start <= at && at < block.old_end
      } else {
        block.new_start <= at && at < block.new_end
      }
    })
    .map(|block| block.index)
}

/// A stop of a declaration without a block of its own and the stop that shows the block of the line of its name
#[derive(Clone, Copy)]
struct Join {
  /// Its step and its stop there
  place: usize,
  at: usize,
  /// The step of the stop that shows it, that stop and its block
  other: usize,
  holder: usize,
  block: u32,
}

/// Joins each declaration without a block of its own to the stop that shows the block of the line of its name
/// (`StepStop::joined`), a stop of its own step first: it is no stop of its own then, and a step whose unit another
/// step's stop shows names it (`ReviewStep::shown_elsewhere`). One whose line no stop shows stays a stop, and so does one
/// whose line only a group shows (a mechanical change, a repeated edit, deleted files), whose section names no unit. A
/// unit of several stops (a part of two concerns) joins once: by the stop of the step whose stop holds its line, else
/// by its first; every stop of it goes, each step it was of naming it
pub(crate) fn join_units(units: &[ReviewUnit], steps: &mut [ReviewStep]) {
  let unit_of: HashMap<&str, &ReviewUnit> = units.iter().map(|unit| (unit.id.as_str(), unit)).collect();
  let group = |id: &str| {
    unit_of
      .get(id)
      .is_some_and(|unit| matches!(unit.kind, UnitKind::Mechanical { .. } | UnitKind::RepeatedEdit { .. } | UnitKind::DeletedFiles { .. }))
  };
  let mut joins: Vec<Join> = Vec::new();
  for (place, step) in steps.iter().enumerate() {
    for (at, stop) in step.stops.iter().enumerate() {
      let Some(unit) = unit_of.get(stop.unit.as_str()) else { continue };
      let alone = unit.slices.iter().any(|slice| slice.path == stop.path && !slice.whole && slice.blocks.is_empty());
      let Some(line) = unit.line.filter(|_| alone) else { continue };
      let old = unit.kind == UnitKind::Removed;
      let holder = std::iter::once(place).chain((0..steps.len()).filter(|&other| other != place)).find_map(|other| {
        steps[other]
          .stops
          .iter()
          .enumerate()
          .filter(|(_, holder)| holder.path == stop.path && holder.unit != stop.unit && !group(&holder.unit))
          .find_map(|(holder, of)| block_at(&unit_of, of, line, old).map(|block| (other, holder, block)))
      });
      if let Some((other, holder, block)) = holder {
        joins.push(Join { place, at, other, holder, block });
      }
    }
  }
  let index = |place: usize| u32::try_from(place).unwrap_or(u32::MAX);
  let mut gone: HashSet<(usize, usize)> = HashSet::new();
  // the stops of each unit by its path, in the order of the steps
  let mut of_unit: Vec<((String, String), Vec<Join>)> = Vec::new();
  for join in joins {
    let stop = &steps[join.place].stops[join.at];
    let key = (stop.unit.clone(), stop.path.clone());
    match of_unit.iter_mut().find(|(of, _)| *of == key) {
      Some((_, stops)) => stops.push(join),
      None => of_unit.push((key, vec![join])),
    }
  }
  for ((unit, _), stops) in of_unit {
    let Some(&Join { place, other, holder, block, .. }) = stops.iter().find(|join| join.place == join.other).or_else(|| stops.first()) else {
      continue;
    };
    steps[other].stops[holder].joined.push(JoinedUnit {
      unit: unit.clone(),
      block,
      step: index(place),
    });
    for &Join { place: of, at, .. } in &stops {
      let shown = ShownElsewhere {
        unit: unit.clone(),
        step: index(other),
      };
      if of != other && !steps[of].shown_elsewhere.contains(&shown) {
        steps[of].shown_elsewhere.push(shown);
      }
      gone.insert((of, at));
    }
  }
  for (place, step) in steps.iter_mut().enumerate() {
    let mut at = 0;
    step.stops.retain(|_| {
      at += 1;
      !gone.contains(&(place, at - 1))
    });
  }
}

/// The files of each step and the main row of each file (`ReviewStep::paths`, `elsewhere`): the step whose stops change
/// the most lines of it, by the lines of the blocks they show, a file without blocks one. A file of a unit of the step
/// another step's stop shows (`ReviewStep::shown_elsewhere`) is a file of the step too, its main row elsewhere: the tree
/// lists it "also in" the step
fn mark_paths(input: &StepsInput<'_>, steps: &mut [ReviewStep]) {
  let unit_of: HashMap<&str, &ReviewUnit> = input.units.iter().map(|unit| (unit.id.as_str(), unit)).collect();
  // the path -> (the step of its main row, its changed lines there)
  let mut main: HashMap<String, (usize, u32)> = HashMap::new();
  for (place, step) in steps.iter_mut().enumerate() {
    let mut weights: Vec<(String, u32)> = Vec::new();
    for stop in &step.stops {
      let lines: u32 = unit_of.get(stop.unit.as_str()).map_or(1, |unit| {
        shown_blocks(unit, stop)
          .map(|block| (block.old_end - block.old_start) + (block.new_end - block.new_start))
          .sum::<u32>()
          .max(1)
      });
      match weights.iter_mut().find(|(path, _)| *path == stop.path) {
        Some((_, weight)) => *weight += lines,
        None => weights.push((stop.path.clone(), lines)),
      }
    }
    for (path, weight) in &weights {
      let entry = main.entry(path.clone()).or_insert((place, *weight));
      if *weight > entry.1 {
        *entry = (place, *weight);
      }
    }
    step.paths = weights.into_iter().map(|(path, _)| path).collect();
  }
  // the file of each unit of a step that another step's stop shows
  let shown_paths: Vec<Vec<String>> = steps
    .iter()
    .map(|step| {
      step
        .shown_elsewhere
        .iter()
        .filter_map(|shown| {
          let holder = steps.get(usize::try_from(shown.step).ok()?)?;
          let stop = holder.stops.iter().find(|stop| stop.joined.iter().any(|joined| joined.unit == shown.unit))?;
          Some(stop.path.clone())
        })
        .collect()
    })
    .collect();
  for (step, paths) in steps.iter_mut().zip(shown_paths) {
    for path in paths {
      if !step.paths.contains(&path) {
        step.paths.push(path);
      }
    }
  }
  for (place, step) in steps.iter_mut().enumerate() {
    step.elsewhere = step
      .paths
      .iter()
      .filter(|path| main.get(path.as_str()).is_some_and(|(home, _)| *home != place))
      .cloned()
      .collect();
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
