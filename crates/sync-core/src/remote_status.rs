//! The remote status of a virtual branch: the commits to push and those a push would drop, by content, so the baseline,
//! rebased copies and older versions of commits the branch or the baseline has don't count
//! (docs/design/remote-status.md). A branch whose commits did not move since its last
//! status runs no walk: the status of the same commits is remembered by the process and in the object cache, and the
//! push times of all branches of an operation are one `reflog`.

use crate::commit_key::CommitKey;
use crate::commit_walk::{WalkedCommit, walk_commits, walk_commits_by_authors, walk_commits_with_messages};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::is_object_id;
use object_cache::cache::ObjectCache;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::sync::{LazyLock, Mutex, OnceLock};
use sync_types::{DroppedCommit, LandedRemoteCommit, RemoteStatusUpdate};
use sync_utils::list_cut::kept;
use tracing::{debug, instrument};

/// At most this many dropped commits are listed (`RemoteStatusUpdate::dropped_commits`), and as many landed ones
/// (`landed_commits`), all of them when only one more (`list_cut::kept`); the counts cover all of them
pub const DROPPED_COMMITS_LIMIT: usize = 10;

/// The entries of a remote-tracking ref's reflog that git's push wrote: `update by push`, a literal git never translates
const PUSH_ENTRY: &str = "--grep-reflog=^update by push$";

/// `<ref>@{<time>}`, the selector of a reflog entry with `--date=unix`, as `(ref, time)`, cut at the last `@{`: a ref
/// name may hold a `{`, never an `@{`
fn parse_selector(selector: &str) -> Option<(&str, u32)> {
  let (name, time) = selector.rsplit_once("@{")?;
  Some((name, time.strip_suffix('}')?.parse().ok()?))
}

/// The time of the newest push of the remote-tracking ref `remote_ref` from this clone, 0 without one (no reflog, or
/// only fetches moved it); `-1` stops the walk of the reflog at the newest push entry
fn read_push_time(git_executor: &GitCommandExecutor, repository_path: &str, remote_ref: &str) -> u32 {
  git_executor
    .execute_command(&["reflog", "show", "-1", PUSH_ENTRY, "--date=unix", "--format=%gd", remote_ref], repository_path)
    .ok()
    .and_then(|selector| parse_selector(&selector).map(|(_, time)| time))
    .unwrap_or(0)
}

/// The newest push time of each of `remote_refs` that has one, from one `reflog` of all of them (`--stdin`, so their
/// number is not bounded by the command line). `%gD` names the full ref; the walk can't stop at the newest push of each
/// ref (`-1` counts the entries of all refs), so it reads them all and keeps the newest. `None` when git fails, as for
/// a ref deleted since its tip was read: one missing ref fails the whole read
fn read_push_times(git_executor: &GitCommandExecutor, repository_path: &str, remote_refs: &[String]) -> Option<HashMap<String, u32>> {
  if remote_refs.is_empty() {
    return Some(HashMap::new());
  }
  let input = remote_refs.iter().fold(String::new(), |mut input, remote_ref| {
    let _ = writeln!(input, "{remote_ref}");
    input
  });
  let selectors = git_executor
    .cmd(&["reflog", "show", "--stdin", PUSH_ENTRY, "--date=unix", "--format=%gD"], repository_path)
    .stdin(input.as_bytes())
    .lines()
    .inspect_err(|e| debug!(error = %e, "cannot read the push times at once"))
    .ok()?;
  let mut times: HashMap<String, u32> = HashMap::new();
  for (name, time) in selectors.iter().filter_map(|selector| parse_selector(selector)) {
    let newest = times.entry(name.to_string()).or_default();
    *newest = (*newest).max(time);
  }
  Some(times)
}

/// The push times of the remote branches of one operation, read by the first status that needs one, for all of them:
/// one `reflog` per operation instead of one per branch. When that read fails, each status reads its own, as many
/// processes as without the batch plus the failed one
pub struct PushTimes {
  remote_refs: Vec<String>,
  times: OnceLock<Option<HashMap<String, u32>>>,
}

impl PushTimes {
  /// The push times of the remote-tracking refs `remote_refs` (`refs/remotes/<remote>/<name>`)
  pub const fn new(remote_refs: Vec<String>) -> Self {
    Self {
      remote_refs,
      times: OnceLock::new(),
    }
  }

  /// The time of the newest push of `remote_ref` from this clone, 0 without one
  pub fn of(&self, git_executor: &GitCommandExecutor, repository_path: &str, remote_ref: &str) -> u32 {
    if !self.remote_refs.iter().any(|known| known == remote_ref) {
      return read_push_time(git_executor, repository_path, remote_ref);
    }
    match self.times.get_or_init(|| read_push_times(git_executor, repository_path, &self.remote_refs)) {
      Some(times) => times.get(remote_ref).copied().unwrap_or(0),
      None => read_push_time(git_executor, repository_path, remote_ref),
    }
  }
}

