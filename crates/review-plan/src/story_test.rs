use crate::ReviewPlan;
use crate::steps::{StepSource, TitleSource};
use crate::story::{MentionSide, MentionTarget, StoryCommit, StoryPlace};
use crate::test_support::plan_of_range;
use domain_types::commit_body::CommitBodyPartKind;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// A mention as a test reads it: its text, its UTF-16 range, and what it names (`unit:<key>`, `file:<path>`, `-` for
/// nothing) with its step
type MentionWords = (String, (u32, u32), String);

/// The mentions of the story of the commit of `subject` (`MentionWords`)
fn mentions(plan: &ReviewPlan, subject: &str) -> Vec<MentionWords> {
  story(plan, subject)
    .mentions
    .iter()
    .map(|mention| {
      let target = match &mention.target {
        Some(MentionTarget::Unit { unit, step, .. }) => {
          let key = plan.units.iter().find(|it| it.id == *unit).and_then(|it| it.key.clone()).unwrap_or_default();
          format!("unit:{key} step {step:?}")
        }
        Some(MentionTarget::File { path, step }) => format!("file:{path} step {step:?}"),
        None => "-".to_string(),
      };
      (mention.text.clone(), (mention.start, mention.end), target)
    })
    .collect()
}

fn story<'a>(plan: &'a ReviewPlan, subject: &str) -> &'a StoryCommit {
  plan.steps.commits.iter().find(|commit| commit.title.to_string() == subject).unwrap()
}

/// The facts of a step as a test reads them: its new API by key, its new links between modules by their files
type StepFacts<'a> = (Vec<&'a str>, Vec<(&'a str, &'a str)>);

fn words(text: &str, (start, end): (u32, u32), target: &str) -> MentionWords {
  (text.to_string(), (start, end), target.to_string())
}

/// A body names a declaration of the range by its name in backticks, by `Owner.member` bare, and a changed file by its
/// path, a tail of it or its name (a line after it, a leading `@` and the arguments of a call or a type aside); offsets
/// count UTF-16 code units, the backticks included. A plain word in backticks is no name, a name in backticks that
/// resolves nothing (two files of that name) stays a mention without a target, a bare one is dropped
#[test]
fn a_body_names_a_declaration_by_its_name_by_owner_member_and_a_file_by_its_path_or_name() {
  let repo = TestRepo::new();
  let manager = |body: &str| format!("package a\n\nclass Manager {{\n  fun open(): Int = {body}\n}}\n");
  let base = repo.create_commit_with_files(
    "Add",
    &[
      ("src/a/Manager.kt", &manager("1")),
      ("src/b/Report.kt", "package b\n\nfun count() = 1\n"),
      ("src/a/Api.kt", "package a\n\nfun first() = 1\n"),
      ("src/b/Api.kt", "package b\n\nfun second() = 1\n"),
    ],
  );
  repo.create_commit_with_files(
    "Manager opens twice\n\nÜ→𝑥 `Manager.open` returns 2 where open() did 1, in src/a/Manager.kt and `Report.kt`.\n\nThe `changes` stay; `Nowhere.at` and ToolWindow.show name nothing; Manager.open, again.\n\nSo do `a/Manager.kt`, `Manager.kt:4`, `@Manager`, `Manager<T>`, `open(path, mode)`; not `Api.kt`.",
    &[("src/a/Manager.kt", &manager("2"))],
  );
  let head = repo.create_commit_with_files(
    "Report counts twice\n\nThe report counts every item twice, as `count()` asks.",
    &[
      ("src/b/Report.kt", "package b\n\nfun count() = 2\n"),
      ("src/a/Api.kt", "package a\n\nfun first() = 2\n"),
      ("src/b/Api.kt", "package b\n\nfun second() = 2\n"),
    ],
  );
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(plan.steps.source, StepSource::Commits);
  assert_eq!(
    mentions(&plan, "Manager opens twice"),
    [
      // "Ü→𝑥 " is 5 code units: the surrogate pair of 𝑥 counts twice
      words("Manager.open", (5, 19), "unit:a.Manager.open step Some(0)"),
      words("src/a/Manager.kt", (53, 69), "file:src/a/Manager.kt step Some(0)"),
      words("Report.kt", (74, 85), "file:src/b/Report.kt step Some(1)"),
      words("Nowhere.at", (20, 32), "-"),
      words("Manager.open", (67, 79), "unit:a.Manager.open step Some(0)"),
      words("a/Manager.kt", (6, 20), "file:src/a/Manager.kt step Some(0)"),
      words("Manager.kt:4", (22, 36), "file:src/a/Manager.kt step Some(0)"),
      // a type whose member changed: its file
      words("Manager", (38, 48), "file:src/a/Manager.kt step Some(0)"),
      words("Manager", (50, 62), "file:src/a/Manager.kt step Some(0)"),
      words("open", (64, 82), "unit:a.Manager.open step Some(0)"),
      // src/a/Api.kt and src/b/Api.kt
      words("Api.kt", (88, 96), "-"),
    ]
  );
  assert_eq!(
    story(&plan, "Manager opens twice")
      .mentions
      .iter()
      .map(|mention| (mention.block, mention.item))
      .collect::<Vec<_>>(),
    [(0, 0), (0, 0), (0, 0), (1, 0), (1, 0), (2, 0), (2, 0), (2, 0), (2, 0), (2, 0), (2, 0)]
  );
  // a declaration after the commits: the new side, the line of its name
  let Some(MentionTarget::Unit { side, line, path, name, .. }) = &story(&plan, "Manager opens twice").mentions[0].target else {
    panic!("{:?}", story(&plan, "Manager opens twice").mentions[0]);
  };
  assert_eq!((side, *line, path.as_str(), name.as_str()), (&MentionSide::New, Some(4), "src/a/Manager.kt", "open"));
  assert_eq!(mentions(&plan, "Report counts twice"), [words("count", (39, 48), "unit:b.count step Some(1)")]);
  // a story of several commits keeps the order of its author: each part is told with its commit
  assert!(plan.steps.commits.iter().flat_map(|commit| &commit.parts).all(|part| part.told == StoryPlace::Commit));
}

