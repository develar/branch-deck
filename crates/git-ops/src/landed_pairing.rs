//! The pairing of a local commit with the upstream commit that carries its change: by identity (`CommitKey`) and by
//! patch id. The update check and its resolve session, the landed report (`sync_core::landed`), the remote status of a
//! virtual branch (`sync_core::remote_status`), the robot commit map (`sync_core::robot_commits`) and `bd pick`
//! (`pick`) read it (spec/repository/landed-commits.spec.md, docs/design/integration-detection.md, "Landed commits").
//!
//! - `CommitKey`: the identity of a commit across a rebase and an amend
//! - the walks: one `rev-list` of the commits a caller matches by `CommitKey` (`walk_commits`), of some authors only
//!   (`walk_commits_by_authors`, `ids_by_key`)
//! - the pairing of an update whose replay conflicts (`pair_landed`): a ladder of proofs (`LandedProof`), one to one,
//!   on the logs of the landed report (`read_local`, `read_upstream`); the pairs by identity alone are candidates
//! - the patch ids: `same_changes` of commit pairs without context, `pair_by_patch_id` and `patch_identical` with it

use crate::autosquash_targets::target_of_each_fixup;
use crate::commit_list::parse_single_commit;
use crate::commit_utils::COMMIT_RECORD_FORMAT;
use crate::merge_tree_output::parse_merge_tree_batch;
use anyhow::{Result, bail};
use domain_types::commit::Commit;
use domain_types::upstream_commit::UpstreamCommit;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
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

/// The context of the diffs a patch id is taken of
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatchContext {
  /// Git's default: as `git rebase` and `rev-list --cherry-mark` compare
  Default,
  /// None (`-U0`): only the changed lines, so a change rebuilt on a changed parent keeps its patch id
  None,
}

/// How `patch-id` hashes a diff
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatchIdMode {
  /// `--stable`: whitespace ignored
  Stable,
  /// `--verbatim`: every byte of the changed lines
  Verbatim,
}

/// The patch id of each commit (`patch-id` of one `diff-tree --stdin -p` of its diff against its first parent), by
/// commit; a commit without changes has none
fn patch_ids(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  commits: impl IntoIterator<Item = impl AsRef<str>>,
  context: PatchContext,
  mode: PatchIdMode,
) -> Result<HashMap<String, String>> {
  let input = lines_of(commits);
  let mut args = vec!["diff-tree", "--stdin", "-p"];
  if context == PatchContext::None {
    args.push("-U0");
  }
  args.extend(["--no-renames", "--no-ext-diff", "--no-textconv"]);
  let patches = git_executor.cmd(&args, repo_path).stdin(input.as_bytes()).bytes()?;
  let mode = match mode {
    PatchIdMode::Stable => "--stable",
    PatchIdMode::Verbatim => "--verbatim",
  };
  let ids = git_executor.cmd(&["patch-id", mode], repo_path).stdin(&patches).bytes()?;
  // `<patch id> <commit>` per commit with changes
  Ok(
    String::from_utf8_lossy(&ids)
      .lines()
      .filter_map(|line| line.split_once(' ').map(|(patch, commit)| (commit.to_string(), patch.to_string())))
      .collect(),
  )
}

/// One line per item: the stdin of a `--stdin` git
fn lines_of(items: impl IntoIterator<Item = impl AsRef<str>>) -> String {
  items.into_iter().fold(String::new(), |mut input, item| {
    let _ = writeln!(input, "{}", item.as_ref());
    input
  })
}

/// Which of the `(older, newer)` pairs make the same changes: the patch ids of their diffs without context (`-U0`; a
/// patch id ignores the line numbers and whitespace) are equal. One `diff-tree --stdin` and one `patch-id`, none
/// without pairs
pub fn same_changes(git_executor: &GitCommandExecutor, repository_path: &str, pairs: &[(&str, &str)]) -> Result<Vec<bool>> {
  if pairs.is_empty() {
    return Ok(Vec::new());
  }
  let patch_of = patch_ids(
    git_executor,
    repository_path,
    pairs.iter().flat_map(|(older, newer)| [older, newer]),
    PatchContext::None,
    PatchIdMode::Stable,
  )?;
  Ok(pairs.iter().map(|(older, newer)| same_value(&patch_of, older, newer)).collect())
}

