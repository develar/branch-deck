use crate::ReviewPlan;
use crate::api::{ApiChange, ApiChangeKind, FormChange, Usages};
use crate::fan_in_test::{Counts, by_name, outside, typed};
use crate::interdiff::PairState;
use crate::test_support::{branch, mark, plan_of, range};
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
      (ApiChangeKind::Removed, "removedCall", path, 3, vec![], Some("fun removedCall(x: Int): Int"), None),
      (
        ApiChangeKind::Changed,
        "changedCall",
        path,
        3,
        vec![FormChange::Parameters],
        Some("fun changedCall(x: Int): Int"),
        Some("fun changedCall(x: Int, y: Int): Int")
      ),
      (
        ApiChangeKind::Changed,
        "narrowedCall",
        path,
        4,
        vec![FormChange::Reach],
        Some("fun narrowedCall(): Int"),
        Some("fun narrowedCall(): Int")
      ),
      // its constructor's property is the type's entry
      (ApiChangeKind::Added, "AddedType", path, 8, vec![], None, Some("AddedType(name: String)")),
    ]
  );
  assert_eq!(plan.api[0].key, "a.removedCall");
  assert_eq!(plan.api.iter().map(|change| change.usages.clone()).collect::<Vec<_>>(), [None, None, None, None]);
  // no accio: why the removal and the change have no usages
  assert_eq!(plan.index_note.as_deref(), Some(crate::fan_in::NO_ACCIO));
  assert_eq!(facts(&plan), [(path, 0, true)]);
}

