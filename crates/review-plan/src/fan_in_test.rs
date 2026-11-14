use crate::api::{ApiChange, ApiChangeKind, FormChange, UsageSite, Usages};
use crate::fan_in::{NO_ACCIO, NO_INDEX, OUTSIDE_SITES, add_usages};
use crate::pair::ChangedLines;
use crate::test_support::{plan_of, range};
use diff_marks::index::{DeclaredAt, MovesAnswer, Order, Reordered, Resolved};
use file_diffs::accio::{CallArgument, DeclarationAt, ReferenceHit, References, SearchCanceled, SymbolHit, SymbolIndex, TestReach};
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::sync::Mutex;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use trial_check::symbol_dependencies::declaration_checks::{ParameterForm, ShownType};

/// An index of usages by place (`DeclarationAt::argument`: `<path>:<line>`, `<revision>:<path>:<line>`): a place it does
/// not know has none; it keeps every query, its places and its revision
pub(crate) struct Counts {
  /// The place -> the hits after the declaration
  answers: HashMap<String, Vec<ReferenceHit>>,
  indexed: bool,
  /// Every query fails with it
  failure: Option<&'static str>,
  canceled: bool,
  /// Each query: `references` or `tests`, its places (`DeclarationAt::argument`) and its revision
  queries: Mutex<Vec<(&'static str, Vec<String>, String)>>,
  /// The place -> the given tests that reach it; a place it does not know none reaches, its answer complete
  tests: HashMap<String, TestReach>,
  /// The places accio finds no declaration at
  unavailable: Vec<String>,
  /// The given tests of each query of `tests`
  given: Mutex<Vec<Vec<String>>>,
  /// What `moves` answers it put in another order (`reordered`)
  reordered: Vec<Reordered>,
}

/// The usages an answer holds at most, as accio's `--limit`
const LIMIT: usize = 100;
/// The path of a usage that `Counts::of` answers: a file the change does not touch
const OUTSIDE: &str = "src/Use.kt";

/// A hit tied to the declaration by a type
pub(crate) fn typed(path: &str, line: u32, kind: &str) -> ReferenceHit {
  serde_json::from_value(serde_json::json!({"file":path,"line":line,"column":1,"snippet":"","kind":kind,"resolved":"typed"})).unwrap()
}

/// A hit only the name ties to the declaration
pub(crate) fn by_name(path: &str, line: u32, kind: &str) -> ReferenceHit {
  ReferenceHit {
    resolved: Resolved::Name,
    ..typed(path, line, kind)
  }
}

/// A typed call from `line` to `end_line` that fits the new form, with its arguments
fn fitting(path: &str, line: u32, end_line: u32, arguments: Vec<CallArgument>) -> ReferenceHit {
  ReferenceHit {
    end_line: Some(end_line),
    arguments: Some(arguments),
    fits: Some(true),
    ..typed(path, line, "call")
  }
}

/// A named argument that binds its parameter
fn named(name: &str) -> CallArgument {
  CallArgument {
    name: Some(name.to_string()),
    parameter: Some(name.to_string()),
    ..CallArgument::default()
  }
}

/// A positional argument that binds `parameter`
fn positional(position: u32, parameter: &str) -> CallArgument {
  CallArgument {
    position: Some(position),
    parameter: Some(parameter.to_string()),
    ..CallArgument::default()
  }
}

impl Counts {
  /// `usages` typed calls of each place, all in `OUTSIDE`
  pub(crate) fn of(answers: &[(&str, usize)]) -> Self {
    let answers = answers.iter().map(|(place, usages)| {
      let lines = 1..=u32::try_from(*usages).unwrap();
      (*place, lines.map(|line| typed(OUTSIDE, line, "call")).collect())
    });
    Self::hits(answers)
  }

  /// A typed call at each path and line of each place
  pub(crate) fn calls(answers: &[(&str, Vec<(&str, u32)>)]) -> Self {
    Self::hits(
      answers
        .iter()
        .map(|(place, calls)| (*place, calls.iter().map(|(path, line)| typed(path, *line, "call")).collect())),
    )
  }

