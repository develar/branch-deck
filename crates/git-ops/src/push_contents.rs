//! The files a push would add to the remote and the bytes each adds: each path the commits of the pushed tips add or
//! change once, by its largest version less the remote's version of the path; the commits are those no remote-tracking
//! branch of the remote reaches. One `log --raw -z` of the commits and one `cat-file --batch-check` of their blobs (0.3 s
//! on the monorepo for 106 commits), not a `rev-list --objects` walk of every tree (1.1 s), which the push's own
//! `pack-objects` makes again (docs/design/git-operations.md, "Push size").

use crate::cat_file_batch::object_sizes;
use anyhow::{Result, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{HashMap, HashSet};
use tracing::instrument;

/// A file a push adds: a path its commits add or change, with the bytes it adds
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushedFile {
  pub path: String,
  /// The bytes the file adds, uncompressed: its largest version in the push less the remote's version of the path
  pub size: u64,
}

/// The id of no object: the `<src>` of an added file, the `<dst>` of a deleted one
const NULL_ID: &str = "0000000000000000000000000000000000000000";

/// What `--format=tformat:@` prints before the diffs of each commit
const COMMIT_MARK: &str = "@";

/// The mode of a gitlink: a commit of a submodule, which the push does not send
const GITLINK_MODE: &str = "160000";

/// The files the commits of `tips` add or change, the commits that no remote-tracking branch of `remote` reaches
/// (`--not --remotes=<remote>`, what the remote has as far as this clone knows), a parent before its children: each path
/// once, in the order the commits first touch it, by the bytes it adds. A path adds its largest version, even one a later
/// commit deletes (the pack carries it), less the remote's version of the path (the `<src>` of its first diff), never
/// below nothing; a path that adds nothing is no file of the push. A blob counts once, and not at all when the remote has
/// it (the `<src>` of a diff that no earlier commit wrote): a file moved or reverted to what the remote holds adds
/// nothing. A gitlink counts not. `tips` are full ids or refs of this repository, never text from outside
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
      &[
        "log",
        "-z",
        "--raw",
        "--no-renames",
        "--no-abbrev",
        "--topo-order",
        "--reverse",
        "--format=tformat:@",
        "--stdin",
      ],
      repository_path,
    )
    .stdin(input.as_bytes())
    .bytes()?;
  let changes = changed_paths(&String::from_utf8_lossy(&output));
  let blobs: Vec<&str> = changes.blobs().collect::<HashSet<_>>().into_iter().collect();
  let sizes: HashMap<&str, u64> = blobs
    .iter()
    .zip(object_sizes(git, repository_path, &blobs)?)
    .filter_map(|(id, size)| Some((*id, size?)))
    .collect();
  Ok(changes.files(&sizes))
}

/// A path the commits of a push change: the remote's version of it and every version the commits give it
struct ChangedPath {
  path: String,
  /// The `<src>` of its first diff: the blob the remote holds at the path, `None` for a new path
  base: Option<String>,
  versions: Vec<String>,
}

/// The paths the raw diffs of a push change, in the order they are first changed, and the blobs the remote has
struct Changes {
  paths: Vec<ChangedPath>,
  /// The `<src>` of each diff that no earlier diff wrote: a blob from outside the push, which the remote has
  remote_blobs: HashSet<String>,
}

impl Changes {
  /// Every blob whose size the files need: the versions and the bases
  fn blobs(&self) -> impl Iterator<Item = &str> {
    self.paths.iter().flat_map(|changed| changed.versions.iter().chain(&changed.base)).map(String::as_str)
  }

  /// Each path by the bytes it adds (`push_contents`), with the sizes of its blobs; a blob without a size counts not
  fn files(&self, sizes: &HashMap<&str, u64>) -> Vec<PushedFile> {
    let mut counted: HashSet<&str> = HashSet::new();
    let mut files = Vec::new();
    for changed in &self.paths {
      let mut largest = None;
      for version in &changed.versions {
        if !self.remote_blobs.contains(version) && counted.insert(version) {
          largest = largest.max(sizes.get(version.as_str()).copied());
        }
      }
      let base = changed.base.as_deref().and_then(|base| sizes.get(base)).copied().unwrap_or(0);
      if let Some(size) = largest.map(|largest| largest.saturating_sub(base)).filter(|&size| size > 0) {
        files.push(PushedFile { path: changed.path.clone(), size });
      }
    }
    files
  }
}

/// The paths the raw diffs change: per commit `@`, NUL, then per file `:<mode> <mode> <src> <dst> <status>`, NUL, the path,
/// NUL (a newline before the first). The `<src>` of a diff is of the commit's parent, so only the blobs of the earlier
/// commits tell it from the remote's: a file moved within one commit has its new path before the old one as often as after
fn changed_paths(output: &str) -> Changes {
  let mut paths: Vec<ChangedPath> = Vec::new();
  let mut index: HashMap<String, usize> = HashMap::new();
  let mut written: HashSet<String> = HashSet::new();
  let mut written_by_commit: Vec<String> = Vec::new();
  let mut remote_blobs: HashSet<String> = HashSet::new();
  let mut fields = output.split('\0');
  while let Some(field) = fields.next() {
    if field == COMMIT_MARK {
      written.extend(std::mem::take(&mut written_by_commit));
      continue;
    }
    let Some(header) = field.trim_start_matches('\n').strip_prefix(':') else {
      continue;
    };
    let Some(path) = fields.next() else {
      break;
    };
    let mut parts = header.split(' ');
    let (Some(src_mode), Some(dst_mode), Some(src), Some(dst)) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
      continue;
    };
    let src = (src != NULL_ID && src_mode != GITLINK_MODE).then_some(src);
    let dst = (dst != NULL_ID && dst_mode != GITLINK_MODE).then_some(dst);
    if let Some(src) = src
      && !written.contains(src)
    {
      remote_blobs.insert(src.to_string());
    }
    let at = *index.entry(path.to_string()).or_insert_with(|| {
      paths.push(ChangedPath {
        path: path.to_string(),
        base: src.map(str::to_string),
        versions: Vec::new(),
      });
      paths.len() - 1
    });
    if let Some(dst) = dst {
      written_by_commit.push(dst.to_string());
      paths[at].versions.push(dst.to_string());
    }
  }
  Changes { paths, remote_blobs }
}