/// The usages at the head of the range rank the card: the most left outside first, the change of more usages above the
/// removal. A change is asked at the line of its name at the head, a removal at the line of its name before the commit
/// that removed it, at that commit's parent
#[test]
fn the_usages_at_the_head_rank_the_card() {
  let (repo, base, head) = api_repo();
  let (removed, changed, narrowed) = (format!("{head}^:src/a/Api.kt:3"), format!("{head}:src/a/Api.kt:3"), format!("{head}:src/a/Api.kt:4"));
  let index = Counts::of(&[(removed.as_str(), 2), (changed.as_str(), 7), (narrowed.as_str(), 0)]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  let ranked: Vec<(&str, Option<Usages>)> = plan.api.iter().map(|change| (change.name.as_str(), change.usages.clone())).collect();
  assert_eq!(
    ranked,
    [
      ("changedCall", Some(outside(7))),
      ("removedCall", Some(outside(2))),
      ("narrowedCall", Some(outside(0))),
      ("AddedType", None)
    ]
  );
  let mut places = vec![removed, changed, narrowed];
  places.sort();
  assert_eq!(index.places(), places);
  assert_eq!(index.revisions(), [head]);
  assert_eq!(plan.index_note, None);
  // the usages of the change and of the removal
  assert_eq!(facts(&plan), [("src/a/Api.kt", 9, true)]);
}

/// A range of no source file of a known language: no API, and its files break nothing and are not untested
#[test]
fn a_range_without_source_files_has_no_api() {
  let repo = TestRepo::new();
  let base = repo.create_commit("Add readme", "README.md", "readme\n");
  let head = repo.create_commit("Change readme", "README.md", "readme 2\n");
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  assert_eq!(facts(&plan), [("README.md", 0, false)]);
  assert_eq!((plan.api, plan.index_note), (vec![], None));
}

fn kotlin_function(name: &str, parameters: &str) -> String {
  format!("package a\n\nfun {name}({parameters}): Int = 1\n")
}

/// The files of `plan` in its order: the path, the usages of the API it removes or changes, and whether it is untested
fn facts(plan: &ReviewPlan) -> Vec<(&str, u32, bool)> {
  plan.files.iter().map(|file| (file.path.as_str(), file.breaks, file.untested)).collect()
}

fn names(plan: &ReviewPlan) -> Vec<(&str, ApiChangeKind)> {
  plan.api.iter().map(|change| (change.name.as_str(), change.kind)).collect()
}

/// The plan of `auth` since its review, whose first commit was rewritten and another added
fn rewritten_and_added_since_the_review(repo: &TestRepo) -> ReviewPlan {
  let since = plan_of(repo, None, &branch(repo, true)).unwrap();
  let states: Vec<PairState> = since.interdiff.as_ref().unwrap().commits.iter().map(|commit| commit.state).collect();
  assert_eq!(states, [PairState::Rewritten, PairState::Added]);
  since
}

/// With `since_reviewed`, the API and the files are those of the diff from the reviewed change to the tip: a rewritten
/// commit counts against its reviewed version, so a declaration it changed is changed, not added again, and an
/// unchanged commit between it and an added one adds nothing
#[test]
fn since_the_review_the_api_is_that_of_the_diff_from_the_reviewed_change() {
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

  let since = rewritten_and_added_since_the_review(&repo);
  assert_eq!(names(&since), [("login", ApiChangeKind::Changed), ("session", ApiChangeKind::Added)]);
  assert_eq!(facts(&since), [("src/a/Login.kt", 0, true), ("src/a/Session.kt", 0, true)]);

  let whole = plan_of(&repo, None, &branch(&repo, false)).unwrap();
  assert_eq!(
    names(&whole),
    [("login", ApiChangeKind::Added), ("logout", ApiChangeKind::Added), ("session", ApiChangeKind::Added)]
  );
  // the plan names a virtual branch as the title bar does
  assert_eq!((whole.branch.as_deref(), whole.title.as_str()), (Some("jane/virtual/auth"), "auth"));
}

/// With `since_reviewed`, an unchanged commit between a rewritten and an added one adds nothing to the API, also when all
/// three change the same file: the diff from the reviewed change holds the rewrite and the addition alone
#[test]
fn since_the_review_an_unchanged_commit_between_two_of_one_file_adds_nothing() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_timestamp("Add readme", "README.md", "readme\n", Some(1_700_000_000));
  repo.git(&["switch", "-q", "-c", "jane/virtual/auth"]);
  let path = "src/a/Api.kt";
  let file = |functions: &[&str]| {
    functions.iter().fold("package a\n\n".to_string(), |mut text, function| {
      text.push_str("fun ");
      text.push_str(function);
      text.push_str(": Int = 1\n\n");
      text
    })
  };
  repo.create_commit_with_timestamp("(auth) Add login", path, &file(&["login()"]), Some(1_700_000_100));
  repo.create_commit_with_timestamp("(auth) Add logout", path, &file(&["login()", "logout()"]), Some(1_700_000_200));
  mark(&repo);

  // the first commit rewritten, the second the same lines, a third on top
  repo.git(&["reset", "-q", "--hard", &base]);
  repo.create_commit_with_timestamp("(auth) Add login", path, &file(&["login(user: String)"]), Some(1_700_000_100));
  repo.create_commit_with_timestamp("(auth) Add logout", path, &file(&["login(user: String)", "logout()"]), Some(1_700_000_200));
  repo.create_commit_with_timestamp("(auth) Add session", path, &file(&["login(user: String)", "logout()", "session()"]), Some(1_700_000_300));

  let since = rewritten_and_added_since_the_review(&repo);
  assert_eq!(names(&since), [("login", ApiChangeKind::Changed), ("session", ApiChangeKind::Added)]);
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
      (ApiChangeKind::Removed, "id", path, 4, vec![], Some("fun id(): Int"), None),
      (
        ApiChangeKind::Changed,
        "of",
        path,
        3,
        vec![FormChange::Parameters],
        Some("fun of(x: Int): Int"),
        Some("fun of(x: Int, y: Int): Int")
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
  let row = |path: &'static str| (ApiChangeKind::Removed, "removedCall", path, 5, vec![], Some("fun removedCall(x: Int): Int"), None);

  let repo = TestRepo::new();
  let base = repo.create_commit("Add api", "src/a/Old.kt", MOVED);
  let head = move_file(&repo, &without_removed);
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  assert_eq!(removed(&plan), [row("src/a/Old.kt")]);
  assert!(plan.files.iter().all(|file| file.path == "src/a/New.kt"), "{:?}", plan.files);

  let repo = TestRepo::new();
  let base = repo.create_commit("Add api", "src/a/Old.kt", MOVED);
  move_file(&repo, MOVED);
  let head = repo.create_commit("Remove a call", "src/a/New.kt", &without_removed);
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  assert_eq!(removed(&plan), [row("src/a/New.kt")]);
}

/// An entry: its name, its kind, and its usages inside and outside the change
type Counted<'a> = (&'a str, ApiChangeKind, Option<(u32, u32)>);