/// The branch a stacked branch is built on, for the remote status of the stacked branch
#[derive(Clone, Copy)]
pub struct StackParent<'a> {
  /// What the stacked branch is built on: the parent's new head the sync built it on, or outside the sync the parent's
  /// virtual branch (`{prefix}/virtual/{name}`). The commits up to it are the parent's, not unpushed commits of the
  /// stacked branch
  pub base: &'a str,
  /// The commits of the parent and of the branches it is built on: a remote commit with the identity of one of them is
  /// restacked, an older version of an ancestor's commit
  pub ancestors: AncestorCommits<'a>,
}

/// The identities (`CommitKey`) of the commits of a stacked branch's ancestors
#[derive(Clone, Copy)]
pub enum AncestorCommits<'a> {
  /// Known to the caller: the sync has the commits of every branch
  Known(&'a HashSet<CommitKey>),
  /// Read from the commits up to `StackParent::base` with one `rev-list`, only when the remote branch lacks a commit of
  /// the branch or of its ancestors by content, or has one the branch lacks
  ReadWhenNeeded,
}

/// The commits of a walk, oldest first, how many of them the user authored, and the commit of each identity
#[derive(Default)]
struct WalkedCommits {
  commits: Vec<String>,
  mine: u32,
  keys: HashMap<CommitKey, String>,
  /// The commits the remote has by content (`=` of `--cherry-mark`), as their versions: not unpushed, but a push
  /// replaces the remote's copy when it is another version (`RemoteCommits::versions`)
  copies: Vec<CommitVersion>,
}

/// What a copy of a commit keeps and a reword or a new author changes: the author, the author time and the raw message.
/// A rebased copy has the same; one with the same changes but another version is not on the remote as it is
#[derive(PartialEq, Eq, Hash)]
struct CommitVersion {
  author: String,
  email: String,
  time: u32,
  message: String,
}

impl CommitVersion {
  fn new(commit: &WalkedCommit, message: String) -> Self {
    Self {
      author: commit.author.clone(),
      email: commit.key.email.clone(),
      time: commit.key.time,
      message,
    }
  }
}

/// The commits of a branch without a remote branch, all of them unpushed
fn walk(git_executor: &GitCommandExecutor, repository_path: &str, revs: &[&str], my_email: Option<&str>) -> Result<WalkedCommits> {
  let commits = walk_commits(git_executor, repository_path, &["--reverse"], revs)?;
  Ok(walked_commits(commits.into_iter().map(|commit| (commit, None)), my_email))
}

fn walked_commits(commits: impl Iterator<Item = (WalkedCommit, Option<String>)>, my_email: Option<&str>) -> WalkedCommits {
  let mut walked = WalkedCommits::default();
  for (commit, message) in commits {
    if commit.mark == '=' {
      walked.copies.extend(message.map(|message| CommitVersion::new(&commit, message)));
      continue;
    }
    if my_email.is_some_and(|my_email| commit.key.email.eq_ignore_ascii_case(my_email)) {
      walked.mine += 1;
    }
    walked.keys.insert(commit.key, commit.id.clone());
    walked.commits.push(commit.id);
  }
  walked
}

/// A remote commit whose content the branch lacks
struct BehindCommit {
  commit: DroppedCommit,
  key: CommitKey,
}

/// The remote side of `<remote>...<local>`: the commits the remote branch has and the branch doesn't
struct RemoteCommits {
  /// The ones whose content the branch lacks, newest first (a child before its parent)
  behind: Vec<BehindCommit>,
  /// The versions of the ones the branch has by content (`=`): a copy of the branch with none of them is reworded
  versions: HashSet<CommitVersion>,
  /// The remote tip is the branch's tip or an ancestor of it: the remote side is empty, without even a
  /// patch-equivalent copy of a local commit. A rebased branch is no fast-forward even when nothing is behind: the push
  /// replaces the remote commits with their rebased copies
  fast_forward: bool,
}

/// One `rev-list --cherry-mark --left-only` of `range` (`<remote>...<local>`). `--cherry-mark` lists a remote commit
/// patch-equivalent to a local one too, marked `=` (`%m`), so the same walk that finds the commits behind tells the
/// fast-forward: no remote commit at all, and the version of each copy the branch has by content
fn walk_remote(git_executor: &GitCommandExecutor, repository_path: &str, range: &str) -> Result<RemoteCommits> {
  let commits = walk_commits_with_messages(git_executor, repository_path, &["--cherry-mark", "--left-only", "--date-order"], &[range])?;
  let fast_forward = commits.is_empty();
  let mut versions = HashSet::new();
  let mut behind = Vec::new();
  for (commit, message) in commits {
    // `=`: the branch has the commit by content
    if commit.mark == '=' {
      versions.insert(CommitVersion::new(&commit, message));
      continue;
    }
    behind.push(BehindCommit {
      commit: DroppedCommit {
        id: commit.id,
        subject: commit.subject,
        author: commit.author,
        author_time: commit.key.time,
      },
      key: commit.key,
    });
  }
  Ok(RemoteCommits { behind, versions, fast_forward })
}

