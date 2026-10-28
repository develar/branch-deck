//! The commit of the files of a worktree as `git add --all` stages them (the tracked and the untracked files that git
//! does not ignore), on HEAD, without a write of its index, its files or its refs: the scans of a host slot before and
//! after the command of a run (`bd lease host snapshot`, `changes`), and the commit of the working tree of a client
//! (`bd host snapshot --working-tree`, `host_client`). spec/cli/lease.spec.md, "Host runs". The patch between two trees
//! streams into a file, never whole in memory.
//!
//! One `status` of the index of the worktree (read only) names the paths that differ from HEAD; their files go into
//! the commit as `add` would take them: a file by `hash-object -w --stdin-paths` (the clean filters and the end of line
//! conversion of its path apply) with the mode of its executable bit, a link by its target, a directory with `.git` as
//! the gitlink of its HEAD, a missing file as a deletion, and an untracked directory (the `normal` mode) by its files
//! that `ls-files --others --exclude-standard` names; then one `fast-import` writes the commit from the tree of HEAD.
//! In idea-1 (1.4M files, a 138 MB index) the `add` of one file into a copy of the index took 1.5 s and the
//! `write-tree` 0.65 s; for 200 files 4.4 s and 1.5 s; for 1000 new files 12 s and 0.4 s; this way takes 0.06 to 0.7 s
//! beside the `status`. A repository whose git does not trust the executable bit or links (`core.fileMode`,
//! `core.symlinks` false), and a path with a newline, which `--stdin-paths` can't name, take the index copy instead

use crate::commit_writer::{EditedCommit, TreeEdit, write_edited_commit};
use crate::lease_slot::LeaseConfig;
use crate::ref_transaction::RefTransaction;
use crate::revs::is_object_id;
use anyhow::{Context, Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use std::io::Write as _;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

/// The most paths of a `status` that the add of the index copy takes by their paths: a pathspec add matches each path
/// against each entry of the index, so above this count one `add --all` of every path is faster
pub const PATHSPEC_ADD_MAX: usize = 256;

/// HEAD, its tree and the raw date of its committer (`<seconds> <zone>`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
  pub commit: String,
  pub tree: String,
  pub committer_date: String,
}

/// HEAD with its tree and its committer date, from one `log -1`
pub fn read_head_commit(git: &GitCommandExecutor, repo: &str) -> Result<Head> {
  let output = git.cmd(&["log", "-1", "--format=%H%x00%T%x00%cd", "--date=raw", "HEAD"], repo).text()?;
  let mut fields = output.split('\0');
  match (fields.next(), fields.next(), fields.next()) {
    (Some(commit), Some(tree), Some(date)) if is_object_id(commit) && is_object_id(tree) => Ok(Head {
      commit: commit.to_string(),
      tree: tree.to_string(),
      committer_date: date.trim().to_string(),
    }),
    _ => bail!("Cannot read HEAD: {output}"),
  }
}

/// One entry of `status --porcelain=v1 -z --no-renames`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
  /// `XY`: the index against HEAD, the files against the index; `??` for an untracked file
  pub code: String,
  pub path: String,
}

impl StatusEntry {
  /// `XY path`, as `git status --porcelain` prints it
  pub fn line(&self) -> String {
    format!("{} {}", self.code, self.path)
  }
}

/// The entries of `status --porcelain=v1 -z --no-renames`: `XY <path>` each, NUL-terminated; without renames no entry
/// has a second path
pub fn status_entries(output: &[u8]) -> Vec<StatusEntry> {
  output
    .split(|&byte| byte == 0)
    .filter(|entry| entry.len() > 3)
    .map(|entry| StatusEntry {
      code: String::from_utf8_lossy(&entry[..2]).into_owned(),
      path: String::from_utf8_lossy(&entry[3..]).into_owned(),
    })
    .collect()
}

/// The status of the worktree `repo` against HEAD, in the untracked mode `config` names, without a write of its index
/// (`GIT_OPTIONAL_LOCKS=0` of the executor); its `core.fsmonitor` and `core.untrackedCache` apply
pub fn local_changes(git: &GitCommandExecutor, repo: &str, config: &LeaseConfig) -> Result<Vec<StatusEntry>> {
  let output = git.cmd(&["status", "--porcelain=v1", "-z", "--no-renames", config.untracked.arg()], repo).bytes()?;
  Ok(status_entries(&output))
}

/// The author, the committer and the message of the commit of `worktree_commit`; the date is the one of the committer
/// of HEAD, so the same tree on the same HEAD is the same commit
#[derive(Debug, Clone, Copy)]
pub struct CommitText<'a> {
  pub name: &'a str,
  pub email: &'a str,
  pub message: &'a str,
}

