//! Resolve sessions of `bd update` (docs/design/git-operations.md, "Resolve session"): an update that conflicts is
//! replayed commit by commit outside the working tree, and the agent resolves one conflicting commit per round. All
//! state lives in git: the session ref `refs/branch-deck/resolve/<worktree id>` points at a state commit whose tree
//! holds `state.json` and the objects of the open round, and whose parents keep the replayed commits reachable. Every
//! step moves the ref with compare-and-swap; nothing else moves until the finish, which moves the branch with the
//! files through `update_branch` (the guarded working-tree update) and only then deletes the ref.

use crate::cat_file_batch::{BatchEntry, NO_LIMIT, read_objects};
use crate::commit_writer::{
  EditedCommit, NewParent, RawCommit, STALE_TEMP_REF_AGE, StoreCommit, StoreEntry, StoreNotes, TreeEdit, WrittenEdit, WrittenStore, current_committer, discard_temp_refs,
  parse_raw_commit, sweep_stale_temp_refs, write_edited_commit, write_store_commits,
};
use crate::landed_pairing::{LandedCandidate, LandedDrop, PathLines, interdiffs};
use crate::merge_tree_output::{MergeTreeOutput, parse_merge_tree_output};
use crate::paths::{ChangedPath, changed_paths, commits_touching};
use crate::ref_transaction::RefTransaction;
use crate::replay::{RunsReplay, first_conflict_in_runs, replay_of};
use crate::resolve_check::{CheckFailure, Labels, block_count, check_resolution, dropped_lines, has_foreign_encoding, has_markers};
use crate::revs::{Worktree, is_ancestor, resolve_commit, short_id};
use crate::rewrite_guard::ensure_no_merges;
use crate::update_branch::{
  FileMove, HeadRef, UPDATE_OPERATION, UpdatePlan, blocking_paths, check_update_guards, files_follow, index_left_behind, likely_blocking_paths, move_branch_and_files_with,
};
use crate::word_replay::resolve_merge_onto;
use crate::word_resolve::{RESOLVE_VERSION, ResolveMode};
use anyhow::{Result, anyhow, bail};
use bstr::BStr;
use domain_types::update_project::UpdateCommit;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fmt::Write as _;
use tracing::{debug, info, instrument};

/// The session refs, one per worktree: shared refs, so a `gc` in any worktree keeps their objects (a `refs/worktree/`
/// ref is invisible to the gc of another worktree)
pub const SESSION_REF_PREFIX: &str = "refs/branch-deck/resolve/";
/// The audit record of a session: a note per resolved commit, written only by the finish
pub const RESOLVE_NOTES_REF: &str = "refs/notes/branch-deck/resolve";
const STATE_FILE: &str = "state.json";
/// The version of `state.json`: 2 since a round can resolve itself by words (`Outcome::Words`, `Resolution::words`).
/// bd is one binary for every session of a machine, so a bd of another version refuses a session by this number
const STATE_VERSION: u32 = 2;
/// The modes of a regular file: the only ones a round supports
const FILE_MODES: [&str; 2] = ["100644", "100755"];
/// The upstream commits listed per conflicting file
const MAX_UPSTREAM_COMMITS_PER_FILE: usize = 10;
/// The longest basename a copy keeps (in bytes, the extension included), below `NAME_MAX` (255) with the key and the
/// name of the temporary file
const MAX_COPY_BASENAME: usize = 100;

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

  /// The names of this file: its path, then the path before its rename
  fn names(&self) -> impl Iterator<Item = &str> {
    std::iter::once(self.path.as_str()).chain(self.renamed_from.as_deref())
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
  /// The resolve by words (`branchdeck.resolveByWords`): the round resolved itself, no agent saw it
  Words,
}

impl Outcome {
  const fn as_str(self) -> &'static str {
    match self {
      Self::Edited => "edited",
      Self::Onto => "onto",
      Self::Commit => "commit",
      Self::Deleted => "deleted",
      Self::Words => "words",
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
  /// The conflicts of git in the file that the words resolved (`Outcome::Words`)
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub conflicts: Option<u32>,
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
  /// The result changes nothing: kept as an empty commit (`--allow-empty`) or dropped (`--drop-empty`); a round that
  /// the words resolved drops it, as `git replay` drops a commit that becomes empty
  #[serde(default, skip_serializing_if = "is_false")]
  pub empty: bool,
  pub paths: Vec<ResolvedPath>,
  /// The conflicts of git that the words resolved (`Outcome::Words`): the round resolved itself; `None` for a round of
  /// the agent
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub words: Option<u32>,
  /// `--landed`: the round dropped its commit as landed, as this upstream commit
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub landed: Option<LandedResolution>,
}

/// A round that `--landed` resolved: the commit is dropped as its upstream twin (`SessionState::landed_candidates`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LandedResolution {
  /// The upstream commit with the identity of the commit
  pub upstream: String,
  /// The user decided it with `--user`: the commit changes lines that the upstream commit lacks (`local_only`)
  #[serde(default, skip_serializing_if = "is_false")]
  pub user: bool,
  /// What the drop lost: the lines the commit (with its fixups) changes and its upstream commit does not
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub local_only: Vec<PathLines>,
}