/// The identities of the commits up to `base` above the baseline: the parent's and those of the branches it is built on,
/// which its virtual branch contains
fn read_ancestor_keys(git_executor: &GitCommandExecutor, repository_path: &str, baseline_exclusion: &str, base: &str) -> Result<HashSet<CommitKey>> {
  Ok(
    walk_commits(git_executor, repository_path, &[], &[baseline_exclusion, base])?
      .into_iter()
      .map(|commit| commit.key)
      .collect(),
  )
}

/// The identities of the commits of a stacked branch's ancestors: the ones the caller knows, else read now
fn ancestor_keys<'a>(git_executor: &GitCommandExecutor, repository_path: &str, baseline_exclusion: &str, parent: StackParent<'a>) -> Result<Cow<'a, HashSet<CommitKey>>> {
  Ok(match parent.ancestors {
    AncestorCommits::Known(keys) => Cow::Borrowed(keys),
    AncestorCommits::ReadWhenNeeded => Cow::Owned(read_ancestor_keys(git_executor, repository_path, baseline_exclusion, parent.base)?),
  })
}

/// The remote's copies of commits of the stack ancestors in another version than the ancestors have now: a commit of the
/// parent reworded since the child's push, which the remote has by content (`=`), so it is not behind. The push of the
/// child replaces them, as it replaces an older version of an ancestor's commit (restacked). `ancestor_copies` are the
/// ancestors' commits the remote has by content, from the walk of the branch; a copy counts when an ancestor's commit
/// has its author and author time
fn reworded_ancestor_copies(remote_versions: &HashSet<CommitVersion>, own: &[CommitVersion], ancestor_copies: &[CommitVersion]) -> u32 {
  let own: HashSet<&CommitVersion> = own.iter().collect();
  let identities: HashSet<(&str, u32)> = ancestor_copies.iter().map(|copy| (copy.email.as_str(), copy.time)).collect();
  let versions: HashSet<&CommitVersion> = ancestor_copies.iter().collect();
  count(
    remote_versions
      .iter()
      .filter(|version| !own.contains(version) && identities.contains(&(version.email.as_str(), version.time)) && !versions.contains(version))
      .count(),
  )
}

/// The identities of a stacked branch's ancestors' commits, read at most once per status and only when one is needed
struct LazyAncestorKeys<'a, 'p> {
  git_executor: &'a GitCommandExecutor,
  repository_path: &'a str,
  baseline_exclusion: &'a str,
  parent: Option<StackParent<'p>>,
  keys: std::cell::OnceCell<Cow<'p, HashSet<CommitKey>>>,
}

impl LazyAncestorKeys<'_, '_> {
  /// None for a branch built on the baseline
  fn get(&self) -> Result<Option<&HashSet<CommitKey>>> {
    let Some(parent) = self.parent else { return Ok(None) };
    if let Some(keys) = self.keys.get() {
      return Ok(Some(keys));
    }
    let keys = ancestor_keys(self.git_executor, self.repository_path, self.baseline_exclusion, parent)?;
    Ok(Some(self.keys.get_or_init(|| keys)))
  }
}

/// The commits behind, sorted by what a push does with them
#[derive(Default)]
struct ClassifiedBehind {
  /// Older versions of commits of the branch, with other changes
  amended: u32,
  /// Older versions of commits of its stack ancestors
  restacked: u32,
  /// Older versions of commits of the baseline, newest first, each with the baseline commit of its identity
  landed: Vec<(BehindCommit, String)>,
  /// The rest, newest first: a push drops them
  dropped: Vec<BehindCommit>,
}

