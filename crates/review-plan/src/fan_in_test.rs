use crate::api::{ApiChange, ApiChangeKind, FormChange, UsageSite, Usages};
use crate::fan_in::{MAX_USAGE_QUERIES, NO_ACCIO, NO_INDEX, OUTSIDE_SITES, add_usages};
use file_diffs::accio::{ReferenceHit, References, SearchCanceled, SymbolHit, SymbolIndex};
use pretty_assertions::assert_eq;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use test_log::test;

/// A hit after the declaration: path, kind, snippet
type Hit<'a> = (&'a str, &'a str, &'a str);
/// The answer of a query: its hits, or the error that fails it
type Answer<'a> = Result<Vec<Hit<'a>>, &'a str>;

/// An index of usages by query: a query it does not know has none, an `Err` fails it, `canceled` is canceled; it keeps
/// the revision of each query and the query
pub(crate) struct Counts {
  /// query -> the hits after the declaration: (path, kind, snippet)
  answers: HashMap<String, Result<Vec<[String; 3]>, String>>,
  indexed: bool,
  queries: Mutex<Vec<(String, Option<String>)>>,
}

/// The query that is canceled
const CANCELED: &str = "canceled";
/// The usages an answer holds at most, as accio's `--limit`
const LIMIT: usize = 100;
/// The path of a usage that `Counts::of` answers: a file the change does not touch
const OUTSIDE: &str = "src/Use.kt";

impl Counts {
  /// `usages` usages of each query, all in `OUTSIDE`
  pub(crate) fn of(answers: &[(&str, Result<usize, &str>)]) -> Self {
    let answers: Vec<(&str, Result<Vec<&str>, &str>)> = answers.iter().map(|(query, answer)| (*query, answer.map(|usages| vec![OUTSIDE; usages]))).collect();
    Self::at(&answers)
  }

