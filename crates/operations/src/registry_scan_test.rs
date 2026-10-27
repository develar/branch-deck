//! The operations registry of a repository has one writer (docs/design/git-operations.md, "Operations registry"): the
//! names of its file and of its lock appear in the production sources only in `registry_file.rs`, each the exact number
//! of times its entry says, so a second reader or writer of the file, which would skip its lock or its schema, fails
//! here, and so does an entry that went stale

use crate::remote_write_scan_test::production_sources;

/// A name of the registry and the files that may name it, with their exact number of lines
const NAMES: &[(&str, &[(&str, usize)])] = &[
  ("operations.json", &[("crates/operations/src/registry_file.rs", 1)]),
  ("operations.lock", &[("crates/operations/src/registry_file.rs", 1)]),
];

#[test]
fn only_the_registry_file_names_the_files_of_the_operations_registry() {
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
    "the files of the operations registry are named outside its writer, or an entry is stale:\n{}\nInstead: read and change \
     the registry through `operations::registry_file::ProcessRegistry`",
    report.join("\n")
  );
}