/// Sorts the commits behind. One with the identity of an unpushed commit of the branch is an older version of it (the
/// new version has other content, so it is unpushed): amended when its changes differ, a rebased copy, counted nowhere,
/// when only their context or position does (the commit was rebuilt on a changed parent or baseline). One with the
/// identity of a commit of an ancestor is restacked; one with the identity of a commit the baseline has above the remote
/// branch landed (`baseline_commits_of`); the rest is the remote's own work, dropped.
fn classify_behind(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  baseline: &str,
  remote_tip: &str,
  behind: Vec<BehindCommit>,
  own: &HashMap<CommitKey, String>,
  ancestors: &LazyAncestorKeys<'_, '_>,
) -> Result<ClassifiedBehind> {
  let (versions, mut dropped): (Vec<BehindCommit>, Vec<BehindCommit>) = behind.into_iter().partition(|commit| own.contains_key(&commit.key));
  let pairs: Vec<(&str, &str)> = versions.iter().map(|commit| (commit.commit.id.as_str(), own[&commit.key].as_str())).collect();
  let rebased = same_changes(git_executor, repository_path, &pairs)?.into_iter().filter(|same| *same).count();
  let amended = count(versions.len() - rebased);

  let restacked = if !dropped.is_empty()
    && let Some(ancestors) = ancestors.get()?
  {
    let before = dropped.len();
    dropped.retain(|commit| !ancestors.contains(&commit.key));
    count(before - dropped.len())
  } else {
    0
  };

  let landed = if dropped.is_empty() {
    Vec::new()
  } else {
    let in_baseline = baseline_commits_of(git_executor, repository_path, baseline, remote_tip, &dropped)?;
    let (landed, rest): (Vec<BehindCommit>, Vec<BehindCommit>) = dropped.into_iter().partition(|commit| in_baseline.contains_key(&commit.key));
    dropped = rest;
    landed
      .into_iter()
      .map(|commit| {
        let baseline_commit = in_baseline[&commit.key].clone();
        (commit, baseline_commit)
      })
      .collect()
  };
  Ok(ClassifiedBehind {
    amended,
    restacked,
    landed,
    dropped,
  })
}

/// The commits of the baseline above the remote branch (`<baseline> ^<remote_tip>`) by their identity, of the authors of
/// `commits` only: one `rev-list`, as Update Project finds the landed commits (`landed::read_upstream`). A remote commit
/// with the identity of one is an older version of a commit that landed, by a Safe Push rebuilt on a newer target or by
/// a cherry-pick after a change below it, so its patch is not the baseline's and `--cherry-mark` misses it
fn baseline_commits_of(git_executor: &GitCommandExecutor, repository_path: &str, baseline: &str, remote_tip: &str, commits: &[BehindCommit]) -> Result<HashMap<CommitKey, String>> {
  let remote_exclusion = format!("^{remote_tip}");
  let emails = commits.iter().map(|commit| commit.key.email.as_str());
  let mut by_key = HashMap::new();
  for commit in walk_commits_by_authors(git_executor, repository_path, emails, &[baseline, remote_exclusion.as_str()])? {
    // newest first: the newest commit of an identity names it
    by_key.entry(commit.key).or_insert(commit.id);
  }
  Ok(by_key)
}

/// Which of the `(older, newer)` pairs make the same changes: the patch ids of their diffs without context (`-U0`; a
/// patch id ignores the line numbers and whitespace) are equal. One `diff-tree --stdin` and one `patch-id`, none
/// without pairs
pub(crate) fn same_changes(git_executor: &GitCommandExecutor, repository_path: &str, pairs: &[(&str, &str)]) -> Result<Vec<bool>> {
  if pairs.is_empty() {
    return Ok(Vec::new());
  }
  let input = pairs.iter().fold(String::new(), |mut input, (older, newer)| {
    let _ = writeln!(input, "{older}\n{newer}");
    input
  });
  let patches = git_executor
    .cmd(&["diff-tree", "--stdin", "-p", "-U0", "--no-renames", "--no-ext-diff", "--no-textconv"], repository_path)
    .stdin(input.as_bytes())
    .bytes()?;
  let ids = git_executor.cmd(&["patch-id", "--stable"], repository_path).stdin(&patches).bytes()?;
  // `<patch id> <commit>` per commit with changes
  let ids = String::from_utf8_lossy(&ids);
  let patch_of: HashMap<&str, &str> = ids.lines().filter_map(|line| line.split_once(' ').map(|(patch, commit)| (commit, patch))).collect();
  Ok(
    pairs
      .iter()
      .map(|(older, newer)| patch_of.get(older).is_some_and(|patch| patch_of.get(newer) == Some(patch)))
      .collect(),
  )
}

fn count(n: usize) -> u32 {
  u32::try_from(n).unwrap_or(u32::MAX)
}

