//! The remote status of a virtual branch: the commits to push and those a push would drop, by content, so the baseline
//! and rebased copies don't count (docs/design/remote-status.md); no git for a branch whose commits did not move since
//! its last status.

use crate::commit_key::CommitKey;
use crate::commit_walk::walk_commits;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::sync::{LazyLock, Mutex};
use sync_types::{DroppedCommit, RemoteStatusUpdate};
use tracing::{debug, instrument};

/// At most this many dropped commits are listed (`RemoteStatusUpdate::dropped_commits`); the count covers all of them
pub const DROPPED_COMMITS_LIMIT: usize = 10;

/// The time of the newest push of the remote-tracking ref `remote_ref` from this clone, 0 without one (no reflog, or
/// only fetches moved it). git's push writes `update by push` into the reflog of the ref, a literal it never
/// translates; `-1` stops the walk of the reflog at the newest such entry. With `--date=unix` its selector is
/// `<ref>@{<time>}`, cut at the last `@{`: a ref name may hold a `{`, never an `@{`
fn get_last_push_time(git_executor: &GitCommandExecutor, repository_path: &str, remote_ref: &str) -> u32 {
  git_executor
    .execute_command(
      &["reflog", "show", "-1", "--grep-reflog=^update by push$", "--date=unix", "--format=%gd", remote_ref],
      repository_path,
    )
    .ok()
    .and_then(|selector| {
      let (_, time) = selector.rsplit_once("@{")?;
      time.strip_suffix('}')?.parse().ok()
    })
    .unwrap_or(0)
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
  /// Read from the commits up to `StackParent::base` with one `rev-list`, only when a remote commit matches no commit of
  /// the branch
  ReadWhenNeeded,
}

/// The commits of a walk, oldest first, how many of them the user authored, and the commit of each identity
#[derive(Default)]
struct WalkedCommits {
  commits: Vec<String>,
  mine: u32,
  keys: HashMap<CommitKey, String>,
}

fn walk(git_executor: &GitCommandExecutor, repository_path: &str, options: &[&str], revs: &[&str], my_email: Option<&str>) -> Result<WalkedCommits> {
  let mut walked = WalkedCommits::default();
  for commit in walk_commits(git_executor, repository_path, &[&["--reverse"], options].concat(), revs)? {
    if my_email.is_some_and(|my_email| commit.key.email.eq_ignore_ascii_case(my_email)) {
      walked.mine += 1;
    }
    walked.keys.insert(commit.key, commit.id.clone());
    walked.commits.push(commit.id);
  }
  Ok(walked)
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
  /// The remote tip is the branch's tip or an ancestor of it: the remote side is empty, without even a
  /// patch-equivalent copy of a local commit. A rebased branch is no fast-forward even when nothing is behind: the push
  /// replaces the remote commits with their rebased copies
  fast_forward: bool,
}