/// A possessive and the dash of a sentence are prose after a name, and a run of names one space apart that names
/// nothing whole links each name that resolves at its own range
#[test]
fn a_name_before_a_possessive_or_a_dash_and_each_name_of_a_run_is_a_mention() {
  let repo = TestRepo::new();
  let splash = |body: &str| format!("package a\n\nclass SplashScreen {{\n  fun hide(): Int = {body}\n}}\n");
  let frame = |body: &str| format!("package a\n\nclass Frame {{\n  fun paint(): Int = {body}\n}}\n");
  let base = repo.create_commit_with_files("Add", &[("src/a/SplashScreen.kt", &splash("1")), ("src/a/Frame.kt", &frame("1"))]);
  let head = repo.create_commit_with_files(
    "The splash hides at once\n\nSplashScreen.hide's frame; Frame.paint— twice.\n\nThen Frame.paint SplashScreen.hide run.",
    &[("src/a/SplashScreen.kt", &splash("2")), ("src/a/Frame.kt", &frame("2"))],
  );
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(
    mentions(&plan, "The splash hides at once"),
    [
      words("SplashScreen.hide", (0, 17), "unit:a.SplashScreen.hide step Some(0)"),
      words("Frame.paint", (27, 38), "unit:a.Frame.paint step Some(0)"),
      words("Frame.paint", (5, 16), "unit:a.Frame.paint step Some(0)"),
      words("SplashScreen.hide", (17, 34), "unit:a.SplashScreen.hide step Some(0)"),
    ]
  );
}

/// A name two declarations of the range share resolves to the one among the stops of the commit's own step, the one
/// that lands of a removal and an addition; from a commit of neither step it resolves nothing, unless its owner tells
/// them apart
#[test]
fn a_name_two_declarations_share_resolves_to_the_one_of_its_step_else_to_nothing() {
  let repo = TestRepo::new();
  let api = |package: &str, value: &str| format!("package {package}\n\nfun open(): Int = {value}\n");
  let with_close = |text: String| format!("{text}\nfun close(): Int = 0\n");
  let base = repo.create_commit_with_files(
    "Add",
    &[
      ("src/a/Api.kt", &with_close(api("a", "1"))),
      ("src/b/Api.kt", &api("b", "1")),
      ("src/c/Other.kt", "package c\n\nfun other() = 1\n"),
    ],
  );
  repo.create_commit_with_files("The a open gives 2\n\nThe `open()` of a gives 2 now.", &[("src/a/Api.kt", &with_close(api("a", "2")))]);
  repo.create_commit_with_files("The b open gives 3\n\nThe `open()` of b gives 3 now.", &[("src/b/Api.kt", &api("b", "3"))]);
  repo.create_commit_with_files(
    "Other gives 2\n\nUnlike `open()` and `b.open`, the `other()` gives 2.",
    &[("src/c/Other.kt", "package c\n\nfun other() = 2\n")],
  );
  let head = repo.create_commit_with_files(
    "Close moves to b\n\nThe `close()` of a is the one of b now.",
    &[("src/a/Api.kt", &api("a", "2")), ("src/b/Api.kt", &with_close(api("b", "3")))],
  );
  let plan = plan_of_range(&repo, &base, &head);
  // the step of the commit shows neither close: the step of `open` shows each, the one that lands its target
  assert_eq!(mentions(&plan, "Close moves to b"), [words("close", (4, 13), "unit:b.close step Some(1)")]);
  assert_eq!(mentions(&plan, "The a open gives 2"), [words("open", (4, 12), "unit:a.open step Some(0)")]);
  assert_eq!(mentions(&plan, "The b open gives 3"), [words("open", (4, 12), "unit:b.open step Some(1)")]);
  assert_eq!(
    mentions(&plan, "Other gives 2"),
    [
      words("open", (7, 15), "-"),
      words("b.open", (20, 28), "unit:b.open step Some(1)"),
      words("other", (34, 43), "unit:c.other step Some(2)"),
    ]
  );
}

