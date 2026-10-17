//! Resolve sessions of `bd update` (docs/design/git-operations.md, "Resolve session"): an update that conflicts is
//! replayed commit by commit outside the working tree, and the agent resolves one conflicting commit per round. All
//! state lives in git: the session ref `refs/branch-deck/resolve/<worktree id>` points at a state commit whose tree
//! holds `state.json` and the objects of the open round, and whose parents keep the replayed commits reachable. Every
//! step moves the ref with compare-and-swap; nothing else moves until the finish, which moves the branch with the
//! files through `update_branch` (the guarded working-tree update) and only then deletes the ref.

use crate::commit_writer::{
  EditedCommit, NewParent, RawCommit, StoreCommit, StoreEntry, StoreNotes, TreeEdit, WrittenStore, current_committer, parse_raw_commit, sweep_stale_temp_refs, write_edited_commit,
  write_store_commits,
};
use crate::merge_tree_output::{MergeTreeOutput, parse_merge_tree_output};
use crate::ref_transaction::RefTransaction;
use crate::resolve_check::{CheckFailure, Labels, block_count, check_resolution, dropped_lines, has_foreign_encoding, has_markers};
use crate::rewrite_guard::ensure_no_merges;
use crate::rewrite_plan::{replay_range_with_env, short_id};
use crate::update_branch::{
  ChangedPath, FileMove, UPDATE_OPERATION, UpdateCommit, UpdatePlan, blocking_paths, changed_paths, check_update_guards, commits_touching, files_follow, index_left_behind,
  likely_blocking_paths, move_branch_and_files_with,
};
use anyhow::{Result, anyhow, bail};
use bstr::BStr;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fmt::Write as _;
use tracing::{debug, info, instrument};

/// The session refs, one per worktree: shared refs, so a `gc` in any worktree keeps their objects (a `refs/worktree/`
/// ref is invisible to the gc of another worktree)
pub const SESSION_REF_PREFIX: &str = "refs/branch-deck/resolve/";
/// The audit record of a session: a note per resolved commit, written only by the finish
pub const RESOLVE_NOTES_REF: &str = "refs/notes/branch-deck/resolve";
const STATE_FILE: &str = "state.json";
const STATE_VERSION: u32 = 1;
/// The id of the main worktree; a linked one is named by its directory under `$GIT_COMMON_DIR/worktrees`
const MAIN_WORKTREE: &str = "main";
/// The modes of a regular file: the only ones a round supports
const FILE_MODES: [&str; 2] = ["100644", "100755"];
/// The upstream commits listed per conflicting file
const MAX_UPSTREAM_COMMITS_PER_FILE: usize = 10;
/// The longest basename a copy keeps (in bytes, the extension included), below `NAME_MAX` (255) with the key and the
/// name of the temporary file
const MAX_COPY_BASENAME: usize = 100;
/// Temporary refs older than this belong to a process that is gone
pub const STALE_TEMP_REF_AGE: std::time::Duration = std::time::Duration::from_secs(3600);

/// Where a session is
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
  /// A round is open: a commit conflicts
  Resolving,
  /// Every local commit is replayed; the finish is next (local changes may block it)
  Ready,
  /// The finish started; the branch has not moved
  Applying,
  /// The branch moved; the files may not have followed yet
  Applied,
}

/// The conflict types a round supports; any other is `ResolveError::Unsupported`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
  /// Both sides changed the text: `edit` has zdiff3 markers (also an add on both sides, without `before`)
  Content,
  /// One side changed the file and the other deleted it: `edit` is the changed version, without markers
  ModifyDelete,
  /// One side renamed the file (`renamed_from` to `path`) and the other deleted it: `edit` is the renamed version
  RenameDelete,
}

impl ConflictKind {
  /// One side has no file: `edit` is the version left in the tree, without markers
  pub const fn is_one_sided(self) -> bool {
    matches!(self, Self::ModifyDelete | Self::RenameDelete)
  }
}

/// A stage of a conflicting path: its mode and blob
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stage {
  pub mode: String,
  pub id: String,
}

/// A conflicting path of a round
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictFile {
  /// 1-based, unique in the round: the copies are named `<key>-<basename>`
  pub key: usize,
  pub path: String,
  pub kind: ConflictKind,
  /// The file in the commit's parent (`c^`)
  pub before: Option<Stage>,
  /// The file in the new parent: the upstream and the commits replayed so far
  pub onto: Option<Stage>,
  /// The file in the commit being replayed
  pub commit: Option<Stage>,
  /// The mode of the resolved file, merged as git does: a change of one side wins
  #[serde(default)]
  pub mode: String,
  /// The path before the rename of a `RenameDelete`
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub renamed_from: Option<String>,
}

impl ConflictFile {
  /// Whether `path` names this file: its path, or the path before its rename
  pub fn is_named(&self, path: &str) -> bool {
    self.path == path || self.renamed_from.as_deref() == Some(path)
  }
}

impl ConflictFile {
  /// The flat name of every copy of this file: `<key>-<basename>`, with the characters a file system may refuse
  /// replaced, so no tree path can reach outside the round directory or collide with another one
  pub fn copy_name(&self) -> String {
    let basename = self.path.rsplit('/').next().unwrap_or_default();
    let safe: String = basename
      .chars()
      .map(|c| {
        if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '\\' | '|' | '?' | '*') {
          '_'
        } else {
          c
        }
      })
      .collect();
    format!("{}-{}", self.key, shortened(&safe))
  }

  /// The mode of the resolved file: the merged mode, else the new parent's, else the commit's
  fn mode(&self) -> &str {
    if !self.mode.is_empty() {
      return &self.mode;
    }
    self.onto.as_ref().or(self.commit.as_ref()).map_or("100644", |stage| stage.mode.as_str())
  }
}

/// `name` cut to `MAX_COPY_BASENAME` bytes on a character boundary, the extension kept
fn shortened(name: &str) -> String {
  if name.len() <= MAX_COPY_BASENAME {
    return name.to_string();
  }
  let extension = name.rfind('.').map(|dot| &name[dot..]).filter(|extension| extension.len() <= 16).unwrap_or("");
  let end = name.floor_char_boundary(MAX_COPY_BASENAME - extension.len());
  format!("{}{extension}", &name[..end])
}

/// The open round: the local commit that conflicts, replayed onto `onto`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundConflict {
  pub commit: String,
  /// The tip it replays onto: the upstream and the local commits replayed so far
  pub onto: String,
  /// `merge-tree --merge-base <commit>^ <onto> <commit>`: the merge result, with markers in the conflicting files
  pub merge_tree: String,
  pub files: Vec<ConflictFile>,
}

impl RoundConflict {
  /// The label after the base markers: the commit's parent, as bd passed it to `merge-tree`
  pub fn base_label(&self) -> String {
    format!("{}^", self.commit)
  }

  /// The labels of the markers in the `edit` copies, with `base_label`
  pub fn labels<'a>(&'a self, base_label: &'a str) -> Labels<'a> {
    Labels {
      onto: &self.onto,
      base: base_label,
      commit: &self.commit,
    }
  }
}

/// How a path of a round was resolved; also the outcome in the notes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
  /// The agent's edit of the `edit` copy
  Edited,
  /// The new parent's version (`--take onto:<path>`)
  Onto,
  /// The commit's version (`--take commit:<path>`)
  Commit,
  /// `--delete <path>`, or a take of a side that deleted it
  Deleted,
}

impl Outcome {
  const fn as_str(self) -> &'static str {
    match self {
      Self::Edited => "edited",
      Self::Onto => "onto",
      Self::Commit => "commit",
      Self::Deleted => "deleted",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedPath {
  pub path: String,
  pub outcome: Outcome,
  /// Lines of the new parent's change (`before` to `onto`) that the result has neither as they are nor rewritten
  /// (report only)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub dropped_upstream_lines: Vec<String>,
  /// Lines of the commit's change (`before` to `commit`) that the result has neither as they are nor rewritten
  /// (report only)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub dropped_commit_lines: Vec<String>,
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "the signature of `skip_serializing_if`")]
const fn is_false(value: &bool) -> bool {
  !*value
}

/// The resolution of one round
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
  pub round: u32,
  /// The local commit as it was
  pub commit: String,
  /// The commit written for it; `None` when the result was empty and `--drop-empty` dropped the commit
  pub resolved: Option<String>,
  /// The result changes nothing: kept as an empty commit (`--allow-empty`) or dropped (`--drop-empty`)
  #[serde(default, skip_serializing_if = "is_false")]
  pub empty: bool,
  pub paths: Vec<ResolvedPath>,
}

impl Resolution {
  /// The resolution dropped the commit (`--drop-empty`)
  pub const fn dropped(&self) -> bool {
    self.empty && self.resolved.is_none()
  }
}

