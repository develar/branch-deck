//! The accio process of a lookup: waited for as long as it runs, killed once the navigation is canceled

use crate::accio::{AccioCli, SearchCanceled, SymbolHit, SymbolIndex, index_dir};
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use test_log::test;
use test_utils::env_vars::EnvVars;
use test_utils::fake_executable::{fake_executable, shell_path};

const ROOT: &str = "/work/repository";

/// An index of `ROOT` in `home`, as `query` looks for it before it starts accio; and a fake accio that touches
/// `started` and then runs `then`
fn indexed_accio(home: &Path, started: &Path, then: &str) -> AccioCli {
  let index = index_dir(ROOT).unwrap();
  assert!(index.starts_with(home), "{} is outside the home of the test", index.display());
  std::fs::create_dir_all(&index).unwrap();
  std::fs::write(index.join("meta.json"), "{}").unwrap();
  AccioCli::at(fake_executable(home, "accio", &format!(": > {}\n{then}", shell_path(started))))
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
  let _env = EnvVars::set(&[("ACCIO_HOME", dir.path())]);
  let (started, go) = (dir.path().join("started"), dir.path().join("go"));
  // accio answers only once the test lets it, as one on a loaded machine: no deadline cuts the query meanwhile
  let answer = r#"{"hits":[{"file":"src/Base.kt","line":1,"kind":"function","name":"outsider"}]}"#;
  let accio = indexed_accio(
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
  let _env = EnvVars::set(&[("ACCIO_HOME", dir.path())]);
  let started = dir.path().join("started");
  let accio = indexed_accio(dir.path(), &started, "exec sleep 600");
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