  /// The hits of each place
  pub(crate) fn hits<'a>(answers: impl IntoIterator<Item = (&'a str, Vec<ReferenceHit>)>) -> Self {
    Self {
      answers: answers.into_iter().map(|(place, hits)| (place.to_string(), hits)).collect(),
      indexed: true,
      failure: None,
      canceled: false,
      queries: Mutex::default(),
      tests: HashMap::new(),
      unavailable: Vec::new(),
      given: Mutex::default(),
      reordered: Vec::new(),
    }
  }

  /// Answers `moves` with the declarations `reordered` put in another order
  pub(crate) fn with_reordered(self, reordered: Vec<Reordered>) -> Self {
    Self { reordered, ..self }
  }

  /// How many times `moves` was asked
  pub(crate) fn moves_asked(&self) -> usize {
    self.queries.lock().unwrap().iter().filter(|(of, ..)| *of == "moves").count()
  }

  /// With the tests that reach each place
  pub(crate) fn with_tests<'a>(self, tests: impl IntoIterator<Item = (&'a str, TestReach)>) -> Self {
    Self {
      tests: tests.into_iter().map(|(place, reach)| (place.to_string(), reach)).collect(),
      ..self
    }
  }

  /// Finds no declaration at `places`
  fn without_declarations(self, places: &[&str]) -> Self {
    Self {
      unavailable: places.iter().map(ToString::to_string).collect(),
      ..self
    }
  }

  pub(crate) fn no_index() -> Self {
    Self { indexed: false, ..Self::of(&[]) }
  }

  fn failing(failure: &'static str) -> Self {
    Self {
      failure: Some(failure),
      ..Self::of(&[])
    }
  }

  fn canceled() -> Self {
    Self { canceled: true, ..Self::of(&[]) }
  }

  /// The places of the queries of `command` (`DeclarationAt::argument`), sorted
  fn asked(&self, command: &str) -> Vec<String> {
    let queries = self.queries.lock().unwrap();
    let mut places: Vec<String> = queries.iter().filter(|(of, ..)| *of == command).flat_map(|(_, places, _)| places.clone()).collect();
    places.sort();
    places
  }

  /// The places whose usages were asked
  pub(crate) fn places(&self) -> Vec<String> {
    self.asked("references")
  }

  /// The places whose tests were asked
  pub(crate) fn tested(&self) -> Vec<String> {
    self.asked("tests")
  }

  /// The given tests of each query of `tests`, in the order asked
  pub(crate) fn given(&self) -> Vec<Vec<String>> {
    self.given.lock().unwrap().clone()
  }

  /// The revisions the queries answered at, each once, sorted
  pub(crate) fn revisions(&self) -> Vec<String> {
    let revisions: std::collections::BTreeSet<String> = self.queries.lock().unwrap().iter().map(|(_, _, revision)| revision.clone()).collect();
    revisions.into_iter().collect()
  }

  /// Records a query of `command` of `declarations`; answers whether it answers at all
  fn ask(&self, command: &'static str, declarations: &[DeclarationAt], revision: &str) -> anyhow::Result<bool> {
    self
      .queries
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .push((command, declarations.iter().map(DeclarationAt::argument).collect(), revision.to_string()));
    if self.canceled {
      return Err(SearchCanceled.into());
    }
    if let Some(failure) = self.failure {
      anyhow::bail!("{failure}");
    }
    Ok(self.indexed)
  }
}

impl SymbolIndex for Counts {
  /// No search by name: the fan-in asks by place
  fn references(&self, _repository_root: &str, name: &str, _revision: Option<&str>) -> anyhow::Result<Option<References>> {
    anyhow::bail!("asked {name} by its name")
  }

  fn references_at(&self, _repository_root: &str, declarations: &[DeclarationAt], revision: &str) -> anyhow::Result<Option<Vec<References>>> {
    if !self.ask("references", declarations, revision)? {
      return Ok(None);
    }
    let answers = declarations
      .iter()
      .map(|declaration| {
        if self.unavailable.contains(&declaration.argument()) {
          return References {
            unavailable: true,
            ..References::default()
          };
        }
        let answer = self.answers.get(&declaration.argument()).cloned().unwrap_or_default();
        // the declaration itself is a hit too, which no usage counts
        let hits: Vec<ReferenceHit> = std::iter::once(typed(&declaration.path, declaration.line, "declaration"))
          .chain(answer.iter().cloned())
          .take(LIMIT)
          .collect();
        References {
          hits,
          by_name: 0,
          complete: answer.len() < LIMIT,
          unavailable: false,
        }
      })
      .collect();
    Ok(Some(answers))
  }

