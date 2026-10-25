//! The pairing of a local commit with the upstream commit that carries its change: by identity (`CommitKey`) and by
//! patch id. The update check and its resolve session, the landed report (`sync_core::landed`), the remote status of a
//! virtual branch (`sync_core::remote_status`), the robot commit map (`sync_core::robot_commits`) and `bd pick`
//! (`pick`) read it (spec/repository/landed-commits.spec.md, docs/design/integration-detection.md, "Landed commits").
//!
//! - `CommitKey`: the identity of a commit across a rebase and an amend
//! - the walks: one `rev-list` of the commits a caller matches by `CommitKey` (`walk_commits`), of some authors only
//!   (`walk_commits_by_authors`, `ids_by_key`)
//! - the key match of an update: the local commits with a counterpart upstream and their fixups (`landed_candidates`),
//!   read with the logs of the landed report (`read_local`, `read_upstream`)
//! - the patch ids: `same_changes` of commit pairs without context, `pair_by_patch_id` and `patch_identical` with it

use crate::autosquash_targets::target_of_each_fixup;
use crate::commit_list::parse_single_commit;
use crate::commit_utils::COMMIT_RECORD_FORMAT;
use anyhow::Result;
use domain_types::commit::Commit;
use domain_types::upstream_commit::UpstreamCommit;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use sync_utils::commit_subject::{stripped_subject, subject_of};
use tracing::debug;

/// The identity of a commit across a rebase and an amend: the author email, the author time and the subject without the
/// `(prefix)`.
///
/// A rebase (Safe Push, `git rebase`, `cherry-pick`) and an amend keep the author and the author time, and a virtual
/// branch commit carries the message of its local commit without the prefix. So a local commit, its copy on a virtual
/// branch, the baseline commit that landed it (`landed`, spec/repository/landed-commits.spec.md) and an older version
/// of it on the remote branch (`remote_status`: amended or restacked, docs/design/remote-status.md) share the key. Two
/// different commits of one author with the same subject in the same second share it too; both uses accept that.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommitKey {
  pub email: String,
  pub time: u32,
  /// The subject without the `(prefix)`, as the virtual branch commit was pushed
  pub subject: String,
}

impl CommitKey {
  /// The key of a commit by its full message
  pub fn from_message(email: &str, time: u32, message: &str) -> Self {
    Self::from_subject(email, time, &subject_of(message))
  }

  /// The key of a commit by its subject as git's `%s` prints it
  pub fn from_subject(email: &str, time: u32, subject: &str) -> Self {
    Self {
      email: email.to_string(),
      time,
      subject: stripped_subject(subject).trim().to_string(),
    }
  }
}

/// `\0` between the fields: the mark (`%m`), the commit and its author; the subject is last, it may contain anything else
const WALK_FORMAT: &str = "--format=%m%x00%H%x00%an%x00%ae%x00%at%x00%s";

/// `WALK_FORMAT` and the raw message (`%B`, lines and all) after one more `\0`, each commit ended by `\x01`, which no
/// message holds
const MESSAGE_WALK_FORMAT: &str = "--format=%m%x00%H%x00%an%x00%ae%x00%at%x00%s%x00%B%x01";

/// A commit of a walk
#[derive(Debug, Clone)]
pub struct WalkedCommit {
  /// `%m`: `<` or `>` for the side of `--left-right`, `=` for a commit `--cherry-mark` finds patch-equivalent to one of
  /// the other side; `>` for every commit of a walk without them
  pub mark: char,
  pub id: String,
  /// The author name
  pub author: String,
  /// The author email, the author time and the subject without the `(prefix)`
  pub key: CommitKey,
  /// As `%s` prints it, with the `(prefix)`
  pub subject: String,
}

/// The commits of `rev-list <options> <revs>`, in the order of the walk. The revs follow `--end-of-options` and come
/// before `--`, so neither a rev that starts with `-` nor one that names a file too is read as something else
pub fn walk_commits(git_executor: &GitCommandExecutor, repository_path: &str, options: &[&str], revs: &[&str]) -> Result<Vec<WalkedCommit>> {
  Ok(
    git_executor
      .execute_command_lines(&walk_args(WALK_FORMAT, options, revs), repository_path)?
      .iter()
      .filter_map(|line| parse_line(line))
      .collect(),
  )
}

/// The commits of `rev-list <revs>` by the authors with `emails` (empty ones left out), in one walk with the authors as
/// fixed strings, so a walk of a monorepo baseline stays small; none, and no process, without an email
pub fn walk_commits_by_authors<'a>(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  emails: impl IntoIterator<Item = &'a str>,
  revs: &[&str],
) -> Result<Vec<WalkedCommit>> {
  let authors: BTreeSet<String> = emails.into_iter().filter(|email| !email.is_empty()).map(|email| format!("--author=<{email}>")).collect();
  if authors.is_empty() {
    return Ok(Vec::new());
  }
  let mut options = vec!["--fixed-strings"];
  options.extend(authors.iter().map(String::as_str));
  walk_commits(git_executor, repository_path, &options, revs)
}

