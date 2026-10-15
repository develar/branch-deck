//! The copies of a resolve round that the agent reads and edits (docs/design/git-operations.md, "Resolve session"):
//! the only files of a session outside git objects, all rebuildable from the session ref. They live in
//! `branch-deck/resolve/` of the worktree's git dir, or in the directory of `bd update --dir`. Per round `r<n>/`, each
//! side in its own directory under the flat name `<key>-<basename>` (`ConflictFile::copy_name`), so no tree path can
//! reach outside or collide with another; `commit.patch` beside them. Bytes are written and read in Rust, as git stores
//! them (no filters); every write goes to a new temporary file that a rename puts in place, a read never follows a
//! symlink, and only what bd wrote is ever deleted.

use anyhow::{Context, Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::update_resolve::{ConflictFile, EditCopy, ResolveError, RoundConflict, RoundObjects, SessionState};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

/// The file that marks a directory as bd's: `--dir` reuses only an empty directory or one with this file
pub const MARKER_FILE: &str = ".branch-deck-resolve";
/// The sides of a conflicting file, each a directory of a round
pub const SIDES: [&str; 4] = ["edit", "before", "onto", "commit"];
pub const PATCH_FILE: &str = "commit.patch";

/// Which copies to write
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Materialize {
  /// Only the missing ones (`bd update --status --materialize`, and a rerun of `bd update`)
  Missing,
  /// All of them, replacing what is there: a new round, or `--force`
  All,
}

/// The directory of a session's copies: its `--dir`, else `branch-deck/resolve` in the git dir of its worktree
pub fn session_root(state: &SessionState) -> PathBuf {
  match &state.dir {
    Some(dir) => PathBuf::from(dir),
    None => Path::new(&state.git_dir).join("branch-deck").join("resolve"),
  }
}

/// The directory of round `round`
pub fn round_dir(root: &Path, round: u32) -> PathBuf {
  root.join(format!("r{round}"))
}

/// The copy of `side` of `file` in round `round`
pub fn copy_path(root: &Path, round: u32, side: &str, file: &ConflictFile) -> PathBuf {
  round_dir(root, round).join(side).join(file.copy_name())
}

/// A directory bd may write into: an existing directory that is not a symlink, or a new one
fn ensure_dir(path: &Path) -> Result<()> {
  match fs::symlink_metadata(path) {
    Ok(metadata) if metadata.is_dir() => Ok(()),
    Ok(_) => bail!("{} is not a directory (a symlink or a file); bd writes no copies there", path.display()),
    Err(e) if e.kind() == ErrorKind::NotFound => fs::create_dir(path).with_context(|| format!("Cannot create {}", path.display())),
    Err(e) => Err(anyhow!("Cannot read {}: {e}", path.display())),
  }
}

/// Write `content` to `dir/name` through a new temporary file and a rename, which replaces whatever entry is there
/// (a symlink included) instead of writing through it. With `Materialize::Missing`, an existing entry stays
fn write_copy(dir: &Path, name: &str, content: &[u8], materialize: Materialize) -> Result<()> {
  let target = dir.join(name);
  if materialize == Materialize::Missing && fs::symlink_metadata(&target).is_ok() {
    return Ok(());
  }
  let temp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
  let _ = fs::remove_file(&temp);
  let mut file = OpenOptions::new()
    .write(true)
    .create_new(true)
    .open(&temp)
    .with_context(|| format!("Cannot create {}", temp.display()))?;
  file.write_all(content).with_context(|| format!("Cannot write {}", temp.display()))?;
  drop(file);
  fs::rename(&temp, &target).inspect_err(|_| {
    let _ = fs::remove_file(&temp);
  })?;
  Ok(())
}

/// Write the copies of the open round of `state` under `root`; `patch` is `commit.patch`
pub fn write_round(root: &Path, state: &SessionState, objects: &RoundObjects, patch: &[u8], materialize: Materialize) -> Result<()> {
  let Some(conflict) = &state.conflict else { return Ok(()) };
  if state.dir.is_none() {
    // `<git dir>/branch-deck`, then `resolve`
    if let Some(parent) = root.parent() {
      ensure_dir(parent)?;
    }
  }
  ensure_dir(root)?;
  write_copy(root, MARKER_FILE, state.git_dir.as_bytes(), Materialize::Missing)?;
  let dir = round_dir(root, state.round);
  ensure_dir(&dir)?;
  write_copy(&dir, PATCH_FILE, patch, materialize)?;
  for side in SIDES {
    let side_dir = dir.join(side);
    ensure_dir(&side_dir)?;
    for file in &conflict.files {
      let content = match side {
        "edit" => objects.merged.get(&file.path),
        _ => stage_of(file, side).and_then(|id| objects.blobs.get(id)),
      };
      if let Some(content) = content {
        write_copy(&side_dir, &file.copy_name(), content, materialize)?;
      }
    }
  }
  Ok(())
}

/// The blob of `side` (`before`, `onto` or `commit`) of `file`
pub fn stage_of<'a>(file: &'a ConflictFile, side: &str) -> Option<&'a str> {
  let stage = match side {
    "before" => &file.before,
    "onto" => &file.onto,
    "commit" => &file.commit,
    _ => &None,
  };
  stage.as_ref().map(|stage| stage.id.as_str())
}