/// The commit of a worktree and the status it came from
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeCommit {
  pub tree: String,
  /// HEAD when the tree is the tree of HEAD, else a new commit on HEAD that no ref names
  pub commit: String,
  pub entries: Vec<StatusEntry>,
}

/// The commit on `head` of the files of `worktree` (its git dir `git_dir`) as `git add --all` stages them (the module
/// header): the `status` of `entries`, or one the call runs, then, when it found changes, the commit from their files
/// (`hash-object`, `fast-import`, and a ref transaction that deletes the temporary ref of the import), or from a copy of
/// the index (`commit_from_index_copy`) where the files can't tell what `add` would stage
pub fn worktree_commit(
  git: &GitCommandExecutor,
  worktree: &str,
  git_dir: &Path,
  head: &Head,
  config: &LeaseConfig,
  entries: Option<Vec<StatusEntry>>,
  text: CommitText<'_>,
) -> Result<WorktreeCommit> {
  let entries = match entries {
    Some(entries) => entries,
    None => local_changes(git, worktree, config)?,
  };
  if entries.is_empty() {
    return Ok(WorktreeCommit {
      tree: head.tree.clone(),
      commit: head.commit.clone(),
      entries,
    });
  }
  let edits = if config.no_file_mode || config.no_symlinks {
    None
  } else {
    file_edits(git, worktree, &entries)?
  };
  let written = match edits {
    Some(edits) => write_commit(git, worktree, head, &head.tree, &edits.as_tree_edits(), text)?,
    None => commit_from_index_copy(git, worktree, git_dir, head, &entries, text)?,
  };
  Ok(WorktreeCommit {
    commit: if written.tree == head.tree { head.commit.clone() } else { written.commit },
    tree: written.tree,
    entries,
  })
}

/// What a path of the status is in the commit
enum FileEdit {
  /// A file by its blob, or a gitlink by its commit
  Object {
    path: String,
    mode: &'static str,
    id: String,
  },
  /// A link by its target
  Link {
    path: String,
    target: Vec<u8>,
  },
  Delete {
    path: String,
  },
}

struct FileEdits(Vec<FileEdit>);

impl FileEdits {
  fn as_tree_edits(&self) -> Vec<TreeEdit<'_>> {
    self
      .0
      .iter()
      .map(|edit| match edit {
        FileEdit::Object { path, mode, id } => TreeEdit::Blob { path, mode, id },
        FileEdit::Link { path, target } => TreeEdit::Content {
          path,
          mode: "120000",
          content: target,
        },
        FileEdit::Delete { path } => TreeEdit::Delete { path },
      })
      .collect()
  }
}

/// A file of the worktree that `hash-object` hashes
struct HashedFile {
  path: String,
  mode: &'static str,
}

/// What `classify` found of some paths
#[derive(Default)]
struct Classified {
  edits: Vec<FileEdit>,
  files: Vec<HashedFile>,
  /// Untracked directories without `.git`, each with its `/`
  dirs: Vec<String>,
}

/// The edits of the tree of HEAD that `add --all` of the paths of `entries` would make, by their files; `None` for a
/// path with a newline, which `--stdin-paths` can't name, and for a nested repository without a commit
fn file_edits(git: &GitCommandExecutor, worktree: &str, entries: &[StatusEntry]) -> Result<Option<FileEdits>> {
  let root = Path::new(worktree);
  let paths: Vec<String> = entries.iter().map(|entry| entry.path.clone()).collect();
  let Some(mut found) = classify(git, root, &paths)? else {
    return Ok(None);
  };
  if !found.dirs.is_empty() {
    let mut args = vec!["--literal-pathspecs", "ls-files", "-z", "--others", "--exclude-standard", "--"];
    args.extend(found.dirs.iter().map(String::as_str));
    let listed = git.cmd(&args, worktree).bytes()?;
    let inside: Vec<String> = listed
      .split(|&byte| byte == 0)
      .filter(|path| !path.is_empty())
      .map(|path| String::from_utf8_lossy(path).into_owned())
      .collect();
    // a directory inside one that `ls-files` names is a nested repository, which `classify` took
    let Some(more) = classify(git, root, &inside)?.filter(|more| more.dirs.is_empty()) else {
      return Ok(None);
    };
    found.edits.extend(more.edits);
    found.files.extend(more.files);
  }
  let Classified { mut edits, files, .. } = found;
  if files.iter().any(|file| file.path.contains('\n')) {
    return Ok(None);
  }
  if !files.is_empty() {
    let input: String = files.iter().flat_map(|file| [file.path.as_str(), "\n"]).collect();
    let ids = git.cmd(&["hash-object", "-w", "--stdin-paths"], worktree).stdin(input.as_bytes()).lines()?;
    if ids.len() != files.len() {
      bail!("git hash-object answered {} ids for {} files", ids.len(), files.len());
    }
    edits.extend(files.into_iter().zip(ids).map(|(file, id)| FileEdit::Object {
      path: file.path,
      mode: file.mode,
      id,
    }));
  }
  Ok(Some(FileEdits(edits)))
}

