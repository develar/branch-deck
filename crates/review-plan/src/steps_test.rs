use crate::ReviewPlan;
use crate::brief::{ContractKind, Substitution, contract_kind, shared_issue};
use crate::fan_in::NO_ACCIO;
use crate::fan_in_test::Counts;
use crate::files::MechanicalKind;
use crate::prose::{ProseChange, ProseForm, utf16_slice};
use crate::steps::{
  CommitRole, ConcernsReason, JoinedUnit, ProofFile, ReviewStep, ShownElsewhere, StepSource, StepStop, StepTest, TestWay, commit_title, join_units, tells_a_story,
};
use crate::test_support::{plan_of, plan_of_range, range};
use crate::units::{ReviewUnit, UnitBlock, UnitKind, UnitSlice};
use domain_types::commit_body::CommitBodyBlock;
use file_diffs::accio::{ReachingTest, TestClass, TestReach};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// A step as a test reads it: the subjects of its commits with their roles, and the units of its stops by name or path
type StepWords = (Vec<(String, CommitRole)>, Vec<String>);

/// Each step of `plan` (`StepWords`)
fn steps(plan: &ReviewPlan) -> Vec<StepWords> {
  plan
    .steps
    .steps
    .iter()
    .map(|step| {
      let commits = step.commits.iter().map(|commit| (commit.title.clone(), commit.role)).collect();
      let stops = step
        .stops
        .iter()
        .map(|stop| {
          let unit = plan.units.iter().find(|unit| unit.id == stop.unit).unwrap();
          let name = unit.name.clone().unwrap_or_else(|| stop.path.clone());
          match (&stop.uses, &stop.blocks) {
            (Some(uses), _) => format!("{name} uses {uses}"),
            (None, Some(blocks)) => format!("{name} {blocks:?}"),
            (None, None) => name,
          }
        })
        .collect();
      (commits, stops)
    })
    .collect()
}

fn commits(of: &[(&str, CommitRole)]) -> Vec<(String, CommitRole)> {
  of.iter().map(|(subject, role)| ((*subject).to_string(), *role)).collect()
}

fn names(of: &[&str]) -> Vec<String> {
  of.iter().map(|name| (*name).to_string()).collect()
}

fn prose_plan(old_spec: &str, new_spec: &str, old_guide: &str, new_guide: &str) -> ReviewPlan {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add prose", &[("spec/commands.spec.md", old_spec), ("docs/guide.md", old_guide)]);
  let head = repo.create_commit_with_files("Rewrite prose", &[("spec/commands.spec.md", new_spec), ("docs/guide.md", new_guide)]);
  plan_of_range(&repo, &base, &head)
}

fn deleted_word_count(plan: &ReviewPlan, word: &str) -> usize {
  plan
    .brief
    .contract
    .iter()
    .flat_map(|entry| &entry.paragraphs)
    .map(|paragraph| match &paragraph.change {
      ProseChange::Changed { deleted, .. } => deleted.iter().filter(|deletion| deletion.text == word).count(),
      _ => 0,
    })
    .sum()
}

const API: &str = "package a\n\nfun open(path: String): Int = 1\n\nfun close(): Int = 2\n";

/// The commits that tell their steps are the steps, each with the changes most of whose lines it wrote, in the order of
/// the story; every block of the range in one stop
#[test]
fn the_commits_that_tell_their_steps_are_the_steps_each_with_the_code_it_wrote() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add", &[("src/a/Api.kt", API), ("src/c/Other.kt", "package c\n\nfun other() = 1\n")]);
  repo.create_commit_with_files(
    "Open takes a mode\n\nThe open of a file names how it opens, so a caller can ask for a read alone.",
    &[(
      "src/a/Api.kt",
      &API.replace("fun open(path: String): Int = 1", "fun open(path: String, mode: Int): Int = mode"),
    )],
  );
  let head = repo.create_commit_with_files(
    "Other counts twice\n\nThe other file counts every item twice, as the report expects.",
    &[("src/c/Other.kt", "package c\n\nfun other() = 2\n")],
  );
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(plan.steps.source, StepSource::Commits);
  assert_eq!(
    steps(&plan),
    [
      (commits(&[("Open takes a mode", CommitRole::Starts)]), names(&["open"])),
      (commits(&[("Other counts twice", CommitRole::Starts)]), names(&["other"])),
    ]
  );
  // the body of each commit once, in the story of the commits
  let bodies: Vec<(&str, &[CommitBodyBlock])> = plan.steps.commits.iter().map(|commit| (commit.title.as_str(), commit.body.blocks.as_slice())).collect();
  assert_eq!(
    bodies,
    [
      (
        "Open takes a mode",
        &[CommitBodyBlock::Paragraph {
          lines: vec!["The open of a file names how it opens, so a caller can ask for a read alone.".to_string()]
        }][..]
      ),
      (
        "Other counts twice",
        &[CommitBodyBlock::Paragraph {
          lines: vec!["The other file counts every item twice, as the report expects.".to_string()]
        }][..]
      ),
    ]
  );
  let coverage = plan.steps.coverage;
  assert_eq!((coverage.missing, coverage.twice, coverage.in_stops), (0, 0, 2), "{coverage:?}");
}

/// The review names a commit by its subject without its `(prefix)` and the issue key the brief names, with a `:`, `,` or
/// dash after it, in the case its author wrote it: the row of its step, its opening, its card and the contents agree;
/// the step of a commit takes its title, the plan the short ids of a range
#[test]
fn a_commit_is_titled_by_its_subject_without_the_issue_key_of_the_brief_in_the_case_of_its_author() {
  assert_eq!(commit_title("IJAI-1 the open asks the layout", Some("IJAI-1")), "the open asks the layout");
  assert_eq!(commit_title("IJAI-1: The open asks", Some("IJAI-1")), "The open asks");
  assert_eq!(commit_title("(layout) IJAI-1 — a splash shows", Some("IJAI-1")), "a splash shows");
  assert_eq!(commit_title("IJAI-12 another issue", Some("IJAI-1")), "IJAI-12 another issue");
  assert_eq!(commit_title("IJAI-1", Some("IJAI-1")), "IJAI-1");
  // of the keys a subject starts with, the issue of the brief goes, the others stay
  assert_eq!(
    commit_title("IJAI-1184 IJAI-5640 Air records the links", Some("IJAI-5640")),
    "IJAI-1184 Air records the links"
  );
  assert_eq!(commit_title("the open asks", None), "the open asks");

  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add", &[("src/a/Api.kt", API), ("src/c/Other.kt", "package c\n\nfun other() = 1\n")]);
  repo.create_commit_with_files(
    "IJAI-1 open takes a mode\n\nThe open of a file names how it opens, so a caller can ask for a read alone.",
    &[(
      "src/a/Api.kt",
      &API.replace("fun open(path: String): Int = 1", "fun open(path: String, mode: Int): Int = mode"),
    )],
  );
  let head = repo.create_commit_with_files(
    "IJAI-1: other counts twice\n\nThe other file counts every item twice, as the report expects.",
    &[("src/c/Other.kt", "package c\n\nfun other() = 2\n")],
  );
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(plan.brief.issue.as_deref(), Some("IJAI-1"));
  let titles: Vec<(Option<&str>, &str)> = plan.steps.steps.iter().map(|step| (step.title.as_deref(), step.commits[0].title.as_str())).collect();
  assert_eq!(
    titles,
    [(Some("open takes a mode"), "open takes a mode"), (Some("other counts twice"), "other counts twice")]
  );
  let told: Vec<&str> = plan.steps.commits.iter().map(|commit| commit.title.as_str()).collect();
  assert_eq!(told, ["open takes a mode", "other counts twice"]);
  assert_eq!(plan.title, format!("{}..{}", &base[..8], &head[..8]));
}

