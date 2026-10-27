//! The operations registry of a repository has one writer (docs/design/git-operations.md, "Operations registry"): the
//! names of its file and of its lock appear in the production sources only in `registry_file.rs`, each the exact number
//! of times its entry says, so a second reader or writer of the file, which would skip its lock or its schema, fails
//! here, and so does an entry that went stale. The journal beside it ("Journal") has one writer the same way,
//! `journal.rs`, which the registry calls under its lock. The fetch lock, which a fetch of the baseline joins ("Update project"), has
//! one taker the same way: `sync_core::fetch_lock`, so no fetch of the baseline runs beside it unjoined

use crate::remote_write_scan_test::production_sources;

/// A name of the registry and the files that may name it, with their exact number of lines
const NAMES: &[(&str, &[(&str, usize)])] = &[
  ("operations.json", &[("crates/operations/src/registry_file.rs", 1)]),
  ("operations.lock", &[("crates/operations/src/registry_file.rs", 1)]),
  ("journal.jsonl", &[("crates/operations/src/journal.rs", 1)]),
  ("fetch.lock", &[("crates/sync-core/src/fetch_lock.rs", 1)]),
];

#[test]
fn only_their_writers_name_the_files_of_the_operations_registry_its_journal_and_the_fetch_lock() {
  let sources = production_sources();
  let mut report = Vec::new();
  for (name, allowed) in NAMES {
    for (path, _) in allowed.iter().filter(|(path, _)| !sources.iter().any(|(source, _)| source == path)) {
      report.push(format!("  {name}: {path} is no production source"));
    }
    for (path, text) in &sources {
      let lines: Vec<usize> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//") && line.contains(name))
        .map(|(index, _)| index + 1)
        .collect();
      let expected = allowed.iter().find(|(allowed, _)| allowed == path).map_or(0, |(_, count)| *count);
      if lines.len() != expected {
        report.push(format!("  {name}: {path}: lines {lines:?}, but its entry says {expected}"));
      }
    }
  }
  assert!(
    report.is_empty(),
    "the files of the operations registry, its journal or the fetch lock are named outside their writer, or an entry is stale:\n{}\n\
     Instead: read and change the registry through `operations::registry_file::ProcessRegistry`, fetch the baseline \
     through `sync_core::baseline::fetch_baseline`",
    report.join("\n")
  );
}