/// `state.json`, the only file of a state commit that readers parse
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionState {
  pub version: u32,
  pub worktree: String,
  /// The git dir of the worktree (absolute): a session never continues in another one
  pub git_dir: String,
  pub branch: String,
  /// The upstream as the user knows it, e.g. `origin/master`; it stays fixed for the session
  pub upstream: String,
  pub base: String,
  pub upstream_head: String,
  /// The previous upstream tips left out of the local commits
  pub excluded: Vec<String>,
  /// The committer of every commit the session writes, with the date of its start, so a rerun writes the same ids
  pub committer: String,
  /// The directory of the copies (`--dir`); `None` for `branch-deck/resolve` in the worktree's git dir
  pub dir: Option<String>,
  /// bd created `dir`, so it removes it when it is empty
  pub dir_created: bool,
  /// HEAD when the session started
  pub started_head: String,
  /// HEAD as the session knows it: peers' commits on top of it become more local commits
  pub head: String,
  /// The local commits to replay, oldest first
  pub local: Vec<String>,
  /// Local commits the upstream likely has already (same author, author time and subject)
  pub likely_landed: Vec<String>,
  /// How many of `local` are replayed (or resolved) into `tip`
  pub replayed: usize,
  /// The upstream with the first `replayed` local commits
  pub tip: String,
  pub round: u32,
  pub status: SessionStatus,
  pub conflict: Option<RoundConflict>,
  pub resolutions: Vec<Resolution>,
  /// Where the notes ref pointed before the finish moved it, for its rollback
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub notes_before: Option<String>,
  /// Local commits left out because the upstream has a commit with the same patch id, as `git rebase` leaves them out
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub landed_dropped: Vec<LandedDrop>,
  /// The indices of `local` where a run of consecutive commits starts after one left out: a replay covers one run
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub run_starts: Vec<usize>,
}

/// A local commit left out of the replay: the upstream has `upstream`, a commit with the same patch id
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LandedDrop {
  pub commit: String,
  pub upstream: String,
}

impl SessionState {
  pub const fn total(&self) -> usize {
    self.local.len()
  }

  /// The end (exclusive) of the run of consecutive commits that holds `local[index]`
  fn run_end(&self, index: usize) -> usize {
    self.run_starts.iter().copied().find(|&start| start > index).unwrap_or(self.local.len())
  }

  /// The committer as `GIT_COMMITTER_*` variables for `git replay`
  fn committer_env(&self) -> Result<[(&'static str, String); 3]> {
    let (name, rest) = self.committer.split_once(" <").ok_or_else(|| anyhow!("Unexpected committer {:?}", self.committer))?;
    let (email, date) = rest.split_once("> ").ok_or_else(|| anyhow!("Unexpected committer {:?}", self.committer))?;
    Ok([
      ("GIT_COMMITTER_NAME", name.to_string()),
      ("GIT_COMMITTER_EMAIL", email.to_string()),
      ("GIT_COMMITTER_DATE", date.to_string()),
    ])
  }
}

/// A session as read from its ref. Only this module builds one, from git: the finish moves HEAD to the `tip` it read,
/// never to a commit a caller chose
#[derive(Debug, Clone)]
pub struct Session {
  ref_name: String,
  /// The state commit the ref points at, for the compare-and-swap of the next step
  commit: String,
  state: SessionState,
  /// The objects of the round, when this step opened it
  objects: Option<RoundObjects>,
  /// HEAD when `read_session` read the session, in the same process
  head_seen: Option<String>,
}

impl Session {
  pub const fn state(&self) -> &SessionState {
    &self.state
  }

  pub fn session_ref(&self) -> &str {
    &self.ref_name
  }

  /// The objects of the open round when the step that returned the session read them, so the copies need no other read
  pub const fn round_objects(&self) -> Option<&RoundObjects> {
    self.objects.as_ref()
  }

  const fn new(ref_name: String, commit: String, state: SessionState) -> Self {
    Self {
      ref_name,
      commit,
      state,
      objects: None,
      head_seen: None,
    }
  }
}

/// The worktree a command runs in
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
  pub id: String,
  /// Absolute
  pub git_dir: String,
}

/// The worktree of `repo_path`: `main`, or the name of a linked worktree
pub fn current_worktree(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Worktree> {
  let lines = git_executor.execute_command_lines(&["rev-parse", "--path-format=absolute", "--git-dir", "--git-common-dir"], repo_path)?;
  let [git_dir, common_dir] = <[String; 2]>::try_from(lines).map_err(|lines| anyhow!("Unexpected rev-parse output: {lines:?}"))?;
  let id = if git_dir == common_dir {
    MAIN_WORKTREE.to_string()
  } else {
    git_dir.trim_end_matches('/').rsplit('/').next().unwrap_or(MAIN_WORKTREE).to_string()
  };
  Ok(Worktree { id, git_dir })
}

/// The session ref of a worktree
pub fn session_ref(worktree_id: &str) -> String {
  format!("{SESSION_REF_PREFIX}{worktree_id}")
}

/// What is wrong with the resolution of a round's files
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalKind {
  /// A conflict marker is left in an `edit` copy
  UnresolvedMarkers,
  /// An `edit` copy changed outside its conflict blocks
  OutsideConflict,
  /// Only the line ends of an `edit` copy changed outside its conflict blocks
  LineEnds,
  /// An `edit` copy starts with a byte order mark or is UTF-16
  Encoding,
  /// No edited regular file at `edit`, no `--take`, no `--delete`
  MissingResolution,
}

/// A refused resolution of some files of a round
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
  pub kind: RefusalKind,
  /// The tree paths
  pub paths: Vec<String>,
  /// The absolute `edit` copy of each path, when the caller knows them
  pub edits: Vec<String>,
  /// The paths that also changed in the working tree, where bd never reads
  pub changed_in_working_tree: Vec<String>,
  /// The round and its commit, with the commit's subject
  pub round: u32,
  pub commit: String,
  pub subject: String,
}

impl Refusal {
  fn names(&self) -> String {
    self
      .paths
      .iter()
      .enumerate()
      .map(|(index, path)| match self.edits.get(index) {
        Some(edit) => format!("{edit} ({path})"),
        None => path.clone(),
      })
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl fmt::Display for Refusal {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let names = self.names();
    write!(f, "Round {}, commit {} {}: ", self.round, short_id(&self.commit), self.subject)?;
    match self.kind {
      RefusalKind::UnresolvedMarkers => write!(f, "Conflict markers are left in {names}; remove the markers of every block")?,
      RefusalKind::OutsideConflict => write!(
        f,
        "The resolution of {names} changes lines outside the conflict blocks; change only the lines between the markers. A change \
         that the commit needs outside the blocks or in another file goes into a `fixup! {}` commit after the update, then `bd fold`",
        self.subject
      )?,
      RefusalKind::LineEnds => write!(
        f,
        "The line ends of {names} changed outside the conflict blocks (CRLF and LF, or the newline at the end of the file); keep \
         the line ends of the edit copy"
      )?,
      RefusalKind::Encoding => write!(
        f,
        "{names} starts with a byte order mark or is UTF-16; write the copy as UTF-8 without a BOM (in Windows PowerShell \
         `[IO.File]::WriteAllText(<edit>, $text)`, not `Set-Content` or `Out-File`)"
      )?,
      RefusalKind::MissingResolution => write!(
        f,
        "No resolution for {names}: edit the copy, or pass --take onto:<path>, --take commit:<path> or --delete <path>"
      )?,
    }
    if !self.changed_in_working_tree.is_empty() {
      write!(
        f,
        ". {} changed in the working tree: bd reads only the edit copies, so make that edit in the copy",
        self.changed_in_working_tree.join(", ")
      )?;
    }
    write!(f, ". {}", unchanged(self.round))
  }
}

/// What a refused `--continue` leaves: nothing
fn unchanged(round: u32) -> String {
  format!("Nothing changed: round {round} stays open, and the edit copies keep your edits; fix them and run --continue again")
}

/// A refusal of a session step; nothing was written
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
  /// The resolution of some files is refused
  Refused(Refusal),
  /// The resolution leaves the commit without changes, and neither `--allow-empty` nor `--drop-empty` decided it
  EmptyResolution { round: u32, commit: String, subject: String },
  /// The conflict is of a type a round can't resolve (binary, file/directory, rename, mode, submodule, symlink)
  Unsupported { paths: Vec<String>, message: String },
  /// The branch was rewritten, HEAD left the branch, or the session belongs to another worktree; `--abort` removes it
  Stale { message: String },
  /// `--round` names another round than the open one
  RoundMoved { expected: u32, actual: u32 },
  /// A `--take` or `--delete` names no conflicting path of the round, or a round has no conflict
  InvalidInput { message: String },
  /// The branch moved, but the files did not follow and the session can't finish them here
  FinishPending { message: String },
}

impl fmt::Display for ResolveError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Refused(refusal) => refusal.fmt(f),
      Self::EmptyResolution { round, commit, subject } => write!(
        f,
        "Round {round}, commit {} {subject}: the resolution leaves it without any change. Ask the user: --allow-empty keeps it \
         as an empty commit, --drop-empty {} drops it; or resolve it another way. {}",
        short_id(commit),
        short_id(commit),
        unchanged(*round)
      ),
      Self::Unsupported { message, .. } | Self::Stale { message } | Self::InvalidInput { message } | Self::FinishPending { message } => f.write_str(message),
      Self::RoundMoved { expected, actual } => write!(
        f,
        "Round {expected} is not open, round {actual} is: run `bd update` to print the open round, then resolve that one"
      ),
    }
  }
}

impl std::error::Error for ResolveError {}