/// A commit that rewrites the code an earlier step wrote follows that step up, its lead and how it was checked
/// (`Tests:`) no story of its own, and so does one whose lines all landed
/// in the changes of another; a commit without a body shows where its lines are, and the changes only such commits
/// wrote are the step "Changes without a story"
#[test]
fn a_round_follows_its_step_up_and_a_commit_without_a_body_shows_where_its_lines_are() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add", &[("src/a/Api.kt", API), ("src/c/Other.kt", "package c\n\nfun other() = 1\n")]);
  let first = "fun open(path: String, mode: Int): Int {\n  val read = mode == 1\n  val write = mode == 2\n  return if (read) 1 else 2\n}";
  repo.create_commit_with_files(
    "Open takes a mode\n\nThe open of a file names how it opens, so a caller can ask for a read alone.",
    &[("src/a/Api.kt", &API.replace("fun open(path: String): Int = 1", first))],
  );
  let again = "fun open(path: String, mode: Int): Int {\n  val reading = mode == 1\n  val writing = mode == 2\n  return if (reading) 1 else 2\n}";
  repo.create_commit_with_files(
    "Open names its flags again\n\nThe review asked for the names of the flags to say what they hold.\n\nTests:\n- `ApiTest` opens to read\n- `ApiTest` opens to write\n- `ApiTest` opens in no mode",
    &[("src/a/Api.kt", &API.replace("fun open(path: String): Int = 1", again))],
  );
  let head = repo.create_commit_with_files("wip", &[("src/c/Other.kt", "package c\n\nfun other() = 2\n")]);
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(
    steps(&plan),
    [
      (
        commits(&[("Open takes a mode", CommitRole::Starts), ("Open names its flags again", CommitRole::FollowsUp)]),
        names(&["open"])
      ),
      (commits(&[("wip", CommitRole::NoStory)]), names(&["other"])),
    ]
  );
  assert_eq!(plan.steps.steps[1].id, "no-story");
  // a commit without a body has no story
  let told: Vec<&str> = plan.steps.commits.iter().map(|commit| commit.title.as_str()).collect();
  assert_eq!(told, ["Open takes a mode", "Open names its flags again"]);
}

/// A commit whose body tells a story of its own (a lead and a labelled part, or more than a few lines) starts its step,
/// though it rewrites the code of an earlier one; the earlier step, none of whose lines lands, names the commit that
/// took it out
#[test]
fn a_commit_with_a_story_of_its_own_starts_a_step_though_it_rewrites_the_code_of_another() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add", &[("src/a/Api.kt", API)]);
  let first = "fun open(path: String, mode: Int): Int {\n  val read = mode == 1\n  val write = mode == 2\n  return if (read) 1 else 2\n}";
  repo.create_commit_with_files(
    "Open takes a mode\n\nThe open of a file names how it opens, so a caller can ask for a read alone.",
    &[("src/a/Api.kt", &API.replace("fun open(path: String): Int = 1", first))],
  );
  let again = "fun open(path: String, mode: Int): Int {\n  val cached = mode == 1\n  val fresh = mode == 2\n  return if (cached) 3 else 4\n}";
  let head = repo.create_commit_with_files(
    "Open reads the cache first\n\nThe open asks the cache before the disk, so a second open is free.\n\nThe cause:\n- every open read the disk\n\nTests:\n- `ApiTest` opens twice",
    &[("src/a/Api.kt", &API.replace("fun open(path: String): Int = 1", again))],
  );
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(
    steps(&plan),
    [
      (commits(&[("Open takes a mode", CommitRole::Starts)]), names(&[])),
      (commits(&[("Open reads the cache first", CommitRole::Starts)]), names(&["open"])),
    ]
  );
  let taken_out: Vec<Option<&str>> = plan.steps.steps.iter().map(|step| step.taken_out_by.as_deref()).collect();
  assert_eq!(taken_out, [Some(head.as_str()), None]);
  assert!(plan.steps.told);
}

/// Commits without a body make the concerns the steps, and so does a range of one commit; one concern that covers the
/// whole range tells no step
#[test]
fn the_concerns_are_the_steps_when_the_commits_tell_no_story() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add", &[("src/a/Api.kt", API)]);
  repo.create_commit_with_files("fix", &[("src/a/Api.kt", &API.replace("= 1", "= 3"))]);
  let head = repo.create_commit_with_files("again", &[("src/a/Api.kt", &API.replace("= 1", "= 4"))]);
  assert_eq!(plan_of_range(&repo, &base, &head).steps.source, StepSource::Concerns { reason: ConcernsReason::NoStory });
  let one = plan_of_range(&repo, &format!("{head}~1"), &head);
  assert_eq!(
    one.steps.source,
    StepSource::Concerns {
      reason: ConcernsReason::OneCommit
    }
  );
  assert_eq!(one.steps.steps.len(), one.concerns.len());
  // one task that covers the whole range is no story to tell in steps: it would only repeat the range
  assert_eq!((one.concerns.len(), one.steps.told), (1, false));
}

/// A declaration a step adds comes first, and the changes of the step that use it right after it, whatever the order
/// of the files
#[test]
fn a_declaration_a_step_adds_comes_first_and_its_uses_right_after_it() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add",
    &[
      ("src/a/Api.kt", API),
      ("src/b/First.kt", "package b\n\nimport a.open\n\nfun first() = open(\"x\")\n"),
      ("src/b/Second.kt", "package b\n\nfun second() = 2\n"),
    ],
  );
  let head = repo.create_commit_with_files(
    "Allowed decides the open\n\nOne rule decides whether a path may open, and both callers ask it.",
    &[
      ("src/a/Api.kt", &format!("{API}\nfun allowed(path: String): Boolean = path.isNotEmpty()\n")),
      (
        "src/b/First.kt",
        "package b\n\nimport a.allowed\nimport a.open\n\nfun first() = if (allowed(\"x\")) open(\"x\") else 0\n",
      ),
      ("src/b/Second.kt", "package b\n\nimport a.allowed\n\nfun second() = if (allowed(\"y\")) 2 else 0\n"),
    ],
  );
  let plan = plan_of_range(&repo, &base, &head);
  let stops = &steps(&plan)[0].1;
  assert_eq!(stops[0], "allowed", "{stops:?}");
  assert!(
    stops.contains(&"first uses allowed".to_string()) && stops.contains(&"second uses allowed".to_string()),
    "{stops:?}"
  );
}

