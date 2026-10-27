//! Two processes of one repository admitted through its operations registry (docs/design/git-operations.md, "Operations
//! registry"): a `bd safe-push` of a child process, held at its start by the fake Patronus, and `bd` of the test process
//! beside it. The child is this test binary again, running `a_safe_push_of_another_process` with what it needs in
//! `CHILD_ENV`

use crate::fixture_test::{PREFIX, assert_error, environment, run_json};
use crate::remote_fixture_test::{Space, safe_push};
use bd_core::context::Environment;
use operations::registry_file::ProcessRegistry;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Child;

/// The environment variable that makes `a_safe_push_of_another_process` run: `<repository>|<home>|<Patronus URL>`
const CHILD_ENV: &str = "BD_CROSS_PROCESS_CHILD";

/// The child of `a_safe_push_beside_the_safe_push_of_another_process_is_busy_with_its_holder`: `bd safe-push auth` of
/// another process; does nothing in a normal run
#[test_log::test]
fn a_safe_push_of_another_process() {
  let Ok(spec) = std::env::var(CHILD_ENV) else { return };
  let [repo, home, patronus_url] = spec.split('|').collect::<Vec<_>>().try_into().unwrap();
  let environment = Environment {
    home: Some(PathBuf::from(home)),
    patronus_url: patronus_url.to_string(),
    ..environment(Path::new(repo))
  };
  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_eq!(exit_code, 0, "{output}");
}

/// A child that is killed when the test ends, so a failure never leaves it waiting at its start
struct Killed(Child);

impl Drop for Killed {
  fn drop(&mut self) {
    let _ = self.0.kill();
    let _ = self.0.wait();
  }
}

/// While the `bd safe-push auth` of another process waits for Patronus to answer its start, a `bd safe-push auth` here is
/// `busy` (exit 3, retryable), naming the operation of the other process, its pid and what it runs, and starts nothing; a
/// push of another branch runs beside it. Once Patronus answered, the other one ends and leaves the registry empty
#[test_log::test]
fn a_safe_push_beside_the_safe_push_of_another_process_is_busy_with_its_holder() {
  let space = Space::new(&["(auth) Add auth", "(docs) Add docs"]);
  let start = space.fake.hold("POST", "/app/rest/v1/robots");
  let spec = [space.repo.path().to_str().unwrap(), space.home.path().to_str().unwrap(), &space.patronus_url].join("|");
  let child = std::process::Command::new(std::env::current_exe().unwrap())
    .args(["--exact", "cross_process_test::a_safe_push_of_another_process", "--nocapture"])
    .env(CHILD_ENV, spec)
    .spawn()
    .unwrap();
  let mut child = Killed(child);
  space.runtime.block_on(start.arrived());
  let registry = ProcessRegistry::of(&space.repo.path().join(".git"));
  let records = registry.records().unwrap().unwrap();
  let [held] = records.as_slice() else {
    panic!("one record of the child: {records:?}");
  };
  assert_eq!(held.pid, child.0.id());

  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (3, "busy"));
  let holder = &output["error"]["holder"];
  assert_eq!(
    (
      holder["op"].clone(),
      holder["tool"].clone(),
      holder["pid"].clone(),
      holder["kind"].clone(),
      holder["subject"].clone()
    ),
    (json!(held.id.0), json!("bd"), json!(child.0.id()), json!("safe_push"), json!("auth")),
    "{output}"
  );
  assert_eq!(output["error"]["retryable"], json!(true));
  assert!(output["error"]["message"].as_str().unwrap().contains("has run the Safe Push of auth"), "{output}");
  // another branch runs beside it
  let (exit_code, output) = run_json(&space.environment(), &["--prefix", PREFIX, "push", "docs"]);
  assert_eq!(exit_code, 0, "{output}");

  start.open();
  assert!(child.0.wait().unwrap().success(), "the safe push of the other process failed");
  assert_eq!(registry.records().unwrap(), Some(Vec::new()));
  // the start of the other process alone reached Patronus
  assert_eq!(space.fake.starts.lock().unwrap().len(), 1);
}
