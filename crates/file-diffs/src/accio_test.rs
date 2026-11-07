//! The accio process of a lookup: waited for as long as it runs, killed once the navigation is canceled, below the app
//! for a plan read ahead of its window

#[cfg(unix)]
use crate::accio::AccioSearch;
use crate::accio::{AccioCli, CallArgument, DeclarationAt, Lenient, Resolved, SearchCanceled, SymbolHit, SymbolIndex};
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use test_log::test;
use test_utils::fake_executable::{fake_executable, shell_path};

const ROOT: &str = "/work/repository";

/// A fake accio in `dir` that touches `started` and then runs `then`
fn fake_accio(dir: &Path, started: &Path, then: &str) -> AccioCli {
  AccioCli::at(fake_executable(dir, "accio", &format!(": > {}\n{then}", shell_path(started))))
}

/// Waits until accio touched `started`: the query runs
fn wait_for(started: &Path) {
  while !started.exists() {
    std::thread::sleep(Duration::from_millis(10));
  }
}

#[test]
fn waits_for_a_slow_index_as_long_as_it_runs() {
  let dir = tempfile::tempdir().unwrap();
  let (started, go) = (dir.path().join("started"), dir.path().join("go"));
  // accio answers only once the test lets it, as one on a loaded machine: no deadline cuts the query meanwhile
  let answer = r#"[{"hits":[{"file":"src/Base.kt","line":1,"kind":"function","name":"outsider"}]}]"#;
  let accio = fake_accio(
    dir.path(),
    &started,
    &format!("while [ ! -e {} ]; do sleep 0.05; done\nprintf '%s' '{answer}'", shell_path(&go)),
  );
  let canceled = || false;
  let search = accio.search(&canceled);
  let hits = std::thread::scope(|scope| {
    let query = scope.spawn(|| search.symbols(ROOT, "outsider", Some("HEAD")));
    wait_for(&started);
    assert!(!query.is_finished(), "the query answered before accio did");
    std::fs::write(&go, "").unwrap();
    query.join().unwrap()
  });
  assert_eq!(
    hits.unwrap(),
    Some(vec![SymbolHit {
      path: "src/Base.kt".to_string(),
      line: 1,
      column: None,
      kind: "function".to_string(),
      name: "outsider".to_string(),
      parent: None,
    }])
  );
}

#[test]
fn a_canceled_search_kills_its_query() {
  let dir = tempfile::tempdir().unwrap();
  let started = dir.path().join("started");
  let accio = fake_accio(dir.path(), &started, "exec sleep 600");
  let flag = AtomicBool::new(false);
  let canceled = || flag.load(Ordering::SeqCst);
  let search = accio.search(&canceled);
  let (result, waited) = std::thread::scope(|scope| {
    let query = scope.spawn(|| {
      let result = search.references(ROOT, "helper", None);
      (result, Instant::now())
    });
    wait_for(&started);
    let canceled_at = Instant::now();
    flag.store(true, Ordering::SeqCst);
    let (result, answered_at) = query.join().unwrap();
    (result, answered_at - canceled_at)
  });
  let error = result.unwrap_err();
  assert!(error.is::<SearchCanceled>(), "{error:#}");
  // killed within the poll interval, not after the 600 s of its sleep
  assert!(waited < Duration::from_secs(60), "{waited:?}");
}

/// The queries of a plan read ahead of its window run below the app, at nice 19, which the processes accio starts
/// inherit; those of a search the user waits for run at the priority of the app, whatever the one the tests run at
#[cfg(unix)]
#[test]
fn a_search_in_the_background_runs_its_queries_at_the_lowest_priority() {
  let dir = tempfile::tempdir().unwrap();
  let niceness = dir.path().join("niceness");
  // its own niceness, then the one of the test that started it
  let body = format!(
    "ps -o nice= -p $$ > {path}\nps -o nice= -p $PPID >> {path}\nprintf '%s' '[{{\"hits\":[]}}]'",
    path = shell_path(&niceness)
  );
  let accio = AccioCli::at(fake_executable(dir.path(), "accio", &body));
  let canceled = || false;
  let read = |search: AccioSearch<'_>| -> (i32, i32) {
    assert_eq!(search.symbols(ROOT, "outsider", None).unwrap(), Some(Vec::new()));
    let text = std::fs::read_to_string(&niceness).unwrap();
    let values: Vec<i32> = text.split_whitespace().map(|value| value.parse().unwrap()).collect();
    (values[0], values[1])
  };
  let (normal, test) = read(accio.search(&canceled));
  assert_eq!(normal, test, "a search the user waits for runs as the app");
  assert_eq!(read(accio.search(&canceled).in_background()), (19, test));
}