/// Compute remote status for a single local virtual branch against its remote-tracking branch of `remote` (the sync and
/// the app compare with `origin`, `bd push --remote` with its remote).
/// `local_ref` must be in the form "{prefix}/virtual/{name}" (no refs/heads/ prefix). `parent` is the branch a stacked
/// branch is built on: its commits are the parent's, not unpushed commits of this branch; those the remote branch lacks
/// are `ancestor_commits_ahead`, since a Safe Push of the remote branch lands the whole piece.
///
/// The sync rebuilds a virtual branch on the latest baseline, so its hashes change while its work stays the same.
/// The status compares content, not hashes: the baseline commits are on the remote already, and a rebased copy of a
/// pushed commit is patch-equivalent to it. Only the commits whose content the remote lacks are unpushed.
///
/// A remote commit whose content the branch lacks is an older version of a commit the branch has when their identities
/// (`CommitKey`) match: amended when the commit is the branch's and its changes differ (a rebased copy when only their
/// context moved), restacked when it is a stack ancestor's (the child's remote branch has the parent's commits as they
/// were), landed when the baseline has a commit with its identity above the remote branch (a Safe Push landed it, also
/// with other changes). A push replaces those; only the rest is dropped work.
#[instrument(
  skip(git_executor, parent),
  fields(
    remote = %remote,
    branch_name = %branch_name,
    local_ref = %local_ref,
    parent = parent.map(|parent| parent.base)
  )
)]
#[expect(clippy::too_many_arguments, reason = "the refs and names of one branch status")]
pub fn compute_remote_status_for_branch(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  remote: &str,
  local_ref: &str,
  branch_name: &str,
  my_email: Option<&str>,
  baseline_branch: &str,
  parent: Option<StackParent<'_>>,
) -> Result<RemoteStatusUpdate> {
  let branch = StatusBranch { local_ref, branch_name, parent };
  let mut statuses = compute_remote_statuses(git_executor, repository_path, remote, &[branch], my_email, baseline_branch)?;
  Ok(statuses.remove(0))
}

/// A branch of `compute_remote_statuses`: its ref (`{prefix}/virtual/{name}`), its name, and the branch it is built on
#[derive(Clone, Copy)]
pub struct StatusBranch<'a> {
  pub local_ref: &'a str,
  pub branch_name: &'a str,
  pub parent: Option<StackParent<'a>>,
}

/// `compute_remote_status_for_branch` of the branches of one operation (a push of a stack, its rejection), in their
/// order: one `for-each-ref` reads the local and the remote tips of all of them, and one `reflog` the push times of
/// several (`PushTimes`). The walks run per branch, none for a branch whose remote branch is at its tip, as after its push
#[instrument(skip(git_executor, branches), fields(remote = %remote, branches = branches.len()))]
pub fn compute_remote_statuses(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  remote: &str,
  branches: &[StatusBranch<'_>],
  my_email: Option<&str>,
  baseline_branch: &str,
) -> Result<Vec<RemoteStatusUpdate>> {
  // one process for the tips of all: whether each remote branch exists, its commit and the branch's. The walks below
  // use these commits, not the ref names, so a fetch or a sync meanwhile can't make the counts disagree with
  // `remote_tip`. A ref name holds no glob character, so each pattern matches its ref only
  let remote_refs: Vec<String> = branches.iter().map(|branch| format!("refs/remotes/{remote}/{}", branch.local_ref)).collect();
  let branch_refs: Vec<String> = branches.iter().map(|branch| format!("refs/heads/{}", branch.local_ref)).collect();
  let mut args = vec!["for-each-ref", "--format=%(objectname) %(refname)"];
  args.extend(remote_refs.iter().chain(&branch_refs).map(String::as_str));
  let tips = git_executor.execute_command_lines(&args, repository_path)?;
  let tip_of = |name: &str| tips.iter().find_map(|line| line.split_once(' ').filter(|(_, refname)| *refname == name).map(|(id, _)| id));
  // the push times of several branches are one `reflog`; of one, the newest entry of its own
  let push_times = (branches.len() > 1).then(|| PushTimes::new(remote_refs.clone()));
  branches
    .iter()
    .zip(remote_refs.iter().zip(&branch_refs))
    .map(|(branch, (remote_ref, branch_ref))| {
      let commits = BranchCommits {
        // without the branch the walks name it, and fail
        local: tip_of(branch_ref).unwrap_or(branch.local_ref),
        baseline: baseline_branch,
        remote_tip: tip_of(remote_ref),
      };
      let status = compute(git_executor, repository_path, &commits, my_email, branch.parent)?;
      let last_push_time = match (&status.remote_tip, &push_times) {
        (None, _) => 0,
        (Some(_), Some(push_times)) => push_times.of(git_executor, repository_path, remote_ref),
        (Some(_), None) => read_push_time(git_executor, repository_path, remote_ref),
      };
      Ok(status.into_update(branch.branch_name, last_push_time))
    })
    .collect()
}

/// What a status is a function of: the commits and the email. A status computed for them holds as long as they are the
/// same; its push time too, but for a push of the same commit again after the remote branch was elsewhere (push X,
/// fetch Y, push X), which the process then shows with the time of the first push
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StatusKey {
  repository_path: String,
  remote: String,
  local: String,
  remote_tip: Option<String>,
  baseline: String,
  parent_base: Option<String>,
  my_email: Option<String>,
}

/// Statuses of this process, push times included; an unchanged branch is answered without git. Cleared when full
static STATUS_CACHE: LazyLock<Mutex<HashMap<StatusKey, RemoteStatusUpdate>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
const STATUS_CACHE_LIMIT: usize = 4096;

/// The version of `CachedStatus` in the object cache, the most dropped commits it lists in the low half (one more than
/// the limit, `list_cut::kept`). Bump the high half when the status of the same commits changes: its fields, the options
/// of its walks, `CommitKey`, `same_changes`
fn cached_status_version() -> u32 {
  (4 << 16) | count(DROPPED_COMMITS_LIMIT + 1)
}

/// A dropped commit in the object cache (`DroppedCommit`)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedDroppedCommit {
  id: String,
  subject: String,
  author: String,
  author_time: u32,
}