/// Sort `paths` by what their files are: a deletion, a link and a gitlink (`rev-parse HEAD` in it) are edits, a file is
/// hashed, an untracked directory without `.git` is listed. `None` for a nested repository without a commit, which the
/// index copy takes
fn classify(git: &GitCommandExecutor, root: &Path, paths: &[String]) -> Result<Option<Classified>> {
  let mut found = Classified::default();
  for path in paths {
    let path = path.trim_end_matches('/').to_string();
    let full = root.join(&path);
    let metadata = match std::fs::symlink_metadata(&full) {
      Ok(metadata) => metadata,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
        found.edits.push(FileEdit::Delete { path });
        continue;
      }
      Err(error) => return Err(anyhow!(error).context(format!("Cannot read {}", full.display()))),
    };
    if metadata.file_type().is_symlink() {
      let target = std::fs::read_link(&full).with_context(|| format!("Cannot read the link {}", full.display()))?;
      found.edits.push(FileEdit::Link {
        path,
        target: target.as_os_str().as_encoded_bytes().to_vec(),
      });
    } else if metadata.is_dir() {
      if !full.join(".git").exists() {
        found.dirs.push(format!("{path}/"));
        continue;
      }
      let (head, code) = git.execute_command_with_status(&["rev-parse", "--verify", "--quiet", "HEAD"], &full.to_string_lossy())?;
      if code != 0 || !is_object_id(&head) {
        return Ok(None);
      }
      found.edits.push(FileEdit::Object { path, mode: "160000", id: head });
    } else {
      found.files.push(HashedFile {
        path,
        mode: if is_executable(&metadata) { "100755" } else { "100644" },
      });
    }
  }
  Ok(Some(found))
}

/// The executable bit of the owner, which git keeps as `100755`
#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
  use std::os::unix::fs::PermissionsExt;
  metadata.permissions().mode() & 0o100 != 0
}

#[cfg(not(unix))]
const fn is_executable(_metadata: &std::fs::Metadata) -> bool {
  false
}

/// The commit `fast-import` wrote, its temporary ref deleted
struct WrittenTree {
  tree: String,
  commit: String,
}