impl ResolveError {
  /// The local commit the refusal is about, when it names one
  pub fn commit(&self) -> Option<&str> {
    match self {
      Self::Refused(refusal) => Some(&refusal.commit),
      Self::EmptyResolution { commit, .. } => Some(commit),
      _ => None,
    }
  }

  /// The paths the refusal is about
  pub fn paths(&self) -> &[String] {
    match self {
      Self::Refused(refusal) => &refusal.paths,
      Self::Unsupported { paths, .. } => paths,
      _ => &[],
    }
  }
}

/// What a step did
#[derive(Debug)]
pub enum Step {
  /// A round is open: a commit conflicts
  Round(Session),
  /// The branch moved from `previous_head` to `new_head` and the files followed; the session ref is gone
  Applied { session: Session, previous_head: String },
  /// Every commit is replayed, but local changes sit on paths the finish writes; the session is kept, ready to finish
  Blocked { session: Session, paths: Vec<String> },
  /// `--continue --dry-run`: the resolution passes the checks; nothing was written but git objects
  Checked { session: Session, resolution: Option<Resolution> },
}

/// Read the session of `worktree_id`, and HEAD, with one `cat-file --batch`; `None` without one
#[instrument(skip(git_executor))]
pub fn read_session(git_executor: &GitCommandExecutor, repo_path: &str, worktree_id: &str) -> Result<Option<Session>> {
  let session_ref = session_ref(worktree_id);
  let input = format!("HEAD\n{session_ref}\n{session_ref}:{STATE_FILE}\n");
  let output = git_executor.execute_command_with_input_raw(&["cat-file", "--batch"], repo_path, &input)?;
  let mut objects = parse_batch(&output, b'\n')?.into_iter();
  let head_seen = objects.next().flatten().filter(|head| head.kind == "commit").map(|head| head.id);
  let (Some(Some(commit)), Some(Some(state))) = (objects.next(), objects.next()) else {
    return Ok(None);
  };
  if commit.kind != "commit" {
    bail!("{session_ref} points at a {}, not a state commit", commit.kind);
  }
  let commit = commit.id;
  let state: SessionState = serde_json::from_slice(&state.content).map_err(|e| anyhow!("Unreadable {STATE_FILE} of {session_ref}: {e}"))?;
  if state.version != STATE_VERSION {
    bail!("{session_ref} has state version {}; this bd reads {STATE_VERSION}", state.version);
  }
  let mut session = Session::new(session_ref, commit, state);
  session.head_seen = head_seen;
  Ok(Some(session))
}

/// Every session of the repository, of any worktree, with one `for-each-ref` and one `cat-file --batch`
pub fn list_sessions(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Vec<Session>> {
  let refs = git_executor.execute_command_lines(&["for-each-ref", "--format=%(refname)", SESSION_REF_PREFIX], repo_path)?;
  if refs.is_empty() {
    return Ok(Vec::new());
  }
  let input = refs.iter().fold(String::new(), |mut input, session_ref| {
    let _ = writeln!(input, "{session_ref}\n{session_ref}:{STATE_FILE}");
    input
  });
  let output = git_executor.execute_command_with_input_raw(&["cat-file", "--batch"], repo_path, &input)?;
  let mut objects = parse_batch(&output, b'\n')?.into_iter();
  let mut sessions = Vec::new();
  for session_ref in refs {
    let (Some(commit), Some(state)) = (objects.next().flatten(), objects.next().flatten()) else {
      continue;
    };
    match serde_json::from_slice::<SessionState>(&state.content) {
      Ok(state) => sessions.push(Session::new(session_ref, commit.id, state)),
      Err(e) => tracing::warn!(%session_ref, error = %e, "unreadable session state"),
    }
  }
  Ok(sessions)
}

/// An object read with `cat-file --batch`
#[derive(Debug)]
struct BatchObject {
  id: String,
  kind: String,
  content: Vec<u8>,
}

/// `<id> <type> <size>\n<content>\n` per object of `cat-file --batch` (`delimiter` `\0` with `-Z`), `None` for a missing one
fn parse_batch(output: &[u8], delimiter: u8) -> Result<Vec<Option<BatchObject>>> {
  let mut objects = Vec::new();
  let mut rest = output;
  while !rest.is_empty() {
    let header_end = rest.iter().position(|&b| b == delimiter).ok_or_else(|| anyhow!("truncated cat-file output"))?;
    let header = std::str::from_utf8(&rest[..header_end])?;
    // `<input> missing`, where the input may hold spaces
    if header.ends_with(" missing") || header.ends_with(" ambiguous") {
      objects.push(None);
      rest = &rest[header_end + 1..];
      continue;
    }
    let mut fields = header.split(' ');
    let (Some(id), Some(kind), Some(size)) = (fields.next(), fields.next(), fields.next()) else {
      bail!("Unexpected cat-file header: {header}");
    };
    let size: usize = size.parse()?;
    let content = rest.get(header_end + 1..header_end + 1 + size).ok_or_else(|| anyhow!("truncated cat-file output"))?;
    objects.push(Some(BatchObject {
      id: id.to_string(),
      kind: kind.to_string(),
      content: content.to_vec(),
    }));
    rest = rest.get(header_end + 2 + size..).unwrap_or_default();
  }
  Ok(objects)
}

/// What a new session starts from: the update check that conflicted, under the repository lock
#[derive(Debug, Clone)]
pub struct SessionStart<'a> {
  /// The check before an apply (`CheckPurpose::Apply`) that conflicted: the session replays what it read, so no caller
  /// can choose the commits the finish moves the branch to
  pub plan: &'a UpdatePlan,
  /// The worktree the session belongs to: the current one
  pub worktree: &'a Worktree,
  pub likely_landed: Vec<String>,
  pub dir: Option<String>,
  pub dir_created: bool,
}

/// Start the session of the current worktree for an update whose replay conflicts: the first conflicting commit is
/// found with a binary search over prefixes of the local commits (⌈log₂ n⌉ replays: `git replay` stops at the first
/// conflict), and its round is stored in a new session ref. Nothing else moves. `Unsupported` when the first conflict
/// can't be resolved in a round; then nothing is written but git objects
#[instrument(skip(git_executor, start), fields(branch = start.plan.checked.branch))]
pub fn start_session(git_executor: &GitCommandExecutor, repo_path: &str, start: SessionStart<'_>) -> Result<Step> {
  if !start.plan.is_applicable_conflict() {
    bail!("A resolve session starts only from the check of an update that conflicts");
  }
  let checked = &start.plan.checked;
  let excluded = start.plan.excluded();
  sweep_stale_temp_refs(git_executor, repo_path, STALE_TEMP_REF_AGE);
  let committer = current_committer(git_executor, repo_path)?;
  let mut revs = vec![checked.head.clone(), format!("^{}", checked.upstream_head)];
  revs.extend(excluded.iter().map(|rev| format!("^{rev}")));
  let mut args = vec!["rev-list", "--reverse"];
  args.extend(revs.iter().map(String::as_str));
  args.push("--");
  let all_local = git_executor.execute_command_lines(&args, repo_path)?;
  let landed_dropped = patch_identical(git_executor, repo_path, &checked.upstream_head, &checked.head, &all_local)?;
  let mut local = Vec::with_capacity(all_local.len());
  let mut run_starts = Vec::new();
  for (index, commit) in all_local.iter().enumerate() {
    if landed_dropped.iter().any(|drop| drop.commit == *commit) {
      continue;
    }
    // a gap before it: a run of consecutive commits starts here
    if index > 0 && !local.is_empty() && local.last() != Some(&all_local[index - 1]) {
      run_starts.push(local.len());
    }
    local.push(commit.clone());
  }
  let worktree = start.worktree;
  let mut state = SessionState {
    version: STATE_VERSION,
    worktree: worktree.id.clone(),
    git_dir: worktree.git_dir.clone(),
    branch: checked.branch.clone(),
    upstream: checked.upstream.clone(),
    base: checked.base.clone(),
    upstream_head: checked.upstream_head.clone(),
    excluded: excluded.to_vec(),
    committer,
    dir: start.dir,
    dir_created: start.dir_created,
    started_head: checked.head.clone(),
    head: checked.head.clone(),
    local,
    likely_landed: start.likely_landed,
    replayed: 0,
    tip: checked.upstream_head.clone(),
    round: 0,
    status: SessionStatus::Resolving,
    conflict: None,
    resolutions: Vec::new(),
    notes_before: None,
    landed_dropped,
    run_starts,
  };
  if !state.landed_dropped.is_empty() {
    info!(left_out = state.landed_dropped.len(), "local commits the upstream has with the same patch id are left out");
  }
  // the check replayed all of them and conflicted; without the landed ones it may not
  let known_conflict = state.landed_dropped.is_empty();
  let objects = advance(git_executor, repo_path, &mut state, known_conflict)?;
  let session_ref = session_ref(&worktree.id);
  let written = write_state(git_executor, repo_path, &state, None)?;
  let mut transaction = RefTransaction::new();
  transaction.create(&session_ref, &written.ids[0]);
  written.delete_temp_refs(&mut transaction);
  if let Err(e) = transaction.commit(git_executor, repo_path, "resolve session") {
    written.discard(git_executor, repo_path);
    return Err(e);
  }
  info!(round = state.round, replayed = state.replayed, total = state.total(), "started a resolve session");
  let mut session = Session::new(session_ref, written.ids[0].clone(), state);
  if session.state.status == SessionStatus::Resolving {
    session.objects = objects;
    return Ok(Step::Round(session));
  }
  let Session { ref_name, commit, state, .. } = session;
  finish(git_executor, repo_path, &ref_name, &commit, state, false, &[])
}

