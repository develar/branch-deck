use crate::api::{ApiChange, ApiChangeKind, FanIn, FormChange};
use crate::fan_in_test::Counts;
use crate::interdiff::PairState;
use crate::risk::FileRisk;
use crate::test_support::{mark, plan_of, range};
use crate::{ReviewPlan, ReviewPlanParams, ReviewSubject};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const BEFORE: &str = "package a

fun removedCall(x: Int): Int = x
fun changedCall(x: Int): Int = x
fun narrowedCall(): Int = 1
private fun hiddenCall(x: Int): Int = x
fun bodyOnly(): Int = 1
";

const AFTER: &str = "package a

fun changedCall(x: Int, y: Int): Int = x + y
internal fun narrowedCall(): Int = 1
private fun hiddenCall(x: Int, y: Int): Int = x
fun bodyOnly(): Int = 2

class AddedType(val name: String)
";

/// An entry of the card: its kind, name, path, line, the parts that changed, the headers before and after
type Row<'a> = (ApiChangeKind, &'a str, &'a str, u32, Vec<FormChange>, Option<&'a str>, Option<&'a str>);

fn rows(api: &[ApiChange]) -> Vec<Row<'_>> {
  api
    .iter()
    .map(|change| {
      (
        change.kind,
        change.name.as_str(),
        change.path.as_str(),
        change.line,
        change.changes.clone(),
        change.before.as_deref(),
        change.after.as_deref(),
      )
    })
    .collect()
}

/// `master` with `Api.kt` as `BEFORE`, and one commit that makes it `AFTER`
fn api_repo() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  let base = repo.create_commit("Add api", "src/a/Api.kt", BEFORE);
  let head = repo.create_commit("Change api", "src/a/Api.kt", AFTER);
  (repo, base, head)
}

/// A removed public function, a new parameter, a narrowed reach and an added public type are listed; a change of a
/// private function and of a body alone are not
#[test]
fn the_card_lists_the_public_declarations_the_range_adds_removes_or_changes() {
  let (repo, base, head) = api_repo();
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  let path = "src/a/Api.kt";
  assert_eq!(
    rows(&plan.api),
    [
      (ApiChangeKind::Removed, "removedCall", path, 3, vec![], Some("fun removedCall(x:Int) : Int"), None),
      (
        ApiChangeKind::Changed,
        "changedCall",
        path,
        3,
        vec![FormChange::Parameters],
        Some("fun changedCall(x:Int) : Int"),
        Some("fun changedCall(x:Int, y:Int) : Int")
      ),
      (
        ApiChangeKind::Changed,
        "narrowedCall",
        path,
        4,
        vec![FormChange::Reach],
        Some("fun narrowedCall() : Int"),
        Some("fun narrowedCall() : Int")
      ),
      // its constructor's property is the type's entry
      (ApiChangeKind::Added, "AddedType", path, 8, vec![], None, Some("Public(name:String)")),
    ]
  );
  assert_eq!(plan.api[0].key, "a.removedCall");
  assert_eq!(plan.api.iter().map(|change| change.fan_in).collect::<Vec<_>>(), [None; 4]);
  // no accio: why the removal and the change have no fan-in
  assert_eq!(plan.index_note.as_deref(), Some(crate::fan_in::NO_ACCIO));
  assert_eq!(
    plan.risk,
    [FileRisk {
      path: path.to_string(),
      max_fan_in: None,
      untested: true,
      risky: true,
    }]
  );
}

