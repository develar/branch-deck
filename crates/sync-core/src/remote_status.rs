use crate::commit_key::CommitKey;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use sync_types::{DroppedCommit, RemoteStatusUpdate};
use tracing::{debug, instrument};

/// At most this many dropped commits are listed (`RemoteStatusUpdate::dropped_commits`); the count covers all of them
pub const DROPPED_COMMITS_LIMIT: usize = 10;

/// Get last push time from reflog if available
#[inline]
fn get_last_push_time(git_executor: &GitCommandExecutor, repository_path: &str, remote_ref: &str) -> u32 {
  // Only check reflog if we need the push time
  if let Ok(lines) = git_executor.execute_command_lines(&["reflog", "show", "--date=unix", remote_ref], repository_path) {
    for line in lines {
      if line.contains("update by push") {
        // Extract unix timestamp from format: "hash refs/remotes/origin/branch@{unix_timestamp}: update by push"
        if let Some(timestamp_start) = line.find('{')
          && let Some(timestamp_end) = line.find("}: update by push")
          && let Ok(timestamp) = line[timestamp_start + 1..timestamp_end].parse::<u32>()
        {
          return timestamp;
        }
      }
    }
  }
  0
}

/// The branch a stacked branch is built on, for the remote status of the stacked branch
#[derive(Clone, Copy)]
pub struct StackParent<'a> {
  /// The virtual branch of the parent (`{prefix}/virtual/{name}`): its commits are the parent's, not unpushed commits
  /// of the stacked branch
  pub virtual_ref: &'a str,
  /// The commits of the parent and of the branches it is built on: a remote commit with the identity of one of them is
  /// restacked, an older version of an ancestor's commit
  pub ancestors: AncestorCommits<'a>,
}

/// The identities (`CommitKey`) of the commits of a stacked branch's ancestors
#[derive(Clone, Copy)]
pub enum AncestorCommits<'a> {
  /// Known to the caller: the sync has the commits of every branch
  Known(&'a HashSet<CommitKey>),
  /// Read from the parent's virtual branch with one `rev-list`, only when a remote commit matches no commit of the branch
  ReadWhenNeeded,
}

/// `\0` between the fields of a walk: the subject is last, it may contain anything else
const WALK_FORMAT: &str = "--format=%H%x00%ae%x00%at%x00%s";

/// The commits of a `rev-list` walk, oldest first, how many of them the user authored, and the commit of each identity
struct WalkedCommits {
  commits: Vec<String>,
  mine: u32,
  keys: HashMap<CommitKey, String>,
}

fn walk(git_executor: &GitCommandExecutor, repository_path: &str, args: &[&str], my_email: Option<&str>) -> Result<WalkedCommits> {
  let mut command = vec!["rev-list", "--reverse", "--no-commit-header", WALK_FORMAT];
  command.extend_from_slice(args);
  let mut walked = WalkedCommits {
    commits: Vec::new(),
    mine: 0,
    keys: HashMap::new(),
  };
  for line in git_executor.execute_command_lines(&command, repository_path)? {
    let mut fields = line.splitn(4, '\0');
    let (hash, email) = (fields.next().unwrap_or_default(), fields.next().unwrap_or_default());
    if my_email.is_some_and(|my_email| email.eq_ignore_ascii_case(my_email)) {
      walked.mine += 1;
    }
    if let Some(time) = fields.next().and_then(|time| time.parse().ok()) {
      walked
        .keys
        .insert(CommitKey::from_subject(email, time, fields.next().unwrap_or_default()), hash.to_string());
    }
    walked.commits.push(hash.to_string());
  }
  Ok(walked)
}

/// A remote commit whose content the branch lacks
struct BehindCommit {
  commit: DroppedCommit,
  key: CommitKey,
}

/// The commits behind of `range` (`<remote>...<local>`), newest first
fn walk_behind(git_executor: &GitCommandExecutor, repository_path: &str, range: &str) -> Result<Vec<BehindCommit>> {
  let lines = git_executor.execute_command_lines(
    &[
      "rev-list",
      "--cherry-pick",
      "--left-only",
      "--date-order",
      "--no-commit-header",
      "--format=%H%x00%at%x00%an%x00%ae%x00%s",
      range,
    ],
    repository_path,
  )?;
  Ok(
    lines
      .iter()
      .filter_map(|line| {
        let mut fields = line.splitn(5, '\0');
        let (id, author_time, author, email) = (fields.next()?, fields.next()?, fields.next()?, fields.next()?);
        let author_time = author_time.parse().unwrap_or(0);
        let subject = fields.next().unwrap_or_default();
        Some(BehindCommit {
          key: CommitKey::from_subject(email, author_time, subject),
          commit: DroppedCommit {
            id: id.to_string(),
            subject: subject.to_string(),
            author: author.to_string(),
            author_time,
          },
        })
      })
      .collect(),
  )
}

