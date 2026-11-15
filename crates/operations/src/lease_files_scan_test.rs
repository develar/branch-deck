//! The files of the lease of a worktree (`sync_core::lease_state`: the mark of a sync, the landed state) have one
//! writer, `lease_state.rs` (docs/design/git-operations.md, "Lease"; docs/contributing/git-rules.md), which writes each
//! under the guard of its caller (`LeaseGuard`): a run of the app's session writes only while the slot holds the lease
//! the run began under.
//! Checked on the production sources: the names of the files and the paths the sync carries of them appear only in the
//! modules this table names, each the exact number of lines its entry says, and none of those modules but
//! `lease_state.rs` writes or removes a file, so a write that skips the guard fails here, a module that takes a path of
//! a file of a lease without an entry fails too, and so does an entry that went stale

use crate::registry_scan_test::report_names;
use crate::remote_write_scan_test::production_sources;

/// The one writer of the files of a lease
const WRITER: &str = "crates/sync-core/src/lease_state.rs";

/// A name and where it is looked for: everywhere, or in the crate whose fields and methods it names (`pub(crate)`, so
/// no other crate can name them)
struct Name {
  text: &'static str,
  within: &'static str,
  /// The files that may name it, with their exact number of lines
  allowed: &'static [(&'static str, usize)],
}

const SYNC_CORE: &str = "crates/sync-core/src/";

const NAMES: &[Name] = &[
  Name {
    text: "\"branch-deck/landed.json\"",
    within: "",
    allowed: &[("crates/sync-core/src/landed.rs", 1)],
  },
  Name {
    text: "\"branch-deck/synced\"",
    within: "",
    allowed: &[("crates/sync-core/src/synced_mark.rs", 1)],
  },
  Name {
    text: "\"branch-deck/synced.d\"",
    within: "",
    allowed: &[("crates/sync-core/src/synced_mark.rs", 1)],
  },
  Name {
    text: "LANDED_FILE",
    within: "",
    allowed: &[("crates/sync-core/src/landed.rs", 3), ("crates/sync-core/src/repo_facts.rs", 3), (WRITER, 2)],
  },
  Name {
    text: "SYNCED_FILE",
    within: "",
    allowed: &[("crates/sync-core/src/synced_mark.rs", 2), ("crates/sync-core/src/repo_facts.rs", 2), (WRITER, 2)],
  },
  Name {
    text: "SYNCED_DIR",
    within: "",
    allowed: &[("crates/sync-core/src/synced_mark.rs", 3), (WRITER, 2)],
  },
  Name {
    text: "LEASE_FILES",
    within: "",
    allowed: &[(WRITER, 2)],
  },
  // the path of the landed state in the facts of a sync and in its update check
  Name {
    text: "landed_state",
    within: SYNC_CORE,
    allowed: &[
      ("crates/sync-core/src/repo_facts.rs", 7),
      ("crates/sync-core/src/baseline.rs", 1),
      ("crates/sync-core/src/sync.rs", 1),
    ],
  },
  // the path of the mark in the facts of a sync
  Name {
    text: "synced_mark()",
    within: SYNC_CORE,
    allowed: &[("crates/sync-core/src/sync.rs", 1)],
  },
];

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
  for Name { text, within, allowed } in NAMES {
    let scanned: Vec<(String, String)> = sources.iter().filter(|(path, _)| path.starts_with(within)).cloned().collect();
    report_names(text, allowed, &scanned, |line| names(line, text), &mut report);
  }
  let mut handlers: Vec<&str> = NAMES
    .iter()
    .flat_map(|name| name.allowed.iter().map(|(path, _)| *path))
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
    "a file of a lease is named outside the modules of the table, a module of the table writes a file, or an entry is \
     stale:\n{}\nInstead: write and remove a file of a lease only through `sync_core::lease_state` (`write_lease_file`, \
     `remove_lease_file`, `lock_landed`), under the guard of the caller (`LeaseGuard`)",
    report.join("\n")
  );
}

/// The scan reads a name as a whole word only
#[test]
fn a_name_of_a_file_of_a_lease_is_read_as_a_whole_word() {
  assert!(names("let path = facts.landed_state;", "landed_state"));
  assert!(!names("view.landed_states", "landed_state"));
  assert!(!names("fn my_landed_state()", "landed_state"));
  assert!(names("let mark = facts.synced_mark();", "synced_mark()"));
  assert!(names("use crate::landed::LANDED_FILE;", "LANDED_FILE"));
  assert!(!names("PREVIOUS_LANDED_FILE", "LANDED_FILE"));
}
