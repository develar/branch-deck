use crate::ReviewPlan;
use crate::brief::{ContractKind, contract_kind, shared_issue};
use crate::fan_in::NO_ACCIO;
use crate::fan_in_test::Counts;
use crate::steps::{CommitRole, ConcernsReason, StepSource, TEST_DEPTH, tells_a_story};
use crate::test_support::{plan_of, plan_of_range, range};
use domain_types::commit_body::CommitBodyBlock;
use file_diffs::accio::{ReachingTest, TestReach, TestStep, WalkCut};
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
      let commits = step.commits.iter().map(|commit| (commit.subject.clone(), commit.role)).collect();
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
  let bodies: Vec<(&str, &[CommitBodyBlock])> = plan.steps.commits.iter().map(|commit| (commit.subject.as_str(), commit.body.blocks.as_slice())).collect();
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

/// A commit that rewrites the code an earlier step wrote follows that step up, and so does one whose lines all landed
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
    "Open names its flags again\n\nThe review asked for the names of the flags to say what they hold.",
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
  let told: Vec<&str> = plan.steps.commits.iter().map(|commit| commit.subject.as_str()).collect();
  assert_eq!(told, ["Open takes a mode", "Open names its flags again"]);
}

/// Commits without a body make the concerns the steps, and so does a range of one commit
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

/// A test that reaches a declaration: its path, line, the name it reads as, its depth and the callers on the way
fn reaching(path: &str, line: u32, reads_as: &str, depth: u32, via: &[&str]) -> ReachingTest {
  ReachingTest {
    path: path.to_string(),
    line,
    declaration: format!("Test.{reads_as}"),
    reads_as: reads_as.to_string(),
    framework: "junit4".to_string(),
    depth,
    via: via
      .iter()
      .map(|declaration| TestStep {
        declaration: (*declaration).to_string(),
        path: "src/a/Frame.kt".to_string(),
        line: 1,
      })
      .collect(),
  }
}

/// A test of a step as a test reads it: its name, whether the range adds or changes it, its depth and the callers on
/// the way
type Reached<'a> = (&'a str, bool, u32, Vec<&'a str>);

/// The declarations the tests reach in the steps of `plan` with their tests, and those none reaches
type ReachedAndUnreached<'a> = (Vec<(&'a str, Vec<Reached<'a>>)>, Vec<(&'a str, Option<u32>)>);

/// The declarations the tests reach in the steps of `plan`, each with its tests, sorted, and those none reaches, with
/// the calls the walk went
fn reached_and_unreached(plan: &ReviewPlan) -> ReachedAndUnreached<'_> {
  let mut reached: Vec<(&str, Vec<Reached<'_>>)> = plan
    .steps
    .steps
    .iter()
    .flat_map(|step| &step.tests.reached)
    .map(|reached| {
      let tests = reached
        .tests
        .iter()
        .map(|test| (test.name.as_str(), test.unit.is_some(), test.depth, test.via.iter().map(String::as_str).collect()))
        .collect();
      (reached.name.as_str(), tests)
    })
    .collect();
  reached.sort();
  let unreached = plan
    .steps
    .steps
    .iter()
    .flat_map(|step| &step.tests.unreached)
    .map(|unreached| (unreached.name.as_str(), unreached.within))
    .collect();
  (reached, unreached)
}

