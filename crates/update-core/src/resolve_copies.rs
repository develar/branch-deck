//! The copies of a resolve round that the agent reads and edits (docs/design/git-operations.md, "Resolve session"):
//! the only files of a session outside git objects, all rebuildable from the session ref. They live in
//! `branch-deck/resolve/` of the worktree's git dir, or in the directory of `bd update --dir`. Per round `r<n>/`, each
//! side in its own directory under the flat name `<key>-<basename>` (`ConflictFile::copy_name`), so no tree path can
//! reach outside or collide with another; `commit.patch` beside them. Bytes are written and read in Rust, as git stores
//! them (no filters); every write goes to a new temporary file that a rename puts in place, a read or a delete never goes
//! through a symlink (of the file or of any directory below the git dir or `--dir`), and only what bd wrote is ever
//! deleted, in a directory whose marker names this worktree's git dir.

use anyhow::{Context, Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::git_common_dir;
use git_ops::update_resolve::{ConflictFile, EditCopy, ResolveError, RoundConflict, RoundObjects, SessionState, list_sessions};
use std::fs;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use sync_utils::atomic_file::{AtomicWrite, write_atomically};

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

/// The directory `components` below the session directory of `state`, when every directory on the way from the git
/// dir (or from `--dir`) is a real directory, not a symlink; `None` otherwise or when one is missing
fn real_dir(state: &SessionState, components: &[&str]) -> Option<PathBuf> {
  let (mut path, mut chain): (PathBuf, Vec<&str>) = match &state.dir {
    Some(dir) => (PathBuf::from(dir), Vec::new()),
    None => (PathBuf::from(&state.git_dir), vec!["branch-deck", "resolve"]),
  };
  chain.extend_from_slice(components);
  let is_dir = |path: &Path| fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir());
  if !is_dir(&path) {
    return None;
  }
  for component in chain {
    path.push(component);
    if !is_dir(&path) {
      return None;
    }
  }
  Some(path)
}

/// Whether the session directory of `state` is its own: a real directory whose marker names its git dir
fn owns_root(state: &SessionState) -> bool {
  real_dir(state, &[]).is_some_and(|root| read_marker(&root).as_deref() == Some(state.git_dir.as_str()))
}

/// The git dir the marker of `dir` names, when it is a regular file
fn read_marker(dir: &Path) -> Option<String> {
  match read_regular_file(&dir.join(MARKER_FILE)) {
    Ok(EditCopy::Content(content)) => String::from_utf8(content).ok(),
    _ => None,
  }
}

/// A directory bd may write into: an existing directory that is not a symlink, or a new one
#[expect(clippy::create_dir, reason = "one level only: `create_dir_all` would also create the parents, through a symlink")]
fn ensure_dir(path: &Path) -> Result<()> {
  match fs::symlink_metadata(path) {
    Ok(metadata) if metadata.is_dir() => Ok(()),
    Ok(_) => bail!("{} is not a directory (a symlink or a file); bd writes no copies there", path.display()),
    Err(e) if e.kind() == ErrorKind::NotFound => fs::create_dir(path).with_context(|| format!("Cannot create {}", path.display())),
    Err(e) => Err(anyhow!("Cannot read {}: {e}", path.display())),
  }
}