  fn tests(&self, _repository_root: &str, declarations: &[DeclarationAt], given: &[String], revision: &str) -> anyhow::Result<Option<Vec<TestReach>>> {
    self.given.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(given.to_vec());
    if !self.ask("tests", declarations, revision)? {
      return Ok(None);
    }
    Ok(Some(
      declarations
        .iter()
        // a place it does not know no given test reaches, every way walked
        .map(|declaration| {
          self.tests.get(&declaration.argument()).cloned().unwrap_or_else(|| TestReach {
            complete: true,
            ..TestReach::default()
          })
        })
        .collect(),
    ))
  }

  /// No declaration lookup: the usages need only references
  fn symbols(&self, _repository_root: &str, _query: &str, _revision: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    Ok(None)
  }

  fn moves(&self, _repository_root: &str, _base: &str, head: &str) -> anyhow::Result<Option<MovesAnswer>> {
    if !self.ask("moves", &[], head)? {
      return Ok(None);
    }
    Ok(Some(MovesAnswer {
      reordered: self.reordered.clone(),
      complete: true,
      ..MovesAnswer::default()
    }))
  }

  /// The modules of the trees of the test repository, never counted among the queries asked
  fn modules(&self, repository_root: &str, paths: &[String], revision: &str) -> anyhow::Result<Option<file_diffs::accio::ModulesAnswer>> {
    Ok(Some(crate::test_support::declared_modules(repository_root, paths, revision)))
  }
}

/// A declaration of `path` named `name` the commits put in another order among those of its owner, from its lines `old`
/// to its lines `new` (from 1, both ends included), its order `order`, as `accio moves` answers it
pub(crate) fn reordered(path: &str, name: &str, old: [u32; 2], new: [u32; 2], order: Order) -> Reordered {
  let at = |[line, end_line]: [u32; 2]| DeclaredAt {
    file: path.to_string(),
    line,
    end_line,
    kind: "function".to_string(),
    name: name.to_string(),
    language: "kotlin".to_string(),
    ..DeclaredAt::default()
  };
  Reordered {
    old: at(old),
    new: at(new),
    order,
    why: String::new(),
  }
}

/// An entry of `name` declared on line 1 of `src/<name>.kt`, asked there at the head
fn change(name: &str, kind: ApiChangeKind) -> ApiChange {
  let path = format!("src/{name}.kt");
  ApiChange {
    path: path.clone(),
    line: 1,
    name: name.to_string(),
    key: format!("a.{name}"),
    kind,
    widened: false,
    changes: Vec::new(),
    before: None,
    after: None,
    usages: None,
    at: DeclarationAt {
      revision: None,
      path: path.clone(),
      line: 1,
      column: None,
    },
    signatures_before: Vec::new(),
    signatures_after: Vec::new(),
    file: path,
  }
}

/// The place of `change` in `Counts`
fn place(name: &str) -> String {
  format!("src/{name}.kt:1")
}

/// The usages of each entry: (name, inside, outside), `None` when not known
fn counts(api: &[ApiChange]) -> Vec<(&str, Option<(u32, u32)>)> {
  api
    .iter()
    .map(|change| (change.name.as_str(), change.usages.as_ref().map(|usages| (usages.inside, usages.outside))))
    .collect()
}

/// `add_usages` with no changed line
fn add(index: Option<&dyn SymbolIndex>, api: &mut [ApiChange]) -> anyhow::Result<Option<String>> {
  add_usages(index, "/repo", "head", || Ok(ChangedLines::default()), api)
}

/// The lines of `paths` changed as a whole
fn whole(paths: &[&str]) -> ChangedLines {
  let mut lines = ChangedLines::default();
  for path in paths {
    lines.add_whole(path);
  }
  lines
}

/// A removal and a change are asked at the head by their places, in one query, and ranked by their usages left outside;
/// an addition is never asked
#[test]
fn the_entries_are_sorted_by_their_usages_left_outside() {
  let index = Counts::of(&[(&place("few"), 1), (&place("many"), 9)]);
  let mut api = vec![
    change("few", ApiChangeKind::Removed),
    change("many", ApiChangeKind::Changed),
    change("fresh", ApiChangeKind::Added),
  ];
  let note = add(Some(&index), &mut api).unwrap();
  assert_eq!(counts(&api), [("many", Some((0, 9))), ("few", Some((0, 1))), ("fresh", None)]);
  assert_eq!(
    (note, index.places(), index.revisions()),
    (None, vec![place("few"), place("many")], vec!["head".to_string()])
  );
}