/// A member is asked by its place, a member of a Kotlin companion too; an override is no entry, its supertype declares
/// the API. A use in a file the change touches is updated, and only the use left outside breaks
#[test]
fn a_member_is_asked_by_its_place_and_an_override_is_no_entry() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add store",
    &[
      (
        "src/a/Store.kt",
        "package a\n\nopen class Store {\n  fun open(x: Int): Int = x\n  override fun toText(): String = \"\"\n  companion object {\n    fun of(x: Int): Store = Store()\n  }\n}\n",
      ),
      ("src/a/User.kt", "package a\n\nfun use(store: Store): Int = store.open(1)\n"),
    ],
  );
  let head = repo.create_commit_with_files(
    "Change store",
    &[
      (
        "src/a/Store.kt",
        "package a\n\nopen class Store {\n  fun open(x: Int, y: Int): Int = x\n  companion object {\n    fun of(x: Int, y: Int): Store = Store()\n  }\n}\n",
      ),
      ("src/a/User.kt", "package a\n\nfun use(store: Store): Int = store.open(1, 2)\n"),
    ],
  );
  let (open, of) = (format!("{head}:src/a/Store.kt:4"), format!("{head}:src/a/Store.kt:6"));
  let index = Counts::calls(&[(&open, vec![("src/a/User.kt", 3), ("lib/Other.kt", 1)]), (&of, vec![("src/a/User.kt", 3)])]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  assert_eq!(index.places(), [open, of]);
  let entries: Vec<Counted<'_>> = plan
    .api
    .iter()
    .map(|change| (change.name.as_str(), change.kind, change.usages.as_ref().map(|usages| (usages.inside, usages.outside))))
    .collect();
  assert_eq!(entries, [("open", ApiChangeKind::Changed, Some((1, 1))), ("of", ApiChangeKind::Changed, Some((1, 0)))]);
  assert_eq!(facts_of(&plan, "src/a/Store.kt"), Some((1, true)));
}

/// A Kotlin `override` changed in form, and Java methods marked `@Override` or `@java.lang.Override`, changed or
/// removed, are no entries: the supertype declares the API. The change of a member that is not one is
#[test]
fn a_changed_override_in_kotlin_or_java_is_no_entry() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add shapes",
    &[
      ("src/a/Store.kt", "package a\n\nclass Store : Base() {\n  override fun render(x: Int): String = \"\"\n}\n"),
      (
        "src/a/Shape.java",
        "package a;\n\npublic class Shape extends Base {\n  @Override\n  public String render(int x) { return \"\"; }\n\n  @java.lang.Override\n  public String paint(int x) { return \"\"; }\n\n  public String draw(int x) { return \"\"; }\n}\n",
      ),
    ],
  );
  let head = repo.create_commit_with_files(
    "Change shapes",
    &[
      ("src/a/Store.kt", "package a\n\nclass Store : Base() {\n  override fun render(x: Int, y: Int): String = \"\"\n}\n"),
      (
        "src/a/Shape.java",
        "package a;\n\npublic class Shape extends Base {\n  @Override\n  public String render(int x, int y) { return \"\"; }\n\n  public String draw(int x, int y) { return \"\"; }\n}\n",
      ),
    ],
  );
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  let entries: Vec<(&str, ApiChangeKind, String)> = plan.api.iter().map(|change| (change.name.as_str(), change.kind, change.at.argument())).collect();
  assert_eq!(entries, [("draw", ApiChangeKind::Changed, format!("{head}:src/a/Shape.java:7"))]);
}

