//! The build config that the home branch of a lease changed since the lease forked from it (`brd workspace`,
//! openspec/specs/cli/workspace/spec.md). The build config is the files of the tree that change how every build of the
//! slot runs: no task edits them, so the holder does not see that they changed, and its builds use the old config until
//! `brd lease pick` moves the lease onto the home tip. The fork point is the merge base of the lease HEAD and the home
//! branch, not `source_head`, which a follow does not move. One `diff-tree` since the fork point, and one `cat-file` only
//! when the diff names a build config file.

use crate::lease_clean::BRANCH_DECK_FILE;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cat_file_batch::{BatchEntry, read_objects};
use git_ops::paths::{ChangedPath, changed_paths_since_fork};

/// A file of the build config: its last components, where it counts, and the top-level table that counts in it
struct BuildConfigFile {
  /// The last components of the path, `/`-separated
  name: &'static str,
  /// At the root of the tree only, else at any depth
  root_only: bool,
  /// Only a change of this top-level table counts; `None`: any change of the file
  table: Option<&'static str>,
}

/// The build config. Cargo reads `.cargo/config.toml` (and the old name `.cargo/config`) and `rust-toolchain.toml` (and
/// `rust-toolchain`, through rustup) from the working directory and each parent, so at any depth, and the `profile`
/// table of the root manifest of a workspace; a member manifest has none. A remote run reads the `[host]` table of
/// `.branchdeck.toml` from the commit of the run; the return and the pick read the other tables from the home checkout
const BUILD_CONFIG: &[BuildConfigFile] = &[
  BuildConfigFile {
    name: ".cargo/config.toml",
    root_only: false,
    table: None,
  },
  BuildConfigFile {
    name: ".cargo/config",
    root_only: false,
    table: None,
  },
  BuildConfigFile {
    name: "rust-toolchain.toml",
    root_only: false,
    table: None,
  },
  BuildConfigFile {
    name: "rust-toolchain",
    root_only: false,
    table: None,
  },
  BuildConfigFile {
    name: "Cargo.toml",
    root_only: false,
    table: Some("profile"),
  },
  BuildConfigFile {
    name: BRANCH_DECK_FILE,
    root_only: true,
    table: Some("host"),
  },
];

/// A blob larger than this has an unknown table
const MAX_TABLE_BLOB: usize = 1024 * 1024;

/// The file of the build config that `path` is
fn build_config_file(path: &[u8]) -> Option<&'static BuildConfigFile> {
  let path = std::str::from_utf8(path).ok()?;
  BUILD_CONFIG
    .iter()
    .find(|file| path == file.name || (!file.root_only && path.strip_suffix(file.name).is_some_and(|parent| parent.ends_with('/'))))
}

/// One side of a file as `cat-file` read it: its blob id and bytes (`None`: larger than `MAX_TABLE_BLOB`), or missing
enum Side {
  Blob { id: String, content: Option<Vec<u8>> },
  Missing,
}

impl Side {
  fn of(entry: BatchEntry<'_>) -> Self {
    match entry {
      BatchEntry::Object { id, content, .. } => Self::Blob {
        id: id.to_string(),
        content: Some(content),
      },
      BatchEntry::TooLarge { id, .. } => Self::Blob {
        id: id.to_string(),
        content: None,
      },
      BatchEntry::Missing => Self::Missing,
    }
  }

  /// Both sides have the same blob, or both lack the file
  fn same_file(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Blob { id, .. }, Self::Blob { id: other, .. }) => id == other,
      (Self::Missing, Self::Missing) => true,
      _ => false,
    }
  }

  /// The top-level table `name` of the blob
  fn table(&self, name: &str) -> Table {
    match self {
      Self::Missing => Table::Absent,
      Self::Blob { content: None, .. } => Table::Unknown,
      Self::Blob { content: Some(content), .. } => match std::str::from_utf8(content).ok().and_then(|text| text.parse::<toml::Table>().ok()) {
        None => Table::Unknown,
        Some(mut table) => table.remove(name).map_or(Table::Absent, Table::Known),
      },
    }
  }
}

/// A top-level table of a blob
enum Table {
  /// The blob or the table is missing
  Absent,
  Known(toml::Value),
  /// The blob is larger than `MAX_TABLE_BLOB` or does not parse as TOML: unequal to every table, itself included
  Unknown,
}

impl Table {
  fn same(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Absent, Self::Absent) => true,
      (Self::Known(table), Self::Known(other)) => table == other,
      _ => false,
    }
  }
}

/// The build config files that `home_branch` changed since the fork point of the lease in `slot`, and that the lease
/// has in another form, in path order: one `diff-tree`, and one `cat-file` only when the diff names a build config file.
/// A file whose blob in the lease HEAD is that of the home tip does not count: a change that the holder committed, or
/// a copy of a commit of the lease on a rewritten home branch. A file with a table counts only when the home branch
/// changed that table since the fork point and the table in the lease HEAD is another one. Every failure is an error:
/// no merge base, a home branch that no longer exists
pub fn build_config_changed(git: &GitCommandExecutor, slot: &str, home_branch: &str) -> Result<Vec<String>> {
  let home = format!("refs/heads/{home_branch}");
  let changed = changed_paths_since_fork(git, slot, "HEAD", &home, |path| build_config_file(path).is_some())?;
  if changed.is_empty() {
    return Ok(Vec::new());
  }
  // per file: the lease HEAD, the home tip, and for a table the fork point; a null id is `Missing`
  let files: Vec<(&ChangedPath, &str, &BuildConfigFile)> = changed
    .iter()
    .filter_map(|change| {
      let path = std::str::from_utf8(change.path()).ok()?;
      Some((change, path, build_config_file(change.path())?))
    })
    .collect();
  let mut names = Vec::new();
  for (change, path, file) in &files {
    names.push(format!("HEAD:{path}"));
    names.push(change.new_id().to_string());
    if file.table.is_some() {
      names.push(change.old_id().to_string());
    }
  }
  let mut sides = read_objects(git, slot, &names, MAX_TABLE_BLOB, Side::of)?.into_iter();
  let mut stale = Vec::new();
  for (_, path, file) in files {
    let (Some(lease), Some(new)) = (sides.next(), sides.next()) else { break };
    let fork = file.table.and_then(|_| sides.next());
    if lease.same_file(&new) {
      continue;
    }
    let counts = match (file.table, fork) {
      (Some(table), Some(fork)) => {
        let new = new.table(table);
        !fork.table(table).same(&new) && !lease.table(table).same(&new)
      }
      _ => true,
    };
    if counts {
      stale.push(path.to_string());
    }
  }
  Ok(stale)
}