/// A given test that reaches a declaration: its path, line, the name it reads as, its depth and what its way goes through
fn reaching(path: &str, line: u32, reads_as: &str, depth: u32, class: TestClass) -> ReachingTest {
  ReachingTest {
    path: path.to_string(),
    line,
    declaration: format!("Test.{reads_as}"),
    reads_as: reads_as.to_string(),
    framework: "junit4".to_string(),
    depth,
    via: Vec::new(),
    class,
    through_hub: false,
  }
}

/// A test of the change as a test reads it: its name, its fewest calls to the code, its nearest way and what of the code
/// it reaches
type ProofTest<'a> = (&'a str, u32, TestWay, Vec<&'a str>);

/// The tests of the steps of `plan`, each list sorted: the tests of the change by file, each with its depth, its way and
/// the code it reaches; the code none reaches; the declarations a test reaches
#[derive(Debug, Default, PartialEq, Eq)]
struct Tested<'a> {
  proof: Vec<(&'a str, Vec<ProofTest<'a>>)>,
  unreached: Vec<&'a str>,
  reached: u32,
}

fn tested(plan: &ReviewPlan) -> Tested<'_> {
  fn names(names: &[String]) -> Vec<&str> {
    names.iter().map(String::as_str).collect()
  }
  let mut tested = Tested::default();
  for tests in plan.steps.steps.iter().map(|step| &step.tests) {
    for file in &tests.proof {
      let of_file = file.tests.iter().map(|test| (test.name.as_str(), test.depth, test.way, names(&test.reaches))).collect();
      tested.proof.push((file.path.as_str(), of_file));
    }
    tested.unreached.extend(tests.unreached.iter().map(|it| it.name.as_str()));
    tested.reached += tests.reached;
  }
  tested.proof.sort();
  tested.unreached.sort_unstable();
  tested
}