/// The commits of `rev-list <revs>` by the authors with `emails` by their identity (`walk_commits_by_authors`): the
/// newest commit of an identity names it. The commits of the baseline above a remote branch or a robot's source
/// revision, which may carry an older version of a commit there
pub fn ids_by_key<'a>(git_executor: &GitCommandExecutor, repository_path: &str, emails: impl IntoIterator<Item = &'a str>, revs: &[&str]) -> Result<HashMap<CommitKey, String>> {
  let mut by_key = HashMap::new();
  for commit in walk_commits_by_authors(git_executor, repository_path, emails, revs)? {
    // newest first: the newest commit of an identity names it
    by_key.entry(commit.key).or_insert(commit.id);
  }
  Ok(by_key)
}

/// The commits of `rev-list <options> <revs>` as `walk_commits` reads them, each with its raw message: what a rebased copy
/// keeps and a reword changes
pub fn walk_commits_with_messages(git_executor: &GitCommandExecutor, repository_path: &str, options: &[&str], revs: &[&str]) -> Result<Vec<(WalkedCommit, String)>> {
  let output = git_executor.cmd(&walk_args(MESSAGE_WALK_FORMAT, options, revs), repository_path).bytes()?;
  Ok(
    String::from_utf8_lossy(&output)
      .split('\x01')
      // the newline rev-list ends each commit's format with
      .map(|record| record.trim_start_matches('\n'))
      .filter(|record| !record.is_empty())
      .filter_map(|record| {
        let (fields, message) = record.rsplit_once('\0')?;
        Some((parse_line(fields)?, message.to_string()))
      })
      .collect(),
  )
}

/// `rev-list` with `format`: the revs follow `--end-of-options` and come before `--`
fn walk_args<'a>(format: &'a str, options: &[&'a str], revs: &[&'a str]) -> Vec<&'a str> {
  let mut args = vec!["rev-list", "--no-commit-header", format];
  args.extend_from_slice(options);
  args.push("--end-of-options");
  args.extend_from_slice(revs);
  args.push("--");
  args
}

/// A line of `WALK_FORMAT`; an author time git can't print is 0
fn parse_line(line: &str) -> Option<WalkedCommit> {
  let mut fields = line.splitn(6, '\0');
  let (mark, id, author, email, time) = (fields.next()?.chars().next()?, fields.next()?, fields.next()?, fields.next()?, fields.next()?);
  let subject = fields.next().unwrap_or_default();
  Some(WalkedCommit {
    mark,
    id: id.to_string(),
    author: author.to_string(),
    key: CommitKey::from_subject(email, time.parse().unwrap_or(0), subject),
    subject: subject.to_string(),
  })
}

/// What a check knows when it asks which local commits the upstream may already have
#[derive(Debug, Clone, Copy)]
pub struct LandedScope<'a> {
  pub head: &'a str,
  pub upstream_head: &'a str,
  pub base: &'a str,
  /// The revisions excluded from the local commits besides the upstream (previous upstream tips)
  pub excluded: &'a [String],
}

/// A commit of the upstream, or one that stays local
#[derive(Debug, Clone)]
pub struct KeyedCommit {
  pub key: CommitKey,
  /// The subject with the `(prefix)`, which names the branch
  pub subject: String,
  pub upstream: UpstreamCommit,
}

/// `\x01` starts a commit, NUL ends a field; the message is last
const KEYED_FORMAT: &str = "--format=%x01%H%x00%ae%x00%at%x00%an%x00%cn%x00%ct%x00%B";

/// The commits of `log --no-merges <filters> <revs>` with their identity and what the landed report shows of them
pub fn read_keyed(git_executor: &GitCommandExecutor, repo_path: &str, revs: &[String], filters: &[String]) -> Result<Vec<KeyedCommit>> {
  let mut args = vec!["log", "--no-merges", KEYED_FORMAT];
  args.extend(filters.iter().map(String::as_str));
  args.push("--end-of-options");
  args.extend(revs.iter().map(String::as_str));
  args.push("--");
  let output = git_executor.execute_command(&args, repo_path)?;
  Ok(
    output
      .split('\x01')
      .filter(|record| !record.trim().is_empty())
      .filter_map(|record| {
        let mut fields = record.splitn(7, '\0');
        let (id, email, author_time, author, committer, commit_time, message) = (
          fields.next()?,
          fields.next()?,
          fields.next()?,
          fields.next()?,
          fields.next()?,
          fields.next()?,
          fields.next()?,
        );
        let subject = subject_of(message);
        Some(KeyedCommit {
          key: CommitKey::from_message(email, author_time.parse().ok()?, message),
          subject,
          upstream: UpstreamCommit {
            id: id.to_string(),
            author: author.to_string(),
            committer: committer.to_string(),
            time: commit_time.parse().ok()?,
          },
        })
      })
      .collect(),
  )
}