/// One `fast-import` of the commit on `head` whose tree is `base_tree` with `edits`, by `text`, dated as the committer
/// of HEAD, its objects loose (`fastimport.unpackLimit` above any import, where git's 100 packs a wide change): a pack
/// per snapshot would leave the repository a small pack per run, and 306 small packs made a warm `bd check` of idea-1
/// take 1.70 s instead of 1.53 s (`fastimport.unpackLimit=0` in docs/design/git-operations.md); a `gc` prunes the
/// loose objects that no ref names. Then one ref transaction deletes its temporary ref, so no ref names the commit
fn write_commit(git: &GitCommandExecutor, repo: &str, head: &Head, base_tree: &str, edits: &[TreeEdit<'_>], text: CommitText<'_>) -> Result<WrittenTree> {
  let ident = format!("{} <{}> {}", text.name, text.email, head.committer_date);
  let message = format!("{}\n", text.message);
  let loose = git
    .with_env("GIT_CONFIG_COUNT", "1")
    .with_env("GIT_CONFIG_KEY_0", "fastimport.unpackLimit")
    .with_env("GIT_CONFIG_VALUE_0", "2147483647");
  let written = write_edited_commit(
    &loose,
    repo,
    &EditedCommit {
      tree: base_tree,
      edits,
      parent: &head.commit,
      author: &ident,
      committer: &ident,
      encoding: None,
      message: message.as_bytes(),
    },
  )?;
  let mut transaction = RefTransaction::new();
  written.delete_temp_ref(&mut transaction);
  transaction.commit(git, repo, "worktree snapshot")?;
  Ok(WrittenTree {
    tree: written.tree,
    commit: written.commit,
  })
}

/// The commit of the worktree from a copy of its index (`IndexCopy`): one add of the paths of `entries` (up to
/// `PATHSPEC_ADD_MAX`, else of every path; again of every path when one went away meanwhile), one `write-tree`, then
/// the commit as `write_commit` writes it. A worktree without an index starts from the tree of HEAD (`read-tree`)
fn commit_from_index_copy(git: &GitCommandExecutor, worktree: &str, git_dir: &Path, head: &Head, entries: &[StatusEntry], text: CommitText<'_>) -> Result<WrittenTree> {
  let copy = IndexCopy::of(git_dir, &git_dir.join("index"))?;
  let index_env = copy.path.to_str().ok_or_else(|| anyhow!("The index copy is no UTF-8 path: {}", copy.path.display()))?;
  let in_copy = [("GIT_INDEX_FILE", index_env)];
  if !copy.copied {
    git.cmd(&["read-tree", &head.commit], worktree).env(&in_copy).output()?;
  }
  // a staged deletion has no file, and the copy has it already
  let paths: Vec<&str> = entries.iter().filter(|entry| entry.code != "D ").map(|entry| entry.path.as_str()).collect();
  if !paths.is_empty() {
    // a path that went away since the status fails the add: then every path once more
    let added = paths.len() <= PATHSPEC_ADD_MAX && {
      let mut pathspec = Vec::new();
      for path in &paths {
        pathspec.extend_from_slice(path.as_bytes());
        pathspec.push(0);
      }
      git
        .cmd(&["--literal-pathspecs", "add", "--all", "--pathspec-from-file=-", "--pathspec-file-nul"], worktree)
        .env(&in_copy)
        .stdin(&pathspec)
        .status()?
        .code
        == 0
    };
    if !added {
      git.cmd(&["add", "--all"], worktree).env(&in_copy).output()?;
    }
  }
  let tree = git.cmd(&["write-tree"], worktree).env(&in_copy).text()?;
  write_commit(git, worktree, head, &tree, &[], text)
}

/// A copy of the index of a worktree in a private directory of its git dir (mode 0700, a random name), removed with the
/// lock file git makes beside it when the copy goes: never a name of the shared temporary directory, which a link
/// planted there would redirect
struct IndexCopy {
  _dir: tempfile::TempDir,
  path: PathBuf,
  /// False when the worktree had no index: the copy does not exist yet
  copied: bool,
}

impl IndexCopy {
  /// A copy of `index` (an APFS clone where it can be) in `git_dir`
  fn of(git_dir: &Path, index: &Path) -> Result<Self> {
    let dir = tempfile::Builder::new()
      .prefix("branch-deck-index-")
      .tempdir_in(git_dir)
      .with_context(|| format!("Cannot make a directory in {}", git_dir.display()))?;
    let path = dir.path().join("index");
    let copied = index.exists();
    if copied {
      std::fs::copy(index, &path).with_context(|| format!("Cannot copy the index {} to {}", index.display(), path.display()))?;
    }
    Ok(Self { _dir: dir, path, copied })
  }
}

/// The size of the patch `write_tree_patch` wrote
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrittenPatch {
  pub bytes: u64,
}

/// The patch from the tree `before` to the tree `after` for `git apply` (`diff-tree -r -p --binary --full-index
/// --no-renames`), streamed into a new file beside `target` (created anew, mode 0600, never through a link), then
/// renamed onto `target`
pub fn write_tree_patch(git: &GitCommandExecutor, repo: &str, before: &str, after: &str, target: &Path) -> Result<WrittenPatch> {
  let dir = target.parent().filter(|dir| !dir.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
  let mut file = tempfile::Builder::new()
    .prefix(".branch-deck-patch-")
    .tempfile_in(dir)
    .with_context(|| format!("Cannot make a file in {}", dir.display()))?;
  let mut bytes = 0u64;
  git
    .cmd(&["diff-tree", "-r", "-p", "--binary", "--full-index", "--no-renames", before, after], repo)
    .stream(|chunk| {
      file.write_all(chunk).with_context(|| format!("Cannot write the patch {}", file.path().display()))?;
      bytes += chunk.len() as u64;
      Ok(ControlFlow::Continue(()))
    })?;
  file.flush()?;
  let (_, path) = file.keep().map_err(|error| anyhow!("Cannot keep the patch: {error}"))?;
  if let Err(error) = sync_utils::fs_retry::rename(&path, target) {
    let _ = sync_utils::fs_retry::remove_file(&path);
    bail!("Cannot move the patch to {}: {error}", target.display());
  }
  Ok(WrittenPatch { bytes })
}
