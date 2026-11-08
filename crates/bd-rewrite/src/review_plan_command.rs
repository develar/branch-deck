//! `brd review-plan`: the plan of a review of a branch or a range (`review_plan::review_plan_core`,
//! spec/branch/review-plan.spec.md), as JSON for an agent or an offline evaluation: for a branch marked reviewed
//! (`brd mark-reviewed`), its commits paired with the reviewed ones and the plain range of the ones added on top; the
//! public declarations the reviewed commits add, remove or change, ranked by their usages at the head left outside the
//! change, from accio (`AccioCli::discover`, never canceled, never an index build); the facts of each changed file,
//! the links between the files, the dependency changes of the build files and the repeated edits; the changed files in
//! dependency order. Reads only; the baseline is that of the checked out branch.
//!
//! Its output has `"schema": 25` (`REVIEW_PLAN_SCHEMA_VERSION`), the other outputs of `bd` 2: the plan is
//! experimental; its version 3 replaced the `risk` of each file with `files` and the `fan_in` of each declaration
//! with `usages`, version 4 added the `units`, the changes a reviewer reads once in the order of the story,
//! version 5 the `generated` files (`mechanical: "generated"`, with why), the lock files among them, and version 6
//! made a deletion real work: no `mechanical: "deleted"`, a unit `deletedFile` per deleted file and `deletedFiles`
//! with its `directory` for 3 or more deleted from one directory, version 7 the `concerns`, version 8 the `elsewhere`
//! of each concern, its files whose main row is in another one, version 9 the name `file` of a concern, version 10
//! the `steps` of the story (spec/branch/review-tour.spec.md), their `coverage`, and the `brief`, version 11 the `api`
//! and `test` of a unit in place of its `public`, version 12 the `still_bound` and `by_name` usages, and `unbroken` in
//! place of `updated`, version 13 the `reached` and `unreached` tests of a step, from accio, in place of `named` and
//! `unnamed`, version 14 the story of each commit once (`steps.commits`: its body by blocks, its `mentions` of code
//! resolved to the units and the files, `target: null` where a name in backticks resolves nothing, `toldIn`) in place
//! of the `body` of each commit of a step, the `newApi`, `moduleLinks`, `takenOutBy` and `title` of a step, `told`,
//! the `title` of each commit in place of its `subject` (without the issue key of the brief) and the `title` of the plan,
//! the `code` of a body and the `addedCode` and `removedCode` of an entry of the brief (`sync_utils::code_spans`), the
//! `toldAsTitle` of a step;
//! version 15 (after 0.43.0) the `parts` of each story, each with its `kind`, `tone`, where it is `told` and whether
//! it is `titled`, in place of its `toldIn` and the `toldAsTitle` of a step, the `kind` and `tone` of each part of a
//! body, the `joined` units of a stop (a declaration without a block of its own, with its step), its `marks` (the lines
//! of the told bodies that name it), the `shownElsewhere` units of a step, the state `reworded` of a commit whose lines
//! are as reviewed and whose message is not, the `message_change` of a reworded or rewritten commit, the `change` of a
//! story and the `removals` of its parts, the source `message` of the steps of one commit whose body tells its parts by
//! their code, and the `titleSource` of a step;
//! version 16 (after 0.44.0) the `paragraphs` of an entry of the brief (`review_plan::prose`: whole paragraphs, each
//! `added`, `removed` or `changed` by word with its `inserted` and `deleted` words, its `code`, its `links` and the
//! `excerpt` of a long one) in place of its `added`, `addedCode`, `removed` and `removedCode`, and its `blocks` in place
//! of its `block`
//! version 19 added the repeated word replacements of the brief (`brief.substitutions`) and their marks in the changed
//! paragraphs, including paragraphs changed only by those replacements;
//! version 20 a widening (the reach alone opened wider, of a declaration no subtype may override) as an `added` entry
//! with `widened` and its form `before`, the reach in the forms as the language writes it, and the `verdict` of the plan;
//! version 21 the form `row` of a paragraph of the brief (a row of a table under the cells of its header), the
//! `excerpt` of every changed paragraph of several sentences, and no paragraph of front matter or of metadata;
//! version 22 the `table` block of a body (its `header`, the `align` of each column and its `rows`, a cell a line of
//! its `code` by its place, the header row first) and the `summary` of a body, the block that sums it up
//! version 23 the `modules` of the changed files and the `module` of each file, the `moduleEdges` of a step in place of
//! its `moduleLinks`, and the mechanical kind `moduleList`;
//! version 24 the `groups` of a large step (`contract`, a `module` or a `folder` with its `also`, `mechanical`, each its
//! `start`, `tests` and `end` in the stops), in place of the `concern` of a stop;
//! version 25 the `registrations` of the plugin descriptors, from accio (`accio parse --descriptor --resolve`, one
//! process per side)

