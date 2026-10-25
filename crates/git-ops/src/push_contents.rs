//! The files a push would send: the blobs that the commits of the pushed tips add or change and that no
//! remote-tracking branch of the remote reaches, with their sizes. One `log --raw -z` of the commits and one
//! `cat-file --batch-check` of their blobs (0.3 s on the monorepo for 106 commits), not a `rev-list --objects` walk of
//! every tree (1.1 s), which the push's own `pack-objects` makes again (docs/design/git-operations.md, "Push size").

use crate::cat_file_batch::object_sizes;
use anyhow::{Result, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashSet;
use tracing::instrument;

/// A file a push sends: a blob at the path a commit added or changed it at
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushedFile {
  pub path: String,
  /// The size of the blob, uncompressed
  pub size: u64,
}

/// The id of no object: the `<dst>` of a deleted file
const NULL_ID: &str = "0000000000000000000000000000000000000000";

/// The mode of a gitlink: a commit of a submodule, which the push does not send
const GITLINK_MODE: &str = "160000";

/// The files the commits of `tips` add or change, the commits that no remote-tracking branch of `remote` reaches
/// (`--not --remotes=<remote>`, what the remote has as far as this clone knows): each blob once, at the first path a
/// commit gave it, a parent before its children. A file the remote has under another commit counts too, a slight overcount of
/// what the push sends; a deleted file and a gitlink count not. `tips` are full ids or refs of this repository, never
/// text from outside
#[instrument(skip(git, tips), fields(tips = tips.len()))]
pub fn push_contents(git: &GitCommandExecutor, repository_path: &str, tips: &[&str], remote: &str) -> Result<Vec<PushedFile>> {
  if tips.is_empty() {
    return Ok(Vec::new());
  }
  if tips
    .iter()
    .chain([&remote])
    .any(|name| name.is_empty() || name.starts_with('-') || name.contains(['\n', '\0']))
  {
    bail!("A tip or the remote of a push is no name: {tips:?}, {remote:?}");
  }
  let mut input = String::new();
  for tip in tips {
    input.push_str(tip);
    input.push('\n');
  }
  input.push_str("--not\n--remotes=");
  input.push_str(remote);
  input.push('\n');
  let output = git
    .cmd(
      &["log", "-z", "--raw", "--no-renames", "--no-abbrev", "--topo-order", "--reverse", "--format=", "--stdin"],
      repository_path,
    )
    .stdin(input.as_bytes())
    .bytes()?;
  let added = added_blobs(&String::from_utf8_lossy(&output));
  let sizes = object_sizes(git, repository_path, &added.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>())?;
  Ok(
    added
      .into_iter()
      .zip(sizes)
      .filter_map(|((_, path), size)| Some(PushedFile { path, size: size? }))
      .collect(),
  )
}

/// `(blob, path)` of each file the raw diffs add or change, each blob once: `:<mode> <mode> <src> <dst> <status>`, NUL,
/// the path, NUL; the empty fields between the commits are skipped
fn added_blobs(output: &str) -> Vec<(String, String)> {
  let mut seen = HashSet::new();
  let mut added = Vec::new();
  let mut fields = output.split('\0');
  while let Some(field) = fields.next() {
    let Some(header) = field.trim_start_matches('\n').strip_prefix(':') else {
      continue;
    };
    let Some(path) = fields.next() else {
      break;
    };
    let mut parts = header.split(' ');
    let (Some(_), Some(mode), Some(_), Some(id)) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
      continue;
    };
    if id != NULL_ID && mode != GITLINK_MODE && seen.insert(id.to_string()) {
      added.push((id.to_string(), path.to_string()));
    }
  }
  added
}