/// A landed commit in the object cache (`LandedRemoteCommit`)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedLandedCommit {
  id: String,
  subject: String,
  baseline_commit: String,
}

/// The status of a branch without its name and its push time: what the walks of its commits find, the value of the
/// object cache (`object_cache::remote_status`)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedStatus {
  /// `None` when the remote branch does not exist
  remote_tip: Option<String>,
  unpushed_commits: Vec<String>,
  my_unpushed_count: u32,
  commits_behind: u32,
  dropped_commits: Vec<CachedDroppedCommit>,
  commits_amended: u32,
  commits_restacked: u32,
  commits_landed: u32,
  landed_commits: Vec<CachedLandedCommit>,
  commits_reworded: u32,
  ancestor_commits_ahead: u32,
  fast_forward: bool,
}

impl CachedStatus {
  fn into_update(self, branch_name: &str, last_push_time: u32) -> RemoteStatusUpdate {
    RemoteStatusUpdate {
      branch_name: branch_name.to_string(),
      remote_exists: self.remote_tip.is_some(),
      remote_tip: self.remote_tip,
      unpushed_commits: self.unpushed_commits,
      commits_behind: self.commits_behind,
      dropped_commits: self
        .dropped_commits
        .into_iter()
        .map(|commit| DroppedCommit {
          id: commit.id,
          subject: commit.subject,
          author: commit.author,
          author_time: commit.author_time,
        })
        .collect(),
      commits_amended: self.commits_amended,
      commits_restacked: self.commits_restacked,
      commits_landed: self.commits_landed,
      landed_commits: self
        .landed_commits
        .into_iter()
        .map(|commit| LandedRemoteCommit {
          id: commit.id,
          subject: commit.subject,
          baseline_commit: commit.baseline_commit,
        })
        .collect(),
      commits_reworded: self.commits_reworded,
      ancestor_commits_ahead: self.ancestor_commits_ahead,
      my_unpushed_count: self.my_unpushed_count,
      fast_forward: self.fast_forward,
      last_push_time,
    }
  }
}

/// The key of a status in the object cache: a hash of the commits (full ids only: a ref name moves) and of the email,
/// and of where the commits of a stacked branch's ancestors come from (the sync knows them, a push outside it reads
/// them), so the two never share an entry. `None` for a ref name among the commits: such a status is only remembered by
/// the process
fn cached_status_key(branch: &BranchCommits<'_>, my_email: Option<&str>, parent: Option<StackParent<'_>>) -> Option<[u8; 32]> {
  let commits = [Some(branch.local), Some(branch.baseline), branch.remote_tip, parent.map(|parent| parent.base)];
  if !commits.iter().flatten().all(|id| is_object_id(id)) {
    return None;
  }
  let mut hasher = Sha256::new();
  let mut field = |value: Option<&str>| {
    // a marker, so a missing value differs from an empty one
    match value {
      Some(value) => {
        hasher.update([1]);
        hasher.update(value.as_bytes());
      }
      None => hasher.update([0]),
    }
    hasher.update([0]);
  };
  field(Some("remote_status"));
  for commit in commits {
    field(commit);
  }
  field(parent.map(|parent| match parent.ancestors {
    AncestorCommits::Known(_) => "known",
    AncestorCommits::ReadWhenNeeded => "read",
  }));
  field(my_email);
  Some(hasher.finalize().into())
}

/// The remote statuses of one operation (a sync, `bd push --remote`) against `remote`, on the commits it read once: the
/// remote tips of its branches and the baseline. The status of the same commits is answered without git from the
/// memory of the process, else from the object cache; a computed one goes to both, and the object cache gets the new
/// ones in one write when the memo is dropped (one transaction, not one per branch). The push times of all branches are
/// one `reflog` (`PushTimes`), read only when a status needs one
pub struct RemoteStatusMemo {
  git_executor: GitCommandExecutor,
  repository_path: String,
  cache: ObjectCache,
  remote: String,
  baseline: String,
  /// Full name of the virtual branch -> the tip of its remote branch
  remote_tips: HashMap<String, String>,
  push_times: PushTimes,
  pending: Mutex<Vec<([u8; 32], CachedStatus)>>,
}