use crate::mark_reviewed_command::refused;
use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::output::ShortIds;
use domain_types::error::ErrorCode;
use file_diffs::accio::{AccioCli, SymbolIndex};
use generated_files::generated_by::GeneratedBy;
use git_ops::landed_pairing::PathLines;
use review_plan::api::{ApiChange, ApiChangeKind, BreakVerdict, FormChange, UsageSite};
use review_plan::brief::ReviewBrief;
use review_plan::concerns::{Concern, ConcernEdges};
use review_plan::dependencies::DependencyChange;
use review_plan::files::{FileFacts, MechanicalKind, RepeatedEditGroup};
use review_plan::interdiff::{Interdiff, MessageChange, PairState};
use review_plan::links::{FileLink, LinkState, LinkedSymbol};
use review_plan::modules::ModuleShape;
use review_plan::registrations::Registration;
use review_plan::steps::ReviewSteps;
use review_plan::units::{OtherReason, ReviewUnit, UnitBlock, UnitKind, UnitSlice};
use review_plan::{PlanSources, ReviewPlanParams, ReviewSubject, SinceReviewedOfRange, UnknownRevision, review_plan_core};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Serialize, Serializer};
use std::borrow::Cow;

/// The version of the output of `brd review-plan`
pub const REVIEW_PLAN_SCHEMA_VERSION: u32 = 25;

/// The `"schema"` field the output of `brd review-plan` starts with (`REVIEW_PLAN_SCHEMA_VERSION`)
#[derive(Debug, Clone, Copy, Default)]
pub struct ReviewPlanSchemaVersion;

impl Serialize for ReviewPlanSchemaVersion {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_u32(REVIEW_PLAN_SCHEMA_VERSION)
  }
}

impl JsonSchema for ReviewPlanSchemaVersion {
  fn inline_schema() -> bool {
    true
  }

  fn schema_name() -> Cow<'static, str> {
    "ReviewPlanSchemaVersion".into()
  }

  fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "integer", "const": REVIEW_PLAN_SCHEMA_VERSION })
  }
}

#[derive(Debug, clap::Args)]
pub struct ReviewPlanArgs {
  /// The branch, named as `brd mark-reviewed` names it (`auth`), or a range `<base>..<head>`
  pub subject: String,
  /// The commit the branch is reviewed on, for a branch stacked on another: its parent, as `brd mark-reviewed --base`;
  /// without it, the merge base with the baseline. Not for a range
  #[arg(long)]
  pub base: Option<String>,
  /// Only the commits added or rewritten since the branch was marked reviewed; not for a range
  #[arg(long)]
  pub since_reviewed: bool,
  /// The edges the concerns join their parts by, for a measurement of each (`none` for none): all of them by default
  #[arg(long, hide = true, value_delimiter = ',')]
  pub concern_edges: Option<Vec<ConcernEdge>>,
}

/// An edge of the concerns (`review_plan::concerns::ConcernEdges`) that `--concern-edges` turns on
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ConcernEdge {
  Uses,
  Tokens,
  Rest,
  None,
}

/// The edges `--concern-edges` names, all of them without it
fn concern_edges(args: &ReviewPlanArgs) -> ConcernEdges {
  args.concern_edges.as_ref().map_or_else(ConcernEdges::default, |edges| ConcernEdges {
    uses: edges.contains(&ConcernEdge::Uses),
    tokens: edges.contains(&ConcernEdge::Tokens),
    rest: edges.contains(&ConcernEdge::Rest),
  })
}