/// A place accio finds no declaration at (a line it does not index) answers no usages: unknown, never "none left
/// outside"; the other entries of the query keep theirs
#[test]
fn a_place_with_no_declaration_leaves_its_usages_unknown() {
  let index = Counts::of(&[(&place("kept"), 2)]).without_declarations(&[&place("unplaced")]);
  let mut api = vec![change("kept", ApiChangeKind::Changed), change("unplaced", ApiChangeKind::Changed)];
  assert_eq!(add(Some(&index), &mut api).unwrap(), None);
  assert_eq!(counts(&api), [("kept", Some((0, 2))), ("unplaced", None)]);
}

/// Two entries of one place are asked once; a removal is asked at the revision its place is of
#[test]
fn an_entry_is_asked_by_its_place_once() {
  let index = Counts::of(&[("src/Store.kt:4", 2), ("c0ffee^:src/Store.kt:4", 1)]);
  let at = |revision: Option<&str>| DeclarationAt {
    revision: revision.map(str::to_string),
    path: "src/Store.kt".to_string(),
    line: 4,
    column: None,
  };
  let mut api = vec![
    ApiChange {
      at: at(None),
      ..change("open", ApiChangeKind::Changed)
    },
    ApiChange {
      at: at(None),
      ..change("open", ApiChangeKind::Changed)
    },
    ApiChange {
      at: at(Some("c0ffee^")),
      ..change("close", ApiChangeKind::Removed)
    },
  ];
  add(Some(&index), &mut api).unwrap();
  assert_eq!(index.places(), ["c0ffee^:src/Store.kt:4", "src/Store.kt:4"]);
  assert_eq!(counts(&api), [("open", Some((0, 2))), ("open", Some((0, 2))), ("close", Some((0, 1)))]);
}

/// A usage in a file the change touches is updated in this change; one in another file is left outside, and the first
/// `OUTSIDE_SITES` of those are listed by path and line. The declaration itself is no usage. More usages left outside
/// rank first, though fewer in all
#[test]
fn the_usages_split_into_those_inside_the_change_and_those_left_outside() {
  let outside: Vec<(&str, u32)> = (1..=u32::try_from(OUTSIDE_SITES).unwrap() + 2).map(|line| ("lib/Other.kt", line)).collect();
  let index = Counts::calls(&[
    (&place("helper"), vec![("src/A.kt", 1), ("src/B.kt", 1), ("src/A.kt", 2)]),
    (&place("open"), [&[("src/A.kt", 3)][..], &outside].concat()),
  ]);
  let mut api = vec![change("helper", ApiChangeKind::Removed), change("open", ApiChangeKind::Changed)];
  let changed = whole(&["src/A.kt", "src/B.kt"]);
  add_usages(Some(&index), "/repo", "head", || Ok(changed), &mut api).unwrap();
  assert_eq!(counts(&api), [("open", Some((1, 7))), ("helper", Some((3, 0)))]);
  let sites: Vec<UsageSite> = (1..=5)
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
  let index = Counts::of(&[(&place("everywhere"), 250)]);
  let mut api = vec![change("everywhere", ApiChangeKind::Changed)];
  add(Some(&index), &mut api).unwrap();
  let usages = api[0].usages.clone().unwrap();
  assert_eq!((usages.inside, usages.outside, usages.complete, usages.unbroken), (0, 99, false, false));
}

/// A hit only the name ties to the declaration (a receiver nothing typed, another language, a namesake's) is no use
/// and no proof of none: left outside, it is counted apart, and the API is not unbroken; inside the change it is not
/// counted at all
#[test]
fn a_hit_only_the_name_ties_is_no_use_and_keeps_the_api_listed() {
  let index = Counts::hits([(
    place("isEmpty").as_str(),
    vec![
      typed("src/Policy.kt", 3, "call"),
      by_name("lib/Scanner.kt", 7, "call"),
      by_name("lib/scan.py", 2, "call"),
      by_name("src/Policy.kt", 9, "call"),
    ],
  )]);
  let mut api = vec![change("isEmpty", ApiChangeKind::Changed)];
  add_usages(Some(&index), "/repo", "head", || Ok(whole(&["src/Policy.kt"])), &mut api).unwrap();
  let usages = api[0].usages.as_ref().unwrap();
  assert_eq!((usages.inside, usages.outside, usages.by_name, usages.unbroken), (1, 0, 2, false));
}