/// The body of one commit whose sections name the code it changes tells its steps: a step per labelled section, with the
/// concern it names, titled by its label; a paragraph that names the code of a section in that section's step, the
/// lead and a section that names nothing before the steps
#[test]
fn the_body_of_one_commit_tells_a_step_by_each_section_that_names_its_code_titled_by_its_label_the_lead_and_the_rest_in_the_prologue() {
  let repo = TestRepo::new();
  let (store, reader) = ("src/store/Store.kt", "src/read/Reader.kt");
  let base = repo.create_commit_with_files(
    "Add",
    &[
      (store, "package store\n\nfun load(key: String): Int = 1\n"),
      (reader, "package read\n\nimport store.load\n\nfun read() = load(\"k\")\n"),
      ("src/time/Clock.kt", "package time\n\nfun tick() = 1\n"),
    ],
  );
  let head = repo.create_commit_with_files(
    "A load names its scope\n\nA load of the store names the scope it reads, so a reader can ask for one alone.\n\nThe `tick()` of the clock gives 2.\n\nNothing else changes.\n\nThe scope:\n- `load()` of src/store/Store.kt takes it\n- `read()` passes 2\n\nThe clock:\n- `tick()` gives 2\n\nTests:\n- none, by hand",
    &[
      (store, "package store\n\nfun load(key: String, scope: Int): Int = scope\n"),
      (reader, "package read\n\nimport store.load\n\nfun read() = load(\"k\", 2)\n"),
      ("src/time/Clock.kt", "package time\n\nfun tick() = 2\n"),
    ],
  );
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(plan.steps.source, StepSource::Message);
  let paths: Vec<&[String]> = plan.steps.steps.iter().map(|step| step.paths.as_slice()).collect();
  assert_eq!(paths, [&[store.to_string(), reader.to_string()][..], &["src/time/Clock.kt".to_string()][..]]);
  // the lead in the overview; the run of no label cut where its place changes: the paragraph that names the clock in
  // its step, the one that names nothing in the overview; the sections of the scope and of the clock in their steps,
  // each the title of its step; the tests in the overview
  let story = story(&plan, "A load names its scope");
  let parts: Vec<(u32, u32, CommitBodyPartKind, StoryPlace, bool)> = story.parts.iter().map(|part| (part.start, part.end, part.kind, part.told, part.titled)).collect();
  let (overview, scope, clock) = (StoryPlace::Overview, StoryPlace::Step { step: 0 }, StoryPlace::Step { step: 1 });
  assert_eq!(
    parts,
    [
      (0, 1, CommitBodyPartKind::Lead, overview, false),
      (1, 2, CommitBodyPartKind::Run, clock, false),
      (2, 3, CommitBodyPartKind::Run, overview, false),
      (3, 5, CommitBodyPartKind::Section, scope, true),
      (5, 7, CommitBodyPartKind::Section, clock, true),
      (7, 9, CommitBodyPartKind::Section, overview, false),
    ]
  );
  // each step is titled by the label of the section it tells, as the author named its task
  let titles: Vec<(Option<&str>, TitleSource)> = plan
    .steps
    .steps
    .iter()
    .map(|step| (step.title.as_ref().map(|title| title.text.as_str()), step.title_source))
    .collect();
  assert_eq!(titles, [(Some("The scope"), TitleSource::Label), (Some("The clock"), TitleSource::Label)]);
  // a line marks the stops it names where it is told: the clock by the paragraph and the item of its step
  let marks: Vec<Vec<(u32, Option<u32>)>> = plan.steps.steps[1]
    .stops
    .iter()
    .map(|stop| stop.marks.iter().map(|mark| (mark.block, mark.step)).collect())
    .collect();
  assert_eq!(marks, [[(1, Some(1)), (6, Some(1))]]);
  // two tasks are a story told in steps
  assert!(plan.steps.told);
  // the body is told once, though both steps list its commit
  assert_eq!(plan.steps.commits.len(), 1);
}