impl Resolution {
  /// The resolution dropped the commit (`--drop-empty`, `--landed`, or a round that the words left without a change)
  pub const fn dropped(&self) -> bool {
    self.empty && self.resolved.is_none()
  }

  /// The round resolved itself by words
  pub const fn by_words(&self) -> bool {
    self.words.is_some()
  }
}

/// `state.json`, the only file of a state commit that readers parse
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionState {
  pub version: u32,
  pub worktree: String,
  /// The git dir of the worktree (absolute): a session never continues in another one
  pub git_dir: String,
  /// The checked out branch; `HEAD` for a detached HEAD, which only a session onto a local branch has (a lease,
  /// `UpstreamRef::local`; `HeadRef::of`)
  pub branch: String,
  /// The upstream as the user knows it, e.g. `origin/master`; it stays fixed for the session
  pub upstream: String,
  /// The upstream is a local branch, the home of a lease (`UpstreamRef::local`): the finish writes no landed report,
  /// nothing is in the baseline by it
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub local_upstream: bool,
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
  /// Local commits the upstream likely has already (same author, author time and subject), with their fixups; no proof
  /// left them out (`landed_pairing::LandedCandidate`)
  pub likely_landed: Vec<String>,
  /// The candidates of `likely_landed`, each with its upstream commit and its fixups: the round of one offers
  /// `--landed`. A session of an older bd has none
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub landed_candidates: Vec<LandedCandidate>,
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
  /// Local commits left out because the upstream proves it has their change (`landed_pairing::LandedProof`), as
  /// `git rebase` leaves out a commit with the same patch id
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub landed_dropped: Vec<LandedDrop>,
  /// The indices of `local` where a run of consecutive commits starts after one left out: a replay covers one run
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub run_starts: Vec<usize>,
}

impl SessionState {
  pub const fn total(&self) -> usize {
    self.local.len()
  }

  /// The candidate whose upstream twin `commit` belongs to: the candidate itself or one of its fixups
  pub fn candidate_of(&self, commit: &str) -> Option<&LandedCandidate> {
    self.landed_candidates.iter().find(|candidate| candidate.unit().any(|id| id == commit))
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
  /// The resolution leaves the commit without changes, and neither `--allow-empty` nor `--drop-empty` decided it;
  /// `twin` is the upstream commit with the commit's identity, when the round has one: `--landed` drops it
  EmptyResolution {
    round: u32,
    commit: String,
    subject: String,
    twin: Option<String>,
  },
  /// `--landed` on a round whose commit has no twin upstream (`SessionState::candidate_of`)
  NotLandedRound { round: u32, commit: String, subject: String },
  /// `--landed` without `--user` on a round whose commit changes lines that its twin lacks: the user decides
  LandedDiffers { round: u32, commit: String, subject: String, upstream: String },
  /// `--landed` on a round whose commit still changes `paths` with `onto` for every conflicting file
  LandedNotEmpty {
    round: u32,
    commit: String,
    subject: String,
    upstream: String,
    paths: Vec<String>,
  },
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
      Self::EmptyResolution {
        round,
        commit,
        subject,
        twin: Some(twin),
      } => write!(
        f,
        "Round {round}, commit {} {subject}: the resolution leaves it without any change, and the baseline has {} with its \
         identity: `--landed {}` instead of the takes drops it as landed. {}",
        short_id(commit),
        short_id(twin),
        short_id(commit),
        unchanged(*round)
      ),
      Self::EmptyResolution {
        round,
        commit,
        subject,
        twin: None,
      } => write!(
        f,
        "Round {round}, commit {} {subject}: the resolution leaves it without any change. Ask the user: `--drop-empty {}` \
         drops the commit and `--allow-empty` keeps it as an empty commit, each only once the user decided; or resolve it \
         another way. {}",
        short_id(commit),
        short_id(commit),
        unchanged(*round)
      ),
      Self::NotLandedRound { round, commit, subject } => write!(
        f,
        "Round {round}, commit {} {subject}: the baseline has no commit with its identity (`session.commit.landed_as`), so \
         --landed can't drop it. Resolve the files and run --continue without --landed. {}",
        short_id(commit),
        unchanged(*round)
      ),
      Self::LandedDiffers { round, commit, subject, upstream } => write!(
        f,
        "Round {round}, commit {} {subject}: the baseline has {} with its identity, but the commit changes lines that {} \
         lacks (`session.commit.landed_as.local_only`), and a drop loses them. Show them to the user and ask: \
         `--landed {} --user` drops the commit only once the user said yes; with a no, resolve the files and run \
         --continue without --landed. {}",
        short_id(commit),
        short_id(upstream),
        short_id(upstream),
        short_id(commit),
        unchanged(*round)
      ),
      Self::LandedNotEmpty {
        round,
        commit,
        subject,
        upstream,
        paths,
      } => write!(
        f,
        "Round {round}, commit {} {subject}: with `onto` for every conflicting file the commit still changes {}, which {} \
         does not have, and --landed drops only a commit without a change. Resolve the files and run --continue without \
         --landed, or ask the user. {}",
        short_id(commit),
        paths.join(", "),
        short_id(upstream),
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
      Self::EmptyResolution { commit, .. } | Self::NotLandedRound { commit, .. } | Self::LandedDiffers { commit, .. } | Self::LandedNotEmpty { commit, .. } => Some(commit),
      _ => None,
    }
  }