/// The resolution of one conflicting path, as the caller read it
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditCopy {
  /// No file at `edit`
  Missing,
  /// A symlink, a directory or another non-regular file at `edit`: never followed
  NotARegularFile,
  Content(Vec<u8>),
}

/// The side of `--take <side>:<path>`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
  Onto,
  Commit,
}

/// The input of `--continue`
#[derive(Debug, Clone, Default)]
pub struct RoundInput {
  pub takes: Vec<(Side, String)>,
  pub deletes: Vec<String>,
  /// The `edit` copy per path, read by the caller (without filters, never through a symlink)
  pub edits: HashMap<String, EditCopy>,
  /// A result without changes is kept as an empty commit
  pub allow_empty: bool,
  /// A result without changes drops the commit this names (a prefix of its id, at least 4 characters)
  pub drop_empty: Option<String>,
}

/// Continue the session in `worktree`: check and commit the resolution of the open round, replay the rest (one
/// `replay`, a binary search only when it conflicts), and finish when every commit is replayed. A finish blocked by local
/// changes keeps the session ready; a later `--continue` only finishes. Peers' commits on top of the session's HEAD
/// become more local commits; a rewritten HEAD is `ResolveError::Stale`. A result without changes is
/// `ResolveError::EmptyResolution` unless the input decides it. With `dry_run`, only the checks run and nothing but git
/// objects is written
#[instrument(skip(git_executor, worktree, session, input))]
pub fn continue_session(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  worktree: &Worktree,
  session: Session,
  round: Option<u32>,
  input: &RoundInput,
  dry_run: bool,
) -> Result<Step> {
  let mut state = session.state.clone();
  if let Some(round) = round
    && round != state.round
  {
    return Err(anyhow::Error::new(ResolveError::RoundMoved {
      expected: round,
      actual: state.round,
    }));
  }
  ensure_own_worktree(worktree, &state)?;
  if state.status == SessionStatus::Applied {
    if !input.takes.is_empty() || !input.deletes.is_empty() {
      return Err(invalid_input(
        "The branch moved already; only the files are left to follow: run --continue without --take or --delete",
      ));
    }
    return if dry_run {
      Ok(Step::Checked { session, resolution: None })
    } else {
      sweep_stale_temp_refs(git_executor, repo_path, STALE_TEMP_REF_AGE);
      finish_files_after_move(git_executor, repo_path, session)
    };
  }
  if state.status == SessionStatus::Applying && !dry_run {
    sweep_stale_temp_refs(git_executor, repo_path, STALE_TEMP_REF_AGE);
  }
  follow_head(git_executor, repo_path, &mut state, session.head_seen.as_deref())?;

  let mut resolved_temp_ref = None;
  let resolution = match state.conflict.clone() {
    Some(conflict) => {
      let (resolution, edits, objects) = check_round(git_executor, repo_path, &state, &conflict, input)?;
      let raw = &objects.commit;
      let tree_edits: Vec<TreeEdit<'_>> = edits.iter().map(PathEdit::as_tree_edit).collect();
      let written = write_edited_commit(
        git_executor,
        repo_path,
        &EditedCommit {
          tree: &conflict.merge_tree,
          edits: &tree_edits,
          parent: &conflict.onto,
          author: &raw.author,
          committer: &state.committer,
          encoding: raw.encoding.as_deref(),
          message: &raw.message,
        },
      )?;
      let empty = written.tree == written.parent_tree;
      let resolved = if empty {
        match empty_choice(state.round, &conflict, raw, input) {
          Ok(keep) => keep.then(|| written.commit.clone()),
          Err(e) => {
            written_discard(git_executor, repo_path, &written.temp_ref);
            return Err(e);
          }
        }
      } else {
        Some(written.commit.clone())
      };
      let resolution = Resolution { resolved, empty, ..resolution };
      if dry_run {
        written_discard(git_executor, repo_path, &written.temp_ref);
        return Ok(Step::Checked {
          session,
          resolution: Some(resolution),
        });
      }
      debug!(commit = %short_id(&conflict.commit), empty, "resolved a round");
      if let Some(resolved) = &resolution.resolved {
        state.tip.clone_from(resolved);
      }
      state.replayed += 1;
      state.conflict = None;
      state.resolutions.push(resolution.clone());
      resolved_temp_ref = Some(written.temp_ref);
      Some(resolution)
    }
    None if !input.takes.is_empty() || !input.deletes.is_empty() || !input.edits.is_empty() => {
      return Err(invalid_input("No round is open: every commit is replayed; run --continue without --take or --delete"));
    }
    None => None,
  };
  if dry_run {
    return Ok(Step::Checked { session, resolution });
  }

  let objects = advance(git_executor, repo_path, &mut state, false)?;
  let temp_refs: Vec<String> = resolved_temp_ref.into_iter().collect();
  if state.status != SessionStatus::Resolving {
    let unsaved = state != session.state;
    return finish(git_executor, repo_path, &session.ref_name, &session.commit, state, unsaved, &temp_refs);
  }
  let written = write_state(git_executor, repo_path, &state, Some(&session.commit))?;
  let mut transaction = RefTransaction::new();
  transaction.update(&session.ref_name, &written.ids[0], Some(&session.commit));
  written.delete_temp_refs(&mut transaction);
  for temp_ref in &temp_refs {
    transaction.delete(temp_ref, None);
  }
  if let Err(e) = transaction.commit(git_executor, repo_path, "resolve session") {
    written.discard(git_executor, repo_path);
    for temp_ref in &temp_refs {
      written_discard(git_executor, repo_path, temp_ref);
    }
    return Err(e);
  }
  let mut session = Session::new(session.ref_name, written.ids[0].clone(), state);
  session.objects = objects;
  Ok(Step::Round(session))
}

/// The local commits (`local`) whose patch id equals that of an upstream commit since the merge base, with that commit:
/// one `rev-list --cherry-mark` marks the commits of both sides that have a counterpart, then one `diff-tree --stdin -p`
/// and one `patch-id --stable` pair them. As `git rebase` does, they are left out of the replay; the patch id is the
/// proof that the upstream has their change
fn patch_identical(git_executor: &GitCommandExecutor, repo_path: &str, upstream_head: &str, head: &str, local: &[String]) -> Result<Vec<LandedDrop>> {
  let marked = git_executor.execute_command_lines(&["rev-list", "--cherry-mark", "--no-merges", &format!("{upstream_head}...{head}"), "--"], repo_path)?;
  let equivalent: Vec<&str> = marked.iter().filter_map(|line| line.strip_prefix('=')).collect();
  if equivalent.is_empty() {
    return Ok(Vec::new());
  }
  let input: String = equivalent.iter().fold(String::new(), |mut input, id| {
    let _ = writeln!(input, "{id}");
    input
  });
  let patches = git_executor.execute_command_with_input_raw(&["diff-tree", "--stdin", "-p", "--no-renames", "--no-ext-diff", "--no-textconv"], repo_path, &input)?;
  let ids = git_executor.execute_command_with_input_bytes(&["patch-id", "--stable"], repo_path, &patches)?;
  // `<patch id> <commit>` per commit
  let mut upstream_by_patch: HashMap<&str, &str> = HashMap::new();
  let mut local_by_patch: Vec<(&str, &str)> = Vec::new();
  let ids = String::from_utf8_lossy(&ids).into_owned();
  for line in ids.lines() {
    let Some((patch, commit)) = line.split_once(' ') else { continue };
    if local.iter().any(|id| id == commit) {
      local_by_patch.push((patch, commit));
    } else {
      upstream_by_patch.entry(patch).or_insert(commit);
    }
  }
  Ok(
    local_by_patch
      .into_iter()
      .filter_map(|(patch, commit)| {
        upstream_by_patch.get(patch).map(|upstream| LandedDrop {
          commit: commit.to_string(),
          upstream: (*upstream).to_string(),
        })
      })
      .collect(),
  )
}

/// What a resolution without changes does: `Ok(true)` keeps the empty commit (`--allow-empty`), `Ok(false)` drops it
/// (`--drop-empty` with the round's commit); without either, `EmptyResolution`
fn empty_choice(round: u32, conflict: &RoundConflict, raw: &RawCommit, input: &RoundInput) -> Result<bool> {
  if input.allow_empty {
    return Ok(true);
  }
  match &input.drop_empty {
    Some(commit) if commit.len() >= 4 && conflict.commit.starts_with(commit.as_str()) => Ok(false),
    Some(commit) => Err(invalid_input(&format!(
      "--drop-empty {commit} does not name the commit of the round, {}",
      short_id(&conflict.commit)
    ))),
    None => Err(anyhow::Error::new(ResolveError::EmptyResolution {
      round,
      commit: conflict.commit.clone(),
      subject: subject_of(raw),
    })),
  }
}

