use crate::ReviewPlan;
use crate::brief::{ContractKind, contract_kind, shared_issue};
use crate::steps::{CommitRole, ConcernsReason, StepSource, tells_a_story, test_name};
use crate::test_support::plan_of_range;
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
  assert_eq!(
    plan.steps.steps[0].commits[0].body,
    "The open of a file names how it opens, so a caller can ask for a read alone."
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
  assert_eq!(plan.steps.steps[1].commits[0].body, "");
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

/// The tests of the range that name a declaration of a step are its tests, a member by its owner named in the test's
/// file too; a declaration no test names is unnamed
#[test]
fn the_tests_that_name_the_code_of_a_step_and_the_code_none_names() {
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
      (
        "test/b/OtherTest.kt",
        "package b\n\nimport org.junit.Test\n\nclass OtherTest {\n  @Test fun `lists`() = check(listOf(1).isEmpty())\n}\n",
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
      ("test/b/OtherTest.kt", "package b\n\nimport org.junit.Test\n\nclass OtherTest {\n  @Test fun `lists`() = check(listOf(2).isEmpty())\n}\n"),
    ],
  );
  let plan = plan_of_range(&repo, &base, &head);
  let mut named: Vec<(&str, Vec<&str>)> = plan
    .steps
    .steps
    .iter()
    .flat_map(|step| &step.tests.named)
    .map(|named| (named.name.as_str(), named.tests.iter().map(|test| test.name.as_str()).collect()))
    .collect();
  named.sort();
  assert_eq!(
    named,
    [("allowed", vec!["the project open may show only the stored layout"]), ("open", vec!["opens"])],
    "{:?}",
    plan.steps
  );
  let unnamed: Vec<&str> = plan.steps.steps.iter().flat_map(|step| &step.tests.unnamed).map(String::as_str).collect();
  assert_eq!(unnamed, ["isEmpty"], "the isEmpty of a list in another test is no use of the manager's");
}

/// A file of no language is cut by its blocks: each block in the step of the commit that wrote it
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
fn a_test_reads_as_its_words() {
  assert_eq!(test_name("`the project open keeps the layout`"), "the project open keeps the layout");
  assert_eq!(test_name("a_changed_declaration_joins_its_uses"), "a changed declaration joins its uses");
  assert_eq!(test_name("test_restores_the_tab"), "restores the tab");
  assert_eq!(test_name("testRestoresTheTab"), "restores the tab");
}

#[test]
fn a_body_of_a_few_words_tells_a_step_a_fixup_none() {
  assert!(tells_a_story("Open takes a mode\n\nThe open names its mode."));
  assert!(!tells_a_story("Open takes a mode"));
  assert!(!tells_a_story("Open takes a mode\n\nSee #12"));
  assert!(!tells_a_story("fixup! Open takes a mode\n\nThe open names its mode."));
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