/// One `rev-list --cherry-mark --left-only` of `range` (`<remote>...<local>`). `--cherry-mark` lists a remote commit
/// patch-equivalent to a local one too, marked `=` (`%m`), so the same walk that finds the commits behind tells the
/// fast-forward: no remote commit at all
fn walk_remote(git_executor: &GitCommandExecutor, repository_path: &str, range: &str) -> Result<RemoteCommits> {
  let commits = walk_commits(git_executor, repository_path, &["--cherry-mark", "--left-only", "--date-order"], &[range])?;
  let fast_forward = commits.is_empty();
  let behind = commits
    .into_iter()
    // `=`: the branch has the commit by content
    .filter(|commit| commit.mark != '=')
    .map(|commit| BehindCommit {
      commit: DroppedCommit {
        id: commit.id,
        subject: commit.subject,
        author: commit.author,
        author_time: commit.key.time,
      },
      key: commit.key,
    })
    .collect();
  Ok(RemoteCommits { behind, fast_forward })
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

/// The commits behind, sorted by what a push does with them
#[derive(Default)]
struct ClassifiedBehind {
  /// Older versions of commits of the branch, with other changes
  amended: u32,
  /// Older versions of commits of its stack ancestors
  restacked: u32,
  /// The rest, newest first: a push drops them
  dropped: Vec<BehindCommit>,
}

/// Sorts the commits behind. One with the identity of an unpushed commit of the branch is an older version of it (the
/// new version has other content, so it is unpushed): amended when its changes differ, a rebased copy, counted nowhere,
/// when only their context or position does (the commit was rebuilt on a changed parent or baseline). One with the
/// identity of a commit of an ancestor is restacked; the rest is the remote's own work, dropped.
fn classify_behind(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  baseline_exclusion: &str,
  behind: Vec<BehindCommit>,
  own: &HashMap<CommitKey, String>,
  parent: Option<StackParent<'_>>,
) -> Result<ClassifiedBehind> {
  let (versions, mut dropped): (Vec<BehindCommit>, Vec<BehindCommit>) = behind.into_iter().partition(|commit| own.contains_key(&commit.key));
  let pairs: Vec<(&str, &str)> = versions.iter().map(|commit| (commit.commit.id.as_str(), own[&commit.key].as_str())).collect();
  let rebased = same_changes(git_executor, repository_path, &pairs)?.into_iter().filter(|same| *same).count();
  let amended = count(versions.len() - rebased);

  let restacked = if let Some(parent) = parent
    && !dropped.is_empty()
  {
    let ancestors = ancestor_keys(git_executor, repository_path, baseline_exclusion, parent)?;
    let before = dropped.len();
    dropped.retain(|commit| !ancestors.contains(&commit.key));
    count(before - dropped.len())
  } else {
    0
  };
  Ok(ClassifiedBehind { amended, restacked, dropped })
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
/// branch is built on: its commits are the parent's, not unpushed commits of this branch.
///
/// The sync rebuilds a virtual branch on the latest baseline, so its hashes change while its work stays the same.
/// The status compares content, not hashes: the baseline commits are on the remote already, and a rebased copy of a
/// pushed commit is patch-equivalent to it. Only the commits whose content the remote lacks are unpushed.
///
/// A remote commit whose content the branch lacks is an older version of a commit the branch has when their identities
/// (`CommitKey`) match: amended when the commit is the branch's and its changes differ (a rebased copy when only their
/// context moved), restacked when it is a stack ancestor's (the child's remote branch has the parent's commits as they
/// were). A push replaces those; only the rest is dropped work.
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
  // one process for the tips of both: whether the remote branch exists, its commit and the branch's. The walks below
  // use these commits, not the ref names, so a fetch or a sync meanwhile can't make the counts disagree with
  // `remote_tip`. A ref name holds no glob character, so each pattern matches its ref only
  let remote_ref = format!("refs/remotes/{remote}/{local_ref}");
  let branch_ref = format!("refs/heads/{local_ref}");
  let tips = git_executor.execute_command_lines(&["for-each-ref", "--format=%(objectname) %(refname)", &remote_ref, &branch_ref], repository_path)?;
  let tip_of = |name: &str| tips.iter().find_map(|line| line.split_once(' ').filter(|(_, refname)| *refname == name).map(|(id, _)| id));
  let branch = BranchRefs {
    remote,
    local_ref,
    // without the branch the walks name it, and fail
    local: tip_of(&branch_ref).unwrap_or(local_ref),
    baseline: baseline_branch,
    remote_tip: tip_of(&remote_ref),
  };
  compute(git_executor, repository_path, &branch, branch_name, my_email, parent)
}

/// The commits a sync knows for the remote status of a branch: it reads the remote tips of all branches with one
/// `for-each-ref` and the baseline once, and the branch's head is the one it built
#[derive(Debug, Clone, Copy)]
pub struct KnownTips<'a> {
  pub local: &'a str,
  /// `None` when the remote branch does not exist
  pub remote: Option<&'a str>,
  pub baseline: &'a str,
}

/// What a status is a function of: the commits and the email. A status computed for them holds as long as they are the
/// same (`last_push_time` is read from the reflog of the remote branch, which a push moves)
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

/// Statuses of the syncs of this process; an unchanged branch is answered without git. Cleared when full
static STATUS_CACHE: LazyLock<Mutex<HashMap<StatusKey, RemoteStatusUpdate>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
const STATUS_CACHE_LIMIT: usize = 4096;