/// The first line of a commit's message
fn subject_of(raw: &RawCommit) -> String {
  String::from_utf8_lossy(raw.message.split(|&b| b == b'\n').next().unwrap_or_default()).into_owned()
}

fn invalid_input(message: &str) -> anyhow::Error {
  anyhow::Error::new(ResolveError::InvalidInput { message: message.to_string() })
}

fn written_discard(git_executor: &GitCommandExecutor, repo_path: &str, temp_ref: &str) {
  if let Err(e) = git_executor.execute_command(&["update-ref", "-d", temp_ref], repo_path) {
    tracing::warn!(%temp_ref, error = %e, "failed to delete temporary ref");
  }
}

/// Delete the session ref with compare-and-swap. A session whose branch moved (`applied`) is deleted only when the files
/// followed and the branch still contains its tip: then nothing is left to roll back, and `bd undo update:` takes the
/// update back. Otherwise `FinishPending`
#[instrument(skip(git_executor, session))]
pub fn abort_session(git_executor: &GitCommandExecutor, repo_path: &str, session: &Session) -> Result<()> {
  if session.state.status == SessionStatus::Applied {
    match moved_session(git_executor, repo_path, session)? {
      Moved::FilesFollowed => {}
      Moved::AtTip { .. } => {
        return Err(anyhow::Error::new(ResolveError::FinishPending {
          message: format!(
            "{} moved already and the files did not follow yet: run `bd update --continue` to let them follow, then `bd undo update:{}..{}` takes the update back",
            session.state.branch, session.state.head, session.state.tip
          ),
        }));
      }
      Moved::Elsewhere(message) => return Err(anyhow::Error::new(ResolveError::FinishPending { message })),
    }
  }
  let mut transaction = RefTransaction::new();
  transaction.delete(&session.ref_name, Some(&session.commit));
  transaction.commit(git_executor, repo_path, "resolve session aborted")
}

/// `Stale` unless the session belongs to `worktree`
fn ensure_own_worktree(worktree: &Worktree, state: &SessionState) -> Result<()> {
  if worktree.git_dir == state.git_dir {
    return Ok(());
  }
  Err(anyhow::Error::new(ResolveError::Stale {
    message: format!(
      "The resolve session {} belongs to the worktree with the git dir {}, not to this one; `bd update --abort --worktree {}` removes it",
      session_ref(&state.worktree),
      state.git_dir,
      state.worktree
    ),
  }))
}

/// Follow the peers: HEAD (`head_seen` when the caller read it already) must still be on the session's branch and
/// contain the session's HEAD. Commits made on top of it are appended to the local commits (more rounds, usually
/// clean); a rewrite is `Stale`
fn follow_head(git_executor: &GitCommandExecutor, repo_path: &str, state: &mut SessionState, head_seen: Option<&str>) -> Result<()> {
  let branch = check_update_guards(git_executor, repo_path)?;
  if branch != state.branch {
    return Err(stale(&format!("HEAD is on {branch} now, not on {}", state.branch)));
  }
  let head = match head_seen {
    Some(head) => head.to_string(),
    None => git_executor.execute_command(&["rev-parse", "HEAD^{commit}"], repo_path)?,
  };
  if head == state.head {
    return Ok(());
  }
  let (_, exit_code) = git_executor.execute_command_with_status(&["merge-base", "--is-ancestor", &state.head, &head], repo_path)?;
  if exit_code != 0 {
    return Err(stale(&format!("{} was rewritten: it no longer contains {}", state.branch, short_id(&state.head))));
  }
  ensure_no_merges(git_executor, repo_path, &head, &[&state.head])?;
  let new = git_executor.execute_command_lines(&["rev-list", "--reverse", &head, &format!("^{}", state.head), "--"], repo_path)?;
  info!(new = new.len(), "commits on top of the session's HEAD join the session");
  // a run of consecutive commits ends at a commit left out as landed
  if !new.is_empty() && state.local.last() != Some(&state.head) {
    state.run_starts.push(state.local.len());
  }
  state.local.extend(new);
  state.head = head;
  // a finished replay replays the new commits too
  if matches!(state.status, SessionStatus::Ready | SessionStatus::Applying) {
    state.status = SessionStatus::Ready;
  }
  Ok(())
}

fn stale(reason: &str) -> anyhow::Error {
  anyhow::Error::new(ResolveError::Stale {
    message: format!("{reason}; the resolve session can't continue: `bd update --abort` removes it, then run `bd update` again"),
  })
}

/// A path edit of a resolved round
#[derive(Debug)]
enum PathEdit {
  Content { path: String, mode: String, content: Vec<u8> },
  Blob { path: String, mode: String, id: String },
  Delete { path: String },
}

impl PathEdit {
  fn as_tree_edit(&self) -> TreeEdit<'_> {
    match self {
      Self::Content { path, mode, content } => TreeEdit::Content { path, mode, content },
      Self::Blob { path, mode, id } => TreeEdit::Blob { path, mode, id },
      Self::Delete { path } => TreeEdit::Delete { path },
    }
  }
}

/// Check the input of a round: every conflicting path needs exactly one resolution, an edit passes the marker checks.
/// Returns the resolution (without the resolved commit), the tree edits, and the objects of the round, read with one
/// `cat-file --batch -Z`
fn check_round(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  state: &SessionState,
  conflict: &RoundConflict,
  input: &RoundInput,
) -> Result<(Resolution, Vec<PathEdit>, RoundObjects)> {
  let known = |path: &str| conflict.files.iter().any(|file| file.is_named(path));
  if let Some(path) = input.takes.iter().map(|(_, path)| path).chain(&input.deletes).find(|path| !known(path)) {
    return Err(invalid_input(&format!("{path} is not a conflicting file of round {}", state.round)));
  }
  let objects = read_round_objects(git_executor, repo_path, conflict)?;
  let base_label = conflict.base_label();
  let labels = conflict.labels(&base_label);
  let blob = |stage: &Option<Stage>| stage.as_ref().and_then(|stage| objects.blobs.get(&stage.id)).map(Vec::as_slice);
  let mut refused: Vec<(RefusalKind, String)> = Vec::new();
  let mut paths = Vec::new();
  let mut edits = Vec::new();
  for file in &conflict.files {
    let take = input.takes.iter().find(|(_, path)| file.is_named(path)).map(|(side, _)| *side);
    let delete = input.deletes.iter().any(|path| file.is_named(path));
    if take.is_some() && delete || input.takes.iter().filter(|(_, path)| file.is_named(path)).count() > 1 {
      return Err(invalid_input(&format!("{} has more than one resolution", file.path)));
    }
    let path = file.path.clone();
    let taken = match (take, delete) {
      (Some(Side::Onto), _) => Some((Outcome::Onto, &file.onto)),
      (Some(Side::Commit), _) => Some((Outcome::Commit, &file.commit)),
      (None, true) => Some((Outcome::Deleted, &None)),
      (None, false) => None,
    };
    // the lines of an edit that replaced the conflict blocks: where the dropped lines are looked for
    let mut replaced: Option<Vec<&[u8]>> = None;
    let (outcome, edit, result): (Outcome, PathEdit, &[u8]) = if let Some((outcome, stage)) = taken {
      match stage {
        Some(stage) => (
          outcome,
          PathEdit::Blob {
            path: path.clone(),
            mode: stage.mode.clone(),
            id: stage.id.clone(),
          },
          objects.blobs.get(&stage.id).map_or(&[][..], Vec::as_slice),
        ),
        None => (Outcome::Deleted, PathEdit::Delete { path: path.clone() }, &[]),
      }
    } else {
      let Some(EditCopy::Content(content)) = input.edits.get(&file.path) else {
        refused.push((RefusalKind::MissingResolution, path));
        continue;
      };
      let merged = objects.merged.get(&file.path).map_or(&[][..], Vec::as_slice);
      let failure = match file.kind {
        ConflictKind::Content => match check_resolution(merged, content, labels) {
          Ok(lines) => {
            replaced = Some(lines);
            None
          }
          Err(failure) => Some(failure),
        },
        // the version left in the tree: unchanged, it decides nothing
        kind if kind.is_one_sided() && content.as_slice() == merged => {
          refused.push((RefusalKind::MissingResolution, path));
          continue;
        }
        _ if has_foreign_encoding(merged, content) => Some(CheckFailure::Encoding),
        _ => has_markers(content, labels).then_some(CheckFailure::UnresolvedMarkers),
      };
      if let Some(failure) = failure {
        let kind = match failure {
          CheckFailure::OutsideConflict => RefusalKind::OutsideConflict,
          CheckFailure::LineEnds => RefusalKind::LineEnds,
          CheckFailure::Encoding => RefusalKind::Encoding,
          CheckFailure::UnresolvedMarkers => RefusalKind::UnresolvedMarkers,
        };
        refused.push((kind, path));
        continue;
      }
      let edit = PathEdit::Content {
        path: path.clone(),
        mode: file.mode().to_string(),
        content: content.clone(),
      };
      (Outcome::Edited, edit, content.as_slice())
    };
    let before = blob(&file.before).unwrap_or_default();
    let dropped = |side: &Option<Stage>| blob(side).map_or_else(Vec::new, |side| dropped_lines(before, side, result, replaced.as_deref()));
    paths.push(ResolvedPath {
      path,
      outcome,
      dropped_upstream_lines: dropped(&file.onto),
      dropped_commit_lines: dropped(&file.commit),
    });
    edits.push(edit);
  }
  // one kind at a time, the one the agent fixes first
  for kind in [
    RefusalKind::MissingResolution,
    RefusalKind::Encoding,
    RefusalKind::OutsideConflict,
    RefusalKind::LineEnds,
    RefusalKind::UnresolvedMarkers,
  ] {
    let paths: Vec<String> = refused.iter().filter(|(refused, _)| *refused == kind).map(|(_, path)| path.clone()).collect();
    if !paths.is_empty() {
      return Err(anyhow::Error::new(ResolveError::Refused(Refusal {
        kind,
        paths,
        edits: Vec::new(),
        changed_in_working_tree: Vec::new(),
        round: state.round,
        commit: conflict.commit.clone(),
        subject: subject_of(&objects.commit),
      })));
    }
  }
  let resolution = Resolution {
    round: state.round,
    commit: conflict.commit.clone(),
    resolved: None,
    empty: false,
    paths,
  };
  Ok((resolution, edits, objects))
}

