use crate::api::{ApiChange, ApiChangeKind, FanIn};
use crate::fan_in::{MAX_FANIN_QUERIES, NO_ACCIO, NO_INDEX, add_fan_in};
use file_diffs::accio::{ReferenceHit, References, SearchCanceled, SymbolHit, SymbolIndex};
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::sync::Mutex;
use test_log::test;

/// An index of usage counts by name: a name it does not know has none, an `Err` fails its query, `canceled` is
/// canceled; it keeps the revision of each query
pub(crate) struct Counts {
  answers: HashMap<String, Result<usize, String>>,
  indexed: bool,
  revisions: Mutex<Vec<Option<String>>>,
}

/// The name whose query is canceled
const CANCELED: &str = "canceled";
/// The usages an answer holds at most, as accio's `--limit`
const LIMIT: usize = 100;

impl Counts {
  pub(crate) fn of(answers: &[(&str, Result<usize, &str>)]) -> Self {
    Self {
      answers: answers.iter().map(|(name, answer)| ((*name).to_string(), answer.map_err(str::to_string))).collect(),
      indexed: true,
      revisions: Mutex::default(),
    }
  }

  fn no_index() -> Self {
    Self { indexed: false, ..Self::of(&[]) }
  }

  /// The revisions of the queries, sorted
  pub(crate) fn revisions(&self) -> Vec<Option<String>> {
    let mut revisions = self.revisions.lock().unwrap().clone();
    revisions.sort();
    revisions
  }
}

fn hit(kind: &str) -> ReferenceHit {
  ReferenceHit {
    path: "src/Use.kt".to_string(),
    line: 1,
    column: 1,
    snippet: String::new(),
    kind: kind.to_string(),
  }
}

impl SymbolIndex for Counts {
  fn references(&self, _repository_root: &str, name: &str, revision: Option<&str>) -> anyhow::Result<Option<References>> {
    self.revisions.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(revision.map(str::to_string));
    if name == CANCELED {
      return Err(SearchCanceled.into());
    }
    if !self.indexed {
      return Ok(None);
    }
    let usages = match self.answers.get(name) {
      Some(Err(error)) => anyhow::bail!("{error}"),
      Some(Ok(usages)) => *usages,
      None => 0,
    };
    // the declaration itself is a hit too, which no fan-in counts
    let hits: Vec<ReferenceHit> = std::iter::once(hit("declaration")).chain((0..usages).map(|_| hit("call"))).take(LIMIT).collect();
    Ok(Some(References { hits, complete: usages < LIMIT }))
  }

  /// No declaration lookup: the fan-in asks only usages
  fn symbols(&self, _repository_root: &str, _query: &str, _revision: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    Ok(None)
  }
}

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
    fan_in: None,
    file: format!("src/{name}.kt"),
  }
}

fn fan_ins(api: &[ApiChange]) -> Vec<(&str, Option<FanIn>)> {
  api.iter().map(|change| (change.name.as_str(), change.fan_in)).collect()
}

const fn usages(usages: u32) -> FanIn {
  FanIn { usages, complete: true }
}

/// A removal and a change are asked at the head and ranked by their usages; an addition is never asked
#[test]
fn the_entries_are_sorted_by_their_fan_in() {
  let index = Counts::of(&[("few", Ok(1)), ("many", Ok(9))]);
  let mut api = vec![
    change("few", ApiChangeKind::Removed),
    change("many", ApiChangeKind::Changed),
    change("fresh", ApiChangeKind::Added),
  ];
  let note = add_fan_in(Some(&index), "/repo", "head", &mut api).unwrap();
  assert_eq!(fan_ins(&api), [("many", Some(usages(9))), ("few", Some(usages(1))), ("fresh", None)]);
  assert_eq!((note, index.revisions()), (None, vec![Some("head".to_string()); 2]));
}

/// The index cuts an answer at its limit: the fan-in says it is not complete
#[test]
fn a_cut_answer_is_not_complete() {
  let index = Counts::of(&[("everywhere", Ok(250))]);
  let mut api = vec![change("everywhere", ApiChangeKind::Changed)];
  add_fan_in(Some(&index), "/repo", "head", &mut api).unwrap();
  assert_eq!(api[0].fan_in, Some(FanIn { usages: 99, complete: false }));
}

/// No accio on the machine, or no index of the repository: no fan-in, and the note says why
#[test]
fn without_an_index_the_fan_in_is_missing_and_the_note_says_why() {
  let mut api = vec![change("gone", ApiChangeKind::Removed)];
  assert_eq!(add_fan_in(None, "/repo", "head", &mut api).unwrap().as_deref(), Some(NO_ACCIO));
  let index = Counts::no_index();
  assert_eq!(add_fan_in(Some(&index), "/repo", "head", &mut api).unwrap().as_deref(), Some(NO_INDEX));
  assert_eq!(fan_ins(&api), [("gone", None)]);

  // nothing to ask: no note
  let mut added = vec![change("fresh", ApiChangeKind::Added)];
  assert_eq!(add_fan_in(None, "/repo", "head", &mut added).unwrap(), None);
}

/// A failing query fails no plan: the others keep their answers, and the note gives the failure
#[test]
fn a_failing_index_keeps_the_api() {
  let index = Counts::of(&[("broken", Err("index is corrupt")), ("fine", Ok(3))]);
  let mut api = vec![change("broken", ApiChangeKind::Removed), change("fine", ApiChangeKind::Changed)];
  let note = add_fan_in(Some(&index), "/repo", "head", &mut api).unwrap();
  assert_eq!(fan_ins(&api), [("fine", Some(usages(3))), ("broken", None)]);
  assert_eq!(note.as_deref(), Some("accio did not answer: index is corrupt"));
}

/// At most `MAX_FANIN_QUERIES` queries, for the first entries of the card
#[test]
fn at_most_twenty_queries_per_plan() {
  let index = Counts::of(&[]);
  let mut api: Vec<ApiChange> = (0..MAX_FANIN_QUERIES + 5)
    .map(|number| change(&format!("call{number:02}"), ApiChangeKind::Changed))
    .collect();
  add_fan_in(Some(&index), "/repo", "head", &mut api).unwrap();
  assert_eq!(index.revisions().len(), MAX_FANIN_QUERIES);
  let answered: Vec<&str> = api.iter().filter(|change| change.fan_in.is_some()).map(|change| change.name.as_str()).collect();
  assert_eq!(answered, (0..MAX_FANIN_QUERIES).map(|number| format!("call{number:02}")).collect::<Vec<_>>());
}

/// A canceled query (a newer request of the window) cancels the plan
#[test]
fn a_canceled_query_cancels_the_plan() {
  let index = Counts::of(&[]);
  let mut api = vec![change(CANCELED, ApiChangeKind::Changed)];
  let error = add_fan_in(Some(&index), "/repo", "head", &mut api).unwrap_err();
  assert!(error.is::<SearchCanceled>(), "{error:?}");
}