/// Both have a value, and it is the same
fn same_value<V: PartialEq>(values: &HashMap<String, V>, one: &str, other: &str) -> bool {
  values.get(one).is_some_and(|value| values.get(other) == Some(value))
}

/// How the upstream proves that it has the change of a local commit (spec/repository/landed-commits.spec.md, "The
/// proof"): only a proven commit is left out of a replay
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LandedProof {
  /// The same patch id with context, as `git rebase` and `rev-list --cherry-mark` compare
  #[default]
  PatchId,
  /// The upstream commit is the local one cherry-picked onto the upstream commit's own parent:
  /// `merge-tree --merge-base L^ T^ L` writes the tree of `T`. Binary files, modes and whitespace included
  Replayed,
  /// The same changed lines (`patch-id --verbatim` of the diffs without context) and the same paths, statuses and
  /// modes, with the same new object of each binary file and gitlink: a change rebuilt where the context lines differ
  /// so much that its cherry-pick conflicts
  ChangedLines,
}

/// A local commit left out of the replay: the upstream has `upstream`, which carries its change by `proof`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LandedDrop {
  pub commit: String,
  pub upstream: String,
  /// A session of an older bd has only `patch_id` drops, without the field
  #[serde(default)]
  pub proof: LandedProof,
}

/// The local commits (`local`) whose patch id equals that of an upstream commit since the merge base, with that commit:
/// one `rev-list --cherry-mark` marks the commits of both sides that have a counterpart, then one `diff-tree --stdin -p`
/// and one `patch-id --stable` pair them one to one (`pair_by_patch_id`). As `git rebase` does, they are left out of the
/// replay; the patch id is the proof that the upstream has their change
pub fn patch_identical(git_executor: &GitCommandExecutor, repo_path: &str, upstream_head: &str, head: &str, local: &[String]) -> Result<Vec<LandedDrop>> {
  let marked = git_executor.execute_command_lines(&["rev-list", "--cherry-mark", "--no-merges", &format!("{upstream_head}...{head}"), "--"], repo_path)?;
  let equivalent: Vec<&str> = marked.iter().filter_map(|line| line.strip_prefix('=')).collect();
  if equivalent.is_empty() {
    return Ok(Vec::new());
  }
  let local_set: HashSet<&str> = local.iter().map(String::as_str).collect();
  let marked: HashSet<&str> = equivalent.iter().copied().collect();
  // in the order of `local`, so the oldest local commit takes the upstream commit
  let local: Vec<&str> = local.iter().map(String::as_str).filter(|commit| marked.contains(commit)).collect();
  let upstream: Vec<&str> = equivalent.into_iter().filter(|commit| !local_set.contains(commit)).collect();
  pair_by_patch_id(git_executor, repo_path, &local, &upstream)
}

/// Each commit of `local` with a commit of `upstream` that has its patch id (`patch-id --stable` of one
/// `diff-tree --stdin -p` of both lists), in the order of `local`: one to one, each takes the first commit of `upstream`
/// with its patch id that no earlier one took, so a change made twice locally (A, a revert of A, A again) with one copy
/// upstream pairs once. A commit without one is left out
pub fn pair_by_patch_id(git_executor: &GitCommandExecutor, repo_path: &str, local: &[&str], upstream: &[&str]) -> Result<Vec<LandedDrop>> {
  if local.is_empty() || upstream.is_empty() {
    return Ok(Vec::new());
  }
  let patch_of = patch_ids(git_executor, repo_path, local.iter().chain(upstream), PatchContext::Default, PatchIdMode::Stable)?;
  let local_set: HashSet<&str> = local.iter().copied().collect();
  let mut upstream_by_patch: HashMap<&str, VecDeque<&str>> = HashMap::new();
  for commit in upstream.iter().filter(|commit| !local_set.contains(*commit)) {
    if let Some(patch) = patch_of.get(*commit) {
      upstream_by_patch.entry(patch).or_default().push_back(commit);
    }
  }
  Ok(
    local
      .iter()
      .filter_map(|commit| {
        let upstream = upstream_by_patch.get_mut(patch_of.get(*commit)?.as_str())?.pop_front()?;
        Some(LandedDrop {
          commit: (*commit).to_string(),
          upstream: upstream.to_string(),
          proof: LandedProof::PatchId,
        })
      })
      .collect(),
  )
}