/// The objects of a round for its copies and its output: the commit and every blob of every conflicting path
#[derive(Debug, Clone)]
pub struct RoundObjects {
  pub commit: RawCommit,
  /// The merge result per path: the `edit` copy
  pub merged: HashMap<String, Vec<u8>>,
  /// By blob id
  pub blobs: HashMap<String, Vec<u8>>,
}

/// Read the objects of `conflict` with one `cat-file --batch -Z`: the commit, the merge result of each path and the
/// blob of each stage. Bytes as stored, without filters
#[instrument(skip(git_executor, conflict), fields(commit = %short_id(&conflict.commit)))]
pub fn read_round_objects(git_executor: &GitCommandExecutor, repo_path: &str, conflict: &RoundConflict) -> Result<RoundObjects> {
  let stages: Vec<&str> = conflict
    .files
    .iter()
    .flat_map(|file| [&file.before, &file.onto, &file.commit])
    .filter_map(|stage| stage.as_ref().map(|stage| stage.id.as_str()))
    .collect();
  let mut input = format!("{}\0", conflict.commit);
  for file in &conflict.files {
    let _ = write!(input, "{}:{}\0", conflict.merge_tree, file.path);
  }
  for id in &stages {
    input.push_str(id);
    input.push('\0');
  }
  let output = git_executor.execute_command_with_input_raw(&["cat-file", "--batch", "-Z"], repo_path, &input)?;
  let mut objects = parse_batch(&output, 0)?.into_iter();
  let object = objects.next().flatten().ok_or_else(|| anyhow!("commit {} not found", conflict.commit))?;
  if object.kind != "commit" {
    bail!("{} is a {}, not a commit", object.id, object.kind);
  }
  let commit = parse_raw_commit(&object.id, &object.content)?;
  let merged = conflict
    .files
    .iter()
    .zip(objects.by_ref())
    .filter_map(|(file, object)| object.map(|object| (file.path.clone(), object.content)))
    .collect();
  let blobs = stages
    .iter()
    .zip(objects)
    .filter_map(|(id, object)| object.map(|object| ((*id).to_string(), object.content)))
    .collect();
  Ok(RoundObjects { commit, merged, blobs })
}

/// `c^..c` of the round's commit, with its message and stat: what the agent reapplies onto `onto`
pub fn commit_patch(git_executor: &GitCommandExecutor, repo_path: &str, commit: &str) -> Result<Vec<u8>> {
  let (output, _) = git_executor.execute_command_raw_accepting(
    &[
      "log",
      "-1",
      "--patch-with-stat",
      "--no-ext-diff",
      "--no-textconv",
      "--default-prefix",
      "-M",
      "--format=fuller",
      commit,
      "--",
    ],
    repo_path,
    &[],
  )?;
  Ok(output)
}

/// The upstream commits since the merge base that changed the conflicting files of the round, newest
/// `MAX_UPSTREAM_COMMITS_PER_FILE` per file, by path
pub fn round_upstream_commits(git_executor: &GitCommandExecutor, repo_path: &str, state: &SessionState) -> Result<HashMap<String, Vec<UpdateCommit>>> {
  let Some(conflict) = &state.conflict else { return Ok(HashMap::new()) };
  let files: Vec<String> = conflict.files.iter().map(|file| file.path.clone()).collect();
  let range = format!("{}..{}", state.base, state.upstream_head);
  let commits = commits_touching(git_executor, repo_path, &[&range], &files, Some(MAX_UPSTREAM_COMMITS_PER_FILE * files.len()))?;
  let mut by_path: HashMap<String, Vec<UpdateCommit>> = HashMap::new();
  // newest first, so each file keeps its newest
  for commit in commits.iter().rev() {
    for file in &commit.files {
      let list = by_path.entry(file.clone()).or_default();
      if list.len() < MAX_UPSTREAM_COMMITS_PER_FILE {
        list.push(commit.clone());
      }
    }
  }
  Ok(by_path)
}

/// Replay the local commits after `replayed` onto `tip`. With `known_conflict` (the replay of all of them conflicted
/// already) or after one replay that conflicts, a binary search over the prefixes finds the first conflicting commit,
/// whose round opens. Every replay runs with the session's committer, so the same input gives the same ids. Without a
/// conflict the session is ready to finish. Returns the objects of the round it opened
fn advance(git_executor: &GitCommandExecutor, repo_path: &str, state: &mut SessionState, known_conflict: bool) -> Result<Option<RoundObjects>> {
  let env = state.committer_env()?;
  let env: Vec<(&str, &str)> = env.iter().map(|(name, value)| (*name, value.as_str())).collect();
  let total = state.local.len();
  let replay = |state: &SessionState, last: usize| -> Result<Option<String>> {
    // linear history within a run: `<last> ^<first>^` is exactly the commits from `replayed` to `last`
    let revs = [state.local[last].clone(), format!("^{}^", state.local[state.replayed])];
    let revs: Vec<&str> = revs.iter().map(String::as_str).collect();
    Ok(replay_range_with_env(git_executor, repo_path, &state.tip, &revs, &env)?)
  };
  // one replay per run of consecutive commits; the first run that conflicts is searched
  let high = loop {
    if state.replayed == total {
      state.status = SessionStatus::Ready;
      return Ok(None);
    }
    let end = state.run_end(state.replayed);
    // the check replayed all of them in one run and conflicted
    if known_conflict && state.run_starts.is_empty() {
      break end - 1;
    }
    match replay(state, end - 1)? {
      Some(tip) => {
        state.tip = tip;
        state.replayed = end;
      }
      None => break end - 1,
    }
  };
  // the first commit whose prefix conflicts: `high` always conflicts, every prefix before `low` replays cleanly
  let (mut low, mut high) = (state.replayed, high);
  let mut tip = state.tip.clone();
  while low < high {
    let middle = low + (high - low) / 2;
    match replay(state, middle)? {
      Some(clean) => {
        tip = clean;
        low = middle + 1;
      }
      None => high = middle,
    }
  }
  state.tip = tip;
  state.replayed = low;
  state.round += 1;
  state.status = SessionStatus::Resolving;
  let (conflict, objects) = open_round(git_executor, repo_path, state)?;
  state.conflict = Some(conflict);
  debug!(round = state.round, commit = %short_id(&state.local[low]), "a round opens");
  Ok(Some(objects))
}

/// The round of the local commit at `state.replayed` onto `state.tip`: one `merge-tree --write-tree -z` with zdiff3
/// markers, then the supported conflict types only, and the objects of the round. A text conflict whose merge result
/// has no block of bd's labels is refused too: its markers can't be checked
fn open_round(git_executor: &GitCommandExecutor, repo_path: &str, state: &SessionState) -> Result<(RoundConflict, RoundObjects)> {
  let commit = &state.local[state.replayed];
  let parent = format!("{commit}^");
  let (output, exit_code) = git_executor.execute_command_raw_accepting(
    &[
      "-c",
      "merge.conflictStyle=zdiff3",
      "merge-tree",
      "--write-tree",
      "-z",
      "--merge-base",
      &parent,
      &state.tip,
      commit,
    ],
    repo_path,
    &[1],
  )?;
  let merge = parse_merge_tree_output(&output)?;
  if exit_code == 0 || merge.entries.is_empty() {
    bail!("{} replays with a conflict, but merge-tree merges it cleanly", short_id(commit));
  }
  let files = conflict_files(&merge).map_err(|e| name_the_commit(git_executor, repo_path, state, e))?;
  let conflict = RoundConflict {
    commit: commit.clone(),
    onto: state.tip.clone(),
    files,
    merge_tree: merge.tree,
  };
  let objects = read_round_objects(git_executor, repo_path, &conflict)?;
  let base_label = conflict.base_label();
  let labels = conflict.labels(&base_label);
  let unreadable: Vec<String> = conflict
    .files
    .iter()
    .filter(|file| file.kind == ConflictKind::Content && objects.merged.get(&file.path).is_none_or(|merged| block_count(merged, labels) == 0))
    .map(|file| file.path.clone())
    .collect();
  if !unreadable.is_empty() {
    let message = format!(
      "The conflict can't be resolved in a round of bd update: bd finds no conflict block it wrote in {}. `bd update --abort`, then update in a terminal",
      unreadable.join(", ")
    );
    let error = anyhow::Error::new(ResolveError::Unsupported { paths: unreadable, message });
    return Err(name_the_commit(git_executor, repo_path, state, error));
  }
  Ok((conflict, objects))
}