/// The usages at the head of the range rank the card: the most first, the change of more usages above the removal
#[test]
fn the_fan_in_at_the_head_ranks_the_card() {
  let (repo, base, head) = api_repo();
  let index = Counts::of(&[("removedCall", Ok(2)), ("changedCall", Ok(7)), ("narrowedCall", Ok(0))]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  let ranked: Vec<(&str, Option<FanIn>)> = plan.api.iter().map(|change| (change.name.as_str(), change.fan_in)).collect();
  let fan_in = |usages: u32| Some(FanIn { usages, complete: true });
  assert_eq!(
    ranked,
    [("changedCall", fan_in(7)), ("removedCall", fan_in(2)), ("narrowedCall", fan_in(0)), ("AddedType", None)]
  );
  assert_eq!(index.revisions(), [Some(head.clone()), Some(head.clone()), Some(head)]);
  assert_eq!(plan.index_note, None);
  assert_eq!((plan.risk[0].max_fan_in, plan.risk[0].risky), (Some(7), true));
}

/// A range of no source file of a known language: no API, and its files carry no risk
#[test]
fn a_range_without_source_files_has_no_api() {
  let repo = TestRepo::new();
  let base = repo.create_commit("Add readme", "README.md", "readme\n");
  let head = repo.create_commit("Change readme", "README.md", "readme 2\n");
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  assert_eq!((plan.api, plan.index_note), (vec![], None));
  assert_eq!(
    plan.risk,
    [FileRisk {
      path: "README.md".to_string(),
      max_fan_in: None,
      untested: false,
      risky: false,
    }]
  );
}

fn kotlin_function(name: &str, parameters: &str) -> String {
  format!("package a\n\nfun {name}({parameters}): Int = 1\n")
}

fn branch_params(repo: &TestRepo, since_reviewed: bool) -> ReviewPlanParams {
  ReviewPlanParams {
    repository_path: repo.path_str().to_string(),
    subject: ReviewSubject::Branch {
      name: "auth".to_string(),
      base: None,
      baseline_branch: Some("master".to_string()),
    },
    since_reviewed,
  }
}

fn names(plan: &ReviewPlan) -> Vec<(&str, ApiChangeKind)> {
  plan.api.iter().map(|change| (change.name.as_str(), change.kind)).collect()
}

/// With `since_reviewed`, the API and the risk are those of the rewritten and the added commits, each against its own
/// parent, though an unchanged commit lies between them
#[test]
fn since_the_review_the_api_is_that_of_the_added_and_rewritten_commits() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_timestamp("Add readme", "README.md", "readme\n", Some(1_700_000_000));
  repo.git(&["switch", "-q", "-c", "jane/virtual/auth"]);
  repo.create_commit_with_timestamp("(auth) Add login", "src/a/Login.kt", &kotlin_function("login", ""), Some(1_700_000_100));
  let logout = repo.create_commit_with_timestamp("(auth) Add logout", "src/a/Logout.kt", &kotlin_function("logout", ""), Some(1_700_000_200));
  mark(&repo);

  // the first commit rewritten below the second, and a third one on top
  repo.git(&["reset", "-q", "--hard", &base]);
  repo.create_commit_with_timestamp("(auth) Add login", "src/a/Login.kt", &kotlin_function("login", "user: String"), Some(1_700_000_100));
  repo.git(&["cherry-pick", "--allow-empty", &logout]);
  repo.create_commit_with_timestamp("(auth) Add session", "src/a/Session.kt", &kotlin_function("session", ""), Some(1_700_000_300));

  let since = plan_of(&repo, None, &branch_params(&repo, true)).unwrap();
  let states: Vec<PairState> = since.interdiff.as_ref().unwrap().commits.iter().map(|commit| commit.state).collect();
  assert_eq!(states, [PairState::Rewritten, PairState::Added]);
  assert_eq!(names(&since), [("login", ApiChangeKind::Added), ("session", ApiChangeKind::Added)]);
  let risky: Vec<&str> = since.risk.iter().map(|risk| risk.path.as_str()).collect();
  assert_eq!(risky, ["src/a/Login.kt", "src/a/Session.kt"]);

  let whole = plan_of(&repo, None, &branch_params(&repo, false)).unwrap();
  assert_eq!(
    names(&whole),
    [("login", ApiChangeKind::Added), ("logout", ApiChangeKind::Added), ("session", ApiChangeKind::Added)]
  );
}