/// A local commit with a counterpart upstream by identity only: nothing proves that the upstream has its change (an
/// amend after the push, a robot that folded its fixups in, an independent fix), so it is never left out by itself
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandedCandidate {
  pub commit: String,
  /// The upstream commit with its `CommitKey`
  pub upstream: String,
  /// The last of the commit and its local fixups (`autosquash_targets`): what leaving it out has to keep is the change
  /// from its parent to there
  pub last: String,
}

/// The local commits of an update paired with the upstream commits that carry their change
/// (spec/repository/landed-commits.spec.md, "The proof"): one to one, each upstream commit pairs with at most one local
/// commit, in the order of the local commits; merge commits never pair
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LandedPairing {
  /// The local commits, oldest first
  pub local: Vec<String>,
  /// The proven pairs, in the order of `local`: a replay leaves them out
  pub drops: Vec<LandedDrop>,
  /// The pairs by identity alone, in the order of `local`
  pub candidates: Vec<LandedCandidate>,
  /// The candidates with their local fixups that are no drop, oldest first: a robot that autosquashes landed a target
  /// with its fixups folded in. A resolve session marks them as likely landed
  pub with_fixups: Vec<String>,
}

impl LandedPairing {
  /// No pairs: the local commits as `rev-list` read them, oldest first
  pub const fn unpaired(local: Vec<String>) -> Self {
    Self {
      local,
      drops: Vec::new(),
      candidates: Vec::new(),
      with_fixups: Vec::new(),
    }
  }

  /// The sets of local commits that a replay which still conflicts without the drops tries to leave out too: the
  /// candidates with their fixups, then the candidates alone. A fixup made after the push makes the first set lose its
  /// change, and the second set is tried
  pub fn candidate_sets(&self) -> Vec<Vec<String>> {
    let alone: Vec<String> = self.candidates.iter().map(|candidate| candidate.commit.clone()).collect();
    match alone.len() {
      0 => Vec::new(),
      count if count == self.with_fixups.len() => vec![alone],
      _ => vec![self.with_fixups.clone(), alone],
    }
  }
}