/// The usages of an entry: the parts that changed, its kind, inside, outside, the paths of the sites outside, complete
type Split<'a> = (Vec<FormChange>, ApiChangeKind, u32, u32, Vec<&'a str>, bool);

/// Only a hit in code is a usage: no import, no doc mention. Of a type whose constructors are its only change, only a
/// call and a subtype count (accio tags a Kotlin `class Sub : Policy(1)` `call` and a Java `extends Policy`
/// `supertype`), and the count is as complete as the answer of accio
#[test]
fn only_code_hits_count_and_of_a_constructor_change_only_calls_and_subtypes() {
  let hits = vec![
    typed("lib/A.kt", 1, "import"),
    typed("lib/A.kt", 2, "type"),
    typed("lib/B.kt", 1, "doc"),
    by_name("docs/policy.md", 1, "doc"),
    typed("lib/C.java", 1, "call"),
    typed("lib/D.kt", 1, "read"),
    typed("src/E.kt", 1, "call"),
    typed("lib/F.kt", 1, "call"),
    typed("lib/G.java", 1, "supertype"),
  ];
  let index = Counts::hits([(place("Policy").as_str(), hits)]);
  let policy = |kind: ApiChangeKind, changes: Vec<FormChange>| ApiChange {
    changes,
    ..change("Policy", kind)
  };
  let mut api = vec![
    policy(ApiChangeKind::Changed, vec![FormChange::Constructors]),
    policy(ApiChangeKind::Changed, vec![FormChange::Constructors, FormChange::Open]),
    policy(ApiChangeKind::Removed, Vec::new()),
  ];
  let changed = whole(&["src/E.kt"]);
  add_usages(Some(&index), "/repo", "head", || Ok(changed.clone()), &mut api).unwrap();
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

  // no call and no subtype outside: no use left outside breaks
  let index = Counts::hits([(place("Policy").as_str(), vec![typed("src/E.kt", 1, "call"), typed("lib/A.kt", 1, "type")])]);
  let mut api = vec![policy(ApiChangeKind::Changed, vec![FormChange::Constructors])];
  add_usages(Some(&index), "/repo", "head", || Ok(changed.clone()), &mut api).unwrap();
  let usages = api[0].usages.as_ref().unwrap();
  assert_eq!((usages.inside, usages.outside, usages.complete, usages.unbroken), (1, 0, true, true));
}

/// A call is inside when any of its lines is one the commits changed: a multi-line call whose arguments the change
/// rewrote, its first line left as it was, is updated in this change
#[test]
fn a_call_is_inside_by_any_of_its_lines() {
  let mut call = typed("src/User.kt", 10, "call");
  call.end_line = Some(13);
  let index = Counts::hits([(place("open").as_str(), vec![call, typed("src/User.kt", 20, "call")])]);
  let mut changed = ChangedLines::default();
  changed.add_ranges("src/User.kt", std::iter::once(12..13).collect());
  let mut api = vec![change("open", ApiChangeKind::Changed)];
  add_usages(Some(&index), "/repo", "head", || Ok(changed), &mut api).unwrap();
  assert_eq!(counts(&api), [("open", Some((1, 1)))]);
}

/// A parameter of a signature: name, type, with a default
fn parameter(name: &str, type_text: &str, optional: bool) -> ParameterForm {
  ParameterForm {
    name: name.to_string(),
    type_text: type_text.to_string(),
    shown_type: ShownType(None),
    optional,
    vararg: false,
  }
}