/// The tests of the change that reach a declaration of the code of a step, as accio answers by its place at the head
/// with the test files of the change given: by their file, each with its nearest way; a test of a given file the change
/// did not touch is none; a declaration no test of the change reaches only when accio walked every way, one it did not
/// unknown; each declaration at the line of its name at the head. Without accio no step knows its tests, and the plan
/// says why
#[test]
fn the_tests_that_reach_the_code_of_a_step_and_the_code_none_reaches() {
  let repo = TestRepo::new();
  let manager = "package a\n\nclass Manager {\n  fun open(): Int = 1\n\n  fun isEmpty(): Boolean = true\n}\n";
  let base = repo.create_commit_with_files(
    "Add",
    &[
      ("src/a/Manager.kt", manager),
      (
        "test/a/ManagerTest.kt",
        "package a\n\nimport org.junit.Test\n\nclass ManagerTest {\n  @Test fun `opens`() = check(Manager().open() == 1)\n}\n",
      ),
    ],
  );
  let head = repo.create_commit_with_files(
    "The manager allows a stored layout\n\nThe open asks the stored layout, and the manager is empty without one.",
    &[
      (
        "src/a/Manager.kt",
        &manager
          .replace("fun open(): Int = 1", "fun open(): Int = 2")
          .replace("= true", "= false")
          .replace("}\n", "\n  fun allowed(id: String): Boolean = id.isNotEmpty()\n}\n"),
      ),
      ("src/a/Frame.kt", "package a\n\nfun paint() = 1\n"),
      (
        "test/a/ManagerTest.kt",
        "package a\n\nimport org.junit.Test\n\nclass ManagerTest {\n  @Test fun `opens`() = check(Manager().open() == 2)\n\n  @Test fun `the project open may show only the stored layout`() = check(Manager().allowed(\"Project\"))\n}\n",
      ),
    ],
  );
  let tests = |tests: Vec<ReachingTest>, complete: bool| TestReach {
    tests,
    complete,
    unavailable: false,
  };
  let index = Counts::of(&[]).with_tests([
    (
      "src/a/Manager.kt:4",
      tests(
        vec![
          reaching("test/a/ManagerTest.kt", 6, "opens", 1, TestClass::Direct),
          // a test of the given file the change did not touch (no test of the range holds its line)
          reaching("test/a/ManagerTest.kt", 40, "an untouched test opens the manager", 1, TestClass::Direct),
        ],
        true,
      ),
    ),
    (
      "src/a/Manager.kt:8",
      tests(
        vec![reaching(
          "test/a/ManagerTest.kt",
          8,
          "the project open may show only the stored layout",
          5,
          TestClass::Product,
        )],
        true,
      ),
    ),
    // accio did not walk every way from `paint`: unknown
    ("src/a/Frame.kt:3", tests(Vec::new(), false)),
  ]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  assert_eq!(index.tested(), ["src/a/Frame.kt:3", "src/a/Manager.kt:4", "src/a/Manager.kt:6", "src/a/Manager.kt:8"]);
  // one query, the test files of the change given
  assert_eq!(index.given(), [vec!["test/a/ManagerTest.kt".to_string()]]);
  assert_eq!(index.revisions(), std::slice::from_ref(&head));
  assert_eq!(
    tested(&plan),
    Tested {
      // the file of the change in each step whose code its tests reach (`open` and `allowed` are two concerns), each test
      // with its way and what it reaches
      proof: vec![
        ("test/a/ManagerTest.kt", vec![("opens", 1, TestWay::Calls, vec!["open"])]),
        (
          "test/a/ManagerTest.kt",
          vec![("the project open may show only the stored layout", 5, TestWay::EndToEnd, vec!["allowed"])]
        ),
      ],
      // no test of the change reaches `isEmpty`, every way walked; `paint` is unknown
      unreached: vec!["isEmpty"],
      reached: 2,
    },
    "{:?}",
    plan.steps
  );
  // the code none reaches at the line of its name at the head, which the review shows
  let unreached: Vec<(&str, u32)> = plan
    .steps
    .steps
    .iter()
    .flat_map(|step| step.tests.unreached.iter().map(|it| (it.path.as_str(), it.line)))
    .collect();
  assert_eq!(unreached, [("src/a/Manager.kt", 6)]);
  assert!(plan.steps.steps.iter().all(|step| step.tests.answered));
  assert_eq!(plan.index_note, None);

  // a place accio finds no declaration at (a line it does not index) answers nothing: no "no test reaches"
  let index = Counts::of(&[]).with_tests([(
    "src/a/Manager.kt:6",
    TestReach {
      unavailable: true,
      ..TestReach::default()
    },
  )]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  assert!(!tested(&plan).unreached.contains(&"isEmpty"), "{:?}", plan.steps);

  // without accio: no step knows its tests
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(tested(&plan), Tested::default());
  assert!(plan.steps.steps.iter().all(|step| !step.tests.answered));
  assert_eq!(plan.index_note.as_deref(), Some(NO_ACCIO));
}

/// A test that reaches two declarations of a step by two ways is listed once, with its fewest calls and its nearest way
#[test]
fn a_test_of_two_ways_is_listed_once_by_its_nearest() {
  let repo = TestRepo::new();
  let manager = "package a\n\nclass Manager {\n  fun open(): Int = 1\n\n  fun close(): Int = 1\n}\n";
  let test = |value: u8| format!("package a\n\nimport org.junit.Test\n\nclass ManagerTest {{\n  @Test fun `opens and closes`() = check(Manager().open() == {value})\n}}\n");
  let base = repo.create_commit_with_files("Add", &[("src/a/Manager.kt", manager), ("test/a/ManagerTest.kt", &test(1))]);
  let head = repo.create_commit_with_files(
    "The manager opens and closes twice\n\nBoth return two.",
    &[("src/a/Manager.kt", &manager.replace("= 1", "= 2")), ("test/a/ManagerTest.kt", &test(2))],
  );
  let opens = |depth, class| TestReach {
    tests: vec![reaching("test/a/ManagerTest.kt", 6, "opens and closes", depth, class)],
    complete: true,
    unavailable: false,
  };
  let index = Counts::of(&[]).with_tests([("src/a/Manager.kt:4", opens(3, TestClass::Product)), ("src/a/Manager.kt:6", opens(2, TestClass::TestCode))]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  assert_eq!(
    tested(&plan).proof,
    [("test/a/ManagerTest.kt", vec![("opens and closes", 2, TestWay::TestCode, vec!["open", "close"])])],
    "{:?}",
    plan.steps
  );
}

/// A range without tests asks accio nothing: no test of the change reaches its code, whatever the index
#[test]
fn a_range_without_tests_asks_nothing_and_no_test_of_it_reaches_its_code() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add", &[("src/a/Frame.kt", "package a\n\nfun paint() = 1\n")]);
  let head = repo.create_commit_with_files("Paint two\n\nThe frame paints two.", &[("src/a/Frame.kt", "package a\n\nfun paint() = 2\n")]);
  let index = Counts::of(&[]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  assert_eq!(index.tested(), Vec::<String>::new());
  assert_eq!(
    tested(&plan),
    Tested {
      unreached: vec!["paint"],
      ..Tested::default()
    }
  );
  assert!(plan.steps.steps.iter().all(|step| step.tests.answered));
}

/// A Rust source of `size` returning `value`, with a fixture of its inline tests returning `prepared` and a test that
/// asserts `asserted`
fn store_source(value: u8, prepared: &str, asserted: u8) -> String {
  format!(
    "pub fn size() -> u8 {{\n  {value}\n}}\n\n#[cfg(test)]\nmod tests {{\n  use super::*;\n\n  fn prepared_store() -> u8 {{\n    {prepared}\n  }}\n\n  #[test]\n  fn size_is_right() {{\n    let _ = prepared_store();\n    assert_eq!(size(), {asserted});\n  }}\n}}\n"
  )
}

/// A fixture of a test (a helper of an inline `mod tests`, a member of a test class) is test code, no code a step asks
/// the tests of; the test the range changes in the same file is its unit
#[test]
fn a_fixture_of_a_test_is_no_code_a_step_asks_the_tests_of() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add", &[("src/store.rs", &store_source(1, "1", 1))]);
  let head = repo.create_commit_with_files("Change the size", &[("src/store.rs", &store_source(2, "2", 2))]);
  let index = Counts::of(&[]).with_tests([(
    "src/store.rs:1",
    TestReach {
      tests: vec![reaching("src/store.rs", 14, "size is right", 1, TestClass::Direct)],
      complete: true,
      unavailable: false,
    },
  )]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  assert_eq!(index.tested(), ["src/store.rs:1"]);
  assert_eq!(
    tested(&plan),
    Tested {
      proof: vec![("src/store.rs", vec![("size is right", 1, TestWay::Calls, vec!["size"])])],
      reached: 1,
      ..Tested::default()
    },
    "{:?}",
    plan.steps
  );
}

/// The declarations at the top of a new file are code of its step, the test functions of a new test file are tests of
/// the change, and the step of those tests proves the step whose code they reach
#[test]
fn a_new_file_is_code_its_tests_are_the_changes_and_their_step_proves_the_other() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add", &[("src/a/Model.kt", "package a\n\nclass Model\n")]);
  repo.create_commit_with_files(
    "The joiner joins the rows of a link\n\nThe joiner runs after the filters on each line.",
    &[(
      "src/a/Joiner.kt",
      "package a\n\nclass Joiner {\n  fun join(rows: List<String>) = rows.joinToString(\"\")\n}\n\nfun canJoin(row: String) = row.isNotEmpty()\n",
    )],
  );
  let head = repo.create_commit_with_files(
    "The tests of the joiner\n\nThe tests join two rows and three.",
    &[(
      "test/a/JoinerTest.kt",
      "package a\n\nimport org.junit.Test\n\nclass JoinerTest {\n  @Test fun `joins two rows`() = check(Joiner().join(listOf(\"a\", \"b\")) == \"ab\")\n}\n",
    )],
  );
  let index = Counts::of(&[]).with_tests([(
    "src/a/Joiner.kt:3",
    TestReach {
      tests: vec![reaching("test/a/JoinerTest.kt", 6, "joins two rows", 1, TestClass::Direct)],
      ..TestReach::default()
    },
  )]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  // the class and the function at the top of the new file; no test file, no member of a type
  assert_eq!(index.tested(), ["src/a/Joiner.kt:3", "src/a/Joiner.kt:7"]);
  let steps = &plan.steps.steps;
  assert_eq!(steps.len(), 2, "{steps:?}");
  assert_eq!(
    steps[0].tests.proof,
    [ProofFile {
      path: "test/a/JoinerTest.kt".to_string(),
      tests: vec![StepTest {
        unit: Some("new:test/a/JoinerTest.kt".to_string()),
        path: "test/a/JoinerTest.kt".to_string(),
        line: 6,
        name: "joins two rows".to_string(),
        depth: 1,
        way: TestWay::Calls,
        reaches: vec!["Joiner".to_string()],
      }],
    }]
  );
  assert_eq!(tested(&plan).unreached, ["canJoin"]);
  assert_eq!((steps[0].proves.as_slice(), steps[1].proves.as_slice()), (&[][..], &[0][..]));
}

/// A file of no language is cut by its blocks: each block in the step of the commit that wrote it; the file has its main
/// row in one step, the step of its most lines
#[test]
fn a_file_of_no_language_is_cut_into_the_steps_of_its_blocks() {
  let repo = TestRepo::new();
  let guide = "# Guide\n\nalpha\n\nbeta\n\ngamma\n\ndelta\n";
  let base = repo.create_commit_with_files("Add", &[("docs/guide.md", guide)]);
  repo.create_commit_with_files(
    "The guide searches with accio\n\nThe guide names accio where it named grep.",
    &[("docs/guide.md", &guide.replace("alpha", "Search with `accio`."))],
  );
  let head = repo.create_commit_with_files(
    "The guide runs the checks\n\nThe guide says which checks run before a commit.",
    &[("docs/guide.md", &guide.replace("alpha", "Search with `accio`.").replace("delta", "Run `pnpm lint`."))],
  );
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(
    steps(&plan).into_iter().map(|(_, stops)| stops).collect::<Vec<_>>(),
    [names(&["guide.md [0]"]), names(&["guide.md [1]"])]
  );
  // the file has its main row in the first of two equal steps, the second shows it "also"
  let rows: Vec<(&[String], &[String])> = plan.steps.steps.iter().map(|step| (step.paths.as_slice(), step.elsewhere.as_slice())).collect();
  let guide = ["docs/guide.md".to_string()];
  assert_eq!(rows, [(guide.as_slice(), [].as_slice()), (guide.as_slice(), guide.as_slice())]);
}

/// The brief names the one issue the commits name, and the rules a spec adds, without their `[@test]` lines, which
/// name the tests that prove them, and prove the step that adds them
#[test]
fn the_brief_names_the_issue_and_the_rules_a_spec_adds_with_their_tests() {
  let repo = TestRepo::new();
  let spec = "---\nname: Window\n---\n\n# Window\n\n## Requirements\n\n- The window opens.\n";
  let base = repo.create_commit_with_files("Add", &[("spec/window.spec.md", spec), ("src/a/Api.kt", API)]);
  repo.create_commit_with_files(
    "IJAI-1 the window keeps its layout\n\nThe window keeps the layout it stored across a restart.",
    &[(
      "spec/window.spec.md",
      &format!("{spec}- The window keeps its stored layout across a restart. The Project view does not take its\n  place.\n  [@test] ../tests/restart.json\n"),
    )],
  );
  let head = repo.create_commit_with_files(
    "IJAI-1 the open asks the layout\n\nThe open asks the stored layout before it shows a tool window.",
    &[("src/a/Api.kt", &API.replace("= 1", "= 5"))],
  );
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(plan.brief.issue.as_deref(), Some("IJAI-1"));
  assert_eq!(
    plan.steps.steps[0].tests.scenarios,
    ["tests/restart.json"],
    "the step that changes the rule is proven by its test"
  );
  let [entry] = plan.brief.contract.as_slice() else { panic!("{:?}", plan.brief) };
  let texts: Vec<(&str, &ProseChange)> = entry.paragraphs.iter().map(|paragraph| (paragraph.text.as_str(), &paragraph.change)).collect();
  assert_eq!(
    (entry.kind, entry.heading.as_deref(), texts.as_slice(), entry.proven_by.as_slice()),
    (
      ContractKind::Spec,
      Some("Requirements"),
      [(
        "The window keeps its stored layout across a restart. The Project view does not take its place.",
        &ProseChange::Added
      )]
      .as_slice(),
      ["tests/restart.json".to_string()].as_slice()
    )
  );
}

/// A replacement repeated across the spec and guide is named once and taken out of its paragraph diffs
#[test]
fn a_substitution_repeated_across_paragraphs_is_named_once_and_marked_in_place() {
  let old_cli = concat!("b", "d");
  let old_spec = format!("# Commands\n\n## Requirements\n\n🧪 Run {old_cli} for local work.\n\nUse {old_cli} and {old_cli} output; check the status.\n");
  let new_spec = "# Commands\n\n## Requirements\n\n🧪 Run brd for local work.\n\nUse brd and brd output; review the status.\n";
  let old_guide = format!("# Agent guide\n\nThe {old_cli} command opens a workspace.\n");
  let new_guide = "# Agent guide\n\nThe brd command opens a workspace.\n";
  let plan = prose_plan(&old_spec, new_spec, &old_guide, new_guide);
  assert_eq!(
    plan.brief.substitutions,
    [Substitution {
      from: "bd".to_string(),
      to: "brd".to_string(),
      places: 4,
      paths: names(&["spec/commands.spec.md", "docs/guide.md"]),
    }]
  );
  let repeated = deleted_word_count(&plan, "bd");
  assert_eq!(repeated, 0, "the repeated `bd` → `brd` edit should be carried by the substitution summary");
  let paragraphs: Vec<_> = plan.brief.contract.iter().flat_map(|entry| &entry.paragraphs).collect();
  let quiet: Vec<_> = paragraphs.iter().filter(|paragraph| matches!(&paragraph.change, ProseChange::Substituted { .. })).collect();
  assert_eq!(quiet.len(), 2);
  let unicode = quiet.iter().find(|paragraph| paragraph.text.starts_with("🧪")).unwrap();
  let ProseChange::Substituted { substitutions } = &unicode.change else {
    panic!("the paragraph changed only by the substitution is quiet");
  };
  assert_eq!(
    substitutions,
    &[crate::prose::SubstitutionMark {
      start: 7,
      end: 10,
      substitution: 0
    }]
  );

  let mixed = paragraphs.iter().find(|paragraph| paragraph.text.starts_with("Use brd and brd")).unwrap();
  let ProseChange::Changed { inserted, deleted, substitutions } = &mixed.change else {
    panic!("the paragraph with another edit stays changed");
  };
  assert_eq!(substitutions.len(), 2);
  assert_eq!(inserted.iter().filter_map(|range| utf16_slice(&mixed.text, *range)).collect::<Vec<_>>(), ["review"]);
  assert_eq!(deleted.iter().map(|deletion| deletion.text.as_str()).collect::<Vec<_>>(), ["check"]);
}

/// A word pair seen twice remains an ordinary paragraph change
#[test]
fn a_pair_seen_twice_is_no_substitution() {
  let old_cli = concat!("b", "d");
  let unchanged_guide = "# Agent guide\n\nThe guide remains unchanged.\n";
  let old_spec = format!("# Commands\n\nUse {old_cli} to open work.\n\nUse {old_cli} to close work.\n");
  let new_spec = "# Commands\n\nUse brd to open work.\n\nUse brd to close work.\n";
  let plan = prose_plan(&old_spec, new_spec, unchanged_guide, unchanged_guide);
  assert_eq!(plan.brief.substitutions, Vec::new());
  let remaining = deleted_word_count(&plan, "bd");
  assert_eq!(remaining, 2);
}

/// A diff inside a fenced block widens to its fences before the brief parses it
#[test]
fn a_fenced_code_change_in_the_brief_keeps_its_code_form_and_lines() {
  let old_cli = concat!("b", "d");
  let old_spec = format!("# Commands\n\n## Requirements\n\nRun the `{old_cli}` command.\n");
  let old_guide = format!("# Guide\n\n```sh\n{old_cli} workspace -C <repo>\nprintf 'ready'\n```\n");
  let new_spec = "# Commands\n\n## Requirements\n\nRun the `brd` command.\n";
  let new_guide = "# Guide\n\n```sh\nbrd workspace -C <repo>\nprintf 'ready'\n```\n";
  let plan = prose_plan(&old_spec, new_spec, &old_guide, new_guide);
  let code: Vec<_> = plan
    .brief
    .contract
    .iter()
    .flat_map(|entry| &entry.paragraphs)
    .filter(|paragraph| matches!(&paragraph.form, ProseForm::Code { .. }))
    .collect();
  assert_eq!(code.len(), 1, "the changed code block is not prose");
  assert_eq!(code[0].text, "brd workspace -C <repo>\nprintf 'ready'");
  assert_eq!(code[0].form, ProseForm::Code { language: Some("sh".to_string()) });
  assert_eq!(code[0].code, Vec::new());
  assert_eq!(code[0].links, Vec::new());
}

/// Of a doc the brief keeps only prose: a changed line of a value (`dark: #88aaff`) says nothing, the heading of its
/// section names the change
#[test]
fn the_brief_of_a_doc_names_the_section_of_a_changed_value_and_quotes_only_its_prose() {
  let repo = TestRepo::new();
  let doc = "# Theme\n\n## Colours\n\ndark: #000000\nprimary: #3366ff, secondary: #ff8800\ncolor: red; background: blue; border: none\n\n## Why\n\nThe theme follows the system.\n";
  let base = repo.create_commit_with_files("Add", &[("docs/theme.md", doc)]);
  let head = repo.create_commit_with_files(
    "The accent is blue

The dark theme takes a blue accent, and the doc says why.",
    &[(
      "docs/theme.md",
      "# Theme\n\n## Colours\n\ndark: #88aaff\nprimary: #3366ee, secondary: #ff8811\ncolor: navy; background: white; border: thin\n\n## Why\n\nThe theme follows the system, its accent blue as the logo is.\n",
    )],
  );
  let plan = plan_of_range(&repo, &base, &head);
  let entries: Vec<(Option<&str>, Vec<&str>)> = plan
    .brief
    .contract
    .iter()
    .map(|entry| (entry.heading.as_deref(), entry.paragraphs.iter().map(|paragraph| paragraph.text.as_str()).collect()))
    .collect();
  assert_eq!(
    entries,
    [
      (Some("Colours"), vec![]),
      (Some("Why"), vec!["The theme follows the system, its accent blue as the logo is."])
    ]
  );
}

/// A mechanical change follows the step of the commit that wrote it, at its end: the imports a step needs are in it
#[test]
fn a_mechanical_change_follows_the_step_that_wrote_it() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add",
    &[("src/a/Api.kt", API), ("src/b/User.kt", "package b\n\nimport a.open\n\nfun user() = open(\"x\")\n")],
  );
  repo.create_commit_with_files(
    "Fresh opens anew\n\nA fresh open forgets the last mode, and the user imports it.",
    &[
      ("src/a/Api.kt", &format!("{API}\nfun fresh(): Int = 3\n")),
      ("src/b/User.kt", "package b\n\nimport a.fresh\nimport a.open\n\nfun user() = open(\"x\")\n"),
    ],
  );
  let head = repo.create_commit_with_files(
    "Close returns four\n\nThe close answers four, the code of a closed file.",
    &[(
      "src/a/Api.kt",
      &format!("{}\nfun fresh(): Int = 3\n", API.replace("fun close(): Int = 2", "fun close(): Int = 4")),
    )],
  );
  let plan = plan_of_range(&repo, &base, &head);
  // `fresh` has no block of its own: the block of `close` holds its lines, and the stop of `close` shows it
  assert_eq!(
    steps(&plan),
    [
      (commits(&[("Fresh opens anew", CommitRole::Starts)]), names(&["src/b/User.kt"])),
      (commits(&[("Close returns four", CommitRole::Starts)]), names(&["close"]))
    ]
  );
  assert_eq!(plan.steps.steps[0].shown_elsewhere.iter().map(|shown| shown.step).collect::<Vec<_>>(), [1]);
}

/// A commit whose lines a later commit rewrote whole shows nothing of its own: it follows the step of that commit
#[test]
fn a_commit_rewritten_whole_follows_the_step_of_the_commit_that_rewrote_it() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add", &[("docs/guide.md", "# Guide\n\nalpha\n")]);
  repo.create_commit_with_files(
    "The guide names grep\n\nThe guide says to search with grep for now.",
    &[("docs/guide.md", "# Guide\n\nalpha\n\nSearch with grep.\n")],
  );
  let head = repo.create_commit_with_files(
    "The guide names accio\n\nThe guide says to search with accio, which knows the symbols.",
    &[("docs/guide.md", "# Guide\n\nalpha\n\nSearch with `accio`, which knows the symbols.\n")],
  );
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(
    steps(&plan),
    [(
      commits(&[("The guide names accio", CommitRole::Starts), ("The guide names grep", CommitRole::FollowsUp)]),
      names(&["guide.md [0]"])
    )]
  );
}