/// Pair the local commits of `scope` with the upstream commits since the merge base (`LandedPairing`), by a ladder of
/// proofs, each rung only for the pairs the one before left (`LandedProof`):
/// 1. `patch_id`: `patch_identical`;
/// 2. `replayed`: the local and the upstream commit with the same `CommitKey`, one `merge-tree --stdin` and one
///    `rev-parse` of the upstream trees for all pairs, each pair against its own upstream parent;
/// 3. `changed_lines`: the pairs whose cherry-pick conflicts on its context, one `diff-tree -p -U0` with one
///    `patch-id --verbatim` and one `diff-tree --raw --numstat`.
///
/// The pairs that no rung proves are the candidates. The local commits by `log`, the upstream ones by their authors
/// (`read_upstream`); `stacks` is the stack setting, by which a fixup finds its target
pub fn pair_landed(git_executor: &GitCommandExecutor, repo_path: &str, scope: LandedScope<'_>, stacks: bool) -> Result<LandedPairing> {
  let mut revs = vec![scope.head.to_string(), format!("^{}", scope.upstream_head)];
  revs.extend(scope.excluded.iter().map(|rev| format!("^{rev}")));
  let local = read_local(git_executor, repo_path, &revs)?;
  let mut pairing = LandedPairing::unpaired(local.iter().map(|commit| commit.id.clone()).collect());
  if local.is_empty() {
    return Ok(pairing);
  }
  let mut drops = patch_identical(git_executor, repo_path, scope.upstream_head, scope.head, &pairing.local)?;
  let mut taken: HashSet<String> = drops.iter().map(|drop| drop.upstream.clone()).collect();
  let proven: HashSet<String> = drops.iter().map(|drop| drop.commit.clone()).collect();

  let upstream = read_upstream(git_executor, repo_path, &local, scope.base, scope.upstream_head)?;
  // oldest first, so the oldest local commit of a key takes the oldest upstream one
  let mut by_key: HashMap<&CommitKey, VecDeque<&str>> = HashMap::new();
  for commit in upstream.iter().rev() {
    if !taken.contains(&commit.upstream.id) {
      by_key.entry(&commit.key).or_default().push_back(commit.upstream.id.as_str());
    }
  }
  let mut pairs: Vec<(&str, &str)> = Vec::new();
  for commit in local.iter().filter(|commit| !proven.contains(&commit.id)) {
    let key = CommitKey::from_message(&commit.author_email, commit.author_timestamp, &commit.message);
    if let Some(upstream) = by_key.get_mut(&key).and_then(VecDeque::pop_front) {
      taken.insert(upstream.to_string());
      pairs.push((commit.id.as_str(), upstream));
    }
  }

  let replayed = replayed_pairs(git_executor, repo_path, &pairs)?;
  let conflicting: Vec<(&str, &str)> = pairs.iter().zip(&replayed).filter(|(_, replayed)| !**replayed).map(|(pair, _)| *pair).collect();
  let changed_lines = same_changed_lines(git_executor, repo_path, &conflicting)?;
  let mut by_lines = conflicting.iter().zip(changed_lines);
  let mut candidates: Vec<(&str, &str)> = Vec::new();
  for (pair, replayed) in pairs.iter().zip(replayed) {
    let proof = if replayed {
      Some(LandedProof::Replayed)
    } else {
      by_lines.next().and_then(|(_, same)| same.then_some(LandedProof::ChangedLines))
    };
    match proof {
      Some(proof) => drops.push(LandedDrop {
        commit: pair.0.to_string(),
        upstream: pair.1.to_string(),
        proof,
      }),
      None => candidates.push(*pair),
    }
  }
  let position: HashMap<&str, usize> = pairing.local.iter().enumerate().map(|(index, id)| (id.as_str(), index)).collect();
  drops.sort_by_key(|drop| position.get(drop.commit.as_str()).copied());

  let target_of = target_of_each_fixup(&local, stacks);
  let candidate_ids: HashSet<&str> = candidates.iter().map(|(commit, _)| *commit).collect();
  let dropped: HashSet<&str> = drops.iter().map(|drop| drop.commit.as_str()).collect();
  let mut last: HashMap<&str, &str> = HashMap::new();
  for commit in &local {
    let target = target_of.get(&commit.id).map_or(commit.id.as_str(), String::as_str);
    if candidate_ids.contains(target) {
      last.insert(target, &commit.id);
      if !dropped.contains(commit.id.as_str()) {
        pairing.with_fixups.push(commit.id.clone());
      }
    }
  }
  pairing.candidates = candidates
    .into_iter()
    .map(|(commit, upstream)| LandedCandidate {
      commit: commit.to_string(),
      upstream: upstream.to_string(),
      last: last.get(commit).copied().unwrap_or(commit).to_string(),
    })
    .collect();
  debug!(
    drops = drops.len(),
    candidates = pairing.candidates.len(),
    with_fixups = pairing.with_fixups.len(),
    "landed pairing"
  );
  pairing.drops = drops;
  Ok(pairing)
}

/// Which `(local, upstream)` pairs are a cherry-pick: the merge of the local commit's change onto the upstream commit's
/// parent (`merge-tree --merge-base L^ T^ L`) writes the upstream commit's tree. One `merge-tree --stdin` for all pairs
/// and one `rev-parse` of the upstream trees, none without pairs
fn replayed_pairs(git_executor: &GitCommandExecutor, repo_path: &str, pairs: &[(&str, &str)]) -> Result<Vec<bool>> {
  if pairs.is_empty() {
    return Ok(Vec::new());
  }
  let input = lines_of(pairs.iter().map(|(local, upstream)| format!("{local}^ -- {upstream}^ {local}")));
  let output = git_executor
    .cmd(&["merge-tree", "--write-tree", "--stdin", "-z", "--no-messages"], repo_path)
    .stdin(input.as_bytes())
    .bytes()?;
  let merges = parse_merge_tree_batch(&output, pairs.len())?;
  let mut args = vec!["rev-parse".to_string()];
  args.extend(pairs.iter().map(|(_, upstream)| format!("{upstream}^{{tree}}")));
  let args: Vec<&str> = args.iter().map(String::as_str).collect();
  let trees = git_executor.execute_command_lines(&args, repo_path)?;
  if trees.len() != pairs.len() {
    bail!("rev-parse printed {} trees for {} commits", trees.len(), pairs.len());
  }
  // a conflicted merge writes markers into its tree: the tree alone tells
  Ok(merges.iter().zip(&trees).map(|(merge, tree)| merge.output.tree == *tree).collect())
}