/// Read the `edit` copy at `path`: the bytes of a regular file, never through a symlink (the file opened must be the
/// one `lstat` saw)
pub fn read_edit_copy(path: &Path) -> Result<EditCopy> {
  let metadata = match fs::symlink_metadata(path) {
    Ok(metadata) => metadata,
    Err(e) if e.kind() == ErrorKind::NotFound => return Ok(EditCopy::Missing),
    Err(e) => return Err(anyhow!("Cannot read {}: {e}", path.display())),
  };
  if !metadata.file_type().is_file() {
    return Ok(EditCopy::NotARegularFile);
  }
  let mut file = fs::File::open(path).with_context(|| format!("Cannot open {}", path.display()))?;
  if !is_same_file(&metadata, &file.metadata()?) {
    return Ok(EditCopy::NotARegularFile);
  }
  let mut content = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or_default());
  file.read_to_end(&mut content).with_context(|| format!("Cannot read {}", path.display()))?;
  Ok(EditCopy::Content(content))
}

#[cfg(unix)]
fn is_same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
  use std::os::unix::fs::MetadataExt;
  a.dev() == b.dev() && a.ino() == b.ino() && b.file_type().is_file()
}

#[cfg(not(unix))]
fn is_same_file(_a: &fs::Metadata, b: &fs::Metadata) -> bool {
  b.file_type().is_file()
}

/// Remove the copies bd wrote for `conflict` (round `round`): only its file names, then the directories left empty
pub fn remove_round(root: &Path, round: u32, conflict: &RoundConflict) {
  let dir = round_dir(root, round);
  let _ = fs::remove_file(dir.join(PATCH_FILE));
  for side in SIDES {
    for file in &conflict.files {
      let path = dir.join(side).join(file.copy_name());
      // never a directory that someone put there
      if fs::symlink_metadata(&path).is_ok_and(|metadata| !metadata.is_dir()) {
        let _ = fs::remove_file(&path);
      }
    }
    let _ = fs::remove_dir(dir.join(side));
  }
  let _ = fs::remove_dir(&dir);
}

/// Remove the marker file and, when bd created it and it is empty, the session directory
pub fn remove_root(root: &Path, state: &SessionState) {
  let _ = fs::remove_file(root.join(MARKER_FILE));
  if state.dir.is_none() || state.dir_created {
    let _ = fs::remove_dir(root);
  }
}

/// Prepare the directory of `--dir`: an absolute path, not a symlink, not inside a worktree (the git dir aside), and
/// empty or marked as bd's; created when missing. Returns the path and whether bd created it
pub fn prepare_dir(git_executor: &GitCommandExecutor, repo_path: &str, dir: &Path) -> Result<(String, bool)> {
  let invalid = |message: String| anyhow::Error::new(ResolveError::InvalidInput { message });
  if !dir.is_absolute() {
    return Err(invalid(format!("--dir must be an absolute path: {}", dir.display())));
  }
  let created = match fs::symlink_metadata(dir) {
    Ok(metadata) if metadata.file_type().is_symlink() => return Err(invalid(format!("--dir {} is a symlink; pass the directory it points to", dir.display()))),
    Ok(metadata) if !metadata.is_dir() => return Err(invalid(format!("--dir {} is not a directory", dir.display()))),
    Ok(_) => {
      let empty = fs::read_dir(dir)?.next().is_none();
      let ours = fs::symlink_metadata(dir.join(MARKER_FILE)).is_ok_and(|metadata| metadata.is_file());
      if !empty && !ours {
        return Err(invalid(format!(
          "--dir {} is neither empty nor a directory of bd ({MARKER_FILE}); pass an empty one",
          dir.display()
        )));
      }
      false
    }
    Err(e) if e.kind() == ErrorKind::NotFound => {
      fs::create_dir(dir).with_context(|| format!("Cannot create --dir {}", dir.display()))?;
      true
    }
    Err(e) => bail!("Cannot read --dir {}: {e}", dir.display()),
  };
  let refuse = |message: String| {
    if created {
      let _ = fs::remove_dir(dir);
    }
    Err(invalid(message))
  };
  let canonical = fs::canonicalize(dir)?;
  let common_dir = git_executor.execute_command(&["rev-parse", "--path-format=absolute", "--git-common-dir"], repo_path)?;
  let common_dir = fs::canonicalize(common_dir).unwrap_or_default();
  let output = git_executor.execute_command_raw(&["worktree", "list", "--porcelain", "-z"], repo_path)?;
  for worktree in output.split('\0').filter_map(|field| field.strip_prefix("worktree ")) {
    let Ok(worktree) = fs::canonicalize(worktree) else { continue };
    if canonical.starts_with(&worktree) && !canonical.starts_with(&common_dir) {
      return refuse(format!(
        "--dir {} is inside the worktree {}: other sessions and tools see the copies there; pass a directory outside every worktree",
        dir.display(),
        worktree.display()
      ));
    }
  }
  Ok((canonical.to_string_lossy().into_owned(), created))
}