/// `brd review-plan`
#[derive(Debug, Serialize, JsonSchema)]
pub struct ReviewPlanOutput {
  pub schema: ReviewPlanSchemaVersion,
  pub repository: String,
  /// The full name of the branch (`{prefix}/virtual/<name>` for a virtual branch); `null` for a range
  pub branch: Option<String>,
  /// The name of what is reviewed: a branch of the prefix by its name (`layout` of `jane/virtual/layout`), a range by
  /// its short ids
  pub title: String,
  /// The reviewed commits are `base..head`: for a branch the merge base of its tip and `--base` (else the baseline),
  /// and its tip
  pub base: String,
  pub head: String,
  /// What changed since the branch was marked reviewed; `null` for a range and for a branch never marked
  pub interdiff: Option<InterdiffOutput>,
  /// The public declarations the reviewed commits add, remove or change in form, no override, the most usages left
  /// outside the change first. The reviewed commits are those of `base..head`; with `--since-reviewed` only the added
  /// and rewritten ones, each against its own parent
  pub api: Vec<ApiChangeOutput>,
  /// Whether a use left outside the change breaks, over the removed and changed entries of `api`: `breaks` when one has
  /// a use left outside that can break; `unknown` when the usages of one are `null`, not `complete` or tied by the name
  /// alone (`by_name`); `noUseBreaks` when every one is `unbroken`, also when `api` holds additions alone
  pub verdict: BreakVerdict,
  /// Every file the reviewed commits change, once, at its last path (a deleted one too): the most `breaks` first, then
  /// the `untested` ones, then in tree order
  pub files: Vec<FileFactsOutput>,
  /// Which changed file uses which, before and after the commits, by the file that uses, then the one that declares
  pub links: Vec<FileLinkOutput>,
  /// The entries the commits add to, remove from or change in the build files (the `orderEntry` of an `*.iml`, the
  /// `depends`, `dependencies` and `content` of a plugin descriptor, the dependency tables of a `Cargo.toml`, the
  /// dependencies of a `package.json`), and the build files they add or delete (`newModule`): by build file in tree
  /// order, then by kind and name. A new version or scope is one `changed` entry
  pub dependencies: Vec<DependencyChange>,
  /// The modules of the changed files (the nearest directory above each with a build file at the head, `""` for none):
  /// one the commits add first (`state`), then the most changed lines of code and tests; each with its `name`, its
  /// `label`, its `files` by role, its lines, its `directories`, the modules it newly `uses` (the links the commits add,
  /// by their place in `links`, and whether a build file `declared` it), those that newly use it (`usedBy`), the plugin
  /// modules that add it as content (`contentOf`) and the entries its build files add that name no module of the review
  /// (`newDependencies`, by their place in `dependencies`)
  pub modules: Vec<ModuleShape>,
  /// What the commits register in the platform: the services, extension points, listeners, extensions, registry keys
  /// (`registryKey`, its `defaultValue` the option marked `*` of a key of `options`), actions and groups, and content
  /// modules their plugin descriptors add, remove or change (`change`; of a `changed` one what it was `before` and the
  /// attributes or `<child>` elements that `changed`), read by accio at the base and the head (with `--since-reviewed`
  /// the ends of `scope`, none when its carry conflicts), each with its `descriptor`, the `module` of the descriptor by
  /// its place in `modules`, a content module the `reviewModule` it names, and the `classes` it names as accio resolved
  /// them at the head (of a removed one at the base): `declared` with `path` and `line`, `ambiguous` with `count`,
  /// `notFound`, `null` without an index (`index_note`); by module, descriptor, then kind. Empty without accio
  pub registrations: Vec<Registration>,
  /// The groups of 3 or more files that get the same edit, the most files first
  pub repeated_edits: Vec<RepeatedEditOutput>,
  /// The changes a reviewer reads once, in the order of the story, of the diff of `base..head` (with `--since-reviewed`
  /// of `scope`): a change of the form of a declaration (`shape`), of its body alone (`behaviour`), a new or removed
  /// declaration, a new file, one unit per kind of mechanical change and per repeated edit over all of their files,
  /// and a change of no declaration (`other`). Every block of the diff is in one unit, every changed file in one at
  /// least; the mechanical units come last
  pub units: Vec<UnitOutput>,
  /// The tasks of the range, in the order of the story: each a set of parts of the units (`unit`, `path`, the `blocks`
  /// by index, `null` for the whole slice) joined by evidence (`evidence`: `uses` of a changed declaration, `group`,
  /// `token` of a rare identifier, `rest`), named by its source (`token`, `replacement`, `declaration`, `file`, `directory`), the parts no
  /// evidence joined last (`rest`). A mechanical file is in none, but a consequence of a concern follows it: a generated
  /// file of the source its header names, a build file of the code that needs its new dependency, a file of imports
  /// alone of the declaration it imports
  pub concerns: Vec<Concern>,
  /// The story of the review (`source`; `told` false when it tells no steps: one concern that covers the range): a
  /// step per commit whose body tells it (`role` `starts`), with the commits
  /// that follow it up (`followsUp`) and those of no story whose lines are in it (`noStory`), the changes only commits
  /// without a body wrote last (`id` `no-story`); of one commit whose body tells two parts or more by the code they name
  /// (`message`), a step per part, in the order of the body, with the concerns it names, then the concerns no part
  /// names; else, with no body or one commit, a step per concern. Each step shows parts of the units (`stops`: `unit`,
  /// `path`, `blocks`; a declaration it adds or changes the form of first, the stops that `uses` it right after; `also` the other steps that changed its lines; `joined`
  /// the declarations without a block of their own on a line of its blocks, each with its `step`) and the
  /// tests of the change accio finds among the callers of its code (`tests`: `answered`, `proof` with each test's `unit`
  /// and `way`, `reached`, `unreached` only on a complete answer), the public declarations it
  /// adds (`newApi`, by the key of their `api` entry) and the new uses between modules it adds (`moduleEdges`: `from`
  /// and `to` by their place in `modules`, the `links` by their place in `links`); a step none of whose code lands, the later commit that took it out (`takenOutBy`); its units the
  /// stop of another step shows (`shownElsewhere`, with that `step`); of a large step its `groups`, each a run
  /// of its stops (`start..end`, its tests from `tests`): the `contract`, a `module` or a `folder` with the small modules
  /// it holds (`also`), the `mechanical` stops last; its `title` and what gives it (`titleSource`:
  /// `commit`, the `label` of the section it tells, the first `sentence` of its paragraph, the first code `name` of it,
  /// or none, `evidence`). `coverage`: every block of the diff in one stop, else in a mechanical unit. `commits`: the story
  /// of each commit with a body once, oldest first: its `body` by blocks (paragraphs, lists, preformatted text,
  /// headings, labels with their `kind`, rules) with its `trailers` apart, the `lead` and its `parts` (a labelled
  /// section, a run of other blocks), each `folded` or not; its `mentions` of code, each with its block, its line or item, its UTF-16 range and its `target` (a `unit`
  /// with its path, side, line and step, or a `file` with its step), `null` for a name in backticks that resolves
  /// nothing; its `parts` as the document tells them, each where it is `told` (`overview`, a `step` of the concerns of
  /// one commit, else with its `commit`) and `titled` when its label titles that step; each stop its `marks`, the lines
  /// of the told bodies that name it
  pub steps: ReviewSteps,
  /// The issue key the commits name (`issue`) and the prose the change adds, removes or changes (`contract`): the rules
  /// of the specs with the tests their `[@test]` lines name (`provenBy`), the records of decisions, the guides of
  /// agents, the docs
  pub brief: ReviewBrief,
  /// Every path the reviewed commits change, once, at its last path (a deleted one too), in dependency order: a file
  /// that declares what another uses before it, a test right after its subject; the tree order inside a cycle and
  /// between files with no edge
  pub order: Vec<String>,
  /// Why a removed or changed declaration has no `usages`, a step no tests, the plan no `registrations` or their classes
  /// no place: accio is not installed, has no index of this repository, or failed; `null` when every query answered or
  /// none was asked
  pub index_note: Option<String>,
}

