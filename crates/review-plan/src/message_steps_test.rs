use crate::ReviewPlan;
use crate::steps::{ConcernsReason, StepSource, TitleSource};
use crate::story::StoryPlace;
use crate::test_support::plan_of_range;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// A step as a test reads it: its title, what titles it, and the names of the files it shows
type StepWords = (Option<String>, TitleSource, Vec<String>);

fn steps(plan: &ReviewPlan) -> Vec<StepWords> {
  plan
    .steps
    .steps
    .iter()
    .map(|step| {
      let mut files: Vec<String> = step.stops.iter().map(|stop| stop.path.rsplit('/').next().unwrap_or_default().to_string()).collect();
      files.dedup();
      (step.title.as_ref().map(ToString::to_string), step.title_source, files)
    })
    .collect()
}

/// The title of each of the first `count` steps of `plan`, its text, and what gives it
fn titles(plan: &ReviewPlan, count: usize) -> Vec<(Option<&str>, TitleSource)> {
  plan
    .steps
    .steps
    .iter()
    .take(count)
    .map(|step| (step.title.as_ref().map(|title| title.text.as_str()), step.title_source))
    .collect()
}

fn words(title: Option<&str>, source: TitleSource, files: &[&str]) -> StepWords {
  (title.map(str::to_string), source, files.iter().map(|file| (*file).to_string()).collect())
}

const GRID: &str = "package grid\n\nfun mark(row: Int): Int = row\n";
const MODEL: &str = "package model\n\ndata class Update(val text: String)\n";
const EVENT: &str = "package model\n\nfun event(text: String) = Update(text)\n";
const HIGHLIGHTER: &str = "package link\n\nimport model.Update\n\nfun highlight(update: Update) = update.text\n";
const OTHER: &str = "package other\n\nfun count() = 1\n\nfun total() = count()\n";
const TEST: &str = "package test\n\nfun checkHighlight() = 1\n";

/// The files of the base: a grid, a model and its event, a highlighter, an unrelated pair, a test
fn base(repo: &TestRepo) -> String {
  repo.create_commit_with_files(
    "Add",
    &[
      ("src/grid/Grid.kt", GRID),
      ("src/model/Model.kt", MODEL),
      ("src/model/Event.kt", EVENT),
      ("src/link/Highlighter.kt", HIGHLIGHTER),
      ("src/other/Other.kt", OTHER),
      ("src/test/HighlightTest.kt", TEST),
    ],
  )
}

/// The change of the story: a mark of the grid, a field of the update its event fills, a joiner the highlighter calls,
/// an unrelated change, a test and a new test
fn change<'a>() -> Vec<(&'a str, String)> {
  vec![
    (
      "src/grid/Grid.kt",
      "package grid\n\nfun endsAtEdge(row: Int): Boolean = row > 0\n\nfun mark(row: Int): Int = if (endsAtEdge(row)) row else 0\n".to_string(),
    ),
    (
      "src/model/Model.kt",
      "package model\n\ndata class Update(val text: String, val edges: List<Int> = emptyList())\n".to_string(),
    ),
    (
      "src/model/Event.kt",
      "package model\n\nfun event(text: String) = Update(text, edges = listOf(1))\n".to_string(),
    ),
    (
      "src/link/Joiner.kt",
      "package link\n\nimport model.Update\n\nclass Joiner {\n  fun join(update: Update): List<Int> = update.edges\n}\n".to_string(),
    ),
    (
      "src/link/Highlighter.kt",
      "package link\n\nimport model.Update\n\nfun highlight(update: Update) = Joiner().join(update).size\n".to_string(),
    ),
    ("src/other/Other.kt", "package other\n\nfun count() = 2\n\nfun total() = count() + 1\n".to_string()),
    ("src/test/HighlightTest.kt", "package test\n\nfun checkHighlight() = 2\n".to_string()),
    ("src/test/JoinerTest.kt", "package test\n\nfun checkJoin() = 1\n".to_string()),
  ]
}

fn commit(repo: &TestRepo, message: &str) -> String {
  let files = change();
  let files: Vec<(&str, &str)> = files.iter().map(|(path, text)| (*path, text.as_str())).collect();
  repo.create_commit_with_files(message, &files)
}

const STORY: &str = "Links join across the edge\n\nA long URL breaks at the edge, so a click opened a cut link.\n\nThe grid marks a row that ends at the edge. `endsAtEdge` owns the rule.\n\nThe update carries the marks in `Update.edges`, and `event()` fills them.\n\n`Joiner` runs after the highlighter on each line.\n\nLook closely: the guards of `Joiner.join`.\n\nTests:\n- `JoinerTest`: joins two rows.\n\nLeft out:\n- a resize.";