/// A Rust method is asked by its place, a name every type declares too (`Store::new`): a call only the name ties (a
/// `Cache::new` accio cannot tell from it) is no usage, and keeps the API from reading as unbroken
#[test]
fn a_rust_constructor_is_asked_by_its_place() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add store",
    &[
      ("src/store.rs", "pub struct Store;\n\nimpl Store {\n  pub fn new(x: u8) -> Self {\n    Store\n  }\n}\n"),
      ("src/user.rs", "pub fn user() {\n  let _ = Store::new(1);\n}\n"),
    ],
  );
  let head = repo.create_commit_with_files(
    "Change store",
    &[
      (
        "src/store.rs",
        "pub struct Store;\n\nimpl Store {\n  pub fn new(x: u8, y: u8) -> Self {\n    Store\n  }\n}\n",
      ),
      ("src/user.rs", "pub fn user() {\n  let _ = Store::new(1, 2);\n}\n"),
    ],
  );
  let place = format!("{head}:src/store.rs:4");
  let index = Counts::hits([(
    place.as_str(),
    vec![typed("src/user.rs", 2, "call"), by_name("lib/cache.rs", 1, "call"), typed("lib/other.rs", 1, "call")],
  )]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  assert_eq!(index.places(), [place]);
  let entries: Vec<(&str, ApiChangeKind, Option<Split>)> = plan
    .api
    .iter()
    .map(|change| {
      (
        change.name.as_str(),
        change.kind,
        change.usages.as_ref().map(|usages| (usages.inside, usages.outside, usages.by_name, usages.unbroken)),
      )
    })
    .collect();
  assert_eq!(entries, [("new", ApiChangeKind::Changed, Some((1, 1, 1, false)))]);
  assert_eq!(facts_of(&plan, "src/store.rs").map(|(breaks, _)| breaks), Some(1));
}

/// The usages of an entry: inside, outside, by the name alone, unbroken
type Split = (u32, u32, u32, bool);

/// The facts of `path`: the usages of the API it removes or changes, and whether it is untested
fn facts_of(plan: &ReviewPlan, path: &str) -> Option<(u32, bool)> {
  plan.files.iter().find(|file| file.path == path).map(|file| (file.breaks, file.untested))
}

/// The usages of a declaration a renamed file removes or changes count in the `breaks` of the file at its new path, while
/// the card names the path its line is of: a rename with a removal in one commit, a change and then a rename that
/// changes no declaration, a rename with a change and then a removal
#[test]
fn the_usages_of_an_api_of_a_renamed_file_count_at_its_new_path() {
  let without_removed = MOVED.replace("fun removedCall(x: Int): Int = x + 3\n", "");
  let with_changed = MOVED.replace("fun fourthCall(x: Int): Int", "fun fourthCall(x: Int, y: Int): Int");
  let index = |removed: &str, fourth: &str| Counts::of(&[(removed, 9), (fourth, 4)]);
  let fan_in = |usages: u32| Some(outside(usages));
  let entries =
    |plan: &ReviewPlan| -> Vec<(String, String, Option<Usages>)> { plan.api.iter().map(|change| (change.name.clone(), change.path.clone(), change.usages.clone())).collect() };
  let test_change = |repo: &TestRepo, content: &str| std::fs::write(repo.path().join("src/a/NewTest.kt"), content).unwrap();

  // the commit that renames the file and removes the declaration changes the file's test too
  let repo = TestRepo::new();
  let base = repo.create_commit("Add api", "src/a/Old.kt", MOVED);
  test_change(&repo, "package a\n");
  let head = move_file(&repo, &without_removed);
  let plan = plan_of(&repo, Some(&index(&format!("{head}^:src/a/Old.kt:5"), "-")), &range(&repo, &base, &head)).unwrap();
  assert_eq!(entries(&plan), [("removedCall".to_string(), "src/a/Old.kt".to_string(), fan_in(9))]);
  assert_eq!(facts_of(&plan, "src/a/New.kt"), Some((9, false)));
  assert_eq!(facts_of(&plan, "src/a/Old.kt"), None);

  // a change, then a rename that changes no declaration: the change is at the new path
  let repo = TestRepo::new();
  let base = repo.create_commit("Add api", "src/a/Old.kt", MOVED);
  let changed = repo.create_commit("Change a call", "src/a/Old.kt", &with_changed);
  test_change(&repo, "package a\n");
  let head = move_file(&repo, &with_changed);
  let plan = plan_of(&repo, Some(&index("-", &format!("{changed}:src/a/Old.kt:6"))), &range(&repo, &base, &head)).unwrap();
  assert_eq!(entries(&plan), [("fourthCall".to_string(), "src/a/New.kt".to_string(), fan_in(4))]);
  assert_eq!(facts_of(&plan, "src/a/New.kt"), Some((4, false)));

  // a rename with a change, then a removal of the changed declaration: the removal names the old path
  let repo = TestRepo::new();
  let base = repo.create_commit("Add api", "src/a/Old.kt", MOVED);
  test_change(&repo, "package a\n");
  let moved = move_file(&repo, &with_changed.replace("fun removedCall(x: Int): Int", "fun removedCall(x: Long): Int"));
  let head = repo.create_commit("Remove a call", "src/a/New.kt", &with_changed.replace("fun removedCall(x: Int): Int = x + 3\n", ""));
  let plan = plan_of(
    &repo,
    Some(&index(&format!("{moved}^:src/a/Old.kt:5"), &format!("{moved}:src/a/New.kt:6"))),
    &range(&repo, &base, &head),
  )
  .unwrap();
  assert_eq!(
    entries(&plan),
    [
      ("removedCall".to_string(), "src/a/Old.kt".to_string(), fan_in(9)),
      ("fourthCall".to_string(), "src/a/New.kt".to_string(), fan_in(4)),
    ]
  );
  // the usages of the removal and of the change
  assert_eq!(facts_of(&plan, "src/a/New.kt"), Some((13, false)));
}