/// The tests that reach a declaration of the code of a step, as accio answers by its place at the head: a test the
/// range adds or changes by its unit, one the change did not touch apart; a declaration no test reaches says how far the
/// walk went. Without accio no step knows its tests, and the plan says why
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
      ("src/a/Manager.kt", &manager.replace("fun open(): Int = 1", "fun open(): Int = 2").replace("= true", "= false").replace("}\n", "\n  fun allowed(id: String): Boolean = id.isNotEmpty()\n}\n")),
      (
        "test/a/ManagerTest.kt",
        "package a\n\nimport org.junit.Test\n\nclass ManagerTest {\n  @Test fun `opens`() = check(Manager().open() == 2)\n\n  @Test fun `the project open may show only the stored layout`() = check(Manager().allowed(\"Project\"))\n}\n",
      ),
    ],
  );
  let tests = |tests: Vec<ReachingTest>, cut: Option<WalkCut>| TestReach { tests, cut };
  let index = Counts::of(&[]).with_tests([
    ("src/a/Manager.kt:4", tests(vec![reaching("test/a/ManagerTest.kt", 6, "opens", 1, &[])], None)),
    (
      "src/a/Manager.kt:8",
      tests(
        vec![
          reaching("test/a/ManagerTest.kt", 8, "the project open may show only the stored layout", 1, &[]),
          reaching("test/a/LayoutTest.kt", 3, "the layout is kept", 2, &["Frame.show"]),
        ],
        None,
      ),
    ),
    ("src/a/Manager.kt:6", tests(Vec::new(), Some(WalkCut::Depth))),
  ]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  assert_eq!(index.tested(), ["src/a/Manager.kt:4", "src/a/Manager.kt:6", "src/a/Manager.kt:8"]);
  assert_eq!(index.revisions(), std::slice::from_ref(&head));
  let (reached, unreached) = reached_and_unreached(&plan);
  assert_eq!(
    reached,
    [
      (
        "allowed",
        vec![
          ("the project open may show only the stored layout", true, 1, vec![]),
          ("the layout is kept", false, 2, vec!["Frame.show"])
        ]
      ),
      ("open", vec![("opens", true, 1, vec![])]),
    ],
    "{:?}",
    plan.steps
  );
  assert_eq!(unreached, [("isEmpty", Some(TEST_DEPTH))]);
  assert!(plan.steps.steps.iter().all(|step| step.tests.answered));
  assert_eq!(plan.index_note, None);

  // without accio: no step knows its tests
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(reached_and_unreached(&plan), (vec![], vec![]));
  assert!(plan.steps.steps.iter().all(|step| !step.tests.answered));
  assert_eq!(plan.index_note.as_deref(), Some(NO_ACCIO));
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
      tests: vec![reaching("src/store.rs", 14, "size is right", 1, &[])],
      cut: None,
    },
  )]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  assert_eq!(index.tested(), ["src/store.rs:1"]);
  assert_eq!(
    reached_and_unreached(&plan),
    (vec![("size", vec![("size is right", true, 1, vec![])])], vec![]),
    "{:?}",
    plan.steps
  );
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
  assert_eq!(
    (entry.kind, entry.heading.as_deref(), entry.added.as_slice(), entry.proven_by.as_slice()),
    (
      ContractKind::Spec,
      Some("Requirements"),
      ["The window keeps its stored layout across a restart. The Project view does not take its place.".to_string()].as_slice(),
      ["tests/restart.json".to_string()].as_slice()
    )
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
  assert_eq!(
    steps(&plan),
    [
      (commits(&[("Fresh opens anew", CommitRole::Starts)]), names(&["fresh", "src/b/User.kt"])),
      (commits(&[("Close returns four", CommitRole::Starts)]), names(&["close"]))
    ]
  );
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

/// A declaration a step removes, whose block a declaration a later step adds holds, is in the step that removed it
#[test]
fn a_removed_declaration_is_in_the_step_that_removed_it() {
  let repo = TestRepo::new();
  let manager = |body: &str| format!("package a\n\nclass Manager {{\n  fun open(): Int = 1\n{body}}}\n");
  let base = repo.create_commit_with_files("Add", &[("src/a/Manager.kt", &manager("\n  fun hooks(): Int = 0\n"))]);
  repo.create_commit_with_files(
    "The manager drops its hooks\n\nNo caller asks the hooks of the manager, so they go.",
    &[("src/a/Manager.kt", &manager(""))],
  );
  let head = repo.create_commit_with_files(
    "The manager allows a layout\n\nOne rule decides whether the open may show a tool window.",
    &[("src/a/Manager.kt", &manager("\n  fun allowed(id: String): Boolean = id.isNotEmpty()\n"))],
  );
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(
    steps(&plan),
    [
      (commits(&[("The manager drops its hooks", CommitRole::Starts)]), names(&["hooks"])),
      (commits(&[("The manager allows a layout", CommitRole::Starts)]), names(&["allowed"])),
    ]
  );
}
