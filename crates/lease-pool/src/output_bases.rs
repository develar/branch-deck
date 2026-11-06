//! The Bazel output bases of the workspaces of the pool, which live outside the slots: Bazel keeps one per workspace
//! under its output user root (`<root>/<md5 of the workspace path>`, `--output_user_root`), and the clean rules of a
//! return, which work inside the slot, never reach it. A slot of idea-1 has three (the slot, `community/`,
//! `community/platform/build-scripts/bazel`), up to 62 GB each, and a return deletes the `bazel-*` links into them
//! (`out/` of the clean rules): so a root is found from the `bazel-out` link of any checkout that still has one (the
//! source checkout, a leased slot), and each output base of a root names its workspace in its `DO_NOT_BUILD_HERE`.
//! The trim of the pool (`trim`) deletes the output bases of the slots it deletes and of workspaces in a pool that
//! are gone.

use crate::bazel_servers::{output_base, server, workspace_links};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The file of an output base that names its workspace, as Bazel writes it
const WORKSPACE_FILE: &str = "DO_NOT_BUILD_HERE";

/// An output base under a root, with the workspace it names
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputBase {
  /// The directory, canonical
  pub path: PathBuf,
  /// The workspace its `DO_NOT_BUILD_HERE` names, as Bazel wrote it
  pub workspace: PathBuf,
}

impl OutputBase {
  /// Whether its workspace is `dir` or in it
  pub fn is_in(&self, dir: &Path) -> bool {
    self.workspace.starts_with(dir)
  }

  /// Whether a Bazel server runs on it, told by its pid file (`bazel_servers::server`): its deletion waits for a later
  /// trim
  pub fn has_server(&self) -> bool {
    server(&self.workspace, &self.path).is_some()
  }
}

/// The output user roots that the `bazel-out` links of the workspaces of `checkouts` lead into: a checkout and each
/// directory right in it with a workspace file (`community/`), the link `bazel-out` or `out/bazel-out`
/// (`--symlink_prefix=out/bazel-`), as `bazel_servers` reads them. Sorted, each once
pub fn output_user_roots<'a>(checkouts: impl IntoIterator<Item = &'a Path>) -> Vec<PathBuf> {
  let mut roots = BTreeSet::new();
  for checkout in checkouts {
    for link in workspace_links(checkout).into_iter().flat_map(|(_, links)| links) {
      if let Some(root) = output_base(&link).and_then(|base| base.parent().map(Path::to_path_buf)) {
        roots.insert(root);
      }
    }
  }
  roots.into_iter().collect()
}

/// The output bases of `roots`: each directory with a `DO_NOT_BUILD_HERE` that names an absolute workspace; one read
/// of a small file each, no walk
pub fn output_bases(roots: &[PathBuf]) -> Vec<OutputBase> {
  let mut bases = Vec::new();
  for root in roots {
    let Ok(entries) = std::fs::read_dir(root) else { continue };
    for entry in entries.flatten() {
      let path = entry.path();
      let Ok(text) = std::fs::read_to_string(path.join(WORKSPACE_FILE)) else { continue };
      let workspace = PathBuf::from(text.trim());
      if workspace.is_absolute() {
        bases.push(OutputBase {
          path: std::fs::canonicalize(&path).unwrap_or(path),
          workspace,
        });
      }
    }
  }
  bases.sort_by(|a, b| a.path.cmp(&b.path));
  bases
}

/// The output bases of `bases` whose slot is gone: the workspace is in one of `pools`, and its slot there
/// (`<pool>/<n>/<name>`) is not, as a slot removed by hand or by a brd before the trim leaves it. A workspace of a slot
/// that is there stays, also one that the commit of the slot lacks (`community/` of an older commit): the next get
/// may move the slot back to it, and the pool deletes it with the slot
pub fn orphans<'a>(bases: &'a [OutputBase], pools: &[PathBuf]) -> Vec<&'a OutputBase> {
  bases
    .iter()
    .filter(|base| {
      pools.iter().any(|pool| {
        let Ok(inside) = base.workspace.strip_prefix(pool) else { return false };
        let slot: PathBuf = inside.components().take(2).collect();
        slot.components().count() == 2 && !pool.join(slot).is_dir()
      })
    })
    .collect()
}

#[cfg(test)]
#[path = "output_bases_test.rs"]
mod output_bases_test;