/// A step lists the public declarations it adds, to a file that was there (a property of a constructor too) and in a
/// new one, and the link between two modules whose first use its stops show, each in one step
#[test]
fn a_step_lists_the_new_api_and_the_new_links_between_modules_it_holds() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add",
    &[
      ("app/BUILD.bazel", "# app\n"),
      ("store/BUILD.bazel", "# store\n"),
      ("store/src/b/Store.kt", "package b\n\nclass Store {\n  fun size(): Int = 0\n}\n"),
      ("app/src/a/User.kt", "package a\n\nclass User\n"),
    ],
  );
  repo.create_commit_with_files(
    "The store loads\n\nThe store gets a load of its own, for the user.",
    &[("store/src/b/Store.kt", "package b\n\nclass Store {\n  fun size(): Int = 0\n\n  fun load(): Int = 1\n}\n")],
  );
  repo.create_commit_with_files(
    "The user keeps a store\n\nThe user holds the store it loads from.",
    &[("app/src/a/User.kt", "package a\n\nimport b.Store\n\nclass User(val store: Store)\n")],
  );
  let head = repo.create_commit_with_files(
    "A session of the app\n\nThe app opens a session per user.",
    &[("app/src/a/Session.kt", "package a\n\nclass Session\n")],
  );
  let plan = plan_of_range(&repo, &base, &head);
  let facts: Vec<StepFacts> = plan
    .steps
    .steps
    .iter()
    .map(|step| {
      let links = step
        .module_edges
        .iter()
        .flat_map(|edge| &edge.links)
        .map(|&link| (plan.links[link as usize].from.as_str(), plan.links[link as usize].to.as_str()))
        .collect();
      (step.new_api.iter().map(String::as_str).collect(), links)
    })
    .collect();
  assert_eq!(
    facts,
    [
      (vec!["b.Store.load"], vec![]),
      (vec!["a.User.store"], vec![("app/src/a/User.kt", "store/src/b/Store.kt")]),
      (vec!["a.Session"], vec![]),
    ]
  );
}