  /// A call at each path of each query
  pub(crate) fn at(answers: &[(&str, Result<Vec<&str>, &str>)]) -> Self {
    let answers: Vec<(&str, Answer<'_>)> = answers
      .iter()
      .map(|(query, answer)| (*query, answer.clone().map(|paths| paths.into_iter().map(|path| (path, "call", "")).collect())))
      .collect();
    Self::hits(&answers)
  }

  /// The hits of each query: (path, kind, snippet)
  pub(crate) fn hits(answers: &[(&str, Answer<'_>)]) -> Self {
    Self {
      answers: answers
        .iter()
        .map(|(query, answer)| {
          let answer = answer
            .clone()
            .map(|hits| hits.into_iter().map(|hit| <[&str; 3]>::from(hit).map(str::to_string)).collect())
            .map_err(str::to_string);
          ((*query).to_string(), answer)
        })
        .collect(),
      indexed: true,
      queries: Mutex::default(),
    }
  }

  fn no_index() -> Self {
    Self { indexed: false, ..Self::of(&[]) }
  }

  /// The revisions of the queries, sorted
  pub(crate) fn revisions(&self) -> Vec<Option<String>> {
    let mut revisions: Vec<Option<String>> = self.queries.lock().unwrap().iter().map(|(_, revision)| revision.clone()).collect();
    revisions.sort();
    revisions
  }

  /// The queries asked, sorted
  pub(crate) fn queries(&self) -> Vec<String> {
    let mut queries: Vec<String> = self.queries.lock().unwrap().iter().map(|(query, _)| query.clone()).collect();
    queries.sort();
    queries
  }
}

fn hit(path: &str, line: u32, kind: &str, snippet: &str) -> ReferenceHit {
  ReferenceHit {
    path: path.to_string(),
    line,
    column: 1,
    snippet: snippet.to_string(),
    kind: kind.to_string(),
  }
}

impl SymbolIndex for Counts {
  fn references(&self, _repository_root: &str, name: &str, revision: Option<&str>) -> anyhow::Result<Option<References>> {
    self
      .queries
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .push((name.to_string(), revision.map(str::to_string)));
    if name == CANCELED {
      return Err(SearchCanceled.into());
    }
    if !self.indexed {
      return Ok(None);
    }
    let answer = match self.answers.get(name) {
      Some(Err(error)) => anyhow::bail!("{error}"),
      Some(Ok(answer)) => answer.clone(),
      None => Vec::new(),
    };
    // the declaration itself is a hit too, which no usage counts
    let usages = answer
      .iter()
      .enumerate()
      .map(|(index, [path, kind, snippet])| hit(path, u32::try_from(index).unwrap() + 1, kind, snippet));
    let hits: Vec<ReferenceHit> = std::iter::once(hit("src/Api.kt", 1, "declaration", "")).chain(usages).take(LIMIT).collect();
    Ok(Some(References {
      hits,
      complete: answer.len() < LIMIT,
    }))
  }

  /// No declaration lookup: the usages need only references
  fn symbols(&self, _repository_root: &str, _query: &str, _revision: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    Ok(None)
  }
}

/// An entry of `name` whose usages are asked by `name`
fn change(name: &str, kind: ApiChangeKind) -> ApiChange {
  ApiChange {
    path: format!("src/{name}.kt"),
    line: 1,
    name: name.to_string(),
    key: format!("a.{name}"),
    kind,
    changes: Vec::new(),
    before: None,
    after: None,
    usages: None,
    query: name.to_string(),
    owner: None,
    file: format!("src/{name}.kt"),
  }
}

/// An entry of the member `name` of `Store`, asked by `Store.name`
fn member(name: &str, kind: ApiChangeKind) -> ApiChange {
  ApiChange {
    query: format!("Store.{name}"),
    owner: Some("Store".to_string()),
    ..change(name, kind)
  }
}

/// The usages of each entry: (name, inside, outside), `None` when not known
fn counts(api: &[ApiChange]) -> Vec<(&str, Option<(u32, u32)>)> {
  api
    .iter()
    .map(|change| (change.name.as_str(), change.usages.as_ref().map(|usages| (usages.inside, usages.outside))))
    .collect()
}

/// `add_usages` with no changed path
fn add(index: Option<&dyn SymbolIndex>, api: &mut [ApiChange]) -> anyhow::Result<Option<String>> {
  add_usages(index, "/repo", "head", &HashSet::new(), api)
}

/// A removal and a change are asked at the head and ranked by their usages left outside; an addition is never asked
#[test]
fn the_entries_are_sorted_by_their_usages_left_outside() {
  let index = Counts::of(&[("few", Ok(1)), ("many", Ok(9))]);
  let mut api = vec![
    change("few", ApiChangeKind::Removed),
    change("many", ApiChangeKind::Changed),
    change("fresh", ApiChangeKind::Added),
  ];
  let note = add(Some(&index), &mut api).unwrap();
  assert_eq!(counts(&api), [("many", Some((0, 9))), ("few", Some((0, 1))), ("fresh", None)]);
  assert_eq!((note, index.revisions()), (None, vec![Some("head".to_string()); 2]));
}

/// A usage in a file the change touches is updated in this change; one in another file is left outside, and the first
/// `OUTSIDE_SITES` of those are listed by path and line. The declaration itself is no usage. More usages left outside
/// rank first, though fewer in all
#[test]
fn the_usages_split_into_those_inside_the_change_and_those_left_outside() {
  let outside: Vec<&str> = vec!["lib/Other.kt"; OUTSIDE_SITES + 2];
  let index = Counts::at(&[
    ("helper", Ok(vec!["src/A.kt", "src/B.kt", "src/A.kt"])),
    ("open", Ok([&["src/A.kt"][..], &outside].concat())),
  ]);
  let mut api = vec![change("helper", ApiChangeKind::Removed), change("open", ApiChangeKind::Changed)];
  let changed: HashSet<&str> = ["src/A.kt", "src/B.kt"].into_iter().collect();
  add_usages(Some(&index), "/repo", "head", &changed, &mut api).unwrap();
  assert_eq!(counts(&api), [("open", Some((1, 7))), ("helper", Some((3, 0)))]);
  let sites: Vec<UsageSite> = (2..=6)
    .map(|line| UsageSite {
      path: "lib/Other.kt".to_string(),
      line,
    })
    .collect();
  assert_eq!(api[0].usages.as_ref().unwrap().outside_sites, sites);
  assert_eq!(api[1].usages.as_ref().unwrap().outside_sites, []);
}

/// The index cuts an answer at its limit: the usages say they are not complete
#[test]
fn a_cut_answer_is_not_complete() {
  let index = Counts::of(&[("everywhere", Ok(250))]);
  let mut api = vec![change("everywhere", ApiChangeKind::Changed)];
  add(Some(&index), &mut api).unwrap();
  let usages = api[0].usages.clone().unwrap();
  assert_eq!((usages.inside, usages.outside, usages.complete), (0, 99, false));
}

/// The query of an entry is its `query` (`Owner.name`), asked once for two entries; a name of the stop list without an
/// owner (bare, or after a package) is never asked, so its usages stay unknown rather than counting every type's
#[test]
fn the_entries_are_asked_by_their_query_and_never_by_a_bare_name_of_the_stop_list() {
  let index = Counts::of(&[("Store.open", Ok(2))]);
  let mut api = vec![
    member("open", ApiChangeKind::Removed),
    ApiChange {
      path: "src/Other.kt".to_string(),
      ..member("open", ApiChangeKind::Changed)
    },
    change("toString", ApiChangeKind::Changed),
    ApiChange {
      query: "a.get".to_string(),
      ..change("get", ApiChangeKind::Removed)
    },
  ];
  let note = add(Some(&index), &mut api).unwrap();
  assert_eq!(index.queries(), ["Store.open"]);
  assert_eq!(counts(&api), [("open", Some((0, 2))), ("open", Some((0, 2))), ("get", None), ("toString", None)]);
  assert_eq!(note, None);

  // only bare names of the stop list: nothing asked, no note
  let mut api = vec![change("hashCode", ApiChangeKind::Removed)];
  assert_eq!(add(None, &mut api).unwrap(), None);
}

/// A member of a name of the stop list is asked by its owner (`Store.new`, `Store.get`), but accio answers every call of
/// the name on a receiver it cannot type: only a line that names the owner counts, and the usages are a lower bound
#[test]
fn a_member_of_a_name_of_the_stop_list_counts_the_lines_that_name_its_owner() {
  let index = Counts::hits(&[
    (
      "Store.new",
      Ok(vec![
        ("src/user.rs", "call", "let store = Store::new(1);"),
        ("lib/other.rs", "call", "let cache = Cache::new(1);"),
        ("lib/other.rs", "call", "let store = crate::Store::new(2);"),
        ("lib/other.rs", "call", "let stores = MyStore::new(3);"),
      ]),
    ),
    (
      "Store.get",
      Ok(vec![("lib/Use.kt", "call", "val x = map.get(1)"), ("lib/Use.kt", "call", "val y = Store.get(key)")]),
    ),
  ]);
  let mut api = vec![
    ApiChange {
      file: "src/store.rs".to_string(),
      ..member("new", ApiChangeKind::Changed)
    },
    member("get", ApiChangeKind::Removed),
  ];
  let changed: HashSet<&str> = std::iter::once("src/user.rs").collect();
  add_usages(Some(&index), "/repo", "head", &changed, &mut api).unwrap();
  assert_eq!(index.queries(), ["Store.get", "Store.new"]);
  assert_eq!(counts(&api), [("new", Some((1, 1))), ("get", Some((0, 1)))]);
  assert!(api.iter().all(|change| !change.usages.as_ref().unwrap().complete));
}

/// The usages of an entry: the parts that changed, its kind, inside, outside, the paths of the sites outside, complete
type Split<'a> = (Vec<FormChange>, ApiChangeKind, u32, u32, Vec<&'a str>, bool);

/// Only a hit in code is a usage: no import, no doc mention, no hit in a file of another family of languages. Of a type
/// whose constructors are its only change, only a call and a subtype count (accio tags a Kotlin `class Sub : Policy(1)`
/// `call` and a Java `extends Policy` `supertype`), and the count is as complete as the answer of accio
#[test]
fn only_code_hits_count_and_of_a_constructor_change_only_calls_and_subtypes() {
  let hits = vec![
    ("lib/A.kt", "import", "import a.Policy"),
    ("lib/A.kt", "type", "override fun policy(): Policy?"),
    ("lib/B.kt", "doc", "/** The [Policy] of the frame */"),
    ("lib/C.java", "call", "return new Policy(1);"),
    ("web/policy.ts", "call", "new Policy(1)"),
    ("lib/D.kt", "read", "val id = policy.id"),
    ("src/E.kt", "call", "Policy(id = 1)"),
    ("lib/F.kt", "call", "class Sub : Policy(1) {"),
    ("lib/G.java", "supertype", "public class JSub extends Policy {"),
  ];
  let index = Counts::hits(&[("a.Policy", Ok(hits))]);
  let policy = |kind: ApiChangeKind, changes: Vec<FormChange>| ApiChange {
    query: "a.Policy".to_string(),
    file: "src/Policy.kt".to_string(),
    changes,
    ..change("Policy", kind)
  };
  let mut api = vec![
    policy(ApiChangeKind::Changed, vec![FormChange::Constructors]),
    policy(ApiChangeKind::Changed, vec![FormChange::Constructors, FormChange::Open]),
    policy(ApiChangeKind::Removed, Vec::new()),
  ];
  let changed: HashSet<&str> = std::iter::once("src/E.kt").collect();
  add_usages(Some(&index), "/repo", "head", &changed, &mut api).unwrap();
  let usages: Vec<Split<'_>> = api
    .iter()
    .map(|change| {
      let usages = change.usages.as_ref().unwrap();
      let sites = usages.outside_sites.iter().map(|site| site.path.as_str()).collect();
      (change.changes.clone(), change.kind, usages.inside, usages.outside, sites, usages.complete)
    })
    .collect();
  let every_code_hit = vec!["lib/A.kt", "lib/C.java", "lib/D.kt", "lib/F.kt", "lib/G.java"];
  assert_eq!(
    usages,
    [
      (Vec::new(), ApiChangeKind::Removed, 1, 5, every_code_hit.clone(), true),
      (vec![FormChange::Constructors, FormChange::Open], ApiChangeKind::Changed, 1, 5, every_code_hit, true),
      (
        vec![FormChange::Constructors],
        ApiChangeKind::Changed,
        1,
        3,
        vec!["lib/C.java", "lib/F.kt", "lib/G.java"],
        true
      ),
    ]
  );

  // no call and no subtype outside: every use is updated in this change
  let index = Counts::hits(&[("a.Policy", Ok(vec![("src/E.kt", "call", "Policy(id = 1)"), ("lib/A.kt", "type", "val p: Policy")]))]);
  let mut api = vec![policy(ApiChangeKind::Changed, vec![FormChange::Constructors])];
  add_usages(Some(&index), "/repo", "head", &changed, &mut api).unwrap();
  let usages = api[0].usages.as_ref().unwrap();
  assert_eq!((usages.inside, usages.outside, usages.complete), (1, 0, true));
}

/// No accio on the machine, or no index of the repository: no usages, and the note says why
#[test]
fn without_an_index_the_usages_are_missing_and_the_note_says_why() {
  let mut api = vec![change("gone", ApiChangeKind::Removed)];
  assert_eq!(add(None, &mut api).unwrap().as_deref(), Some(NO_ACCIO));
  let index = Counts::no_index();
  assert_eq!(add(Some(&index), &mut api).unwrap().as_deref(), Some(NO_INDEX));
  assert_eq!(counts(&api), [("gone", None)]);

  // nothing to ask: no note
  let mut added = vec![change("fresh", ApiChangeKind::Added)];
  assert_eq!(add(None, &mut added).unwrap(), None);
}

/// A failing query fails no plan: the others keep their answers, and the note gives the failure
#[test]
fn a_failing_index_keeps_the_api() {
  let index = Counts::of(&[("broken", Err("index is corrupt")), ("fine", Ok(3))]);
  let mut api = vec![change("broken", ApiChangeKind::Removed), change("fine", ApiChangeKind::Changed)];
  let note = add(Some(&index), &mut api).unwrap();
  assert_eq!(counts(&api), [("fine", Some((0, 3))), ("broken", None)]);
  assert_eq!(note.as_deref(), Some("accio did not answer: index is corrupt"));
}

/// At most `MAX_USAGE_QUERIES` queries, for the first entries of the card
#[test]
fn at_most_twenty_queries_per_plan() {
  let index = Counts::of(&[]);
  let mut api: Vec<ApiChange> = (0..MAX_USAGE_QUERIES + 5)
    .map(|number| change(&format!("call{number:02}"), ApiChangeKind::Changed))
    .collect();
  add(Some(&index), &mut api).unwrap();
  assert_eq!(index.revisions().len(), MAX_USAGE_QUERIES);
  let answered: Vec<&str> = api.iter().filter(|change| change.usages.is_some()).map(|change| change.name.as_str()).collect();
  assert_eq!(answered, (0..MAX_USAGE_QUERIES).map(|number| format!("call{number:02}")).collect::<Vec<_>>());
}

/// A canceled query (a newer request of the window) cancels the plan
#[test]
fn a_canceled_query_cancels_the_plan() {
  let index = Counts::of(&[]);
  let mut api = vec![change(CANCELED, ApiChangeKind::Changed)];
  let error = add(Some(&index), &mut api).unwrap_err();
  assert!(error.is::<SearchCanceled>(), "{error:?}");
}

/// The usages of a fixture, for the tests of the plan: `outside` usages left outside, none inside, complete
pub(crate) fn outside(outside: u32) -> Usages {
  Usages {
    inside: 0,
    outside,
    outside_sites: (1..=outside.min(5)).map(|line| UsageSite { path: OUTSIDE.to_string(), line }).collect(),
    complete: true,
    updated: outside == 0,
  }
}

/// An API is updated in this change only when its count is complete and none of its uses is left outside: a cut count
/// or one of a member of `STOP_NAMES` at 0 is not, nor a count with a use outside
#[test]
fn an_api_is_updated_only_by_a_complete_count_with_no_use_left_outside() {
  let changed: HashSet<&str> = std::iter::once("src/inside.kt").collect();
  let index = Counts::hits(&[
    ("inside", Ok(vec![("src/inside.kt", "call", "open(1)")])),
    ("outside", Ok(vec![("src/inside.kt", "call", "open(1)"), ("src/other.kt", "call", "open(2)")])),
  ]);
  let mut api = vec![change("inside", ApiChangeKind::Changed), change("outside", ApiChangeKind::Changed)];
  add_usages(Some(&index), "/repo", "head", &changed, &mut api).unwrap();
  let updated: Vec<(&str, bool)> = api.iter().map(|change| (change.name.as_str(), change.usages.as_ref().unwrap().updated)).collect();
  assert_eq!(updated, [("outside", false), ("inside", true)]);
}