/// Which `(local, upstream)` pairs change the same lines in the same way: equal `patch-id --verbatim` of the diffs
/// without context, and equal `raw_changes`. Three processes, none without pairs
fn same_changed_lines(git_executor: &GitCommandExecutor, repo_path: &str, pairs: &[(&str, &str)]) -> Result<Vec<bool>> {
  if pairs.is_empty() {
    return Ok(Vec::new());
  }
  let commits = || pairs.iter().flat_map(|(local, upstream)| [local, upstream]);
  let patch_of = patch_ids(git_executor, repo_path, commits(), PatchContext::None, PatchIdMode::Verbatim)?;
  let raw_of = raw_changes(git_executor, repo_path, commits())?;
  Ok(
    pairs
      .iter()
      .map(|(local, upstream)| same_value(&patch_of, local, upstream) && raw_of.get(*local) == raw_of.get(*upstream))
      .collect(),
  )
}

/// A path that a commit changes as `diff-tree --raw` shows it, without what the patch of its lines shows
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RawChange {
  path: Vec<u8>,
  status: String,
  old_mode: String,
  new_mode: String,
  /// The new object of a binary file or a gitlink, which no text diff shows; `None` for a text file, whose blob ids
  /// differ with the context
  object: Option<String>,
}

/// The mode of a gitlink
const GITLINK_MODE: &str = "160000";

/// The `RawChange`s of each commit against its first parent, sorted by path: one
/// `diff-tree --stdin -r -z --raw --numstat`, whose numstat tells a binary file (`-` for the counts)
fn raw_changes(git_executor: &GitCommandExecutor, repo_path: &str, commits: impl IntoIterator<Item = impl AsRef<str>>) -> Result<HashMap<String, Vec<RawChange>>> {
  let input = lines_of(commits);
  let output = git_executor
    .cmd(
      &["diff-tree", "--stdin", "-r", "-z", "--no-renames", "--raw", "--numstat", "--no-ext-diff", "--no-textconv"],
      repo_path,
    )
    .stdin(input.as_bytes())
    .bytes()?;
  // per commit: `<commit>\0`, then `:<old mode> <new mode> <old id> <new id> <status>\0<path>\0` per path, then
  // `<added>\t<deleted>\t<path>\0` per path, `-` for the counts of a binary file
  let mut entries: HashMap<String, Vec<(RawChange, String)>> = HashMap::new();
  let mut binary: HashSet<(String, Vec<u8>)> = HashSet::new();
  let mut commit = String::new();
  let mut fields = output.split(|&b| b == 0).filter(|field| !field.is_empty());
  while let Some(field) = fields.next() {
    if let Some(header) = field.strip_prefix(b":") {
      let header = String::from_utf8_lossy(header);
      let parts: Vec<&str> = header.split(' ').collect();
      let ([old_mode, new_mode, _, new_id, status], Some(path)) = (parts.as_slice(), fields.next()) else {
        bail!("Unexpected diff-tree --raw entry {header:?}");
      };
      let object = (*old_mode == GITLINK_MODE || *new_mode == GITLINK_MODE).then(|| (*new_id).to_string());
      let change = RawChange {
        path: path.to_vec(),
        status: (*status).to_string(),
        old_mode: (*old_mode).to_string(),
        new_mode: (*new_mode).to_string(),
        object,
      };
      entries.entry(commit.clone()).or_default().push((change, (*new_id).to_string()));
      continue;
    }
    let mut numstat = field.splitn(3, |&b| b == b'\t');
    if let (Some(added), Some(_), Some(path)) = (numstat.next(), numstat.next(), numstat.next()) {
      if added == b"-" {
        binary.insert((commit.clone(), path.to_vec()));
      }
      continue;
    }
    commit = String::from_utf8_lossy(field).into_owned();
  }
  Ok(
    entries
      .into_iter()
      .map(|(commit, list)| {
        let mut changes: Vec<RawChange> = list
          .into_iter()
          .map(|(mut change, new_id)| {
            if binary.contains(&(commit.clone(), change.path.clone())) {
              change.object = Some(new_id);
            }
            change
          })
          .collect();
        changes.sort();
        (commit, changes)
      })
      .collect(),
  )
}

#[cfg(test)]
#[path = "landed_pairing_test.rs"]
mod landed_pairing_test;