/// A call left outside that accio says fits the new form, each of whose arguments binds a parameter of the same name
/// and type as before, still binds: it neither breaks nor changes its meaning. A call whose positional argument now
/// binds another parameter, one that misfits, a spread, a named argument of a parameter whose type changed and a call
/// of a change of more than the parameters can break
#[test]
fn a_call_left_outside_that_binds_what_it_bound_before_still_binds() {
  // the constructor of `ProjectFrameUiPolicy` loses its first and its last parameter
  let before = vec![vec![
    parameter("toolWindowToActivate", "String?", true),
    parameter("projectPaneToActivateId", "String?", true),
    parameter("activatesProjectViewOnStartup", "Boolean", true),
    parameter("idsToHide", "Set", true),
  ]];
  let after = vec![vec![
    parameter("projectPaneToActivateId", "String?", true),
    parameter("activatesProjectViewOnStartup", "Boolean", true),
  ]];
  let misfit = ReferenceHit {
    fits: Some(false),
    arguments: Some(vec![CallArgument {
      name: Some("idsToHide".to_string()),
      unbound: true,
      ..CallArgument::default()
    }]),
    ..typed("lib/Hide.kt", 1, "call")
  };
  let spread = fitting(
    "lib/Spread.kt",
    1,
    1,
    vec![CallArgument {
      position: Some(0),
      spread: true,
      parameter: Some("projectPaneToActivateId".to_string()),
      ..CallArgument::default()
    }],
  );
  let hits = vec![
    fitting("lib/Welcome.kt", 54, 57, vec![named("projectPaneToActivateId"), named("activatesProjectViewOnStartup")]),
    fitting("lib/Agent.kt", 3, 3, vec![named("activatesProjectViewOnStartup")]),
    // before, its first argument bound `toolWindowToActivate`
    fitting("lib/Shifted.kt", 1, 1, vec![positional(0, "projectPaneToActivateId")]),
    misfit,
    spread,
  ];
  let index = Counts::hits([(place("ProjectFrameUiPolicy").as_str(), hits)]);
  let policy = |changes: Vec<FormChange>, before: Vec<Vec<ParameterForm>>| ApiChange {
    changes,
    signatures_before: before,
    signatures_after: after.clone(),
    ..change("ProjectFrameUiPolicy", ApiChangeKind::Changed)
  };
  let mut api = vec![
    policy(vec![FormChange::Constructors], before.clone()),
    policy(vec![FormChange::Constructors, FormChange::Reach], before),
  ];
  add(Some(&index), &mut api).unwrap();
  let split: Vec<(u32, u32, Vec<&str>)> = api
    .iter()
    .map(|change| {
      let usages = change.usages.as_ref().unwrap();
      (usages.still_bound, usages.outside, usages.outside_sites.iter().map(|site| site.path.as_str()).collect())
    })
    .collect();
  assert_eq!(
    split,
    [
      // the reach changed too: a call that fits can still break
      (0, 5, vec!["lib/Welcome.kt", "lib/Agent.kt", "lib/Shifted.kt", "lib/Hide.kt", "lib/Spread.kt"]),
      (2, 3, vec!["lib/Shifted.kt", "lib/Hide.kt", "lib/Spread.kt"]),
    ]
  );
  assert!(api.iter().all(|change| !change.usages.as_ref().unwrap().unbroken));

  // a named argument of a parameter whose type changed can break
  let retyped = vec![vec![
    parameter("toolWindowToActivate", "String?", true),
    parameter("projectPaneToActivateId", "String", true),
    parameter("activatesProjectViewOnStartup", "Boolean", true),
  ]];
  let index = Counts::hits([(
    place("ProjectFrameUiPolicy").as_str(),
    vec![fitting("lib/Welcome.kt", 54, 57, vec![named("projectPaneToActivateId")])],
  )]);
  let mut api = vec![policy(vec![FormChange::Constructors], retyped)];
  add(Some(&index), &mut api).unwrap();
  assert_eq!(api[0].usages.as_ref().map(|usages| (usages.still_bound, usages.outside)), Some((0, 1)));

  // every call left outside still binds: no use left outside breaks
  let index = Counts::hits([(
    place("ProjectFrameUiPolicy").as_str(),
    vec![fitting("lib/Welcome.kt", 54, 57, vec![named("activatesProjectViewOnStartup")])],
  )]);
  let mut api = vec![policy(vec![FormChange::Constructors], after.clone())];
  add(Some(&index), &mut api).unwrap();
  let usages = api[0].usages.as_ref().unwrap();
  assert_eq!((usages.still_bound, usages.outside, usages.unbroken), (1, 0, true));
}