/// Each language names a member its own way: Rust by its module path, a crate the key does not hold aside, or as
/// `Type.member`; TypeScript as `Type#member` or `Type.member`, its key led by the path of its file; a declaration of a
/// new or a deleted file by the summary of the file, a type whose members changed their file, and a name in backticks a
/// file is named for that file, a bare one no file
#[test]
fn a_name_resolves_by_the_separators_of_its_language() {
  let repo = TestRepo::new();
  let rust = |value: &str| format!("pub struct Body;\n\nimpl Body {{\n  pub fn shown(&self) -> u32 {{\n    {value}\n  }}\n}}\n\npub fn read() -> u32 {{\n  {value}\n}}\n");
  let script = |value: &str| format!("export class Store {{\n  load(): number {{\n    return {value}\n  }}\n}}\n");
  let base = repo.create_commit_with_files(
    "Add",
    &[
      ("body/Cargo.toml", "[package]\nname = \"body\"\n"),
      ("body/src/lib.rs", "pub mod text;\n"),
      ("body/src/text.rs", &rust("1")),
      ("web/src/store.ts", &script("1")),
      ("web/src/old-title.ts", "export function formatTitle(): string {\n  return \"a\"\n}\n"),
    ],
  );
  repo.git(&["rm", "-q", "web/src/old-title.ts"]);
  repo.create_commit_with_files(
    "The page measures\n\nA new `useMeasuredHeights` at line 3 of its file, and a `ReviewPrologue` above the cards; `formatTitle()` goes. A bare ReviewPrologue or YouTrack names no file.",
    &[
      (
        "web/src/use-measured-heights.ts",
        "import { x } from \"./x\"\n\nexport function useMeasuredHeights(): number {\n  return x\n}\n",
      ),
      ("web/src/review-prologue.ts", "export const PROLOGUE = 1\n"),
      ("web/src/youtrack.ts", "export const TRACKER = 1\n"),
    ],
  );
  repo.create_commit_with_files(
    "The body shows 2\n\nThe `text::Body::shown` and `body::text::read` give 2, and so does Body.shown.",
    &[("body/src/text.rs", &rust("2"))],
  );
  let head = repo.create_commit_with_files(
    "The store loads 2\n\nThe `Store#load` gives 2, and so does `Store.load()` of `Store`.",
    &[("web/src/store.ts", &script("2"))],
  );
  let plan = plan_of_range(&repo, &base, &head);
  let targets = |subject: &str| mentions(&plan, subject).into_iter().map(|(text, _, target)| (text, target)).collect::<Vec<_>>();
  let pair = |text: &str, target: &str| (text.to_string(), target.to_string());
  assert_eq!(
    targets("The body shows 2"),
    [
      pair("text::Body::shown", "unit:text.Body.shown step Some(1)"),
      pair("body::text::read", "unit:text.read step Some(1)"),
      pair("Body.shown", "unit:text.Body.shown step Some(1)"),
    ]
  );
  // a declaration of a new file and of a deleted one, by the summary of the file at its line; a name in backticks the
  // file is named for, which a bare one (a product name in prose) is not; the name of the declaration, to select in the
  // file
  let places: Vec<(MentionSide, Option<u32>, &str, &str)> = story(&plan, "The page measures")
    .mentions
    .iter()
    .filter_map(|mention| match &mention.target {
      Some(MentionTarget::Unit { side, line, path, name, .. }) => Some((*side, *line, path.as_str(), name.as_str())),
      _ => None,
    })
    .collect();
  assert_eq!(
    places,
    [
      (MentionSide::New, Some(3), "web/src/use-measured-heights.ts", "useMeasuredHeights"),
      (MentionSide::Old, Some(1), "web/src/old-title.ts", "formatTitle")
    ]
  );
  assert_eq!(
    targets("The page measures"),
    [
      pair("useMeasuredHeights", "unit: step Some(0)"),
      pair("ReviewPrologue", "file:web/src/review-prologue.ts step Some(0)"),
      pair("formatTitle", "unit: step Some(0)"),
    ]
  );
  assert_eq!(
    targets("The store loads 2"),
    [
      pair("Store#load", "unit:web/src/store.ts.Store.load step Some(2)"),
      pair("Store.load", "unit:web/src/store.ts.Store.load step Some(2)"),
      // a type whose member changed: its file, though not named for it
      pair("Store", "file:web/src/store.ts step Some(2)"),
    ]
  );
}

/// A mark of a stop as a test reads it: its commit, its block and the step that tells it
type MarkWords<'a> = (&'a str, u32, Option<u32>);