impl RemoteStatusMemo {
  /// The statuses against `remote`, whose remote-tracking tips under the prefix are `remote_tips` (by the full name of
  /// the virtual branch), on the baseline commit `baseline`
  pub fn new(git_executor: GitCommandExecutor, repository_path: &str, cache: ObjectCache, remote: &str, baseline: String, remote_tips: HashMap<String, String>) -> Self {
    let remote_refs = remote_tips.keys().map(|full_name| format!("refs/remotes/{remote}/{full_name}")).collect();
    Self {
      git_executor,
      repository_path: repository_path.to_string(),
      cache,
      remote: remote.to_string(),
      baseline,
      remote_tips,
      push_times: PushTimes::new(remote_refs),
      pending: Mutex::default(),
    }
  }

  /// `compute_remote_status_for_branch` of the branch `local_ref` (`{prefix}/virtual/{name}`) at the commit `local`:
  /// the walks run on the commits, not on the refs, and the status of the same commits runs none
  #[instrument(skip(self, parent), fields(branch_name = %branch_name, local = %local))]
  pub fn status(&self, local_ref: &str, branch_name: &str, local: &str, my_email: Option<&str>, parent: Option<StackParent<'_>>) -> Result<RemoteStatusUpdate> {
    let branch = BranchCommits {
      local,
      baseline: &self.baseline,
      remote_tip: self.remote_tips.get(local_ref).map(String::as_str),
    };
    let key = StatusKey {
      repository_path: self.repository_path.clone(),
      remote: self.remote.clone(),
      local: local.to_string(),
      remote_tip: branch.remote_tip.map(str::to_string),
      baseline: self.baseline.clone(),
      parent_base: parent.map(|parent| parent.base.to_string()),
      my_email: my_email.map(str::to_string),
    };
    let remembered = STATUS_CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&key).cloned();
    if let Some(status) = remembered {
      debug!("remote status of the same commits");
      return Ok(RemoteStatusUpdate {
        branch_name: branch_name.to_string(),
        ..status
      });
    }

    let cache_key = cached_status_key(&branch, my_email, parent);
    let version = cached_status_version();
    let cached = cache_key.and_then(|cache_key| self.cache.get_remote_status::<CachedStatus>(version, &cache_key));
    let walked = if let Some(cached) = cached {
      debug!("remote status of the same commits from the object cache");
      cached
    } else {
      let walked = compute(&self.git_executor, &self.repository_path, &branch, my_email, parent)?;
      // a remote branch at the tip ran no walk: nothing to keep
      if let Some(cache_key) = cache_key
        && branch.remote_tip != Some(local)
      {
        self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((cache_key, walked.clone()));
      }
      walked
    };
    let last_push_time = match branch.remote_tip {
      Some(_) => self
        .push_times
        .of(&self.git_executor, &self.repository_path, &format!("refs/remotes/{}/{local_ref}", self.remote)),
      None => 0,
    };
    let status = walked.into_update(branch_name, last_push_time);

    let mut remembered = STATUS_CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if remembered.len() >= STATUS_CACHE_LIMIT {
      remembered.clear();
    }
    remembered.insert(key, status.clone());
    Ok(status)
  }
}

impl Drop for RemoteStatusMemo {
  fn drop(&mut self) {
    let pending = std::mem::take(&mut *self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    self.cache.put_remote_statuses(cached_status_version(), &pending);
  }
}

/// The commits one status is computed from
struct BranchCommits<'a> {
  /// The branch's tip: its commit, or its ref when it has none
  local: &'a str,
  /// The baseline: its ref or its commit
  baseline: &'a str,
  /// `None` when the remote branch does not exist
  remote_tip: Option<&'a str>,
}

