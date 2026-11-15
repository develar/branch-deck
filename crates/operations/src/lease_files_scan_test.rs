//! The files of the lease of a worktree (`sync_core::lease_state`: the mark of a sync, the landed state) have one
//! writer, `lease_state.rs` (docs/design/git-operations.md, "Lease"; docs/contributing/git-rules.md), which writes each
//! under the guard of its caller (`LeaseGuard`) and the lock of the lease, and one place that builds their paths,
//! `LeasePaths`. Checked on the production sources:
//! - no file but `lease_state.rs` names a file of the lease beside `branch-deck` (`landed.json`, `synced.d`, `synced`),
//!   whatever builds the path at run time (`join`, `concat!`, `format!`);
//! - the paths of the lease (`LeasePaths`, `LEASE_DIR`) appear only in the modules this table names, each the exact
//!   number of lines its entry says, and none of those modules but `lease_state.rs` writes or removes a file.
//!
//! So a write that skips the guard fails here, a module that takes a path of a file of a lease without an entry fails
//! too, and so does an entry that went stale

use crate::registry_scan_test::report_names;
use crate::remote_write_scan_test::production_sources;

/// The one writer of the files of a lease, which also builds their paths
const WRITER: &str = "crates/sync-core/src/lease_state.rs";

/// The names of the paths of the lease, and the files that may name them, with their exact number of lines
const NAMES: &[(&str, &[(&str, usize)])] = &[
  (
    "LeasePaths",
    &[
      ("crates/sync-core/src/landed.rs", 7),
      ("crates/sync-core/src/repo_facts.rs", 5),
      ("crates/sync-core/src/baseline.rs", 1),
      ("crates/sync-core/src/synced_mark.rs", 3),
      (WRITER, 7),
    ],
  ),
  (
    "LEASE_DIR",
    &[("crates/sync-core/src/landed.rs", 3), ("crates/sync-core/src/repo_facts.rs", 3), (WRITER, 2)],
  ),
];

/// The names of the files of the lease in the directory `branch-deck`, as a literal names them
const LEAVES: &[&str] = &["landed.json", "synced.d", "\"synced\"", "/synced\""];

/// What writes or removes a file, or takes a lock beside one
const WRITES: &[&str] = &[
  "write_atomically(",
  "fs::write(",
  "File::create(",
  "OpenOptions::new(",
  "create_dir_all(",
  "remove_file(",
  "remove_dir_all(",
  "rename(",
  "lock_file(",
  "fs_retry::",
];

/// Whether `line` names `name` as a whole word: `landed_state` is not `landed_states`
fn names(line: &str, name: &str) -> bool {
  let word = |c: char| c.is_alphanumeric() || c == '_';
  line.match_indices(name).any(|(at, _)| {
    let before = line[..at].chars().next_back().is_none_or(|c| !word(c) || !name.starts_with(word));
    let after = line[at + name.len()..].chars().next().is_none_or(|c| !word(c) || !name.ends_with(word));
    before && after
  })
}

fn code_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
  text
    .lines()
    .enumerate()
    .filter(|(_, line)| !line.trim_start().starts_with("//"))
    .map(|(index, line)| (index + 1, line))
}

#[test]
fn only_their_one_writer_writes_the_files_of_a_lease_and_only_the_modules_of_the_table_name_them() {
  let sources = production_sources();
  let mut report = Vec::new();
  for (name, allowed) in NAMES {
    report_names(name, allowed, &sources, |line| names(line, name), &mut report);
  }
  report.extend(leaves_beside_the_lease_dir(&sources));
  let mut handlers: Vec<&str> = NAMES
    .iter()
    .flat_map(|(_, allowed)| allowed.iter().map(|(path, _)| *path))
    .filter(|path| *path != WRITER)
    .collect();
  handlers.sort_unstable();
  handlers.dedup();
  for handler in handlers {
    let Some((_, text)) = sources.iter().find(|(path, _)| path == handler) else { continue };
    for (number, line) in code_lines(text).filter(|(_, line)| WRITES.iter().any(|write| line.contains(write))) {
      report.push(format!("  {handler}:{number} names a file of a lease and writes a file: {}", line.trim()));
    }
  }
  assert!(
    report.is_empty(),
    "a file of a lease is named outside its one writer, a module of the table writes a file, or an entry is stale:\n{}\n\
     Instead: build the paths with `sync_core::lease_state::LeasePaths`, and write and remove a file of a lease only \
     through `sync_core::lease_state` (`write_lease_file`, `remove_lease_file`, `lock_landed`), under the guard of the \
     caller (`LeaseGuard`)",
    report.join("\n")
  );
}

/// The lines of `sources` but the writer's that name a file of the lease (`LEAVES`) in a source that names the
/// directory of the lease (`branch-deck` in a line of code, which only a literal can hold), however the path is put
/// together
fn leaves_beside_the_lease_dir(sources: &[(String, String)]) -> Vec<String> {
  let mut report = Vec::new();
  for (path, text) in sources.iter().filter(|(path, _)| path != WRITER) {
    if !code_lines(text).any(|(_, line)| line.contains("branch-deck")) {
      continue;
    }
    for (number, line) in code_lines(text).filter(|(_, line)| LEAVES.iter().any(|leaf| line.contains(leaf))) {
      report.push(format!("  {path}:{number} names a file of a lease beside `branch-deck`: {}", line.trim()));
    }
  }
  report
}

/// A path of a file of a lease put together at run time is one the scan finds: the probe of a writer that builds it
#[test]
fn a_path_of_a_file_of_a_lease_built_at_run_time_is_found() {
  let source = |text: &str| vec![("crates/update-core/src/pick.rs".to_owned(), text.to_owned())];
  let found = |text: &str| leaves_beside_the_lease_dir(&source(text)).len();
  assert_eq!(found("let state = git_dir.join(\"branch-deck\").join(\"landed.json\");"), 1);
  assert_eq!(found("const MARK: &str = concat!(\"branch-deck/\", \"synced\");"), 1);
  assert_eq!(found("let dir = format!(\"{git_dir}/branch-deck\");\nlet mark = dir.join(\"synced.d\");"), 1);
  assert_eq!(found("let lock = git_dir.join(\"branch-deck\").join(\"stamp\");"), 0, "another file of the directory");
  assert_eq!(found("ops.part(changes.synced.is_some(), \"synced\", &view.synced);"), 0, "no directory of the lease");
}

/// The scan reads a name as a whole word only
#[test]
fn a_name_of_a_path_of_a_lease_is_read_as_a_whole_word() {
  assert!(names("use crate::lease_state::{LEASE_DIR, LeasePaths};", "LeasePaths"));
  assert!(names("use crate::lease_state::{LEASE_DIR, LeasePaths};", "LEASE_DIR"));
  assert!(!names("struct MyLeasePaths;", "LeasePaths"));
  assert!(!names("const LEASE_DIRS: u8 = 0;", "LEASE_DIR"));
}