/// A line of a told body marks the stops it names, each where it is told: a declaration the stop of its unit, a file
/// every stop of its path in the step of its target, once per block; a follow-up, folded under its step, marks nothing
#[test]
fn a_line_of_a_told_body_marks_the_stops_it_names_where_it_is_told() {
  let repo = TestRepo::new();
  let manager = |open: &str, close: &str| format!("package a\n\nclass Manager {{\n  fun open(): Int = {open}\n\n  fun close(): Int = {close}\n}}\n");
  let frame = |paint: &str| format!("package a\n\nclass Frame {{\n  fun paint(): Int = {paint}\n}}\n");
  let base = repo.create_commit_with_files("Add", &[("src/a/Manager.kt", &manager("1", "0")), ("src/a/Frame.kt", &frame("1"))]);
  repo.create_commit_with_files(
    "Manager opens twice\n\nThe `Manager.open` returns 2, and src/a/Frame.kt paints twice; the close gives 5.",
    &[("src/a/Manager.kt", &manager("2", "5")), ("src/a/Frame.kt", &frame("2"))],
  );
  let head = repo.create_commit_with_files("Manager opens thrice\n\nThe `Manager.open` returns 3 now.", &[("src/a/Manager.kt", &manager("3", "5"))]);
  let plan = plan_of_range(&repo, &base, &head);
  let [step] = plan.steps.steps.as_slice() else { panic!("{:?}", plan.steps.steps) };
  let first = story(&plan, "Manager opens twice").id.clone();
  let name_of = |id: &str| plan.units.iter().find(|unit| unit.id == id).and_then(|unit| unit.name.clone()).unwrap();
  let mut marks: Vec<(String, Vec<MarkWords<'_>>)> = step
    .stops
    .iter()
    .map(|stop| (name_of(&stop.unit), stop.marks.iter().map(|mark| (mark.commit.as_str(), mark.block, mark.step)).collect()))
    .collect();
  marks.sort();
  // `close`, in the file of `Manager.open`, is not what the line names
  assert_eq!(
    marks,
    [
      ("close".to_string(), vec![]),
      ("open".to_string(), vec![(first.as_str(), 0, Some(0))]),
      ("paint".to_string(), vec![(first.as_str(), 0, Some(0))])
    ]
  );
}

/// A part of a story as a test reads it: its blocks, whether folded, and the removed blocks it shows (hunk, the block
/// before which, `None` at its end)
type ChangedPart = ((u32, u32), bool, Vec<(u32, Option<u32>)>);

/// The parts of the story of `subject` (`ChangedPart`)
fn changed_parts(plan: &ReviewPlan, subject: &str) -> Vec<ChangedPart> {
  story(plan, subject)
    .parts
    .iter()
    .map(|part| {
      let removals = part.removals.iter().map(|removal| (removal.hunk, removal.before)).collect();
      ((part.start, part.end), part.folded, removals)
    })
    .collect()
}

/// A body rewritten since the review marks it where its story shows it: a part that holds an added block or shows
/// removed ones is not folded, a verification section neither; the removed blocks stand in one part, before the block
/// they stood before, or at the end of the part of the block they followed. A commit shown alone with its reviewed one
/// tells the same, and a body that did not change marks nothing
#[test]
fn a_changed_part_of_a_body_is_marked_and_unfolded_a_verification_too() {
  let repo = TestRepo::new();
  repo.create_commit_with_timestamp("Add readme", "README.md", "readme\n", Some(1_700_000_000));
  repo.git(&["switch", "-q", "-c", "jane/virtual/auth"]);
  let reviewed_body = "(auth) Add logout\n\nThe logout clears the session of the user and every token it holds.\n\nWhy: the token outlives the tab, so a shared machine keeps a session open.\n\nTests: the logout test passes.\n\nEvidence: the old session is gone after a restart.";
  let reviewed = repo.create_commit_with_timestamp(reviewed_body, "logout.rs", "fn logout() {}\n", Some(1_700_000_100));
  crate::test_support::mark(&repo);
  let now = "(auth) Add logout\n\nThe logout clears the session of the user and every token it holds.\n\nWhy: the token outlives the tab, so a shared machine keeps a session open.\n\nTests: the logout test and the restart test pass.";
  let reworded = repo.commit_with(&["--amend", "-m", now], &[("GIT_COMMITTER_DATE", "1700000300 +0000")]);

  let plan = crate::test_support::plan_of(&repo, None, &crate::test_support::branch(&repo, false)).unwrap();
  let title = "Add logout";
  // the blocks now: the lead, Why and its paragraph, Tests and its paragraph
  assert_eq!(
    changed_parts(&plan, title),
    [((0, 1), false, vec![]), ((1, 3), false, vec![]), ((3, 5), false, vec![(0, Some(4))])]
  );
  let change = story(&plan, title).change.as_ref().unwrap();
  assert_eq!(
    change
      .hunks
      .iter()
      .map(|hunk| (hunk.start, hunk.added.blocks.len(), hunk.removed.blocks.len()))
      .collect::<Vec<_>>(),
    [(4, 1, 3)]
  );

  let alone = crate::ReviewPlanParams {
    subject: crate::ReviewSubject::Commit {
      id: reworded,
      reviewed: Some(reviewed[..12].to_string()),
    },
    ..crate::test_support::branch(&repo, false)
  };
  let alone = crate::test_support::plan_of(&repo, None, &alone).unwrap();
  assert_eq!(story(&alone, title).change.as_ref(), Some(change));
  assert_eq!(changed_parts(&alone, title), changed_parts(&plan, title));

  // without its reviewed commit, and of a commit git does not have, the story marks nothing and folds as its grammar
  for reviewed in [None, Some("0123456789ab".to_string())] {
    let params = crate::ReviewPlanParams {
      subject: crate::ReviewSubject::Commit { id: repo.head(), reviewed },
      ..crate::test_support::branch(&repo, false)
    };
    let plain = crate::test_support::plan_of(&repo, None, &params).unwrap();
    assert_eq!(story(&plain, title).change, None);
    assert_eq!(changed_parts(&plain, title), [((0, 1), false, vec![]), ((1, 3), false, vec![]), ((3, 5), true, vec![])]);
  }
}