/// The status of `branch` without its name and its push time
fn compute(git_executor: &GitCommandExecutor, repository_path: &str, branch: &BranchCommits<'_>, my_email: Option<&str>, parent: Option<StackParent<'_>>) -> Result<CachedStatus> {
  let BranchCommits {
    local,
    baseline: baseline_branch,
    remote_tip,
  } = *branch;
  let baseline_exclusion = format!("^{baseline_branch}");
  let parent_exclusion = parent.map(|parent| format!("^{}", parent.base));

  let Some(remote_tip) = remote_tip else {
    let mut revs = vec![baseline_exclusion.as_str()];
    revs.extend(parent_exclusion.as_deref());
    revs.push(local);
    let walked = walk(git_executor, repository_path, &revs, my_email)?;
    return Ok(CachedStatus {
      remote_tip: None,
      my_unpushed_count: walked.mine,
      unpushed_commits: walked.commits,
      commits_behind: 0,
      dropped_commits: Vec::new(),
      commits_amended: 0,
      commits_restacked: 0,
      commits_landed: 0,
      landed_commits: Vec::new(),
      commits_reworded: 0,
      ancestor_commits_ahead: 0,
      fast_forward: true,
    });
  };

  let ancestors = LazyAncestorKeys {
    git_executor,
    repository_path,
    baseline_exclusion: &baseline_exclusion,
    parent,
    keys: std::cell::OnceCell::new(),
  };
  let (
    unpushed,
    ClassifiedBehind {
      amended,
      restacked,
      landed,
      dropped,
    },
    reworded,
    ancestor_commits_ahead,
    fast_forward,
  ) = if remote_tip == local {
    // the remote branch is at the branch's tip, as after its push: nothing to push and nothing behind, without a walk
    (WalkedCommits::default(), ClassifiedBehind::default(), 0, 0, true)
  } else {
    // the local commits whose content the remote lacks: patch-equivalent (rebased) copies and the baseline are left out.
    // The ancestors' commits are in the walk too: a Safe Push of the remote branch lands them, so the ones the remote
    // lacks make it an older piece (`ancestor_commits_ahead`); the rest of the status is of the branch's own commits
    let range = format!("{remote_tip}...{local}");
    let walked = walk_commits_with_messages(
      git_executor,
      repository_path,
      &["--reverse", "--cherry-mark", "--right-only"],
      &[baseline_exclusion.as_str(), &range],
    )?;
    let ancestor_keys = if walked.is_empty() { None } else { ancestors.get()? };
    let (ancestor_side, own_side): (Vec<_>, Vec<_>) = walked.into_iter().partition(|(commit, _)| ancestor_keys.is_some_and(|keys| keys.contains(&commit.key)));
    let unpushed = walked_commits(own_side.into_iter().map(|(commit, message)| (commit, Some(message))), my_email);

    // the remote commits whose content the branch lacks. The baseline is not excluded here, so a remote commit that
    // reached the baseline as a copy (cherry-pick, squash merge) is matched by that copy.
    let RemoteCommits { behind, versions, fast_forward } = walk_remote(git_executor, repository_path, &range)?;
    // a copy the remote has by content but in no version of the branch's: its message or author changed here
    let reworded = count(unpushed.copies.iter().filter(|copy| !versions.contains(copy)).count());
    // an ancestor's commit the remote lacks: new, or a newer version of a remote commit, which is restacked below
    let behind_keys: HashSet<&CommitKey> = behind.iter().map(|commit| &commit.key).collect();
    let ancestor_commits_ahead = count(ancestor_side.iter().filter(|(commit, _)| commit.mark != '=' && !behind_keys.contains(&commit.key)).count());
    let ancestor_copies: Vec<CommitVersion> = ancestor_side
      .into_iter()
      .filter(|(commit, _)| commit.mark == '=')
      .map(|(commit, message)| CommitVersion::new(&commit, message))
      .collect();
    let mut classified = classify_behind(git_executor, repository_path, baseline_branch, remote_tip, behind, &unpushed.keys, &ancestors)?;
    classified.restacked += reworded_ancestor_copies(&versions, &unpushed.copies, &ancestor_copies);
    debug!(
      amended = classified.amended,
      restacked = classified.restacked,
      landed = classified.landed.len(),
      dropped = classified.dropped.len(),
      reworded,
      ancestor_commits_ahead,
      fast_forward,
      "commits behind"
    );
    (unpushed, classified, reworded, ancestor_commits_ahead, fast_forward)
  };

  let dropped_count = dropped.len();
  let landed_count = landed.len();
  Ok(CachedStatus {
    remote_tip: Some(remote_tip.to_string()),
    my_unpushed_count: unpushed.mine,
    unpushed_commits: unpushed.commits,
    commits_behind: count(dropped.len()),
    dropped_commits: dropped
      .into_iter()
      .take(kept(dropped_count, DROPPED_COMMITS_LIMIT))
      .map(|behind| CachedDroppedCommit {
        id: behind.commit.id,
        subject: behind.commit.subject,
        author: behind.commit.author,
        author_time: behind.commit.author_time,
      })
      .collect(),
    commits_amended: amended,
    commits_restacked: restacked,
    commits_landed: count(landed_count),
    landed_commits: landed
      .into_iter()
      .take(kept(landed_count, DROPPED_COMMITS_LIMIT))
      .map(|(behind, baseline_commit)| CachedLandedCommit {
        id: behind.commit.id,
        subject: behind.commit.subject,
        baseline_commit,
      })
      .collect(),
    commits_reworded: reworded,
    ancestor_commits_ahead,
    fast_forward,
  })
}

/// Forgets the statuses of this process, as a new process starts without them
#[cfg(test)]
pub(crate) fn forget_statuses() {
  STATUS_CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
}