/// A trailing lambda binds the last parameter: it still binds when the last parameter keeps its name and type; a
/// positional argument past a vararg binds the vararg
#[test]
fn a_trailing_lambda_and_a_vararg_bind_by_their_place() {
  let block = parameter("block", "Function0", false);
  let before = vec![vec![parameter("name", "String", false), block.clone()]];
  let after = vec![vec![parameter("name", "String", false), parameter("id", "Int", true), block]];
  let lambda = CallArgument {
    trailing_lambda: true,
    parameter: Some("block".to_string()),
    ..CallArgument::default()
  };
  let mut vararg = parameter("paths", "String", false);
  vararg.vararg = true;
  let index = Counts::hits([
    (place("open").as_str(), vec![fitting("lib/A.kt", 1, 3, vec![positional(0, "name"), lambda])]),
    (
      place("join").as_str(),
      vec![fitting("lib/B.kt", 1, 1, vec![positional(0, "paths"), positional(1, "paths")])],
    ),
  ]);
  let mut api = vec![
    ApiChange {
      changes: vec![FormChange::Parameters],
      signatures_before: before,
      signatures_after: after,
      ..change("open", ApiChangeKind::Changed)
    },
    ApiChange {
      changes: vec![FormChange::Parameters],
      signatures_before: vec![vec![vararg.clone()]],
      signatures_after: vec![vec![vararg, parameter("separator", "String", true)]],
      ..change("join", ApiChangeKind::Changed)
    },
  ];
  add(Some(&index), &mut api).unwrap();
  let bound: Vec<(&str, u32, u32)> = api
    .iter()
    .map(|change| {
      let usages = change.usages.as_ref().unwrap();
      (change.name.as_str(), usages.still_bound, usages.outside)
    })
    .collect();
  assert_eq!(bound, [("join", 1, 0), ("open", 1, 0)]);
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

/// A failing query fails no plan: the entries keep no usages, and the note gives the failure
#[test]
fn a_failing_index_keeps_the_api() {
  let index = Counts::failing("index is corrupt");
  let mut api = vec![change("broken", ApiChangeKind::Removed), change("fine", ApiChangeKind::Changed)];
  let note = add(Some(&index), &mut api).unwrap();
  assert_eq!(counts(&api), [("broken", None), ("fine", None)]);
  assert_eq!(note.as_deref(), Some("accio did not answer: index is corrupt"));
}

/// A canceled query (a newer request of the window) cancels the plan
#[test]
fn a_canceled_query_cancels_the_plan() {
  let index = Counts::canceled();
  let mut api = vec![change("open", ApiChangeKind::Changed)];
  let error = add(Some(&index), &mut api).unwrap_err();
  assert!(error.is::<SearchCanceled>(), "{error:?}");
}

/// The usages of a fixture, for the tests of the plan: `outside` usages left outside, none inside, complete
pub(crate) fn outside(outside: u32) -> Usages {
  Usages {
    inside: 0,
    outside,
    outside_sites: (1..=outside.min(5)).map(|line| UsageSite { path: OUTSIDE.to_string(), line }).collect(),
    still_bound: 0,
    by_name: 0,
    complete: true,
    unbroken: outside == 0,
  }
}

/// An API is unbroken only when its count is complete and no use left outside can break
#[test]
fn an_api_is_unbroken_only_by_a_complete_count_with_no_use_left_outside_that_breaks() {
  let changed = whole(&["src/inside.kt"]);
  let index = Counts::calls(&[
    (&place("inside"), vec![("src/inside.kt", 1)]),
    (&place("outside"), vec![("src/inside.kt", 1), ("src/other.kt", 2)]),
  ]);
  let mut api = vec![change("inside", ApiChangeKind::Changed), change("outside", ApiChangeKind::Changed)];
  add_usages(Some(&index), "/repo", "head", || Ok(changed), &mut api).unwrap();
  let unbroken: Vec<(&str, bool)> = api.iter().map(|change| (change.name.as_str(), change.usages.as_ref().unwrap().unbroken)).collect();
  assert_eq!(unbroken, [("outside", false), ("inside", true)]);
}

/// A use is inside only on a line the commits changed: of a file one commit changes, a line of its hunks; of a file two
/// commits change, a line of the blocks of its first blob against its last; of an added file, any line. A use on a line
/// the change left as it was is outside, in a changed file too. The plan asks a change at the line of its name in the
/// last commit that touched it, and accio answers from the head
#[test]
fn a_use_is_inside_only_on_a_line_the_commits_changed() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add api",
    &[
      ("src/a/Api.kt", "package a\n\nfun open(x: Int) {}\n"),
      ("src/a/Twice.kt", "open(1)\nopen(2)\nopen(3)\n"),
      ("src/a/Once.kt", "open(1)\nopen(2)\nopen(3)\n"),
    ],
  );
  let changed = repo.create_commit_with_files(
    "Change api",
    &[
      ("src/a/Api.kt", "package a\n\nfun open(x: Int, y: Int) {}\n"),
      ("src/a/Twice.kt", "open(1)\nopen(2, 0)\nopen(3)\n"),
    ],
  );
  let head = repo.create_commit_with_files(
    "Use api",
    &[
      ("src/a/Twice.kt", "open(1)\nopen(2, 2)\nopen(3)\n"),
      ("src/a/Once.kt", "open(1)\nopen(2)\nopen(3, 3)\n"),
      ("src/a/New.kt", "open(4, 4)\n"),
    ],
  );
  let place = format!("{changed}:src/a/Api.kt:3");
  let index = Counts::calls(&[(
    place.as_str(),
    vec![
      ("src/a/Twice.kt", 1),
      ("src/a/Twice.kt", 2),
      ("src/a/Once.kt", 2),
      ("src/a/Once.kt", 3),
      ("src/a/New.kt", 1),
    ],
  )]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  assert_eq!((index.places(), index.revisions()), (vec![place], vec![head]));
  let usages = plan.api[0].usages.as_ref().unwrap();
  let sites: Vec<(&str, u32)> = usages.outside_sites.iter().map(|site| (site.path.as_str(), site.line)).collect();
  assert_eq!((usages.inside, usages.outside, sites), (3, 2, vec![("src/a/Twice.kt", 1), ("src/a/Once.kt", 2)]));
}

