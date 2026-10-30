//! The accio process of a lookup: waited for as long as it runs, killed once the navigation is canceled

use crate::accio::{AccioCli, Lenient, SearchCanceled, SymbolHit, SymbolIndex};
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
  let answer = r#"{"hits":[{"file":"src/Base.kt","line":1,"kind":"function","name":"outsider"}]}"#;
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

/// Every query runs with `--no-build`; accio without a usable index answers `available: false`, which the lookup answers
/// as no index, its reason kept for the plan, never as a failure
#[test]
fn an_index_accio_cannot_use_answers_no_index_with_its_reason() {
  let dir = tempfile::tempdir().unwrap();
  let started = dir.path().join("started");
  let unavailable = r#"{"available":false,"note":"accio is building the index of this repository","target":"helper"}"#;
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