#[test]
fn a_body_of_a_few_words_tells_a_step_a_fixup_none() {
  assert!(tells_a_story("Open takes a mode\n\nThe open names its mode."));
  assert!(!tells_a_story("Open takes a mode"));
  assert!(!tells_a_story("Open takes a mode\n\nSee #12"));
  assert!(!tells_a_story("fixup! Open takes a mode\n\nThe open names its mode."));
  // the trailers tell nothing: an agent's commit has a `Co-Authored-By` whether it says anything or not
  assert!(!tells_a_story("Open takes a mode\n\nCo-Authored-By: Claude Opus <noreply@anthropic.com>"));
  assert!(!tells_a_story(
    "Open takes a mode\n\nSigned-off-by: Ann Lee <ann@example.com>\nCo-Authored-By: Claude Opus <noreply@anthropic.com>"
  ));
  assert!(tells_a_story(
    "Open takes a mode\n\nThe open names its mode.\n\nCo-Authored-By: Claude Opus <noreply@anthropic.com>"
  ));
  assert!(!tells_a_story("Open takes a mode\n\nSee #12\n\nCo-Authored-By: Claude Opus <noreply@anthropic.com>"));
}

#[test]
fn the_issue_is_one_key_half_the_commits_name_and_none_contradicts() {
  assert_eq!(shared_issue(["IJAI-1 a", "IJAI-1 b", "c"].into_iter()).as_deref(), Some("IJAI-1"));
  assert_eq!(shared_issue(["IJAI-1 a", "IJAI-2 b"].into_iter()), None);
  assert_eq!(shared_issue(["IJAI-1 a", "b", "c"].into_iter()), None);
  // a subject that starts with two keys names both: the key every one names is the issue
  assert_eq!(
    shared_issue(["IJAI-5640 a", "IJAI-1184 IJAI-5640 b", "IJAI-5640 c"].into_iter()).as_deref(),
    Some("IJAI-5640")
  );
}