/// Outside the JVM the header holds the parameters: a new parameter of a `pub fn` changes its header, and a
/// `pub(crate)` one is not public
#[test]
fn a_rust_function_changes_by_its_header() {
  let repo = TestRepo::new();
  let base = repo.create_commit("Add lib", "src/lib.rs", "pub fn open(path: &str) -> u8 { 1 }\npub(crate) fn inner(x: u8) {}\n");
  let head = repo.create_commit(
    "Change lib",
    "src/lib.rs",
    "pub fn open(path: &str, mode: u8) -> u8 { mode }\npub(crate) fn inner(x: u8, y: u8) {}\n",
  );
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  let changed: Vec<(&str, ApiChangeKind, &[FormChange])> = plan.api.iter().map(|change| (change.name.as_str(), change.kind, change.changes.as_slice())).collect();
  assert_eq!(changed, [("open", ApiChangeKind::Changed, &[FormChange::Header][..])]);
  assert_ne!(plan.api[0].before, plan.api[0].after);
}

/// A public declaration of a short name is listed: the checks of the sync skip names under three characters, the card
/// does not
#[test]
fn a_public_declaration_of_a_short_name_is_listed() {
  let repo = TestRepo::new();
  let base = repo.create_commit("Add ids", "src/a/Ids.kt", "package a\n\nfun of(x: Int): Int = x\nfun id(): Int = 1\n");
  let head = repo.create_commit("Change ids", "src/a/Ids.kt", "package a\n\nfun of(x: Int, y: Int): Int = x\n");
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  let path = "src/a/Ids.kt";
  assert_eq!(
    rows(&plan.api),
    [
      (ApiChangeKind::Removed, "id", path, 4, vec![], Some("fun id() : Int"), None),
      (
        ApiChangeKind::Changed,
        "of",
        path,
        3,
        vec![FormChange::Parameters],
        Some("fun of(x:Int) : Int"),
        Some("fun of(x:Int, y:Int) : Int")
      ),
    ]
  );
}

/// The functions of a file large enough that git sees its move with a removal as a rename
const MOVED: &str = "package a

fun firstCall(x: Int): Int = x + 1
fun secondCall(x: Int): Int = x + 2
fun removedCall(x: Int): Int = x + 3
fun fourthCall(x: Int): Int = x + 4
fun fifthCall(x: Int): Int = x + 5
fun sixthCall(x: Int): Int = x + 6
fun seventhCall(x: Int): Int = x + 7
fun eighthCall(x: Int): Int = x + 8
";

/// Move `src/a/Old.kt` to `src/a/New.kt` with `content`, in one commit
fn move_file(repo: &TestRepo, content: &str) -> String {
  repo.git(&["mv", "src/a/Old.kt", "src/a/New.kt"]);
  std::fs::write(repo.path().join("src/a/New.kt"), content).unwrap();
  repo.git(&["add", "-A"]);
  repo.commit_with(&["-m", "Move api"], &[])
}

/// The entries of `removedCall`
fn removed(plan: &ReviewPlan) -> Vec<Row<'_>> {
  rows(&plan.api).into_iter().filter(|row| row.1 == "removedCall").collect()
}

/// A removal names the file and the line its form was read at: before the commit that renames the file and removes the
/// declaration, the old path; after a plain rename, the new path, whose lines are the same
#[test]
fn a_removal_after_a_rename_names_the_path_its_line_is_of() {
  let without_removed = MOVED.replace("fun removedCall(x: Int): Int = x + 3\n", "");
  let row = |path: &'static str| (ApiChangeKind::Removed, "removedCall", path, 5, vec![], Some("fun removedCall(x:Int) : Int"), None);

  let repo = TestRepo::new();
  let base = repo.create_commit("Add api", "src/a/Old.kt", MOVED);
  let head = move_file(&repo, &without_removed);
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  assert_eq!(removed(&plan), [row("src/a/Old.kt")]);
  assert!(plan.risk.iter().all(|risk| risk.path == "src/a/New.kt"), "{:?}", plan.risk);

  let repo = TestRepo::new();
  let base = repo.create_commit("Add api", "src/a/Old.kt", MOVED);
  move_file(&repo, MOVED);
  let head = repo.create_commit("Remove a call", "src/a/New.kt", &without_removed);
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  assert_eq!(removed(&plan), [row("src/a/New.kt")]);
}