/// The entries of `plan`: name, kind and the usages inside and outside the change
fn counted(plan: &ReviewPlan) -> Vec<Counted<'_>> {
  plan
    .api
    .iter()
    .map(|change| (change.name.as_str(), change.kind, change.usages.as_ref().map(|usages| (usages.inside, usages.outside))))
    .collect()
}

const STORE_TEST_BEFORE: &str = "package a

import org.junit.jupiter.api.Test

class StoreTest {
  @Test
  fun opens() {}
}

fun registered(x: Int): Int = x
";

const STORE_TEST_AFTER: &str = "package a

import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test

class StoreTest {
  val fixture = projectFixture()

  @BeforeEach
  fun setUp() {}

  @Test
  fun opens(x: Int) {}

  @Test
  fun `a workspace path decodes`() {}

  class Nested {
    fun nestedHelper(): Int = 1
  }
}

fun registered(x: Int, y: Int): Int = x

fun helper(): Int = 1
";

/// Test code is no API: a backtick `@Test`, a changed `@Test`, a `@BeforeEach`, a fixture field and a nested class of a
/// test class, a new test class, Java `@Test` and `JUnit` 3 `testX`, and a Rust `#[test]` in its inline `mod tests` give
/// no entry; the public helpers beside them in the same files are listed
#[test]
fn a_test_function_and_its_class_are_no_entry() {
  let repo = TestRepo::new();
  let lib_before = "pub fn open() -> u8 {\n  1\n}\n";
  let base = repo.create_commit_with_files("Add tests", &[("src/a/StoreTest.kt", STORE_TEST_BEFORE), ("src/lib.rs", lib_before)]);
  let head = repo.create_commit_with_files(
    "Change tests",
    &[
      ("src/a/StoreTest.kt", STORE_TEST_AFTER),
      (
        "src/a/FooTest.kt",
        "package a\n\nimport org.junit.jupiter.api.Test\n\nclass FooTest {\n  @Test\n  fun works() {}\n\n  fun inClass(): Int = 1\n}\n",
      ),
      (
        "src/a/ShapeTest.java",
        "package a;\n\nimport org.junit.jupiter.api.Test;\n\npublic class ShapeTest {\n  @Test\n  public void draws() {}\n\n  public int shapeHelper() { return 1; }\n}\n",
      ),
      (
        "src/a/LegacyTest.java",
        "package a;\n\nimport junit.framework.TestCase;\n\npublic class LegacyTest extends TestCase {\n  public void setUp() {}\n\n  public void testOpens() {}\n}\n",
      ),
      (
        "src/lib.rs",
        "pub fn open() -> u8 {\n  1\n}\n\n#[cfg(test)]\npub mod tests {\n  #[test]\n  pub fn opens() {}\n\n  pub fn fixture() -> u8 {\n    1\n  }\n}\n",
      ),
    ],
  );
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  assert_eq!(names(&plan), [("registered", ApiChangeKind::Changed), ("helper", ApiChangeKind::Added)]);
}