/// The local commits of `revs`, oldest first, as the sync reads them
pub fn read_local(git_executor: &GitCommandExecutor, repo_path: &str, revs: &[String]) -> Result<Vec<Commit>> {
  let format = format!("--pretty=format:{COMMIT_RECORD_FORMAT}");
  let mut args = vec!["log", "--reverse", "--no-merges", format.as_str(), "--end-of-options"];
  args.extend(revs.iter().map(String::as_str));
  args.push("--");
  let output = git_executor.execute_command(&args, repo_path)?;
  output.split('\x1e').map(str::trim).filter(|record| !record.is_empty()).map(parse_single_commit).collect()
}

/// The upstream commits since `base` by the authors of the `local` commits: one `log`, the authors as fixed strings, so a
/// monorepo upstream stays small
pub fn read_upstream(git_executor: &GitCommandExecutor, repo_path: &str, local: &[Commit], base: &str, upstream_head: &str) -> Result<Vec<KeyedCommit>> {
  let emails: BTreeSet<&str> = local.iter().map(|c| c.author_email.as_str()).filter(|email| !email.is_empty()).collect();
  let mut filters = vec!["--fixed-strings".to_string()];
  filters.extend(emails.iter().map(|email| format!("--author=<{email}>")));
  read_keyed(git_executor, repo_path, &[format!("{base}..{upstream_head}")], &filters)
}

/// The upstream commit of each key; the newest when two share one
pub fn by_key(upstream: &[KeyedCommit]) -> HashMap<&CommitKey, &UpstreamCommit> {
  let mut by_key = HashMap::with_capacity(upstream.len());
  for commit in upstream {
    by_key.entry(&commit.key).or_insert(&commit.upstream);
  }
  by_key
}

/// For a replay that conflicts (`update_branch::LandedLookup`): the local commits with a counterpart upstream together
/// with the fixups of those, then the ones with a counterpart alone, each oldest first. A robot that autosquashes pushed
/// the target with its fixups folded in, so the target alone adds other content than the upstream has; a fixup made
/// after the push makes the first set lose its change, and the second set is tried. `stacks` is the stack setting, by
/// which a fixup finds its target (`autosquash_targets`)
pub fn landed_candidates(git_executor: &GitCommandExecutor, repo_path: &str, scope: LandedScope<'_>, stacks: bool) -> Result<Vec<Vec<String>>> {
  let mut revs = vec![scope.head.to_string(), format!("^{}", scope.upstream_head)];
  revs.extend(scope.excluded.iter().map(|rev| format!("^{rev}")));
  let local = read_local(git_executor, repo_path, &revs)?;
  if local.is_empty() {
    return Ok(Vec::new());
  }
  let upstream = read_upstream(git_executor, repo_path, &local, scope.base, scope.upstream_head)?;
  let by_key = by_key(&upstream);
  let matched: HashSet<&str> = local
    .iter()
    .filter(|commit| by_key.contains_key(&CommitKey::from_message(&commit.author_email, commit.author_timestamp, &commit.message)))
    .map(|commit| commit.id.as_str())
    .collect();
  if matched.is_empty() {
    return Ok(Vec::new());
  }
  let target_of = target_of_each_fixup(&local, stacks);
  let with_fixups: Vec<String> = local
    .iter()
    .filter(|commit| matched.contains(commit.id.as_str()) || target_of.get(&commit.id).is_some_and(|target| matched.contains(target.as_str())))
    .map(|commit| commit.id.clone())
    .collect();
  let alone: Vec<String> = local.iter().filter(|commit| matched.contains(commit.id.as_str())).map(|commit| commit.id.clone()).collect();
  debug!(with_fixups = with_fixups.len(), alone = alone.len(), "landed candidates");
  Ok(if with_fixups.len() == alone.len() { vec![alone] } else { vec![with_fixups, alone] })
}

/// The context of the diffs a patch id is taken of
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatchContext {
  /// Git's default: as `git rebase` and `rev-list --cherry-mark` compare
  Default,
  /// None (`-U0`): only the changed lines, so a change rebuilt on a changed parent keeps its patch id
  None,
}