#[test]
fn a_file_of_prose_is_a_spec_a_decision_a_guide_or_a_doc_by_its_place() {
  assert_eq!(contract_kind("plugins/air/spec/frame/window.spec.md"), Some(ContractKind::Spec));
  assert_eq!(contract_kind("spec/branch/review-tour.md"), Some(ContractKind::Spec));
  assert_eq!(contract_kind("docs/decisions/0001-layout.md"), Some(ContractKind::Decision));
  assert_eq!(contract_kind("community/.ai/guide.md"), Some(ContractKind::Guide));
  assert_eq!(contract_kind("AGENTS.md"), Some(ContractKind::Guide));
  assert_eq!(contract_kind("plugins/air/docs/flows/flow-restart.txt"), Some(ContractKind::Doc));
  assert_eq!(contract_kind("spec/a.spec.ts"), None);
  assert_eq!(contract_kind("src/notes.md"), None);
}

/// A manager whose hooks a commit drops, then a commit of `rule` in their place: the repository and the base
fn hooks_dropped(rule: &str, body: &str) -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  let manager = |body: &str| format!("package a\n\nclass Manager {{\n  fun open(): Int = 1\n{body}}}\n");
  let base = repo.create_commit_with_files("Add", &[("src/a/Manager.kt", &manager("\n  fun hooks(): Int = 0\n"))]);
  repo.create_commit_with_files(
    "The manager drops its hooks\n\nNo caller asks the hooks of the manager, so they go.",
    &[("src/a/Manager.kt", &manager(""))],
  );
  let head = repo.create_commit_with_files(&format!("The manager allows a layout\n\n{body}"), &[("src/a/Manager.kt", &manager(rule))]);
  (repo, base, head)
}