/// Every query runs with `--no-build`; accio without a usable index answers `available: false`, which the lookup answers
/// as no index, its reason kept for the plan, never as a failure
#[test]
fn an_index_accio_cannot_use_answers_no_index_with_its_reason() {
  let dir = tempfile::tempdir().unwrap();
  let started = dir.path().join("started");
  let unavailable = r#"[{"available":false,"note":"accio is building the index of this repository","target":"helper"}]"#;
  let accio = fake_accio(
    dir.path(),
    &started,
    &format!("[ \"$2\" = --no-build ] || {{ echo \"no --no-build: $*\" >&2; exit 2; }}\nprintf '%s' '{unavailable}'"),
  );
  let canceled = || false;
  let search = accio.search(&canceled);
  let lenient = Lenient::new(&search);
  assert_eq!(lenient.indexed_references(ROOT, "helper", Some("HEAD")).unwrap(), None);
  assert_eq!(lenient.unavailable().as_deref(), Some("accio is building the index of this repository"));
  assert_eq!(lenient.error(), None);
}

#[test]
fn an_index_format_mismatch_says_which_accio_version_to_run() {
  let dir = tempfile::tempdir().unwrap();
  let canceled = || false;
  for (note, version, action) in [
    ("archive /Users/develar/.cache/accio/index.shard has format 34, expected 32", "a newer", "update accio"),
    ("archive /Users/develar/.cache/accio/index.shard has format 30, expected 32", "an older", "run accio index"),
  ] {
    let unavailable = format!(r#"[{{"available":false,"note":"{note}","target":"helper"}}]"#);
    let executable = fake_executable(dir.path(), "accio", &format!("printf '%s' '{unavailable}'"));
    let accio = AccioCli::at(executable.clone());
    let search = accio.search(&canceled);
    let lenient = Lenient::new(&search);
    assert_eq!(lenient.indexed_references(ROOT, "helper", Some("HEAD")).unwrap(), None);
    let expected = format!("the index of this repository is from {version} accio than {}: {action}", executable.display());
    assert_eq!(lenient.unavailable().as_deref(), Some(expected.as_str()));
  }
}

/// The places of a plan are asked in one process, `--at` each, at the revision of the answer: one entry per place in
/// their order, each hit with what ties it to the declaration, and a call with its last line, its arguments, the
/// parameter each binds and whether it fits
#[test]
fn the_places_of_a_plan_are_asked_in_one_process() {
  let dir = tempfile::tempdir().unwrap();
  let (started, args) = (dir.path().join("started"), dir.path().join("args"));
  let call = r#"{"file":"lib/Welcome.kt","line":54,"end_line":57,"column":3,"kind":"call","resolved":"typed","fits":true,"arguments":[{"name":"pane","parameter":"pane"},{"position":1,"parameter":"id"}]}"#;
  let answer = format!(
    r#"[{{"at":"c1:src/Policy.kt:3","complete":true,"hits":[{call},{{"file":"lib/a.py","line":2,"column":1,"kind":"call","resolved":"name"}}]}},{{"at":"c0^:src/Old.kt:5","complete":false,"hits":[]}}]"#
  );
  let accio = fake_accio(dir.path(), &started, &format!("printf '%s\\n' \"$@\" > {}\nprintf '%s' '{answer}'", shell_path(&args)));
  let canceled = || false;
  let search = accio.search(&canceled);
  let places = [
    DeclarationAt {
      revision: Some("c1".to_string()),
      path: "src/Policy.kt".to_string(),
      line: 3,
    },
    DeclarationAt {
      revision: Some("c0^".to_string()),
      path: "src/Old.kt".to_string(),
      line: 5,
    },
  ];
  let answers = search.references_at(ROOT, &places, "head").unwrap().unwrap();
  let asked = std::fs::read_to_string(&args).unwrap();
  let asked: Vec<&str> = asked.lines().collect();
  assert_eq!(
    asked,
    [
      "references",
      "--no-build",
      "--format",
      "json",
      "--project",
      ROOT,
      "--limit",
      "100",
      "--ref",
      "head",
      "--at",
      "c1:src/Policy.kt:3",
      "--at",
      "c0^:src/Old.kt:5"
    ]
  );
  let hits: Vec<(&str, Resolved, Option<u32>, Option<bool>)> = answers[0].hits.iter().map(|hit| (hit.path.as_str(), hit.resolved, hit.end_line, hit.fits)).collect();
  assert_eq!(hits, [("lib/Welcome.kt", Resolved::Typed, Some(57), Some(true)), ("lib/a.py", Resolved::Name, None, None)]);
  assert_eq!(
    answers[0].hits[0].arguments.as_deref().unwrap()[1],
    CallArgument {
      position: Some(1),
      parameter: Some("id".to_string()),
      ..CallArgument::default()
    }
  );
  assert_eq!((answers[1].hits.len(), answers[1].complete), (0, false));
}