/// `compute_remote_status_for_branch` for a caller that knows the commits (`KnownTips`): the walks run on these commits,
/// not on the refs, and the status of the same commits is remembered for the next call, which then runs no git
#[instrument(skip(git_executor, parent), fields(branch_name = %branch_name, local = tips.local, remote_tip = tips.remote))]
#[expect(clippy::too_many_arguments, reason = "the refs and names of one branch status")]
pub fn compute_remote_status_with_tips(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  remote: &str,
  local_ref: &str,
  branch_name: &str,
  my_email: Option<&str>,
  parent: Option<StackParent<'_>>,
  tips: KnownTips<'_>,
) -> Result<RemoteStatusUpdate> {
  let key = StatusKey {
    repository_path: repository_path.to_string(),
    remote: remote.to_string(),
    local: tips.local.to_string(),
    remote_tip: tips.remote.map(str::to_string),
    baseline: tips.baseline.to_string(),
    parent_base: parent.map(|parent| parent.base.to_string()),
    my_email: my_email.map(str::to_string),
  };
  let cached = STATUS_CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&key).cloned();
  if let Some(status) = cached {
    debug!("remote status of the same commits");
    return Ok(RemoteStatusUpdate {
      branch_name: branch_name.to_string(),
      ..status
    });
  }
  let branch = BranchRefs {
    remote,
    local_ref,
    local: tips.local,
    baseline: tips.baseline,
    remote_tip: tips.remote,
  };
  let status = compute(git_executor, repository_path, &branch, branch_name, my_email, parent)?;
  let mut cache = STATUS_CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
  if cache.len() >= STATUS_CACHE_LIMIT {
    cache.clear();
  }
  cache.insert(key, status.clone());
  Ok(status)
}

/// The refs or commits one status is computed from
struct BranchRefs<'a> {
  remote: &'a str,
  /// `{prefix}/virtual/{name}`, for the name of the remote branch
  local_ref: &'a str,
  /// The branch's tip: its commit, or its ref when it has none
  local: &'a str,
  /// The baseline: its ref or its commit
  baseline: &'a str,
  /// `None` when the remote branch does not exist
  remote_tip: Option<&'a str>,
}

fn compute(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch: &BranchRefs<'_>,
  branch_name: &str,
  my_email: Option<&str>,
  parent: Option<StackParent<'_>>,
) -> Result<RemoteStatusUpdate> {
  let BranchRefs {
    remote,
    local_ref,
    local,
    baseline: baseline_branch,
    remote_tip,
  } = *branch;
  let remote_ref = format!("refs/remotes/{remote}/{local_ref}");
  let baseline_exclusion = format!("^{baseline_branch}");
  let parent_exclusion = parent.map(|parent| format!("^{}", parent.base));

  let Some(remote_tip) = remote_tip else {
    let mut revs = vec![baseline_exclusion.as_str()];
    revs.extend(parent_exclusion.as_deref());
    revs.push(local);
    let walked = walk(git_executor, repository_path, &[], &revs, my_email)?;
    return Ok(RemoteStatusUpdate {
      branch_name: branch_name.to_string(),
      remote_exists: false,
      remote_tip: None,
      my_unpushed_count: walked.mine,
      unpushed_commits: walked.commits,
      commits_behind: 0,
      dropped_commits: Vec::new(),
      commits_amended: 0,
      commits_restacked: 0,
      fast_forward: true,
      last_push_time: 0,
    });
  };

  let (unpushed, ClassifiedBehind { amended, restacked, dropped }, fast_forward) = if remote_tip == local {
    // the remote branch is at the branch's tip, as after its push: nothing to push and nothing behind, without a walk
    (WalkedCommits::default(), ClassifiedBehind::default(), true)
  } else {
    // the local commits whose content the remote lacks: patch-equivalent (rebased) copies and the baseline are left out
    let range = format!("{remote_tip}...{local}");
    let mut revs = vec![baseline_exclusion.as_str()];
    revs.extend(parent_exclusion.as_deref());
    revs.push(&range);
    let unpushed = walk(git_executor, repository_path, &["--cherry-pick", "--right-only"], &revs, my_email)?;

    // the remote commits whose content the branch lacks. The baseline is not excluded here, so a remote commit that
    // reached the baseline as a copy (cherry-pick, squash merge) is matched by that copy.
    let RemoteCommits { behind, fast_forward } = walk_remote(git_executor, repository_path, &range)?;
    let classified = classify_behind(git_executor, repository_path, &baseline_exclusion, behind, &unpushed.keys, parent)?;
    debug!(
      amended = classified.amended,
      restacked = classified.restacked,
      dropped = classified.dropped.len(),
      fast_forward,
      "commits behind"
    );
    (unpushed, classified, fast_forward)
  };

  let last_push_time = get_last_push_time(git_executor, repository_path, &remote_ref);

  Ok(RemoteStatusUpdate {
    branch_name: branch_name.to_string(),
    remote_exists: true,
    remote_tip: Some(remote_tip.to_string()),
    my_unpushed_count: unpushed.mine,
    unpushed_commits: unpushed.commits,
    commits_behind: count(dropped.len()),
    dropped_commits: dropped.into_iter().take(DROPPED_COMMITS_LIMIT).map(|behind| behind.commit).collect(),
    commits_amended: amended,
    commits_restacked: restacked,
    fast_forward,
    last_push_time,
  })
}