/// The risk of `path`: its most usages and whether the card raises it
fn risk_of(plan: &ReviewPlan, path: &str) -> Option<(Option<u32>, bool, bool)> {
  plan.risk.iter().find(|risk| risk.path == path).map(|risk| (risk.max_fan_in, risk.untested, risk.risky))
}

/// The fan-in of a declaration a renamed file removes or changes counts in the risk of the file at its new path, while
/// the card names the path its line is of: a rename with a removal in one commit, a change and then a rename that
/// changes no declaration, a rename with a change and then a removal
#[test]
fn the_fan_in_of_an_api_of_a_renamed_file_counts_at_its_new_path() {
  let without_removed = MOVED.replace("fun removedCall(x: Int): Int = x + 3\n", "");
  let with_changed = MOVED.replace("fun fourthCall(x: Int): Int", "fun fourthCall(x: Int, y: Int): Int");
  let index = Counts::of(&[("removedCall", Ok(9)), ("fourthCall", Ok(4))]);
  let fan_in = |usages: u32| Some(FanIn { usages, complete: true });
  let entries = |plan: &ReviewPlan| -> Vec<(String, String, Option<FanIn>)> { plan.api.iter().map(|change| (change.name.clone(), change.path.clone(), change.fan_in)).collect() };
  let test_change = |repo: &TestRepo, content: &str| std::fs::write(repo.path().join("src/a/NewTest.kt"), content).unwrap();

  // the commit that renames the file and removes the declaration changes the file's test too
  let repo = TestRepo::new();
  let base = repo.create_commit("Add api", "src/a/Old.kt", MOVED);
  test_change(&repo, "package a\n");
  let head = move_file(&repo, &without_removed);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  assert_eq!(entries(&plan), [("removedCall".to_string(), "src/a/Old.kt".to_string(), fan_in(9))]);
  assert_eq!(risk_of(&plan, "src/a/New.kt"), Some((Some(9), false, true)));
  assert_eq!(risk_of(&plan, "src/a/Old.kt"), None);

  // a change, then a rename that changes no declaration: the change is at the new path
  let repo = TestRepo::new();
  let base = repo.create_commit("Add api", "src/a/Old.kt", MOVED);
  repo.create_commit("Change a call", "src/a/Old.kt", &with_changed);
  test_change(&repo, "package a\n");
  let head = move_file(&repo, &with_changed);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  assert_eq!(entries(&plan), [("fourthCall".to_string(), "src/a/New.kt".to_string(), fan_in(4))]);
  assert_eq!(risk_of(&plan, "src/a/New.kt"), Some((Some(4), false, true)));

  // a rename with a change, then a removal of the changed declaration: the removal names the old path
  let repo = TestRepo::new();
  let base = repo.create_commit("Add api", "src/a/Old.kt", MOVED);
  test_change(&repo, "package a\n");
  move_file(&repo, &with_changed.replace("fun removedCall(x: Int): Int", "fun removedCall(x: Long): Int"));
  let head = repo.create_commit("Remove a call", "src/a/New.kt", &with_changed.replace("fun removedCall(x: Int): Int = x + 3\n", ""));
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  assert_eq!(
    entries(&plan),
    [
      ("removedCall".to_string(), "src/a/Old.kt".to_string(), fan_in(9)),
      ("fourthCall".to_string(), "src/a/New.kt".to_string(), fan_in(4)),
    ]
  );
  assert_eq!(risk_of(&plan, "src/a/New.kt"), Some((Some(9), false, true)));
}