/// An entry of a plan: its name, the parts that changed, and its usages still bound, outside and unbroken
type Bound<'a> = (&'a str, Vec<FormChange>, Option<(u32, u32, bool)>);

/// The constructor of a Kotlin type that loses a parameter: the plan gives the change the parameters of its
/// constructors before and after the commits, so a call left outside that names only the kept ones still binds
#[test]
fn a_constructor_call_left_outside_that_names_only_kept_parameters_still_binds() {
  let repo = TestRepo::new();
  let base = repo.create_commit(
    "Add policy",
    "src/a/Policy.kt",
    "package a\n\nclass Policy(\n  val toActivate: String? = null,\n  val pane: String? = null,\n)\n",
  );
  let head = repo.create_commit("Drop a hook", "src/a/Policy.kt", "package a\n\nclass Policy(\n  val pane: String? = null,\n)\n");
  let place = format!("{head}:src/a/Policy.kt:3");
  let index = Counts::hits([(place.as_str(), vec![fitting("lib/Welcome.kt", 4, 6, vec![named("pane")])])]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  let entries: Vec<Bound<'_>> = plan
    .api
    .iter()
    .map(|change| {
      (
        change.name.as_str(),
        change.changes.clone(),
        change.usages.as_ref().map(|usages| (usages.still_bound, usages.outside, usages.unbroken)),
      )
    })
    .collect();
  // its property goes with the parameter of the constructor
  assert_eq!(
    entries,
    [
      ("toActivate", Vec::new(), Some((0, 0, true))),
      ("Policy", vec![FormChange::Constructors], Some((1, 0, true)))
    ]
  );
}

/// An override of a changed open member in a subtype left outside can break, as accio ties it to the member by its type
#[test]
fn an_override_left_outside_can_break() {
  let index = Counts::hits([(place("start").as_str(), vec![typed("lib/Sub.kt", 4, "override")])]);
  let mut api = vec![ApiChange {
    changes: vec![FormChange::Parameters],
    ..change("start", ApiChangeKind::Changed)
  }];
  add(Some(&index), &mut api).unwrap();
  let usages = api[0].usages.as_ref().unwrap();
  assert_eq!((usages.outside, usages.unbroken), (1, false));
}