/// An `Unsupported` refusal of the round of `state` with the round and the commit (id and subject) in front
fn name_the_commit(git_executor: &GitCommandExecutor, repo_path: &str, state: &SessionState, error: anyhow::Error) -> anyhow::Error {
  let Some(ResolveError::Unsupported { paths, message }) = error.downcast_ref::<ResolveError>() else {
    return error;
  };
  let commit = &state.local[state.replayed];
  let subject = git_executor.execute_command(&["log", "-1", "--format=%s", commit, "--"], repo_path).unwrap_or_default();
  let mut message = format!("Round {}, commit {} {subject}: {message}", state.round, short_id(commit));
  if !state.resolutions.is_empty() {
    message.push_str(". The rounds resolved so far are lost with --abort");
  }
  anyhow::Error::new(ResolveError::Unsupported { paths: paths.clone(), message })
}

/// The conflicting paths of a merge, each a supported type; any other type refuses the whole round
pub(crate) fn conflict_files(merge: &MergeTreeOutput) -> Result<Vec<ConflictFile>> {
  let mut unsupported: Vec<(String, String)> = Vec::new();
  let supported_kind = |kind: &str| matches!(kind, "CONFLICT (contents)" | "CONFLICT (modify/delete)" | "CONFLICT (add/add)" | "CONFLICT (rename/delete)");
  // `<new path>`, `<old path>` of each rename/delete
  let renamed: Vec<(&[u8], &[u8])> = merge
    .messages
    .iter()
    .filter(|message| message.kind == "CONFLICT (rename/delete)")
    .filter_map(|message| Some((message.paths.first()?.as_slice(), message.paths.get(1)?.as_slice())))
    .collect();
  for message in merge
    .messages
    .iter()
    .filter(|message| message.kind.starts_with("CONFLICT") && !supported_kind(&message.kind))
  {
    for path in &message.paths {
      unsupported.push((String::from_utf8_lossy(path).into_owned(), message.kind.clone()));
    }
  }
  let mut files = Vec::new();
  for path in merge.conflicted_paths() {
    let display = String::from_utf8_lossy(path).into_owned();
    if unsupported.iter().any(|(unsupported, _)| *unsupported == display) {
      continue;
    }
    let Some(path) = std::str::from_utf8(path).ok().filter(|path| is_valid_tree_path(path)) else {
      unsupported.push((display, "an invalid path".to_string()));
      continue;
    };
    let stage = |number: u8| {
      merge
        .entries
        .iter()
        .find(|entry| entry.path == path.as_bytes() && entry.stage == number)
        .map(|entry| Stage {
          mode: entry.mode.clone(),
          id: entry.id.clone(),
        })
    };
    let (before, onto, commit) = (stage(1), stage(2), stage(3));
    let modes: Vec<&str> = [&before, &onto, &commit].into_iter().flatten().map(|stage| stage.mode.as_str()).collect();
    if let Some(mode) = modes.iter().find(|mode| !FILE_MODES.contains(mode)) {
      unsupported.push((path.to_string(), format!("mode {mode} (a symlink or a submodule)")));
      continue;
    }
    // as git merges a mode: the side that changed it wins
    let mode = match (&before, &onto, &commit) {
      (_, Some(onto), Some(commit)) if onto.mode == commit.mode => onto.mode.clone(),
      (Some(before), Some(onto), Some(commit)) if before.mode == onto.mode => commit.mode.clone(),
      (Some(before), Some(onto), Some(commit)) if before.mode == commit.mode => onto.mode.clone(),
      (_, Some(_), Some(_)) => {
        unsupported.push((path.to_string(), "distinct modes".to_string()));
        continue;
      }
      (_, Some(side), None) | (_, None, Some(side)) => side.mode.clone(),
      _ => String::new(),
    };
    let renamed_from = renamed
      .iter()
      .find(|(new, _)| *new == path.as_bytes())
      .and_then(|(_, old)| std::str::from_utf8(old).ok())
      .map(str::to_string);
    let kind = match (&before, &onto, &commit) {
      (_, Some(_), Some(_)) => ConflictKind::Content,
      (Some(_), Some(_), None) | (Some(_), None, Some(_)) if renamed_from.is_some() => ConflictKind::RenameDelete,
      (Some(_), Some(_), None) | (Some(_), None, Some(_)) => ConflictKind::ModifyDelete,
      _ => {
        unsupported.push((path.to_string(), "an unknown stage set".to_string()));
        continue;
      }
    };
    files.push(ConflictFile {
      key: files.len() + 1,
      path: path.to_string(),
      kind,
      before,
      onto,
      commit,
      mode,
      renamed_from,
    });
  }
  if !unsupported.is_empty() {
    unsupported.dedup();
    let message = format!(
      "The conflict can't be resolved in a round of bd update: {}. Only text conflicts, modify/delete and rename/delete are supported; `bd update --abort`, then update in a terminal",
      unsupported.iter().map(|(path, kind)| format!("{path} ({kind})")).collect::<Vec<_>>().join(", ")
    );
    let mut paths: Vec<String> = unsupported.into_iter().map(|(path, _)| path).collect();
    paths.dedup();
    return Err(anyhow::Error::new(ResolveError::Unsupported { paths, message }));
  }
  Ok(files)
}

/// A tree path that is safe to name a file by: no empty, `.` or `..` component, no `.git` or a name the file system
/// takes for it (`GIT~1`, `.git.`, `.g\u{200c}it`, as `core.protectNTFS` and `core.protectHFS`), no NUL; on Windows also
/// no `\`, no drive prefix and no device name (`CON`). The copies use only the last component, but the path also goes
/// into the state tree and the output
pub fn is_valid_tree_path(path: &str) -> bool {
  let options = gix_validate::path::component::Options {
    protect_windows: cfg!(windows),
    protect_hfs: true,
    protect_ntfs: true,
  };
  !path.contains('\0') && path.split('/').all(|component| gix_validate::path::component(BStr::new(component), None, options).is_ok())
}

/// Write the state commit of `state` on a temporary ref, by the session's committer: `state.json`, the round's merge
/// tree and stage blobs as entries, the previous state as the parent, the tip and HEAD as merge parents
fn write_state(git_executor: &GitCommandExecutor, repo_path: &str, state: &SessionState, previous: Option<&str>) -> Result<WrittenStore> {
  let json = serde_json::to_vec_pretty(state)?;
  let entries = state_entries(state);
  let commit = state_commit(state, &json, &entries, previous.map(NewParent::Existing));
  write_store_commits(git_executor, repo_path, &[commit], None, Some(&state.committer))
}

/// The tree entries of a state besides `state.json`: `(mode, id, path)` of `round/merge`, the merge tree, which keeps
/// the copies of the open round rebuildable, and of `round/{before,onto,commit}/<path>`, the stage blobs
fn state_entries(state: &SessionState) -> Vec<(&'static str, String, String)> {
  let Some(conflict) = &state.conflict else { return Vec::new() };
  let mut entries = vec![("040000", conflict.merge_tree.clone(), "round/merge".to_string())];
  for file in &conflict.files {
    for (side, stage) in [("before", &file.before), ("onto", &file.onto), ("commit", &file.commit)] {
      if let Some(stage) = stage {
        entries.push(("100644", stage.id.clone(), format!("round/{side}/{}", file.path)));
      }
    }
  }
  entries
}

fn state_commit<'a>(state: &'a SessionState, json: &'a [u8], entries: &'a [(&'static str, String, String)], parent: Option<NewParent<'a>>) -> StoreCommit<'a> {
  let mut merges = vec![state.tip.as_str()];
  if state.head != state.tip {
    merges.push(state.head.as_str());
  }
  StoreCommit {
    files: vec![(STATE_FILE, json)],
    entries: entries.iter().map(|(mode, id, path)| StoreEntry { mode, id, path }).collect(),
    parent,
    merges,
    message: "Resolve session of bd update",
  }
}

/// The note of a resolved commit: deterministic, so a rerun writes the same notes commit content
fn note_of(state: &SessionState, resolution: &Resolution) -> String {
  let mut note = format!(
    "branch-deck resolve\nsession: {} {}\nround: {}\ncommit: {}\n",
    state.worktree,
    state.committer.rsplit(' ').nth(1).unwrap_or_default(),
    resolution.round,
    resolution.commit
  );
  for path in &resolution.paths {
    let _ = writeln!(note, "{}\t{}", path.outcome.as_str(), path.path);
  }
  note
}