/// Write `content` to `dir/name` through a new temporary file and a rename (`write_atomically`), which replaces
/// whatever entry is there (a symlink included) instead of writing through it. With `Materialize::Missing`, an
/// existing entry stays
fn write_copy(dir: &Path, name: &str, content: &[u8], materialize: Materialize) -> Result<()> {
  let target = dir.join(name);
  if materialize == Materialize::Missing && fs::symlink_metadata(&target).is_ok() {
    return Ok(());
  }
  write_atomically(&target, content, AtomicWrite::default()).with_context(|| format!("Cannot write {}", target.display()))
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

/// Read the `edit` copy of `file` in the open round of `state`: the bytes of a regular file, never through a symlink,
/// neither of the file nor of a directory on the way
pub fn read_edit_copy(state: &SessionState, file: &ConflictFile) -> Result<EditCopy> {
  let Some(dir) = real_dir(state, &[&format!("r{}", state.round), "edit"]) else {
    let root = session_root(state);
    return Ok(if copy_path(&root, state.round, "edit", file).exists() {
      EditCopy::NotARegularFile
    } else {
      EditCopy::Missing
    });
  };
  read_regular_file(&dir.join(file.copy_name()))
}

/// The bytes of the regular file at `path` (the file opened must be the one `lstat` saw)
#[expect(clippy::filetype_is_file, reason = "regular files only: a symlink, a device or a FIFO is not an edit copy")]
#[expect(clippy::verbose_file_reads, reason = "the bytes come from the handle whose metadata matched the `lstat`")]
fn read_regular_file(path: &Path) -> Result<EditCopy> {
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
#[expect(clippy::filetype_is_file, reason = "regular files only: a symlink, a device or a FIFO is not an edit copy")]
fn is_same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
  use std::os::unix::fs::MetadataExt;
  a.dev() == b.dev() && a.ino() == b.ino() && b.file_type().is_file()
}

#[cfg(not(unix))]
#[expect(clippy::filetype_is_file, reason = "regular files only: a symlink, a device or a FIFO is not an edit copy")]
fn is_same_file(_a: &fs::Metadata, b: &fs::Metadata) -> bool {
  b.file_type().is_file()
}

/// Remove the copies bd wrote for `conflict` (round `round` of the session of `state`): only its file names, then the
/// directories left empty. Nothing is removed through a symlinked directory, or in a directory whose marker names
/// another git dir
pub fn remove_round(state: &SessionState, round: u32, conflict: &RoundConflict) {
  if !owns_root(state) {
    return;
  }
  let round_name = format!("r{round}");
  let Some(dir) = real_dir(state, &[&round_name]) else { return };
  let _ = sync_utils::fs_retry::remove_file(dir.join(PATCH_FILE));
  for side in SIDES {
    let Some(side_dir) = real_dir(state, &[&round_name, side]) else { continue };
    for file in &conflict.files {
      let path = side_dir.join(file.copy_name());
      // never a directory that someone put there
      if fs::symlink_metadata(&path).is_ok_and(|metadata| !metadata.is_dir()) {
        let _ = sync_utils::fs_retry::remove_file(&path);
      }
    }
    let _ = sync_utils::fs_retry::remove_dir(side_dir);
  }
  let _ = sync_utils::fs_retry::remove_dir(&dir);
}

/// Remove the marker file and, when bd created it and it is empty, the session directory; only in a directory whose
/// marker names this session's git dir
pub fn remove_root(state: &SessionState) {
  if !owns_root(state) {
    return;
  }
  let Some(root) = real_dir(state, &[]) else { return };
  let _ = sync_utils::fs_retry::remove_file(root.join(MARKER_FILE));
  if state.dir.is_none() || state.dir_created {
    let _ = sync_utils::fs_retry::remove_dir(root);
  }
}

/// Prepare the directory of `--dir` for the session of the worktree with `git_dir`: an absolute path, not a symlink, not
/// inside a worktree (the git dir aside), empty or marked as bd's for this git dir, and used by no other session;
/// created when missing. Returns the path and whether bd created it
pub fn prepare_dir(git_executor: &GitCommandExecutor, repo_path: &str, dir: &Path, git_dir: &str) -> Result<(String, bool)> {
  let invalid = |message: String| anyhow::Error::new(ResolveError::InvalidInput { message });
  if !dir.is_absolute() {
    return Err(invalid(format!("--dir must be an absolute path: {}", dir.display())));
  }
  let created = match fs::symlink_metadata(dir) {
    Ok(metadata) if metadata.file_type().is_symlink() => return Err(invalid(format!("--dir {} is a symlink; pass the directory it points to", dir.display()))),
    Ok(metadata) if !metadata.is_dir() => return Err(invalid(format!("--dir {} is not a directory", dir.display()))),
    Ok(_) => {
      let empty = fs::read_dir(dir)?.next().is_none();
      match read_marker(dir) {
        Some(owner) if owner != git_dir => {
          return Err(invalid(format!(
            "--dir {} holds the copies of the worktree with the git dir {owner}; pass a directory of its own",
            dir.display()
          )));
        }
        None if !empty => {
          return Err(invalid(format!(
            "--dir {} is neither empty nor a directory of bd ({MARKER_FILE}); pass an empty one",
            dir.display()
          )));
        }
        _ => {}
      }
      false
    }
    Err(e) if e.kind() == ErrorKind::NotFound => {
      // the parent must exist: `create_dir_all` would create it, also through a symlink
      #[expect(clippy::create_dir, reason = "--dir is created one level deep, under an existing parent")]
      fs::create_dir(dir).with_context(|| format!("Cannot create --dir {}", dir.display()))?;
      true
    }
    Err(e) => bail!("Cannot read --dir {}: {e}", dir.display()),
  };
  let refuse = |message: String| {
    if created {
      let _ = sync_utils::fs_retry::remove_dir(dir);
    }
    Err(invalid(message))
  };
  let canonical = fs::canonicalize(dir)?;
  if let Some(session) = list_sessions(git_executor, repo_path)?
    .into_iter()
    .find(|session| session.state().dir.as_deref().is_some_and(|used| Path::new(used) == canonical))
  {
    return refuse(format!(
      "--dir {} holds the copies of the resolve session of the worktree {}; pass a directory of its own",
      dir.display(),
      session.state().worktree
    ));
  }
  let common_dir = git_common_dir(git_executor, repo_path)?;
  let common_dir = fs::canonicalize(common_dir).unwrap_or_default();
  let output = git_executor.cmd(&["worktree", "list", "--porcelain", "-z"], repo_path).bytes()?;
  let output = String::from_utf8_lossy(&output);
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