/// A declaration a step removes, whose block a declaration a later step adds holds, is no stop of its own: the stop of
/// that block shows it, marked as of the step that removed it, and that step names it among its facts, lists the file
/// "also in" it with its main row in the later step, and shows no card; the coverage stays exact
#[test]
fn a_removed_declaration_whose_block_a_later_step_shows_is_a_part_of_that_stop_and_of_no_stop_of_its_own() {
  let (repo, base, head) = hooks_dropped(
    "\n  fun allowed(id: String): Boolean = id.isNotEmpty()\n",
    "One rule decides whether the open may show a tool window.",
  );
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(
    steps(&plan),
    [
      (commits(&[("The manager drops its hooks", CommitRole::Starts)]), names(&[])),
      (commits(&[("The manager allows a layout", CommitRole::Starts)]), names(&["allowed"])),
    ]
  );
  let hooks = plan.units.iter().find(|unit| unit.name.as_deref() == Some("hooks")).unwrap();
  let [dropped, allowed] = plan.steps.steps.as_slice() else {
    panic!("{:?}", plan.steps.steps)
  };
  assert_eq!(dropped.shown_elsewhere, [ShownElsewhere { unit: hooks.id.clone(), step: 1 }]);
  assert!(dropped.stops.is_empty() && dropped.taken_out_by.is_none(), "{dropped:?}");
  assert_eq!(
    (dropped.paths.as_slice(), dropped.elsewhere.as_slice()),
    (["src/a/Manager.kt".to_string()].as_slice(), ["src/a/Manager.kt".to_string()].as_slice())
  );
  assert_eq!(
    allowed.stops[0].joined,
    [JoinedUnit {
      unit: hooks.id.clone(),
      block: 0,
      step: 0
    }]
  );
  assert_eq!(allowed.paths, ["src/a/Manager.kt"]);
  assert!(allowed.elsewhere.is_empty(), "{allowed:?}");
  let coverage = plan.steps.coverage;
  assert_eq!((coverage.missing, coverage.twice, coverage.in_stops), (0, 0, 1), "{coverage:?}");
}

/// A declaration without a block of its own that the step of the block wrote too joins its stop, of that step, and is
/// no stop of its own; one of another step beside it is of that step
#[test]
fn a_declaration_without_a_block_joins_the_stop_of_the_block_of_its_line_each_of_its_own_step() {
  let (repo, base, head) = hooks_dropped(
    "\n  fun allowed(id: String): Boolean = id.isNotEmpty()\n\n  fun passes(): Int = 3\n",
    "One rule decides whether the open may show a tool window, and its passes count.",
  );
  let plan = plan_of_range(&repo, &base, &head);
  let name_of = |id: &str| plan.units.iter().find(|unit| unit.id == id).and_then(|unit| unit.name.clone()).unwrap();
  let shown: Vec<(String, Vec<(String, u32)>)> = plan.steps.steps[1]
    .stops
    .iter()
    .map(|stop| (name_of(&stop.unit), stop.joined.iter().map(|joined| (name_of(&joined.unit), joined.step)).collect()))
    .collect();
  // the block of `allowed` holds `hooks`, which step 1 removed, and `passes`, which its own commit added
  assert_eq!(shown, [("allowed".to_string(), vec![("hooks".to_string(), 0), ("passes".to_string(), 1)])]);
  assert_eq!(plan.steps.steps[0].stops, []);
  let coverage = plan.steps.coverage;
  assert_eq!((coverage.missing, coverage.twice), (0, 0), "{coverage:?}");
}

