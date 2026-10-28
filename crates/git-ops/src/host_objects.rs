//! The objects and the refs of the runs that the agent of `bd-host` serves (`lease_pool::host_refs`;
//! spec/cli/lease.spec.md, "Host objects"): which commits a repository has, the pack of a client written into
//! the object store, the refs under `refs/branch-deck/host/` that keep its commits until the run or the base no longer
//! needs them, and the blob text and the tree ids of a commit. No lock and no policy: the times of the refs and their
//! deletion are `lease_pool::host_refs`'

use crate::cat_file_batch::{BatchEntry, for_each_object, object_types};
use crate::object_texts::into_text;
use crate::ref_transaction::RefTransaction;
use crate::revs::is_object_id;
use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use std::io::Read;
use sync_utils::git_bool::parse_git_bool;

/// The namespace of the refs of the host
pub const HOST_REFS: &str = "refs/branch-deck/host/";
/// The ref of a run, `refs/branch-deck/host/run/<id>`: its commit until the give-back of the run
pub const RUN_REFS: &str = "refs/branch-deck/host/run/";
/// The ref of a base, `refs/branch-deck/host/base/<name>`: the commit the later packs of a client are thin against
pub const BASE_REFS: &str = "refs/branch-deck/host/base/";
/// git's default of `receive.unpackLimit` and `transfer.unpackLimit`
const DEFAULT_UNPACK_LIMIT: u32 = 100;
/// The bytes of the header of a pack: `PACK`, the version, the count of its objects
pub const PACK_HEADER: usize = 12;
/// How much of a pack one write to git takes
const CHUNK: usize = 64 * 1024;

/// The commits of `ids` (full ids) that the repository has as commit objects, in their order: one `cat-file
/// --batch-check`
pub fn present_commits(git: &GitCommandExecutor, repo: &str, ids: &[String]) -> Result<Vec<String>> {
  if let Some(bad) = ids.iter().find(|id| !is_object_id(id)) {
    bail!("{bad:?} is no full commit id");
  }
  let types = object_types(git, repo, ids)?;
  Ok(ids.iter().zip(types).filter(|(_, kind)| *kind == Some("commit")).map(|(id, _)| id.clone()).collect())
}

/// The header of a pack
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackHeader {
  pub version: u32,
  pub objects: u32,
}

/// The header of a pack: `PACK`, version 2 or 3, the count of its objects, each 4 bytes in network order
pub fn parse_pack_header(header: &[u8; PACK_HEADER]) -> Result<PackHeader> {
  let word = |at: usize| u32::from_be_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]]);
  if &header[..4] != b"PACK" {
    bail!("The input is no pack: it starts with {:?}", String::from_utf8_lossy(&header[..4]));
  }
  let version = word(4);
  if !matches!(version, 2 | 3) {
    bail!("The pack has the version {version}, which git does not read");
  }
  Ok(PackHeader { version, objects: word(8) })
}

/// What `ingest_config` read
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngestConfig {
  /// `receive.unpackLimit`, else `transfer.unpackLimit`, else 100: a pack of fewer objects is unpacked as loose objects
  pub unpack_limit: u32,
  /// `branchdeck.leaseMaintenance`: start git's maintenance after a pack landed
  pub maintenance: bool,
}

/// `receive.unpackLimit`, `transfer.unpackLimit` and `branchdeck.leaseMaintenance`: one `config --get-regexp`, which
/// exits with 1 when none is set. The last value of a key wins, as in git
pub fn ingest_config(git: &GitCommandExecutor, repo: &str) -> Result<IngestConfig> {
  let output = git
    .cmd(
      &["config", "--get-regexp", r"^(receive\.unpacklimit|transfer\.unpacklimit|branchdeck\.leasemaintenance)$"],
      repo,
    )
    .accept(&[1])
    .output()?;
  let (mut receive, mut transfer, mut maintenance) = (None, None, false);
  for line in output.into_text().lines() {
    let (key, value) = line.split_once(' ').map_or((line, None), |(key, value)| (key, Some(value)));
    let limit = || value.and_then(|value| value.trim().parse::<u32>().ok());
    match key {
      "receive.unpacklimit" => receive = limit(),
      "transfer.unpacklimit" => transfer = limit(),
      "branchdeck.leasemaintenance" => maintenance = parse_git_bool(value) == Some(true),
      _ => {}
    }
  }
  Ok(IngestConfig {
    unpack_limit: receive.or(transfer).unwrap_or(DEFAULT_UNPACK_LIMIT),
    maintenance,
  })
}

/// Where the objects of a pack went
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored {
  /// `unpack-objects`: each object a loose object
  Loose,
  /// `index-pack --stdin --fix-thin`: a pack of the repository
  Pack,
}

/// What `ingest_pack` wrote
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ingested {
  pub objects: u32,
  pub stored: Stored,
}