/// A public declaration the reviewed commits add, remove or change
#[derive(Debug, Serialize, JsonSchema)]
pub struct ApiChangeOutput {
  /// The file that declares it after the commits; of a removal, the file before the first commit that touched it, the
  /// old path when that commit renamed it
  pub path: String,
  pub line: u32,
  pub name: String,
  pub kind: ApiChangeKind,
  /// Of an addition: it only opens the reach of a declaration no subtype may override wider (`private` or `internal`
  /// to public), which breaks no use; `before` holds the form it had
  pub widened: bool,
  /// Of a change: the parts of its form that changed
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub changes: Vec<FormChange>,
  /// The form before the commits as code reads (the header with the parameters after the name, a type by its
  /// constructors, the reach as the language writes it), of a change, a removal or a widening
  pub before: Option<String>,
  /// The form after the commits, of a change or an addition
  pub after: Option<String>,
  /// Of a removal or a change: its usages at the head, from `accio references --at` its place (of a removal, at the
  /// parent of the commit that first touched it); `null` without an answer (`index_note`)
  pub usages: Option<UsagesOutput>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UsagesOutput {
  /// On the lines the reviewed commits changed: updated in this change
  pub inside: u32,
  /// On the other lines, of a changed file too: left outside the change, where a removal or a change may break the code
  pub outside: u32,
  /// The first 5 usages left outside that can break
  pub outside_sites: Vec<UsageSite>,
  /// Calls left outside that still bind the parameters they bound before: accio says they fit the new form, and each
  /// argument binds a parameter of the same name and type as before
  pub still_bound: u32,
  /// Hits in code left outside that only the name ties to the declaration: unknown, not counted as uses
  pub by_name: u32,
  /// False when accio cut its answer at 100 usages
  pub complete: bool,
  /// No use left outside can break: `complete`, no use left outside that can break, none tied by the name alone
  pub unbroken: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FileFactsOutput {
  pub path: String,
  /// The path before the first commit that changes it, when the commits rename it
  pub old_path: Option<String>,
  /// Why its change is mechanical (a build file, only imports, only whitespace, a pure rename, a generated file);
  /// `null` for real work, a deletion too
  pub mechanical: Option<MechanicalKind>,
  /// Why it is generated, when `mechanical` is `generated`: its name (a lock file, a minified file, a source map), the
  /// attributes of the head (`linguist-generated`, `-diff`), or the header a tool wrote into it, with its line
  pub generated: Option<GeneratedBy>,
  /// The changed files it uses and those that use it after the commits
  pub uses: u32,
  pub used_by: u32,
  /// The links it uses that the commits added
  pub new_links: u32,
  /// The usages left outside the change, at the head, of the API it removes or changes
  pub breaks: u32,
  /// `breaks` counts every usage left outside; false makes it a lower bound: a usage count of its API is unknown or
  /// incomplete
  pub breaks_complete: bool,
  /// A source file changed while none of its tests changed
  pub untested: bool,
  /// The index of its group in `repeated_edits`
  pub repeated_edit: Option<u32>,
  /// Every hunk of it is the edit of its group
  pub repeated_edit_only: bool,
  /// Its module, by its place in `modules`
  pub module: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FileLinkOutput {
  /// The file that uses
  pub from: String,
  /// The file that declares
  pub to: String,
  /// The names it uses, each with the line of its first use in `from`: after the commits, or before them for a removed
  /// link
  pub symbols: Vec<LinkedSymbol>,
  pub state: LinkState,
  /// The two files are in different modules
  pub cross_module: bool,
  /// The commits added it between two modules: a new dependency of a module
  pub new_between_modules: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UnitOutput {
  /// Names it in this plan: of a declaration its file, key and signature
  pub id: String,
  pub kind: UnitKindOutput,
  /// Of a `shape`: the parts of the form that changed
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub changes: Vec<FormChange>,
  /// Of a `mechanical` unit: its kind
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub mechanical: Option<MechanicalKind>,
  /// Of a `repeatedEdit`: its group in `repeated_edits`
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub group: Option<u32>,
  /// Of an `other` unit: why no declaration holds it
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reason: Option<OtherReason>,
  /// Of a `deletedFiles` unit: the directory the commits delete its files from
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub directory: Option<String>,
  /// Of a declaration its name, of a file the name of the file
  pub name: Option<String>,
  /// Of a declaration: namespace, containers and name, as the `api` entry of a public one
  pub key: Option<String>,
  /// Of a declaration: what tells it from an overload of the same key
  pub signature: Option<String>,
  /// Of a declaration: it has its entry in `api`, or would have it by a change of its form: an addition of API, any
  /// other change of API or unstable API on a side
  pub api: bool,
  /// Of a declaration: test code, a test, a fixture of its class or module
  pub test: bool,
  /// Of a declaration: the line of its name, after the commits (before them for a removal)
  pub line: Option<u32>,
  /// What it changes in each file
  pub slices: Vec<UnitSliceOutput>,
  /// The change, wherever it sits: the same across a rebase or an amend that keeps it
  pub fingerprint: String,
}

/// What a unit changes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum UnitKindOutput {
  /// The form of a declaration, as a use sees it
  Shape,
  /// The body of a declaration alone
  Behaviour,
  /// A declaration added to a file that was there
  Added,
  /// A declaration removed from a file that stays
  Removed,
  /// A file the commits add
  NewFile,
  /// A file the commits delete
  DeletedFile,
  /// 3 or more files the commits delete from one directory
  DeletedFiles,
  /// One kind of mechanical change, over every file that has it
  Mechanical,
  /// The same edit in 3 or more files
  RepeatedEdit,
  /// A change of no declaration, or of a file whose declarations are not known
  Other,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UnitSliceOutput {
  pub path: String,
  /// The path before the commits, when they rename the file
  pub old_path: Option<String>,
  /// The unit takes the whole file: a new, deleted, renamed or binary file, or a file whose change is mechanical
  pub whole: bool,
  /// Its blocks of the diff, by their place in it and their lines on each side from 1, both included (`null` for a
  /// side with none)
  pub blocks: Vec<UnitBlockOutput>,
  pub fingerprint: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UnitBlockOutput {
  pub index: u32,
  pub old: Option<[u32; 2]>,
  pub new: Option<[u32; 2]>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RepeatedEditOutput {
  /// In tree order
  pub files: Vec<String>,
  /// The first lines the edit removes and adds, trimmed
  pub removed: Vec<String>,
  pub added: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct InterdiffOutput {
  /// The pair `brd mark-reviewed` kept
  pub reviewed_base: String,
  pub reviewed_tip: String,
  /// The commits of the branch, oldest first, then the dropped ones; with `--since-reviewed`, only the added, the
  /// rewritten and the reworded ones
  pub commits: Vec<PairedCommitOutput>,
  /// What changed since the review as one diff, `base..head`: the commits added on top as a plain range when nothing
  /// else changed (`head..head`, empty, when nothing changed); after a rewrite, a drop or a commit added below a
  /// reviewed one, from the reviewed change carried onto the base (`carried`); `null` when that carry conflicts
  pub scope: Option<RangeOutput>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PairedCommitOutput {
  /// The commit of the branch; `null` when dropped
  pub id: Option<String>,
  /// The reviewed commit it pairs with: the same commit, a copy with the same identity (author, author time, subject),
  /// or one reworded with the same changed lines; `null` when added
  pub reviewed_id: Option<String>,
  pub subject: String,
  pub state: PairState,
  /// Of a rewritten commit: the lines it changes and its reviewed version did not (`+` added, `-` removed), at most 20
  /// paths of 20 lines
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub new_only: Vec<PathLines>,
  /// Of a rewritten commit: the lines its reviewed version changed and it does not
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub reviewed_only: Vec<PathLines>,
  /// Of a reworded commit, and of a rewritten one whose message changed too: its subject before (without the
  /// `(prefix)`) when it differs, and the `hunks` of its body by blocks, each the blocks added at `start` and the
  /// reviewed blocks removed before it
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub message_change: Option<MessageChange>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RangeOutput {
  pub base: String,
  pub head: String,
  /// `base` is the reviewed change carried onto the base of the branch, a commit on no ref (`git diff base head` shows
  /// what changed since the review); else a commit of the branch or its base
  pub carried: bool,
}

/// The subject of `args`: a range when it holds `..`, else a branch
fn subject_of(args: &ReviewPlanArgs) -> Result<ReviewSubject, CliError> {
  match args.subject.split_once("..") {
    Some(_) if args.base.is_some() => Err(CliError::new(ErrorCode::InvalidArguments, "--base is for a branch, not for a range")),
    Some((base, head)) => Ok(ReviewSubject::Range {
      base: base.to_string(),
      head: head.to_string(),
    }),
    None => Ok(ReviewSubject::Branch {
      name: args.subject.clone(),
      base: args.base.clone(),
      baseline_branch: None,
    }),
  }
}

pub fn review_plan(context: &RepoContext, args: &ReviewPlanArgs) -> Result<ReviewPlanOutput, CliError> {
  let params = ReviewPlanParams {
    repository_path: context.repository.clone(),
    subject: subject_of(args)?,
    since_reviewed: args.since_reviewed,
    concern_edges: concern_edges(args),
  };
  let (cache, never_canceled) = (context.cache(), || false);
  let search = AccioCli::discover().map(|accio| accio.search(&never_canceled));
  let sources = PlanSources {
    cache: &cache,
    file_diffs: context.file_diffs(),
    index: search.as_ref().map(|search| search as &dyn SymbolIndex),
  };
  let plan = review_plan_core(&context.git, sources, &context.prefix, &params).map_err(|error| {
    if error.downcast_ref::<UnknownRevision>().is_some() || error.downcast_ref::<SinceReviewedOfRange>().is_some() {
      return CliError::new(ErrorCode::InvalidArguments, error.to_string());
    }
    refused(error, args.base.is_some())
  })?;
  let interdiff_ids = plan.interdiff.iter().flat_map(|interdiff| {
    let commits = interdiff.commits.iter().flat_map(|commit| commit.id.iter().chain(&commit.reviewed_id));
    let scope = interdiff.scope.iter().flat_map(|scope| [&scope.base, &scope.head]);
    [&interdiff.reviewed_base, &interdiff.reviewed_tip].into_iter().chain(commits).chain(scope)
  });
  let step_ids = plan
    .steps
    .steps
    .iter()
    .flat_map(|step| step.commits.iter().map(|commit| &commit.id))
    .chain(plan.steps.commits.iter().map(|commit| &commit.id));
  let short = context.short_ids([&plan.base, &plan.head].into_iter().chain(interdiff_ids).chain(step_ids).map(String::as_str));
  let mut steps = plan.steps;
  for step in &mut steps.steps {
    for commit in &mut step.commits {
      commit.id = short.get(&commit.id).to_string();
    }
    if step.concern.is_none() && step.id != "no-story" {
      step.id = short.get(&step.id).to_string();
    }
  }
  for commit in &mut steps.commits {
    commit.id = short.get(&commit.id).to_string();
  }
  Ok(ReviewPlanOutput {
    schema: ReviewPlanSchemaVersion,
    repository: context.repository.clone(),
    base: short.get(&plan.base).to_string(),
    head: short.get(&plan.head).to_string(),
    interdiff: plan.interdiff.as_ref().map(|interdiff| interdiff_output(interdiff, &short)),
    branch: plan.branch,
    title: plan.title,
    api: plan.api.into_iter().map(api_output).collect(),
    verdict: plan.verdict,
    files: plan.files.into_iter().map(file_output).collect(),
    links: plan.links.into_iter().map(link_output).collect(),
    dependencies: plan.dependencies,
    modules: plan.modules,
    registrations: plan.registrations,
    repeated_edits: plan.repeated_edits.into_iter().map(repeated_edit_output).collect(),
    units: plan.units.into_iter().map(unit_output).collect(),
    concerns: plan.concerns,
    steps,
    brief: plan.brief,
    order: plan.order,
    index_note: plan.index_note,
  })
}

fn api_output(change: ApiChange) -> ApiChangeOutput {
  ApiChangeOutput {
    path: change.path,
    line: change.line,
    name: change.name,
    kind: change.kind,
    widened: change.widened,
    changes: change.changes,
    before: change.before,
    after: change.after,
    usages: change.usages.map(|usages| UsagesOutput {
      inside: usages.inside,
      outside: usages.outside,
      outside_sites: usages.outside_sites,
      still_bound: usages.still_bound,
      by_name: usages.by_name,
      complete: usages.complete,
      unbroken: usages.unbroken,
    }),
  }
}

fn file_output(facts: FileFacts) -> FileFactsOutput {
  FileFactsOutput {
    path: facts.path,
    old_path: facts.old_path,
    mechanical: facts.mechanical,
    generated: facts.generated,
    uses: facts.uses,
    used_by: facts.used_by,
    new_links: facts.new_links,
    breaks: facts.breaks,
    breaks_complete: facts.breaks_complete,
    untested: facts.untested,
    repeated_edit: facts.repeated_edit,
    repeated_edit_only: facts.repeated_edit_only,
    module: facts.module,
  }
}

fn link_output(link: FileLink) -> FileLinkOutput {
  FileLinkOutput {
    from: link.from,
    to: link.to,
    symbols: link.symbols,
    state: link.state,
    cross_module: link.cross_module,
    new_between_modules: link.new_between_modules,
  }
}

fn unit_output(unit: ReviewUnit) -> UnitOutput {
  let block = |block: UnitBlock| {
    let lines = |start: u32, end: u32| (end > start).then_some([start + 1, end]);
    UnitBlockOutput {
      index: block.index,
      old: lines(block.old_start, block.old_end),
      new: lines(block.new_start, block.new_end),
    }
  };
  let slice = |slice: UnitSlice| UnitSliceOutput {
    path: slice.path,
    old_path: slice.old_path,
    whole: slice.whole,
    blocks: slice.blocks.into_iter().map(block).collect(),
    fingerprint: slice.fingerprint,
  };
  let mut output = UnitOutput {
    id: unit.id,
    kind: UnitKindOutput::Other,
    changes: Vec::new(),
    mechanical: None,
    group: None,
    reason: None,
    directory: None,
    name: unit.name,
    key: unit.key,
    signature: unit.signature,
    api: unit.api,
    test: unit.test,
    line: unit.line,
    slices: unit.slices.into_iter().map(slice).collect(),
    fingerprint: unit.fingerprint,
  };
  output.kind = match unit.kind {
    UnitKind::Shape { changes } => {
      output.changes = changes;
      UnitKindOutput::Shape
    }
    UnitKind::Behaviour => UnitKindOutput::Behaviour,
    UnitKind::Added => UnitKindOutput::Added,
    UnitKind::Removed => UnitKindOutput::Removed,
    UnitKind::NewFile => UnitKindOutput::NewFile,
    UnitKind::DeletedFile => UnitKindOutput::DeletedFile,
    UnitKind::DeletedFiles { directory } => {
      output.directory = Some(directory);
      UnitKindOutput::DeletedFiles
    }
    UnitKind::Mechanical { mechanical } => {
      output.mechanical = Some(mechanical);
      UnitKindOutput::Mechanical
    }
    UnitKind::RepeatedEdit { group } => {
      output.group = Some(group);
      UnitKindOutput::RepeatedEdit
    }
    UnitKind::Other { reason } => {
      output.reason = Some(reason);
      UnitKindOutput::Other
    }
  };
  output
}

fn repeated_edit_output(group: RepeatedEditGroup) -> RepeatedEditOutput {
  RepeatedEditOutput {
    files: group.files,
    removed: group.removed,
    added: group.added,
  }
}

fn interdiff_output(interdiff: &Interdiff, short: &ShortIds) -> InterdiffOutput {
  let id = |id: &Option<String>| id.as_deref().map(|id| short.get(id).to_string());
  InterdiffOutput {
    reviewed_base: short.get(&interdiff.reviewed_base).to_string(),
    reviewed_tip: short.get(&interdiff.reviewed_tip).to_string(),
    commits: interdiff
      .commits
      .iter()
      .map(|commit| {
        let changes = commit.changes.clone().unwrap_or_default();
        PairedCommitOutput {
          id: id(&commit.id),
          reviewed_id: id(&commit.reviewed_id),
          subject: commit.subject.clone(),
          state: commit.state,
          new_only: changes.new_only,
          reviewed_only: changes.reviewed_only,
          message_change: commit.message_change.clone(),
        }
      })
      .collect(),
    scope: interdiff.scope.as_ref().map(|scope| RangeOutput {
      base: short.get(&scope.base).to_string(),
      head: short.get(&scope.head).to_string(),
      carried: scope.carried,
    }),
  }
}
