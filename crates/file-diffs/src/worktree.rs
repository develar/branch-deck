//! The changed files of the working tree against `HEAD`, from one `status --porcelain=v2 -z`, with the new side read
//! from the disk and its blob id computed as git computes it for the bytes (`git hash-object --no-filters`).

use crate::model::BlobText;
use crate::service::Entry;
use anyhow::{Result, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::MAX_BLOB_BYTES;
use git_ops::commit_utils::blob_id;
use git_ops::object_texts::{into_text, is_binary};
use object_cache::commit_files::FileStatus;
use std::path::{Component, Path};
use std::sync::Arc;

/// The mode of a missing side and of a submodule in `status --porcelain=v2`
const NO_MODE: &str = "000000";
const SUBMODULE_MODE: &str = "160000";

/// The uncommitted files: staged, unstaged and untracked, each against `HEAD`. A file that one side of a merge
/// conflict changed is its `ours` version against the working tree.
pub(crate) fn uncommitted_entries(git: &GitCommandExecutor, repository_path: &str) -> Result<Vec<Entry>> {
  let output = git
    .cmd(&["status", "--porcelain=v2", "-z", "--untracked-files=all", "--no-ahead-behind"], repository_path)
    .bytes()?;
  let output = String::from_utf8_lossy(&output);
  let mut fields = output.split('\0').filter(|field| !field.is_empty());
  let mut records = Vec::new();
  while let Some(field) = fields.next() {
    let record = match field.as_bytes()[0] {
      b'1' => Record::parse(field, 9, 6, 3)?,
      b'2' => {
        let mut record = Record::parse(field, 10, 6, 3)?;
        record.old_path = Some(fields.next().ok_or_else(|| anyhow::anyhow!("rename without its source: {field}"))?.to_string());
        record.renamed = field.as_bytes().get(2).copied();
        record
      }
      // a conflict: `u XY sub m1 m2 m3 mW h1 h2 h3 path`, the `ours` stage against the working tree
      b'u' => Record::parse(field, 11, 8, 4)?,
      b'?' => Record {
        path: field[2..].to_string(),
        old_mode: NO_MODE.to_string(),
        old_id: None,
        index_id: None,
        submodule: false,
        old_path: None,
        renamed: None,
      },
      // `!` (ignored, not asked for) and the headers of `--branch`
      _ => continue,
    };
    records.push(record);
  }
  // the length of an id of the repository, for the hash of a file: SHA-1 unless an entry says otherwise
  let id_length = records.iter().find_map(|record| record.old_id.as_ref().map(String::len)).unwrap_or(40);

  records
    .into_iter()
    .map(|record| {
      if record.submodule {
        // the commit of the working tree side would cost a process in the submodule: the index has it once staged
        return Ok(Entry {
          status: if record.old_mode == NO_MODE { FileStatus::Added } else { FileStatus::Modified },
          path: record.path,
          old_path: record.old_path,
          old_id: None,
          new_id: None,
          binary: false,
          submodule: Some((record.old_id.filter(|_| record.old_mode == SUBMODULE_MODE), record.index_id)),
          worktree: None,
        });
      }
      let worktree = read_file(repository_path, &record.path, id_length)?;
      let old_id = record.old_id.filter(|_| record.old_mode != NO_MODE);
      let status = match (&old_id, &worktree, record.renamed) {
        (_, _, Some(b'R')) => FileStatus::Renamed,
        (_, _, Some(b'C')) => FileStatus::Copied,
        (None, _, _) => FileStatus::Added,
        (_, None, _) => FileStatus::Deleted,
        _ => FileStatus::Modified,
      };
      Ok(Entry {
        path: record.path,
        old_path: record.old_path,
        status,
        old_id,
        new_id: worktree.as_ref().map(|blob| blob.id().to_string()),
        binary: false,
        submodule: None,
        worktree: worktree.map(Arc::new),
      })
    })
    .collect()
}

/// One entry of `status --porcelain=v2`
struct Record {
  path: String,
  old_mode: String,
  old_id: Option<String>,
  /// The id in the index, for a submodule
  index_id: Option<String>,
  submodule: bool,
  old_path: Option<String>,
  /// The index status letter of a rename or a copy
  renamed: Option<u8>,
}

impl Record {
  /// A record of `count` space-separated fields, the path last, the `HEAD` id at `id` and its mode at `mode`
  fn parse(field: &str, count: usize, id: usize, mode: usize) -> Result<Self> {
    let parts: Vec<&str> = field.splitn(count, ' ').collect();
    if parts.len() != count {
      bail!("unexpected status entry: {field}");
    }
    Ok(Self {
      path: parts[count - 1].to_string(),
      old_mode: parts[mode].to_string(),
      old_id: Some(parts[id].to_string()).filter(|id| id.bytes().any(|b| b != b'0')),
      // `hI` follows `hH` in the records of a change and a rename
      index_id: parts
        .get(id + 1)
        .filter(|_| count < 11)
        .map(|id| (*id).to_string())
        .filter(|id| id.bytes().any(|b| b != b'0')),
      submodule: parts[2].starts_with('S'),
      old_path: None,
      renamed: None,
    })
  }
}

/// The blob of a working tree file whose id the views ask for: `None` when the path is outside the repository or the
/// file is gone
pub(crate) fn read_worktree_blob(repository_path: &str, path: &str, id: &str) -> Result<Option<BlobText>> {
  let relative = Path::new(path);
  if relative.is_absolute() || relative.components().any(|component| !matches!(component, Component::Normal(_))) {
    bail!("Not a path in the repository: {path}");
  }
  read_file(repository_path, path, id.len())
}

/// The file at `path` as a blob: a symlink is its target, as git stores it; `None` when there is no file
fn read_file(repository_path: &str, path: &str, id_length: usize) -> Result<Option<BlobText>> {
  let full = Path::new(repository_path).join(path);
  let Ok(metadata) = std::fs::symlink_metadata(&full) else {
    return Ok(None);
  };
  let sample_id = "0".repeat(id_length);
  let bytes = if metadata.file_type().is_symlink() {
    std::fs::read_link(&full)?.to_string_lossy().into_owned().into_bytes()
  } else if metadata.is_file() {
    if metadata.len() > MAX_BLOB_BYTES as u64 {
      // the id needs the bytes, but no view reads them: the size alone
      let bytes = std::fs::read(&full)?;
      return Ok(Some(BlobText::TooLarge {
        id: blob_id(&sample_id, &bytes),
        size: u32::try_from(bytes.len()).unwrap_or(u32::MAX),
      }));
    }
    std::fs::read(&full)?
  } else {
    return Ok(None);
  };
  let id = blob_id(&sample_id, &bytes);
  Ok(Some(if is_binary(&bytes) {
    BlobText::Binary { id }
  } else {
    BlobText::Text { id, text: into_text(bytes) }
  }))
}