  /// The paths the refusal is about
  pub fn paths(&self) -> &[String] {
    match self {
      Self::Refused(refusal) => &refusal.paths,
      Self::Unsupported { paths, .. } | Self::LandedNotEmpty { paths, .. } => paths,
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
  let names = ["HEAD".to_string(), session_ref.clone(), format!("{session_ref}:{STATE_FILE}")];
  let mut objects = read_batch(git_executor, repo_path, &names)?.into_iter();
  let head_seen = objects.next().flatten().filter(|head| head.kind == "commit").map(|head| head.id);
  let (Some(Some(commit)), Some(Some(state))) = (objects.next(), objects.next()) else {
    return Ok(None);
  };
  if commit.kind != "commit" {
    bail!("{session_ref} points at a {}, not a state commit", commit.kind);
  }
  let commit = commit.id;
  // the version first: a state of another version may have fields or values this bd can't parse
  let version: StateVersion = serde_json::from_slice(&state.content).map_err(|e| anyhow!("Unreadable {STATE_FILE} of {session_ref}: {e}"))?;
  if version.version != STATE_VERSION {
    bail!("{session_ref} has state version {}; this bd reads {STATE_VERSION}", version.version);
  }
  let state: SessionState = serde_json::from_slice(&state.content).map_err(|e| anyhow!("Unreadable {STATE_FILE} of {session_ref}: {e}"))?;
  let mut session = Session::new(session_ref, commit, state);
  session.head_seen = head_seen;
  Ok(Some(session))
}

/// The one field of `state.json` that every version has
#[derive(Deserialize)]
struct StateVersion {
  version: u32,
}

/// Every session of the repository, of any worktree, with one `for-each-ref` and one `cat-file --batch`
pub fn list_sessions(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Vec<Session>> {
  let refs = git_executor.execute_command_lines(&["for-each-ref", "--format=%(refname)", SESSION_REF_PREFIX], repo_path)?;
  if refs.is_empty() {
    return Ok(Vec::new());
  }
  let names: Vec<String> = refs.iter().flat_map(|session_ref| [session_ref.clone(), format!("{session_ref}:{STATE_FILE}")]).collect();
  let mut objects = read_batch(git_executor, repo_path, &names)?.into_iter();
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
struct Object {
  id: String,
  kind: String,
  content: Vec<u8>,
}

/// The object of each of `names`, in this order, from one `cat-file --batch` (`cat_file_batch`); `None` for a missing one
fn read_batch(git_executor: &GitCommandExecutor, repo_path: &str, names: &[String]) -> Result<Vec<Option<Object>>> {
  read_objects(git_executor, repo_path, names, NO_LIMIT, |entry| match entry {
    BatchEntry::Object { id, kind, content } => Some(Object {
      id: id.to_string(),
      kind: kind.to_string(),
      content,
    }),
    BatchEntry::TooLarge { .. } | BatchEntry::Missing => None,
  })
}

/// What a new session starts from: the update check that conflicted, under the repository lock
#[derive(Debug, Clone)]
pub struct SessionStart<'a> {
  /// The check before an apply (`CheckPurpose::Apply`) that conflicted: the session replays what it read, so no caller
  /// can choose the commits the finish moves the branch to
  pub plan: &'a UpdatePlan,
  /// The worktree the session belongs to: the current one
  pub worktree: &'a Worktree,
  pub dir: Option<String>,
  pub dir_created: bool,
  /// A conflicting commit whose conflict of git resolves by words is resolved without a round of the agent
  pub resolve: ResolveMode,
}

/// Start the session of the current worktree for an update whose replay conflicts: the session replays the local commits
/// of the check's `ReplayPlan`, without the commits it left out, so it pairs nothing again. Its first round opens at the
/// first conflict the check found (`ReplayPlan::first_conflict`), after one replay per run of the commits before it,
/// and is stored in a new session ref. Nothing else moves. With `ResolveMode::On`, a conflicting
/// commit that its words resolve is a round that resolves itself (`advance`). `Unsupported` when the first conflict
/// can't be resolved in a round; then nothing is written but git objects
#[instrument(skip(git_executor, start), fields(branch = start.plan.checked.branch))]
pub fn start_session(git_executor: &GitCommandExecutor, repo_path: &str, start: SessionStart<'_>) -> Result<Step> {
  if !start.plan.is_applicable_conflict() {
    bail!("A resolve session starts only from the check of an update that conflicts");
  }
  let checked = &start.plan.checked;
  let excluded = start.plan.excluded();
  let replay = start.plan.replay_plan().ok_or_else(|| anyhow!("The check of a conflicting update has no replay plan"))?;
  sweep_stale_temp_refs(git_executor, repo_path, STALE_TEMP_REF_AGE);
  let committer = current_committer(git_executor, repo_path)?;
  let worktree = start.worktree;
  let mut state = SessionState {
    version: STATE_VERSION,
    worktree: worktree.id.clone(),
    git_dir: worktree.git_dir.clone(),
    branch: checked.branch.clone(),
    upstream: checked.upstream.clone(),
    local_upstream: checked.local_upstream,
    base: checked.base.clone(),
    upstream_head: checked.upstream_head.clone(),
    excluded: excluded.to_vec(),
    committer,
    dir: start.dir,
    dir_created: start.dir_created,
    started_head: checked.head.clone(),
    head: checked.head.clone(),
    local: replay.local.clone(),
    likely_landed: replay.likely_landed().to_vec(),
    landed_candidates: replay.pairing.candidates.clone(),
    replayed: 0,
    tip: checked.upstream_head.clone(),
    round: 0,
    status: SessionStatus::Resolving,
    conflict: None,
    resolutions: Vec::new(),
    notes_before: None,
    landed_dropped: replay.left_out().to_vec(),
    run_starts: replay.run_starts.clone(),
  };
  if !state.landed_dropped.is_empty() {
    info!(left_out = state.landed_dropped.len(), "local commits the upstream proves it has are left out");
  }
  // the round of the check's first conflict opens without a search
  let (objects, words_refs) = advance(git_executor, repo_path, &mut state, replay.first_conflict, start.resolve)?;
  let session_ref = session_ref(&worktree.id);
  let written = match write_state(git_executor, repo_path, &state, None) {
    Ok(written) => written,
    Err(e) => {
      discard(git_executor, repo_path, None, &words_refs);
      return Err(e);
    }
  };
  let mut transaction = RefTransaction::new();
  transaction.create(&session_ref, &written.ids[0]);
  written.delete_temp_refs(&mut transaction);
  for resolved_ref in &words_refs {
    resolved_ref.delete(&mut transaction);
  }
  if let Err(e) = transaction.commit(git_executor, repo_path, "resolve session") {
    discard(git_executor, repo_path, Some(&written), &words_refs);
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
  /// `--landed <commit>`: drop the round's commit as its upstream twin
  pub landed: Option<LandedInput>,
}

/// `--landed <commit> [--user]`: take `onto` for every file of the round and drop its commit as landed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandedInput {
  /// The round's commit, a prefix of its id of at least 4 characters
  pub commit: String,
  /// `--user`: the user decided the drop, so a commit with lines its twin lacks is dropped too
  pub user: bool,
}

/// Continue the session in `worktree`: check and commit the resolution of the open round, replay the rest (one
/// `replay`, a binary search only when it conflicts), and finish when every commit is replayed. A finish blocked by local
/// changes keeps the session ready; a later `--continue` only finishes. Peers' commits on top of the session's HEAD
/// become more local commits; a rewritten HEAD is `ResolveError::Stale`. A result without changes is
/// `ResolveError::EmptyResolution` unless the input decides it. With `resolve`, the later rounds that the words resolve
/// resolve themselves (`advance`). With `dry_run`, only the checks run and nothing but git objects is written
#[instrument(skip(git_executor, worktree, session, input))]
#[expect(clippy::too_many_arguments, reason = "the step of a session: what the caller read and decided")]
pub fn continue_session(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  worktree: &Worktree,
  session: Session,
  round: Option<u32>,
  input: &RoundInput,
  dry_run: bool,
  resolve: ResolveMode,
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

  let mut resolved_ref = None;
  let resolution = match state.conflict.clone() {
    Some(conflict) => {
      // `--landed`: `onto` for every file, then the decision on the commit
      let landed_takes;
      let round_input = if input.landed.is_some() {
        if !input.takes.is_empty() || !input.deletes.is_empty() || input.allow_empty || input.drop_empty.is_some() {
          return Err(invalid_input(
            "--landed takes `onto` for every file and drops the commit: pass it without --take, --delete, --allow-empty and --drop-empty",
          ));
        }
        landed_takes = RoundInput {
          takes: conflict.files.iter().map(|file| (Side::Onto, file.path.clone())).collect(),
          ..RoundInput::default()
        };
        &landed_takes
      } else {
        input
      };
      let (resolution, edits, objects) = check_round(git_executor, repo_path, &state, &conflict, round_input)?;
      let raw = &objects.commit;
      let landed = input
        .landed
        .as_ref()
        .map(|landed| landed_decision(git_executor, repo_path, &state, &conflict, raw, landed))
        .transpose()?;
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
      let resolved = if let Some(landed) = &landed {
        if !empty {
          let paths = changed_paths(git_executor, repo_path, &conflict.onto, &written.commit)
            .map_or_default(|changed| changed.into_iter().map(|path| String::from_utf8_lossy(&path.path).into_owned()).collect());
          discard(git_executor, repo_path, None, &[ResolvedRef::of(&written)]);
          return Err(anyhow::Error::new(ResolveError::LandedNotEmpty {
            round: state.round,
            commit: conflict.commit,
            subject: subject_of(raw),
            upstream: landed.upstream.clone(),
            paths,
          }));
        }
        None
      } else if empty {
        match empty_choice(&state, &conflict, raw, input) {
          Ok(keep) => keep.then(|| written.commit.clone()),
          Err(e) => {
            discard(git_executor, repo_path, None, &[ResolvedRef::of(&written)]);
            return Err(e);
          }
        }
      } else {
        Some(written.commit.clone())
      };
      let mut resolution = Resolution {
        resolved,
        empty,
        landed,
        ..resolution
      };
      if resolution.landed.is_some() {
        // a drop as landed reports what it lost as `local_only`, the lines of the twin round as a whole
        for path in &mut resolution.paths {
          path.dropped_commit_lines.clear();
        }
      }
      if dry_run {
        discard(git_executor, repo_path, None, &[ResolvedRef::of(&written)]);
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
      resolved_ref = Some(ResolvedRef::of(&written));
      Some(resolution)
    }
    None if !input.takes.is_empty() || !input.deletes.is_empty() || !input.edits.is_empty() || input.landed.is_some() => {
      return Err(invalid_input(
        "No round is open: every commit is replayed; run --continue without --take, --delete or --landed",
      ));
    }
    None => None,
  };
  if dry_run {
    return Ok(Step::Checked { session, resolution });
  }

  let mut resolved_refs: Vec<ResolvedRef> = resolved_ref.into_iter().collect();
  let objects = match advance(git_executor, repo_path, &mut state, None, resolve) {
    Ok((objects, words_refs)) => {
      resolved_refs.extend(words_refs);
      objects
    }
    // e.g. a later round that can't be resolved: nothing is saved, and every rerun writes the same commit again
    Err(e) => {
      discard(git_executor, repo_path, None, &resolved_refs);
      return Err(e);
    }
  };
  if state.status != SessionStatus::Resolving {
    let unsaved = state != session.state;
    return finish(git_executor, repo_path, &session.ref_name, &session.commit, state, unsaved, &resolved_refs);
  }
  let written = write_state(git_executor, repo_path, &state, Some(&session.commit))?;
  let mut transaction = RefTransaction::new();
  transaction.update(&session.ref_name, &written.ids[0], Some(&session.commit));
  written.delete_temp_refs(&mut transaction);
  for resolved_ref in &resolved_refs {
    resolved_ref.delete(&mut transaction);
  }
  if let Err(e) = transaction.commit(git_executor, repo_path, "resolve session") {
    discard(git_executor, repo_path, Some(&written), &resolved_refs);
    return Err(e);
  }
  let mut session = Session::new(session.ref_name, written.ids[0].clone(), state);
  session.objects = objects;
  Ok(Step::Round(session))
}

/// The `--landed` decision of the open round: it names the round's commit, which has a twin upstream
/// (`SessionState::candidate_of`), and a commit that changes lines its twin lacks (`interdiffs`) is dropped only with
/// `--user`
fn landed_decision(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  state: &SessionState,
  conflict: &RoundConflict,
  raw: &RawCommit,
  input: &LandedInput,
) -> Result<LandedResolution> {
  if input.commit.len() < 4 || !conflict.commit.starts_with(input.commit.as_str()) {
    return Err(invalid_input(&format!(
      "--landed {} does not name the commit of the round, {}",
      input.commit,
      short_id(&conflict.commit)
    )));
  }
  let Some(candidate) = state.candidate_of(&conflict.commit) else {
    return Err(anyhow::Error::new(ResolveError::NotLandedRound {
      round: state.round,
      commit: conflict.commit.clone(),
      subject: subject_of(raw),
    }));
  };
  let interdiff = interdiffs(git_executor, repo_path, &[(candidate.unit().collect(), candidate.upstream.as_str())])?
    .pop()
    .unwrap_or_default();
  if !interdiff.local_only.is_empty() && !input.user {
    return Err(anyhow::Error::new(ResolveError::LandedDiffers {
      round: state.round,
      commit: conflict.commit.clone(),
      subject: subject_of(raw),
      upstream: candidate.upstream.clone(),
    }));
  }
  Ok(LandedResolution {
    upstream: candidate.upstream.clone(),
    user: input.user,
    local_only: interdiff.local_only,
  })
}

/// What a resolution without changes does: `Ok(true)` keeps the empty commit (`--allow-empty`), `Ok(false)` drops it
/// (`--drop-empty` with the round's commit); without either, `EmptyResolution`
fn empty_choice(state: &SessionState, conflict: &RoundConflict, raw: &RawCommit, input: &RoundInput) -> Result<bool> {
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
      round: state.round,
      commit: conflict.commit.clone(),
      subject: subject_of(raw),
      twin: state.candidate_of(&conflict.commit).map(|candidate| candidate.upstream.clone()),
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

/// The temporary ref that keeps a resolved commit (`write_edited_commit`) reachable until a state commit does
#[derive(Debug)]
struct ResolvedRef {
  name: String,
  commit: String,
}

impl ResolvedRef {
  fn of(written: &WrittenEdit) -> Self {
    Self {
      name: written.temp_ref.clone(),
      commit: written.commit.clone(),
    }
  }

  /// Delete the ref in `transaction`, with compare-and-swap on the resolved commit
  fn delete(&self, transaction: &mut RefTransaction) {
    transaction.delete(&self.name, Some(&self.commit));
  }
}

/// Delete the temporary refs of a step that wrote no state, or whose state the session ref doesn't take: the state's
/// (`written`) and the resolved commits', in one transaction (`discard_temp_refs`); a failure is only logged
fn discard(git_executor: &GitCommandExecutor, repo_path: &str, written: Option<&WrittenStore>, resolved_refs: &[ResolvedRef]) {
  let Some(first) = written
    .map(|written| written.temp_ref.as_str())
    .or_else(|| resolved_refs.first().map(|resolved| resolved.name.as_str()))
  else {
    return;
  };
  discard_temp_refs(git_executor, repo_path, first, |transaction| {
    if let Some(written) = written {
      written.delete_temp_refs(transaction);
    }
    for resolved_ref in resolved_refs {
      resolved_ref.delete(transaction);
    }
  });
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

/// The branch HEAD is on now, after the guards of an update; `HEAD` when it is detached and the session started on a
/// detached HEAD (`SessionState::branch`). A session on a branch refuses a detached HEAD as `check_update_guards` does
fn head_of_session(git_executor: &GitCommandExecutor, repo_path: &str, state: &SessionState) -> Result<String> {
  match HeadRef::of(&state.branch) {
    HeadRef::Detached => Ok(HeadRef::read(git_executor, repo_path)?.name().to_string()),
    HeadRef::Branch(_) => check_update_guards(git_executor, repo_path),
  }
}

/// Follow the peers: HEAD (`head_seen` when the caller read it already) must still be on the session's branch and
/// contain the session's HEAD. Commits made on top of it are appended to the local commits (more rounds, usually
/// clean); a rewrite is `Stale`
fn follow_head(git_executor: &GitCommandExecutor, repo_path: &str, state: &mut SessionState, head_seen: Option<&str>) -> Result<()> {
  let branch = head_of_session(git_executor, repo_path, state)?;
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
  if !is_ancestor(git_executor, repo_path, &state.head, &head)? {
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
  // by name: a round can have thousands of files, and an agent may take or delete each of them
  let known: HashSet<&str> = conflict.files.iter().flat_map(ConflictFile::names).collect();
  if let Some(path) = input.takes.iter().map(|(_, path)| path).chain(&input.deletes).find(|path| !known.contains(path.as_str())) {
    return Err(invalid_input(&format!("{path} is not a conflicting file of round {}", state.round)));
  }
  let mut takes: HashMap<&str, Vec<Side>> = HashMap::new();
  for (side, path) in &input.takes {
    takes.entry(path).or_default().push(*side);
  }
  let deletes: HashSet<&str> = input.deletes.iter().map(String::as_str).collect();
  let objects = read_round_objects(git_executor, repo_path, conflict)?;
  let base_label = conflict.base_label();
  let labels = conflict.labels(&base_label);
  let blob = |stage: &Option<Stage>| stage.as_ref().and_then(|stage| objects.blobs.get(&stage.id)).map(Vec::as_slice);
  let mut refused: Vec<(RefusalKind, String)> = Vec::new();
  let mut paths = Vec::new();
  let mut edits = Vec::new();
  for file in &conflict.files {
    let sides: Vec<Side> = file.names().filter_map(|name| takes.get(name)).flatten().copied().collect();
    let delete = file.names().any(|name| deletes.contains(name));
    if !sides.is_empty() && delete || sides.len() > 1 {
      return Err(invalid_input(&format!("{} has more than one resolution", file.path)));
    }
    let take = sides.first().copied();
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
      conflicts: None,
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
    words: None,
    landed: None,
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
  let names: Vec<String> = std::iter::once(conflict.commit.clone())
    .chain(conflict.files.iter().map(|file| format!("{}:{}", conflict.merge_tree, file.path)))
    .chain(stages.iter().map(|id| (*id).to_string()))
    .collect();
  let mut objects = read_batch(git_executor, repo_path, &names)?.into_iter();
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
  git_executor
    .cmd(
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
    )
    .bytes()
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

/// Replay the local commits after `replayed` onto `tip`, one `replay` per run of consecutive commits; in the first run
/// that conflicts, a binary search over the prefixes finds the first conflicting commit, each probe from the longest
/// clean prefix so far. `first_conflict` is the index of the first conflict that the check found
/// (`ReplayPlan::first_conflict`): the commits before it replay, and its round opens without a search.
/// With `ResolveMode::On`, a commit whose conflict of git resolves by words is a round that resolves itself: it counts
/// in `round`, its resolution has `words`, and the replay goes on. The first other one opens its round. Every replay runs
/// with the session's committer, so the same input gives the same ids. Without a conflict the session is ready to
/// finish. Returns the objects of the round it opened, and the temporary refs of the commits that the words resolved,
/// which the caller deletes with the state write (on an error they are gone)
fn advance(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  state: &mut SessionState,
  first_conflict: Option<usize>,
  resolve: ResolveMode,
) -> Result<(Option<RoundObjects>, Vec<ResolvedRef>)> {
  let mut words_refs = Vec::new();
  match advance_resolving(git_executor, repo_path, state, first_conflict, resolve, &mut words_refs) {
    Ok(objects) => Ok((objects, words_refs)),
    Err(e) => {
      discard(git_executor, repo_path, None, &words_refs);
      Err(e)
    }
  }
}

fn advance_resolving(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  state: &mut SessionState,
  mut first_conflict: Option<usize>,
  resolve: ResolveMode,
  words_refs: &mut Vec<ResolvedRef>,
) -> Result<Option<RoundObjects>> {
  let env = state.committer_env()?;
  let env: Vec<(&str, &str)> = env.iter().map(|(name, value)| (*name, value.as_str())).collect();
  let total = state.local.len();
  loop {
    if state.replayed == total {
      state.status = SessionStatus::Ready;
      return Ok(None);
    }
    // up to the conflict the check found; a commit before it that the words resolved conflicts on the way
    let end = first_conflict.filter(|&index| index >= state.replayed).unwrap_or(total);
    let replay = replay_of(git_executor, repo_path, &state.local, &env);
    let (low, tip) = match first_conflict_in_runs(&state.run_starts, state.replayed, end, state.tip.clone(), false, replay)? {
      RunsReplay::Conflict { index, tip } => (index, tip),
      RunsReplay::Clean(tip) if end < total => {
        first_conflict = None;
        (end, tip)
      }
      RunsReplay::Clean(tip) => {
        state.tip = tip;
        state.replayed = total;
        continue;
      }
    };
    state.tip = tip;
    state.replayed = low;
    state.round += 1;
    let merge = round_merge(git_executor, repo_path, state)?;
    if resolve == ResolveMode::On
      && let Some(words) = resolve_merge_onto(git_executor, repo_path, &state.local[low], &state.tip, &state.committer, &merge)?
    {
      let written = &words.written;
      let empty = written.tree == written.parent_tree;
      let resolution = Resolution {
        round: state.round,
        commit: state.local[low].clone(),
        resolved: (!empty).then(|| written.commit.clone()),
        empty,
        paths: words
          .resolution
          .files
          .iter()
          .map(|file| ResolvedPath {
            path: file.path.clone(),
            outcome: Outcome::Words,
            dropped_upstream_lines: Vec::new(),
            dropped_commit_lines: Vec::new(),
            conflicts: Some(file.conflicts),
          })
          .collect(),
        words: Some(words.resolution.conflicts()),
        landed: None,
      };
      if !empty {
        state.tip.clone_from(&written.commit);
      }
      words_refs.push(ResolvedRef::of(written));
      debug!(round = state.round, commit = %short_id(&state.local[low]), "a round resolves by words");
      state.resolutions.push(resolution);
      state.replayed += 1;
      continue;
    }
    state.status = SessionStatus::Resolving;
    let (conflict, objects) = open_round(git_executor, repo_path, state, merge)?;
    state.conflict = Some(conflict);
    debug!(round = state.round, commit = %short_id(&state.local[low]), "a round opens");
    return Ok(Some(objects));
  }
}

/// The merge of the local commit at `state.replayed` onto `state.tip`: one `merge-tree --write-tree -z` with zdiff3
/// markers
fn round_merge(git_executor: &GitCommandExecutor, repo_path: &str, state: &SessionState) -> Result<MergeTreeOutput> {
  let commit = &state.local[state.replayed];
  let parent = format!("{commit}^");
  let output = git_executor
    .cmd(
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
    )
    .accept(&[1])
    .output()?;
  let merge = parse_merge_tree_output(&output.stdout)?;
  if output.code == 0 || merge.entries.is_empty() {
    bail!("{} replays with a conflict, but merge-tree merges it cleanly", short_id(commit));
  }
  Ok(merge)
}

/// The round of the local commit at `state.replayed` onto `state.tip` from its merge (`round_merge`): the supported
/// conflict types only, and the objects of the round. A text conflict whose merge result has no block of bd's labels is
/// refused too: its markers can't be checked
fn open_round(git_executor: &GitCommandExecutor, repo_path: &str, state: &SessionState, merge: MergeTreeOutput) -> Result<(RoundConflict, RoundObjects)> {
  let commit = &state.local[state.replayed];
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
  // `<new path>` -> `<old path>` of each rename/delete, the first message of a path
  let mut renamed: HashMap<&[u8], &[u8]> = HashMap::new();
  for message in merge.messages.iter().filter(|message| message.kind == "CONFLICT (rename/delete)") {
    if let (Some(new), Some(old)) = (message.paths.first(), message.paths.get(1)) {
      renamed.entry(new.as_slice()).or_insert(old.as_slice());
    }
  }
  // the paths of the messages of an unsupported type: a monorepo update can conflict in thousands of paths
  let mut refused: HashSet<&[u8]> = HashSet::new();
  for message in merge
    .messages
    .iter()
    .filter(|message| message.kind.starts_with("CONFLICT") && !supported_kind(&message.kind))
  {
    for path in &message.paths {
      refused.insert(path);
      unsupported.push((String::from_utf8_lossy(path).into_owned(), message.kind.clone()));
    }
  }
  let mut files = Vec::new();
  // the stages of a path are consecutive, in git's order
  for stages in merge.entries.chunk_by(|a, b| a.path == b.path) {
    let path = stages[0].path.as_slice();
    if refused.contains(path) {
      continue;
    }
    let Some(path) = std::str::from_utf8(path).ok().filter(|path| is_valid_tree_path(path)) else {
      unsupported.push((String::from_utf8_lossy(path).into_owned(), "an invalid path".to_string()));
      continue;
    };
    let stage = |number: u8| {
      stages.iter().find(|entry| entry.stage == number).map(|entry| Stage {
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
    let renamed_from = renamed.get(path.as_bytes()).and_then(|old| std::str::from_utf8(old).ok()).map(str::to_string);
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
/// no name Windows can't hold: no `\`, no drive prefix, no device name (`CON`), none of `<>:"|?*` or a control
/// character, no trailing dot or space. The copies use only the last component, but the path also goes into the state
/// tree and the output
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
  if let Some(conflicts) = resolution.words {
    let _ = writeln!(note, "resolved-by-words: {RESOLVE_VERSION} {conflicts}");
  }
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
/// `unsaved`. `resolved_refs` (of the resolved commit of the last round) are deleted with the first transaction
fn finish(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  ref_name: &str,
  base_commit: &str,
  mut state: SessionState,
  unsaved: bool,
  resolved_refs: &[ResolvedRef],
) -> Result<Step> {
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
      for resolved_ref in resolved_refs {
        resolved_ref.delete(&mut transaction);
      }
      if let Err(e) = transaction.commit(git_executor, repo_path, "resolve session") {
        discard(git_executor, repo_path, Some(&written), resolved_refs);
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
  let notes_before = resolve_commit(git_executor, repo_path, RESOLVE_NOTES_REF)?;
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
  for resolved_ref in resolved_refs {
    resolved_ref.delete(&mut transaction);
  }
  if let Err(e) = transaction.commit(git_executor, repo_path, "resolve session applying") {
    discard(git_executor, repo_path, Some(&written), resolved_refs);
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
      head_ref: &HeadRef::of(&state.branch),
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
  let branch = head_of_session(git_executor, repo_path, state)?;
  let actual = git_executor.execute_command(&["rev-parse", "HEAD^{commit}"], repo_path)?;
  let changed = changed_paths(git_executor, repo_path, &state.head, &state.tip)?;
  let followed = changed.is_empty() || !index_left_behind(git_executor, repo_path, &changed)?;
  let contains_tip = actual == state.tip || is_ancestor(git_executor, repo_path, &state.tip, &actual)?;
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
      if let Some(notes_now) = resolve_commit(git_executor, repo_path, RESOLVE_NOTES_REF)?
        && state.notes_before.as_deref() != Some(notes_now.as_str())
      {
        match &state.notes_before {
          Some(before) => rollback_with.update(RESOLVE_NOTES_REF, before, Some(&notes_now)),
          None => rollback_with.delete(RESOLVE_NOTES_REF, Some(&notes_now)),
        };
      }
      files_follow(
        git_executor,
        repo_path,
        &HeadRef::of(&state.branch),
        &state.head,
        &state.tip,
        &changed,
        UPDATE_OPERATION,
        &rollback_with,
      )?;
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