/// The finish of a ready session, in four compare-and-swap steps: the session ref moves from `base_commit` to
/// `applying`; one transaction moves the branch (deref `HEAD`, checked under its lock as every update) with the session
/// ref to `applied` and the notes; the files follow through the guarded update, and on a failure the branch, the
/// session ref and the notes move back together; then the session ref is deleted. A crash leaves `applying` (nothing
/// moved: the next `--continue` finishes) or `applied` (the branch moved: the next `--continue` lets the files follow).
/// Local changes on the paths it writes refuse it first (`Step::Blocked`); `state` is saved as `ready` then when it is
/// `unsaved`. `temp_refs` (the resolved commit of the last round) are deleted with the first transaction
fn finish(git_executor: &GitCommandExecutor, repo_path: &str, ref_name: &str, base_commit: &str, mut state: SessionState, unsaved: bool, temp_refs: &[String]) -> Result<Step> {
  let (head, tip) = (state.head.clone(), state.tip.clone());
  state.status = SessionStatus::Ready;
  let changed = changed_paths(git_executor, repo_path, &head, &tip)?;
  let blocked = if changed.is_empty() {
    Vec::new()
  } else {
    blocking_paths(git_executor, repo_path, &head, &tip, &changed)?
  };
  if !blocked.is_empty() {
    let mut commit = base_commit.to_string();
    if unsaved {
      let written = write_state(git_executor, repo_path, &state, Some(base_commit))?;
      let mut transaction = RefTransaction::new();
      transaction.update(ref_name, &written.ids[0], Some(base_commit));
      written.delete_temp_refs(&mut transaction);
      for temp_ref in temp_refs {
        transaction.delete(temp_ref, None);
      }
      if let Err(e) = transaction.commit(git_executor, repo_path, "resolve session") {
        written.discard(git_executor, repo_path);
        return Err(e);
      }
      commit.clone_from(&written.ids[0]);
    }
    return Ok(Step::Blocked {
      session: Session::new(ref_name.to_string(), commit, state),
      paths: blocked,
    });
  }

  let notes: Vec<(String, String)> = state
    .resolutions
    .iter()
    .filter_map(|resolution| resolution.resolved.as_ref().map(|id| (id.clone(), note_of(&state, resolution))))
    .collect();
  let (notes_before, exit_code) = git_executor.execute_command_with_status(&["rev-parse", "--verify", "--quiet", &format!("{RESOLVE_NOTES_REF}^{{commit}}")], repo_path)?;
  let notes_before = (exit_code == 0).then_some(notes_before);
  let store_notes = StoreNotes {
    notes_ref: RESOLVE_NOTES_REF,
    previous: notes_before.as_deref(),
    notes: &notes,
  };
  let mut applying = state.clone();
  applying.status = SessionStatus::Applying;
  applying.notes_before.clone_from(&notes_before);
  let mut applied = applying.clone();
  applied.status = SessionStatus::Applied;
  let (applying_json, applied_json) = (serde_json::to_vec_pretty(&applying)?, serde_json::to_vec_pretty(&applied)?);
  let entries = state_entries(&applying);
  let commits = [
    state_commit(&applying, &applying_json, &entries, Some(NewParent::Existing(base_commit))),
    state_commit(&applied, &applied_json, &entries, Some(NewParent::Written(0))),
  ];
  let written = write_store_commits(git_executor, repo_path, &commits, (!notes.is_empty()).then_some(&store_notes), Some(&state.committer))?;
  let (applying_commit, applied_commit) = (written.ids[0].clone(), written.ids[1].clone());

  // 1. applying: nothing moved yet
  let mut transaction = RefTransaction::new();
  transaction.update(ref_name, &applying_commit, Some(base_commit));
  for temp_ref in temp_refs {
    transaction.delete(temp_ref, None);
  }
  if let Err(e) = transaction.commit(git_executor, repo_path, "resolve session applying") {
    written.discard(git_executor, repo_path);
    return Err(e);
  }
  // 2. and 3. the branch with the session ref and the notes, then the files; both roll back together
  let mut with = RefTransaction::new();
  with.update(ref_name, &applied_commit, Some(&applying_commit));
  let mut rollback_with = RefTransaction::new();
  rollback_with.update(ref_name, &applying_commit, Some(&applied_commit));
  if let Some((notes_commit, _)) = &written.notes {
    store_notes.add_to(&mut with, notes_commit);
    match &notes_before {
      Some(before) => rollback_with.update(RESOLVE_NOTES_REF, before, Some(notes_commit)),
      None => rollback_with.delete(RESOLVE_NOTES_REF, Some(notes_commit)),
    };
  }
  written.delete_temp_refs(&mut with);
  let moved = move_branch_and_files_with(
    git_executor,
    repo_path,
    &FileMove {
      branch: &state.branch,
      head: &head,
      new_head: &tip,
      files_change: !changed.is_empty(),
      operation: UPDATE_OPERATION,
      with: &with,
      rollback_with: &rollback_with,
    },
  );
  if let Err(e) = moved {
    // the temporary refs stay when the move itself failed; the session stays `applying` or `applied`
    written.discard(git_executor, repo_path);
    return Err(e);
  }
  // 4. done
  let mut transaction = RefTransaction::new();
  transaction.delete(ref_name, Some(&applied_commit));
  transaction.commit(git_executor, repo_path, "resolve session finished")?;
  info!(rounds = state.round, %tip, "finished the resolve session");
  Ok(Step::Applied {
    session: Session::new(ref_name.to_string(), applied_commit, applied),
    previous_head: head,
  })
}

/// Where the branch of an `applied` session is
enum Moved {
  /// The files followed the move, and the branch still contains the tip: only the session ref is left
  FilesFollowed,
  /// HEAD is at the tip, the files did not follow
  AtTip { changed: Vec<ChangedPath> },
  /// HEAD left the tip before the files followed: what to tell the user
  Elsewhere(String),
}

fn moved_session(git_executor: &GitCommandExecutor, repo_path: &str, session: &Session) -> Result<Moved> {
  let state = &session.state;
  let branch = check_update_guards(git_executor, repo_path)?;
  let actual = git_executor.execute_command(&["rev-parse", "HEAD^{commit}"], repo_path)?;
  let changed = changed_paths(git_executor, repo_path, &state.head, &state.tip)?;
  let followed = changed.is_empty() || !index_left_behind(git_executor, repo_path, &changed)?;
  let contains_tip = actual == state.tip || {
    let (_, exit_code) = git_executor.execute_command_with_status(&["merge-base", "--is-ancestor", &state.tip, &actual], repo_path)?;
    exit_code == 0
  };
  if branch == state.branch && contains_tip && followed {
    return Ok(Moved::FilesFollowed);
  }
  if branch == state.branch && actual == state.tip {
    return Ok(Moved::AtTip { changed });
  }
  Ok(Moved::Elsewhere(format!(
    "The resolve session moved {} from {} to {}, but HEAD is at {} on {branch} now and the index still holds {} for the paths the \
     update changes. bd can't finish this session: look at `git status` with the user. `git update-ref -d {} {}` removes the \
     session",
    state.branch,
    short_id(&state.head),
    short_id(&state.tip),
    short_id(&actual),
    short_id(&state.head),
    session.ref_name,
    session.commit
  )))
}

/// Recover a session whose branch moved (`applied`) but whose files may not have followed: when the index holds no
/// path of the old HEAD any more and the branch contains the tip, the files did follow and only the session ref is left
/// (also after peers committed on top). At the tip, the files follow now through the guarded update, and when local
/// changes block it or it fails, the branch, the session ref and the notes move back together (`applying`). Anything
/// else is `FinishPending`
fn finish_files_after_move(git_executor: &GitCommandExecutor, repo_path: &str, session: Session) -> Result<Step> {
  let state = &session.state;
  match moved_session(git_executor, repo_path, &session)? {
    Moved::FilesFollowed => {}
    Moved::Elsewhere(message) => return Err(anyhow::Error::new(ResolveError::FinishPending { message })),
    Moved::AtTip { changed } => {
      let mut rollback_with = RefTransaction::new();
      let applying = git_executor.execute_command(&["rev-parse", &format!("{}^", session.commit)], repo_path)?;
      rollback_with.update(&session.ref_name, &applying, Some(&session.commit));
      let (notes_now, exit_code) = git_executor.execute_command_with_status(&["rev-parse", "--verify", "--quiet", &format!("{RESOLVE_NOTES_REF}^{{commit}}")], repo_path)?;
      if exit_code == 0 && state.notes_before.as_deref() != Some(notes_now.as_str()) {
        match &state.notes_before {
          Some(before) => rollback_with.update(RESOLVE_NOTES_REF, before, Some(&notes_now)),
          None => rollback_with.delete(RESOLVE_NOTES_REF, Some(&notes_now)),
        };
      }
      files_follow(git_executor, repo_path, &state.branch, &state.head, &state.tip, &changed, &rollback_with)?;
    }
  }
  let mut transaction = RefTransaction::new();
  transaction.delete(&session.ref_name, Some(&session.commit));
  transaction.commit(git_executor, repo_path, "resolve session finished")?;
  let previous_head = state.head.clone();
  Ok(Step::Applied { session, previous_head })
}

/// The local changes that likely block the finish, known before it: changes on the paths the upstream changes since
/// the merge base and HEAD has otherwise (the check of every sync, at a cost that grows with those paths). The finish
/// runs the exact check
pub fn session_blocked_paths(git_executor: &GitCommandExecutor, repo_path: &str, state: &SessionState) -> Result<Vec<String>> {
  likely_blocking_paths(git_executor, repo_path, &state.head, &state.base, &state.upstream_head)
}

#[cfg(test)]
#[path = "update_resolve_test.rs"]
mod update_resolve_test;