/// The identities of the commits of `virtual_ref` above the baseline: the parent's and those of the branches it is
/// built on, which its virtual branch contains
fn read_ancestor_keys(git_executor: &GitCommandExecutor, repository_path: &str, baseline_exclusion: &str, virtual_ref: &str) -> Result<HashSet<CommitKey>> {
  let lines = git_executor.execute_command_lines(
    &["rev-list", "--no-commit-header", "--format=%ae%x00%at%x00%s", baseline_exclusion, virtual_ref],
    repository_path,
  )?;
  Ok(
    lines
      .iter()
      .filter_map(|line| {
        let mut fields = line.splitn(3, '\0');
        let (email, time) = (fields.next()?, fields.next()?.parse().ok()?);
        Some(CommitKey::from_subject(email, time, fields.next().unwrap_or_default()))
      })
      .collect(),
  )
}

/// The commits behind, sorted by what a push does with them
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
  let rebased = same_changes(git_executor, repository_path, &pairs)?;
  let amended = count(versions.len() - rebased);

  let mut restacked = 0;
  if let Some(parent) = parent
    && !dropped.is_empty()
  {
    let read;
    let ancestors = match parent.ancestors {
      AncestorCommits::Known(keys) => keys,
      AncestorCommits::ReadWhenNeeded => {
        read = read_ancestor_keys(git_executor, repository_path, baseline_exclusion, parent.virtual_ref)?;
        &read
      }
    };
    let before = dropped.len();
    dropped.retain(|commit| !ancestors.contains(&commit.key));
    restacked = count(before - dropped.len());
  }
  Ok(ClassifiedBehind { amended, restacked, dropped })
}

/// How many of the `(older, newer)` pairs make the same changes: the patch ids of their diffs without context
/// (`-U0`; a patch id ignores the line numbers and whitespace) are equal. One `diff-tree --stdin` and one `patch-id`,
/// none without pairs
fn same_changes(git_executor: &GitCommandExecutor, repository_path: &str, pairs: &[(&str, &str)]) -> Result<usize> {
  if pairs.is_empty() {
    return Ok(0);
  }
  let input = pairs.iter().fold(String::new(), |mut input, (older, newer)| {
    let _ = writeln!(input, "{older}\n{newer}");
    input
  });
  let patches = git_executor.execute_command_with_input_raw(
    &["diff-tree", "--stdin", "-p", "-U0", "--no-renames", "--no-ext-diff", "--no-textconv"],
    repository_path,
    &input,
  )?;
  let ids = git_executor.execute_command_with_input_bytes(&["patch-id", "--stable"], repository_path, &patches)?;
  // `<patch id> <commit>` per commit with changes
  let ids = String::from_utf8_lossy(&ids);
  let patch_of: HashMap<&str, &str> = ids.lines().filter_map(|line| line.split_once(' ').map(|(patch, commit)| (commit, patch))).collect();
  Ok(
    pairs
      .iter()
      .filter(|(older, newer)| patch_of.get(older).is_some_and(|patch| patch_of.get(newer) == Some(patch)))
      .count(),
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
    parent = parent.map(|parent| parent.virtual_ref)
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
  let remote_ref = format!("{remote}/{local_ref}");
  let baseline_exclusion = format!("^{baseline_branch}");
  let parent_exclusion = parent.map(|parent| format!("^{}", parent.virtual_ref));

  // one process for both: whether the remote branch exists, and its tip. The walks below use this commit, not the ref
  // name, so a fetch meanwhile can't make the counts disagree with `remote_tip`
  let (remote_tip, exit_code) = git_executor.execute_command_with_status(
    &["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("refs/remotes/{remote_ref}")],
    repository_path,
  )?;
  if exit_code != 0 {
    let mut args = vec![baseline_exclusion.as_str()];
    args.extend(parent_exclusion.as_deref());
    args.push(local_ref);
    let walked = walk(git_executor, repository_path, &args, my_email)?;
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
  }

  // the local commits whose content the remote lacks: patch-equivalent (rebased) copies and the baseline are left out
  let range = format!("{remote_tip}...{local_ref}");
  let mut args = vec!["--cherry-pick", "--right-only", baseline_exclusion.as_str()];
  args.extend(parent_exclusion.as_deref());
  args.push(&range);
  let unpushed = walk(git_executor, repository_path, &args, my_email)?;

  // the remote commits whose content the branch lacks. The baseline is not excluded here, so a remote commit that
  // reached the baseline as a copy (cherry-pick, squash merge) is matched by that copy.
  let behind = walk_behind(git_executor, repository_path, &range)?;
  let any_behind = !behind.is_empty();
  let ClassifiedBehind { amended, restacked, dropped } = classify_behind(git_executor, repository_path, &baseline_exclusion, behind, &unpushed.keys, parent)?;
  debug!(amended, restacked, dropped = dropped.len(), "commits behind");

  // a rebased branch is not a fast-forward of its remote even when nothing is lost: the push replaces the remote
  // commits with their rebased copies
  let fast_forward = !any_behind
    && git_executor
      .execute_command_with_status(&["merge-base", "--is-ancestor", &remote_tip, local_ref], repository_path)?
      .1
      == 0;

  let last_push_time = get_last_push_time(git_executor, repository_path, &remote_ref);

  Ok(RemoteStatusUpdate {
    branch_name: branch_name.to_string(),
    remote_exists: true,
    remote_tip: Some(remote_tip.clone()),
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