/// An entry point is no API: a Kotlin top-level `main` added or changed, and a Java `public static void main` added or
/// changed, give no entry; a public function beside them is listed
#[test]
fn a_main_function_is_no_entry() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add app",
    &[
      ("src/b/Main.kt", "package b\n\nfun main() {}\n"),
      ("src/a/App.java", "package a;\n\npublic class App {\n  public void run() {}\n}\n"),
      ("src/a/Cli.java", "package a;\n\npublic class Cli {\n  public static void main(String[] args) {}\n}\n"),
    ],
  );
  let head = repo.create_commit_with_files(
    "Change app",
    &[
      ("src/b/Main.kt", "package b\n\nfun main(args: Array<String>) {}\n"),
      (
        "src/a/App.java",
        "package a;\n\npublic class App {\n  public void run() {}\n\n  public static void main(String[] args) {}\n}\n",
      ),
      ("src/c/Tool.kt", "package c\n\nfun main() {}\n\nfun usage(): Int = 1\n"),
      (
        "src/a/Cli.java",
        "package a;\n\npublic class Cli {\n  public static void main(String[] args) throws Exception {\n    run();\n  }\n}\n",
      ),
    ],
  );
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  assert_eq!(names(&plan), [("usage", ApiChangeKind::Added)]);
  let main_units: Vec<(&str, bool)> = plan
    .units
    .iter()
    .filter(|unit| unit.name.as_deref() == Some("main"))
    .map(|unit| (unit.slices[0].path.as_str(), unit.api))
    .collect();
  assert_eq!(main_units, [("src/a/App.java", false), ("src/a/Cli.java", false), ("src/b/Main.kt", false)]);
}

/// A change of a form to or from API is listed: a private function made public with a parameter more (no API before,
/// API after), an internal one made private with a parameter more (unstable API before, no API after); a private one
/// that gets a parameter more is not. The unit of each is `api` by the same rule
#[test]
fn a_change_of_form_to_or_from_api_is_listed() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add calls",
    &[(
      "src/a/Calls.kt",
      "package a\n\nprivate fun widened(x: Int): Int = x\n\n@ApiStatus.Internal\nfun hidden(x: Int): Int = x\n\nprivate fun local(x: Int): Int = x\n",
    )],
  );
  let head = repo.create_commit_with_files(
    "Change calls",
    &[(
      "src/a/Calls.kt",
      "package a\n\nfun widened(x: Int, y: Int): Int = x\n\nprivate fun hidden(x: Int, y: Int): Int = x\n\nprivate fun local(x: Int, y: Int): Int = x\n",
    )],
  );
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  assert_eq!(names(&plan), [("widened", ApiChangeKind::Changed), ("hidden", ApiChangeKind::Changed)]);
  let units: Vec<(&str, bool)> = plan.units.iter().filter_map(|unit| Some((unit.name.as_deref()?, unit.api))).collect();
  assert_eq!(units, [("widened", true), ("hidden", true), ("local", false)]);
}

const UNSTABLE_BEFORE: &str = "package a

import org.jetbrains.annotations.ApiStatus

@ApiStatus.Internal
fun removedInternal(x: Int): Int = x

@ApiStatus.Internal
fun changedInternal(x: Int): Int = x

@ApiStatus.Internal
class InternalStore {
  fun existing(): Int = 1
}
";

const UNSTABLE_AFTER: &str = "package a

import org.jetbrains.annotations.ApiStatus
import org.jetbrains.annotations.TestOnly
import org.jetbrains.annotations.VisibleForTesting

@ApiStatus.Internal
fun changedInternal(x: Int, y: Int): Int = x

@ApiStatus.Internal
fun addedInternal(): Int = 1

@ApiStatus.Experimental
fun addedExperimental(): Int = 1

@VisibleForTesting
fun addedVisible(): Int = 1

@TestOnly
fun addedTestOnly(): Int = 1

@ApiStatus.Internal
class InternalStore {
  fun existing(): Int = 1

  fun added(): Int = 1
}
";

