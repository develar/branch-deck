//! `spec-links`: the check of the links of the specs and the design docs to the code of this repository
//! (`pnpm lint:specs`, the crate's `lib.rs`), run from anywhere in the repository; exit 1 with each problem on a line.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let problems = spec_links::check_all(&root);
  if problems.is_empty() {
    return ExitCode::SUCCESS;
  }
  let mut stderr = std::io::stderr().lock();
  for problem in &problems {
    let _ = writeln!(stderr, "{problem}");
  }
  let _ = writeln!(
    stderr,
    "{} broken link(s) of the specs: a [@test] link names a test that exists, a target a file, a rule its spec, a presenter is a target, a link into a spec its heading, the requirements of a group together, no path of the old layout, openspec validate --strict passes, AGENTS.md within its budget (crates/spec-links)",
    problems.len()
  );
  ExitCode::FAILURE
}