/// The paragraphs of one commit that name the code of different concerns are its steps, in the order of its body: each
/// titled by its first sentence, with the concerns it names; a concern two paragraphs name is cut by its own joins; a
/// Look closely is told in the step of the paragraph before it; the tests are the step of the `Tests:` section with the
/// test code no paragraph names; a concern no paragraph names stays a step of its own after them
#[test]
fn the_paragraphs_of_one_commit_that_name_its_code_are_its_steps_in_its_order() {
  let repo = TestRepo::new();
  let base = base(&repo);
  let head = commit(&repo, STORY);
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(plan.steps.source, StepSource::Message);
  assert_eq!(
    steps(&plan),
    [
      words(Some("The grid marks a row that ends at the edge."), TitleSource::Sentence, &["Grid.kt"]),
      words(
        Some("The update carries the marks in Update.edges, and event() fills them."),
        TitleSource::Sentence,
        &["Event.kt", "Model.kt"]
      ),
      words(
        Some("Joiner runs after the highlighter on each line."),
        TitleSource::Sentence,
        &["Highlighter.kt", "Joiner.kt"]
      ),
      words(Some("Tests"), TitleSource::Label, &["JoinerTest.kt", "HighlightTest.kt"]),
      words(None, TitleSource::Evidence, &["Other.kt"]),
    ]
  );
  // each part where it is told: the lead and Left out in the overview, each paragraph in its step, unfolded, the Look
  // closely in the step of the joiner, the tests in theirs, named once as the title; what the author left out shows
  let story = &plan.steps.commits[0];
  let parts: Vec<(u32, StoryPlace, bool, bool)> = story.parts.iter().map(|part| (part.start, part.told, part.folded, part.titled)).collect();
  let step = |step| StoryPlace::Step { step };
  assert_eq!(
    parts,
    [
      (0, StoryPlace::Overview, false, false),
      (1, step(0), false, false),
      (2, step(1), false, false),
      (3, step(2), false, false),
      (4, step(2), false, false),
      (6, step(3), false, true),
      (8, StoryPlace::Overview, false, false),
    ]
  );
  assert!(plan.steps.told);
  assert_eq!((plan.steps.coverage.missing, plan.steps.coverage.twice), (0, 0));
}

/// A step of the message keeps its id when a reword names more of its code: its id is its first part's, not its words
#[test]
fn a_step_of_the_message_keeps_its_id_when_its_paragraph_names_more() {
  let repo = TestRepo::new();
  let base = base(&repo);
  let first = commit(&repo, STORY);
  let ids = |plan: &ReviewPlan| plan.steps.steps.iter().map(|step| step.id.clone()).collect::<Vec<_>>();
  let before = ids(&plan_of_range(&repo, &base, &first));
  repo.reset_hard(&base).unwrap();
  let reworded = commit(&repo, &STORY.replace("`endsAtEdge` owns the rule.", "`endsAtEdge` owns the rule, `mark` asks it."));
  assert_eq!(ids(&plan_of_range(&repo, &base, &reworded)), before);
}

/// A paragraph whose first sentence is longer than a subject is titled by the first name it resolves, set as code; a
/// list written right under its paragraph is one part with it
#[test]
fn a_long_first_sentence_titles_its_step_by_the_first_name_it_resolves_and_a_list_under_a_paragraph_is_one_part() {
  let repo = TestRepo::new();
  let base = base(&repo);
  let long = "The grid marks every row whose last cell holds a glyph and ends with a hard line break, which only the emulator knows";
  let message = format!(
    "Links join across the edge\n\nA long URL breaks at the edge.\n\n{long}, so `endsAtEdge` owns it.\n\nThe update carries the marks in `Update.edges`.\n- the event fills them\n- the joiner reads them\n\n`Joiner` runs after the highlighter."
  );
  let head = commit(&repo, &message);
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(plan.steps.source, StepSource::Message);
  let titles = titles(&plan, 3);
  assert_eq!(
    titles,
    [
      (Some("endsAtEdge"), TitleSource::Name),
      (Some("The update carries the marks in Update.edges."), TitleSource::Sentence),
      (Some("Joiner runs after the highlighter."), TitleSource::Sentence),
    ]
  );
  // the paragraph and the list right under it in one step
  let story = &plan.steps.commits[0];
  let parts: Vec<(u32, u32, StoryPlace)> = story.parts.iter().map(|part| (part.start, part.end, part.told)).collect();
  assert_eq!(parts[2], (2, 4, StoryPlace::Step { step: 1 }));
}

/// A table of a run tells what its cells name, as a paragraph does: a step of its own, titled by the first name it
/// resolves (a table has no first sentence)
#[test]
fn a_table_tells_a_step_titled_by_the_first_name_it_resolves() {
  let repo = TestRepo::new();
  let base = base(&repo);
  let message = "Links join across the edge\n\nA long URL breaks at the edge.\n\nThe grid marks a row that ends at the edge. `endsAtEdge` owns the rule.\n\n| Part | Where |\n| --- | --- |\n| the joiner | `Joiner` |\n| the marks | `Update.edges` |";
  let head = commit(&repo, message);
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(plan.steps.source, StepSource::Message);
  let titles = titles(&plan, 2);
  assert_eq!(
    titles,
    [
      (Some("The grid marks a row that ends at the edge."), TitleSource::Sentence),
      (Some("Joiner"), TitleSource::Name),
    ]
  );
  let story = &plan.steps.commits[0];
  let parts: Vec<(u32, u32, StoryPlace)> = story.parts.iter().map(|part| (part.start, part.end, part.told)).collect();
  assert_eq!(parts[2], (2, 3, StoryPlace::Step { step: 1 }));
}

/// The concerns stay the steps of a body that names code in one paragraph alone, of two sections that name the same
/// code (a cause and its fix: the first keeps it), and of a body of verification alone
#[test]
fn the_concerns_stay_the_steps_of_a_body_that_tells_no_two_parts_by_their_code() {
  let repo = TestRepo::new();
  let base = base(&repo);
  let one_commit = StepSource::Concerns {
    reason: ConcernsReason::OneCommit,
  };
  for message in [
    "Links join\n\nThe lead.\n\nThe grid marks a row, so `endsAtEdge` and `Joiner` and `Update.edges` change.",
    "Links join\n\nThe lead.\n\nThe cause:\n`Joiner` saw each line alone.\n\nThe fix:\n`Joiner` reads the marks of `Joiner.join`.",
    "Links join\n\nTests:\n- `JoinerTest` and `endsAtEdge`",
  ] {
    repo.reset_hard(&base).unwrap();
    let head = commit(&repo, message);
    assert_eq!(plan_of_range(&repo, &base, &head).steps.source, one_commit, "{message}");
  }
}