/// Unstable API is no new API: an added `@ApiStatus.Internal`, `@ApiStatus.Experimental`, `@VisibleForTesting` or
/// `@TestOnly` function and a member added to an `@ApiStatus.Internal` class give no entry; a removed and a changed
/// internal function are listed with their usages, callers in other modules can break
#[test]
fn unstable_api_is_no_new_api_but_its_change_is_listed() {
  let repo = TestRepo::new();
  let path = "src/a/Internal.kt";
  let base = repo.create_commit("Add internal api", path, UNSTABLE_BEFORE);
  let head = repo.create_commit("Change internal api", path, UNSTABLE_AFTER);
  let removed = format!("{head}^:{path}:6");
  let changed = format!("{head}:{path}:8");
  let index = Counts::of(&[(removed.as_str(), 3), (changed.as_str(), 5)]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  assert_eq!(
    counted(&plan),
    [
      ("changedInternal", ApiChangeKind::Changed, Some((0, 5))),
      ("removedInternal", ApiChangeKind::Removed, Some((0, 3))),
    ]
  );
  let mut places = vec![removed, changed];
  places.sort();
  assert_eq!(index.places(), places);
  assert_eq!(facts_of(&plan, path).map(|(breaks, _)| breaks), Some(8));
}

/// A declaration that becomes API in the same form, its `@ApiStatus.Internal` removed, is new API, a type with its
/// members as its one entry; one that becomes unstable in the same form is no entry, and one that becomes unstable with
/// a new parameter is changed
#[test]
fn a_promoted_declaration_is_new_api() {
  let repo = TestRepo::new();
  let path = "src/a/Promoted.kt";
  let registry = "src/a/Registry.kt";
  repo.create_commit(
    "Add registry",
    registry,
    "package a\n\nimport org.jetbrains.annotations.ApiStatus\n\n@ApiStatus.Internal\nclass Registry {\n  fun add(): Int = 1\n}\n",
  );
  repo.create_commit(
    "Add api",
    path,
    "package a\n\nimport org.jetbrains.annotations.ApiStatus\n\n@ApiStatus.Internal\nfun promoted(x: Int): Int = x\n\nfun demoted(): Int = 1\n\nfun narrowed(x: Int): Int = x\n",
  );
  let base = repo.head();
  repo.create_commit("Promote registry", registry, "package a\n\nclass Registry {\n  fun add(): Int = 1\n}\n");
  let head = repo.create_commit(
    "Change api",
    path,
    "package a\n\nimport org.jetbrains.annotations.ApiStatus\n\nfun promoted(x: Int): Int = x\n\n@ApiStatus.Internal\nfun demoted(): Int = 1\n\n@ApiStatus.Internal\nfun narrowed(x: Int, y: Int): Int = x\n",
  );
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  assert_eq!(
    rows(&plan.api),
    [
      (
        ApiChangeKind::Changed,
        "narrowed",
        path,
        11,
        vec![FormChange::Parameters],
        Some("fun narrowed(x: Int): Int"),
        Some("fun narrowed(x: Int, y: Int): Int")
      ),
      (ApiChangeKind::Added, "promoted", path, 5, vec![], None, Some("fun promoted(x: Int): Int")),
      (ApiChangeKind::Added, "Registry", registry, 3, vec![], None, Some("Registry()")),
    ]
  );
}

#[test]
fn a_form_of_type_parameters_reads_as_code() {
  // the header as code-symbols writes it, its tokens one space apart, the parameters after the name
  let form = |tokens: &str| crate::api::as_written(tokens.split(' '));
  assert_eq!(form("fun < T > first(items: List<T>) : T"), "fun <T> first(items: List<T>): T");
  assert_eq!(form("fun < T > String . trim2() : T ?"), "fun <T> String.trim2(): T?");
  assert_eq!(form("public static < T > List < T > of()"), "public static <T> List<T> of()");
  assert_eq!(form("fun hooks() : Map < String , List < T > >"), "fun hooks(): Map<String, List<T>>");
}

/// The constructors of a type read as calls of it, a line each, a non-public one after its visibility; a parameter by its
/// name and type, a default as `= …`, a vararg after `vararg`; a type of several names (its structure gone) no type at
/// all, never `List/Window`; never the name of a `Reach`
#[test]
fn a_form_of_constructors_and_parameters_reads_as_code() {
  assert_eq!(
    crate::api::shown_constructors("Event", "Public(text:String,styles:List/StyleRange,start:Long,ends:List/Long=);Container()"),
    "Event(text: String, styles, start: Long, ends = …)\nprivate Event()"
  );
  assert_eq!(crate::api::shown_constructors("Port", "Module(value:Int?)"), "internal Port(value: Int?)");
  assert_eq!(crate::api::shown_parameters("items:String[]*,handler:String/Int/Unit"), "vararg items: String[], handler");
}