/// Write the pack that `input` streams into the object store of `repo`. The 12 bytes of its header come first, read
/// across short reads; a pack of fewer than `unpack_limit` objects goes to `unpack-objects`, as `receive-pack` does,
/// which resolves the bases of a thin pack from the object store, else to `index-pack --stdin --fix-thin`: a pack file
/// per small push makes every later read look in one more pack. The rest streams in chunks, never whole in memory. A
/// git that fails or ends early is the error, with its words, after the rest of `input` was read and dropped, so the
/// sender is never blocked
pub fn ingest_pack(git: &GitCommandExecutor, repo: &str, input: &mut dyn Read, unpack_limit: u32) -> Result<Ingested> {
  let mut header = [0u8; PACK_HEADER];
  let parsed = input
    .read_exact(&mut header)
    .map_err(|error| anyhow!("Cannot read the header of the pack: {error}"))
    .and_then(|()| parse_pack_header(&header));
  let parsed = match parsed {
    Ok(parsed) => parsed,
    Err(error) => {
      drain(input);
      return Err(error);
    }
  };
  let (args, stored): (&[&str], Stored) = if parsed.objects < unpack_limit {
    (&["unpack-objects", "-q"], Stored::Loose)
  } else {
    (&["index-pack", "--stdin", "--fix-thin"], Stored::Pack)
  };
  let mut git_process = git.cmd(args, repo).interactive(b'\n')?;
  let mut sent = git_process.send_bytes(&header);
  let mut buffer = vec![0u8; CHUNK];
  loop {
    let read = match input.read(&mut buffer) {
      Ok(0) => break,
      Ok(read) => read,
      Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
      Err(error) => {
        sent = sent.and(Err(anyhow!("Cannot read the pack: {error}")));
        break;
      }
    };
    // git can end early on a bad pack: the rest is read and dropped, and its exit tells why
    if sent.is_ok() {
      sent = git_process.send_bytes(&buffer[..read]);
    }
  }
  let finished = git_process.finish();
  finished?;
  sent?;
  Ok(Ingested { objects: parsed.objects, stored })
}

/// Read `input` to its end, dropping it
fn drain(input: &mut dyn Read) {
  let _ = std::io::copy(input, &mut std::io::sink());
}

/// Point the ref `full_ref` of the host at `commit`, whatever it pointed at: one `RefTransaction`
pub fn point_host_ref(git: &GitCommandExecutor, repo: &str, full_ref: &str, commit: &str) -> Result<()> {
  RefTransaction::new().update(full_ref, commit, None).commit(git, repo, "host ref")
}

/// Delete the refs `full_refs` of the host, also one that is gone already: one `RefTransaction`
pub fn delete_host_refs(git: &GitCommandExecutor, repo: &str, full_refs: &[String]) -> Result<()> {
  if full_refs.is_empty() {
    return Ok(());
  }
  let mut transaction = RefTransaction::new();
  for full_ref in full_refs {
    transaction.delete(full_ref, None);
  }
  transaction.commit(git, repo, "host ref")
}

/// The largest blob `show_commit` answers the text of
pub const MAX_SHOWN_BLOB: usize = 1024 * 1024;

/// A file of a commit and its text
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownBlob {
  pub id: String,
  /// The bytes as text, invalid UTF-8 replaced
  pub text: String,
}

/// What `show_commit` read of a commit
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownCommit {
  /// The blob at the path of `blob`; `None` when the commit has no file there
  pub blob: Option<ShownBlob>,
  /// The id of each path of `ids`, in this order: a blob, the tree of a directory, the commit of a gitlink; `None` when
  /// the commit has none
  pub ids: Vec<Option<String>>,
}

/// Why `show_commit` answers nothing
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShowRefusal {
  #[error("The repository has no commit {0}")]
  NoCommit(String),
  #[error("The path {0:?} is no path of a tree: it is empty, absolute, or has an empty, `.` or `..` component")]
  BadPath(String),
  #[error("The file {path} of {commit} has {size} bytes, more than {MAX_SHOWN_BLOB}")]
  TooLarge { commit: String, path: String, size: usize },
}

/// The text of the file `blob` and the object ids of `ids` (paths of the tree of `commit`, `/`-separated) from one `cat-file
/// --batch -Z` of `<commit>^{commit}`, `<commit>:<blob>` and `<commit>:<path>` of each id; a tree or a blob of an id is
/// skipped unread past 1 MiB
pub fn show_commit(git: &GitCommandExecutor, repo: &str, commit: &str, blob: Option<&str>, ids: &[String]) -> Result<ShownCommit> {
  if let Some(bad) = blob.into_iter().chain(ids.iter().map(String::as_str)).find(|path| !is_tree_path(path)) {
    bail!(ShowRefusal::BadPath(bad.to_string()));
  }
  let mut names = vec![format!("{commit}^{{commit}}")];
  names.extend(blob.map(|path| format!("{commit}:{path}")));
  names.extend(ids.iter().map(|path| format!("{commit}:{}", path.trim_end_matches('/'))));
  let id_offset = 1 + usize::from(blob.is_some());
  let mut found_commit = false;
  let mut shown = ShownCommit {
    blob: None,
    ids: vec![None; ids.len()],
  };
  for_each_object(git, repo, &names, MAX_SHOWN_BLOB, |index, entry| {
    let (id, kind) = match &entry {
      BatchEntry::Object { id, kind, .. } | BatchEntry::TooLarge { id, kind, .. } => ((*id).to_string(), *kind),
      BatchEntry::Missing => return Ok(()),
    };
    match index {
      0 => found_commit = kind == "commit",
      _ if index < id_offset => match entry {
        BatchEntry::Object { kind: "blob", content, .. } => shown.blob = Some(ShownBlob { id, text: into_text(content) }),
        BatchEntry::TooLarge { kind: "blob", size, .. } => bail!(ShowRefusal::TooLarge {
          commit: commit.to_string(),
          path: blob.unwrap_or_default().to_string(),
          size,
        }),
        _ => {}
      },
      _ => shown.ids[index - id_offset] = Some(id),
    }
    Ok(())
  })?;
  if !found_commit {
    bail!(ShowRefusal::NoCommit(commit.to_string()));
  }
  Ok(shown)
}

/// Whether `path` names an entry of a tree: relative, `/`-separated, no empty, `.` or `..` component (a trailing `/` of
/// a directory aside)
fn is_tree_path(path: &str) -> bool {
  let path = path.strip_suffix('/').unwrap_or(path);
  !path.is_empty() && !path.contains('\\') && path.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}