/// The patch id of each commit (`patch-id --stable` of one `diff-tree --stdin -p` of its diff against its first
/// parent), by commit; a commit without changes has none
fn patch_ids(git_executor: &GitCommandExecutor, repo_path: &str, commits: impl IntoIterator<Item = impl AsRef<str>>, context: PatchContext) -> Result<HashMap<String, String>> {
  let input = commits.into_iter().fold(String::new(), |mut input, id| {
    let _ = writeln!(input, "{}", id.as_ref());
    input
  });
  let mut args = vec!["diff-tree", "--stdin", "-p"];
  if context == PatchContext::None {
    args.push("-U0");
  }
  args.extend(["--no-renames", "--no-ext-diff", "--no-textconv"]);
  let patches = git_executor.cmd(&args, repo_path).stdin(input.as_bytes()).bytes()?;
  let ids = git_executor.cmd(&["patch-id", "--stable"], repo_path).stdin(&patches).bytes()?;
  // `<patch id> <commit>` per commit with changes
  Ok(
    String::from_utf8_lossy(&ids)
      .lines()
      .filter_map(|line| line.split_once(' ').map(|(patch, commit)| (commit.to_string(), patch.to_string())))
      .collect(),
  )
}

/// Which of the `(older, newer)` pairs make the same changes: the patch ids of their diffs without context (`-U0`; a
/// patch id ignores the line numbers and whitespace) are equal. One `diff-tree --stdin` and one `patch-id`, none
/// without pairs
pub fn same_changes(git_executor: &GitCommandExecutor, repository_path: &str, pairs: &[(&str, &str)]) -> Result<Vec<bool>> {
  if pairs.is_empty() {
    return Ok(Vec::new());
  }
  let patch_of = patch_ids(git_executor, repository_path, pairs.iter().flat_map(|(older, newer)| [older, newer]), PatchContext::None)?;
  Ok(
    pairs
      .iter()
      .map(|(older, newer)| patch_of.get(*older).is_some_and(|patch| patch_of.get(*newer) == Some(patch)))
      .collect(),
  )
}

/// A local commit left out of the replay: the upstream has `upstream`, a commit with the same patch id
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LandedDrop {
  pub commit: String,
  pub upstream: String,
}

/// The local commits (`local`) whose patch id equals that of an upstream commit since the merge base, with that commit:
/// one `rev-list --cherry-mark` marks the commits of both sides that have a counterpart, then one `diff-tree --stdin -p`
/// and one `patch-id --stable` pair them. As `git rebase` does, they are left out of the replay; the patch id is the
/// proof that the upstream has their change
pub fn patch_identical(git_executor: &GitCommandExecutor, repo_path: &str, upstream_head: &str, head: &str, local: &[String]) -> Result<Vec<LandedDrop>> {
  let marked = git_executor.execute_command_lines(&["rev-list", "--cherry-mark", "--no-merges", &format!("{upstream_head}...{head}"), "--"], repo_path)?;
  let equivalent: Vec<&str> = marked.iter().filter_map(|line| line.strip_prefix('=')).collect();
  if equivalent.is_empty() {
    return Ok(Vec::new());
  }
  let local_set: HashSet<&str> = local.iter().map(String::as_str).collect();
  let (local, upstream): (Vec<&str>, Vec<&str>) = equivalent.into_iter().partition(|commit| local_set.contains(commit));
  pair_by_patch_id(git_executor, repo_path, &local, &upstream)
}

/// Each commit of `local` with the first commit of `upstream` that has its patch id (`patch-id --stable` of one
/// `diff-tree --stdin -p` of both lists), in the order of `local`; a commit without one is left out
pub fn pair_by_patch_id(git_executor: &GitCommandExecutor, repo_path: &str, local: &[&str], upstream: &[&str]) -> Result<Vec<LandedDrop>> {
  if local.is_empty() || upstream.is_empty() {
    return Ok(Vec::new());
  }
  let patch_of = patch_ids(git_executor, repo_path, local.iter().chain(upstream), PatchContext::Default)?;
  let local_set: HashSet<&str> = local.iter().copied().collect();
  let mut upstream_by_patch: HashMap<&str, &str> = HashMap::new();
  for commit in upstream.iter().filter(|commit| !local_set.contains(*commit)) {
    if let Some(patch) = patch_of.get(*commit) {
      upstream_by_patch.entry(patch).or_insert(commit);
    }
  }
  Ok(
    local
      .iter()
      .filter_map(|commit| {
        let upstream = upstream_by_patch.get(patch_of.get(*commit)?.as_str())?;
        Some(LandedDrop {
          commit: (*commit).to_string(),
          upstream: (*upstream).to_string(),
        })
      })
      .collect(),
  )
}

#[cfg(test)]
#[path = "landed_pairing_test.rs"]
mod landed_pairing_test;