/// A declaration a later step adds beside a block of an earlier step, its lines in that block, is shown by the stop of
/// that block and is the new API of the step that wrote it, which shows no card and names where it is shown
#[test]
fn a_declaration_a_later_step_adds_in_the_block_of_another_is_the_new_api_of_the_step_that_wrote_it() {
  let repo = TestRepo::new();
  let manager = |body: &str| format!("package a\n\nclass Manager {{\n  fun open(): Int = 1\n{body}}}\n");
  let allowed = "\n  fun allowed(id: String): Boolean {\n    val named = id.isNotEmpty()\n    val long = id.length > 2\n    return named && long\n  }\n";
  let base = repo.create_commit_with_files("Add", &[("src/a/Manager.kt", &manager(""))]);
  repo.create_commit_with_files(
    "The manager allows a layout\n\nOne rule decides whether the open may show a tool window.",
    &[("src/a/Manager.kt", &manager(allowed))],
  );
  let head = repo.create_commit_with_files(
    "The manager counts its passes\n\nThe frame paints as many passes as the manager asks.",
    &[("src/a/Manager.kt", &manager(&format!("{allowed}\n  fun passes(): Int = 3\n")))],
  );
  let plan = plan_of_range(&repo, &base, &head);
  let key_of = |name: &str| plan.units.iter().find(|unit| unit.name.as_deref() == Some(name)).and_then(|unit| unit.key.clone()).unwrap();
  let id_of = |name: &str| plan.units.iter().find(|unit| unit.name.as_deref() == Some(name)).map(|unit| unit.id.clone()).unwrap();
  let [allows, counts] = plan.steps.steps.as_slice() else {
    panic!("{:?}", plan.steps.steps)
  };
  assert_eq!(
    allows.stops[0].joined,
    [JoinedUnit {
      unit: id_of("passes"),
      block: 0,
      step: 1
    }]
  );
  assert_eq!(allows.new_api, [key_of("allowed")]);
  assert_eq!(counts.new_api, [key_of("passes")]);
  assert_eq!(counts.shown_elsewhere, [ShownElsewhere { unit: id_of("passes"), step: 0 }]);
  assert!(counts.stops.is_empty(), "{counts:?}");
}

/// A unit of the steps of the concerns by its id, its kind, the line of its name and its blocks in `src/a/Manager.kt`
fn unit_of(id: &str, kind: UnitKind, line: Option<u32>, blocks: Vec<UnitBlock>) -> ReviewUnit {
  ReviewUnit {
    id: id.to_string(),
    kind,
    name: Some(id.to_string()),
    key: Some(id.to_string()),
    signature: None,
    api: false,
    test: false,
    line,
    slices: vec![UnitSlice {
      path: "src/a/Manager.kt".to_string(),
      old_path: None,
      whole: false,
      blocks,
      fingerprint: String::new(),
    }],
    fingerprint: String::new(),
  }
}

/// A step of the concerns whose stops show the units of `units` in `src/a/Manager.kt`
fn concern_step(id: &str, units: &[&str]) -> ReviewStep {
  ReviewStep {
    stops: units.iter().map(|unit| StepStop::new((*unit).to_string(), "src/a/Manager.kt".to_string(), None)).collect(),
    ..ReviewStep::new(id.to_string())
  }
}

/// A declaration without a block that is a part of two concerns joins the stop of its block once, of the step whose stop
/// holds its line; its stop in the other step goes too, that step naming where it is shown. A block only a group shows
/// (a mechanical change) takes no unit: the declaration stays a stop of its own
#[test]
fn a_declaration_without_a_block_of_two_concerns_joins_once_and_never_a_group() {
  let block = UnitBlock {
    index: 0,
    old_start: 2,
    old_end: 6,
    new_start: 2,
    new_end: 8,
  };
  let units = [
    unit_of("allowed", UnitKind::Added, Some(3), vec![block.clone()]),
    unit_of("hooks", UnitKind::Removed, Some(4), Vec::new()),
  ];
  let mut steps = [concern_step("layout", &["allowed", "hooks"]), concern_step("hooks", &["hooks"])];
  join_units(&units, &mut steps);
  let [layout, hooks] = &steps;
  assert_eq!(
    layout.stops.iter().map(|stop| (stop.unit.as_str(), stop.joined.clone())).collect::<Vec<_>>(),
    [(
      "allowed",
      vec![JoinedUnit {
        unit: "hooks".to_string(),
        block: 0,
        step: 0
      }]
    )]
  );
  assert!(layout.shown_elsewhere.is_empty(), "{layout:?}");
  assert_eq!(
    hooks.shown_elsewhere,
    [ShownElsewhere {
      unit: "hooks".to_string(),
      step: 0
    }]
  );
  assert!(hooks.stops.is_empty(), "{hooks:?}");

  let units = [
    unit_of(
      "imports",
      UnitKind::Mechanical {
        mechanical: MechanicalKind::ImportsOnly,
      },
      None,
      vec![block],
    ),
    unit_of("hooks", UnitKind::Removed, Some(4), Vec::new()),
  ];
  let mut steps = [concern_step("layout", &["imports", "hooks"])];
  join_units(&units, &mut steps);
  assert_eq!(steps[0].stops.iter().map(|stop| stop.unit.as_str()).collect::<Vec<_>>(), ["imports", "hooks"]);
  assert!(steps[0].stops.iter().all(|stop| stop.joined.is_empty()), "{:?}", steps[0]);
}

/// A declaration the last step removes, whose block a declaration an earlier step adds holds, is shown by the stop of
/// that block: the last step shows no card, names where it is shown and lists the file "also in" it
#[test]
fn a_removed_declaration_whose_block_an_earlier_step_shows_leaves_the_last_step_no_stop() {
  let repo = TestRepo::new();
  let test = |body: &str| format!("package b\n\nimport org.junit.Test\n\nclass WelcomeTest {{\n{body}}}\n");
  let focuses = "  @Test fun `the welcome focuses its project`() = check(focus() >= 0)\n";
  let keeps = "\n  @Test fun `the welcome keeps the stored layout`() {\n    val focused = focus()\n    check(focused >= 0)\n  }\n";
  let base = repo.create_commit_with_files("Add", &[("test/b/WelcomeTest.kt", &test(focuses))]);
  repo.create_commit_with_files(
    "The welcome test checks a stored layout\n\nThe welcome test checks that the welcome keeps the stored layout of its project.",
    &[("test/b/WelcomeTest.kt", &test(&format!("{focuses}{keeps}")))],
  );
  let head = repo.create_commit_with_files(
    "The welcome test drops the focus check\n\nThe check of the focus repeats the check of the layout, so it goes.",
    &[("test/b/WelcomeTest.kt", &test(keeps))],
  );
  let plan = plan_of_range(&repo, &base, &head);
  let id_of = |name: &str| plan.units.iter().find(|unit| unit.name.as_deref() == Some(name)).map(|unit| unit.id.clone()).unwrap();
  let [checks, drops] = plan.steps.steps.as_slice() else {
    panic!("{:?}", plan.steps.steps)
  };
  assert_eq!(
    checks
      .stops
      .iter()
      .map(|stop| stop.joined.iter().map(|joined| (joined.unit.clone(), joined.step)).collect::<Vec<_>>())
      .collect::<Vec<_>>(),
    [vec![(id_of("`the welcome focuses its project`"), 1)]]
  );
  assert!(drops.stops.is_empty(), "{drops:?}");
  assert_eq!(
    drops.shown_elsewhere,
    [ShownElsewhere {
      unit: id_of("`the welcome focuses its project`"),
      step: 0
    }]
  );
  assert_eq!(
    (drops.paths.as_slice(), drops.elsewhere.as_slice()),
    (["test/b/WelcomeTest.kt".to_string()].as_slice(), ["test/b/WelcomeTest.kt".to_string()].as_slice())
  );
}
