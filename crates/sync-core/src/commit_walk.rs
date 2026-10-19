//! One `rev-list` walk of the commits a caller matches across a rebase by their identity (`CommitKey`): the mark, the id,
//! the author and the subject of each commit, for the remote status and the robot commit map.

use crate::commit_key::CommitKey;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::BTreeSet;

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

#[cfg(test)]
#[path = "commit_walk_test.rs"]
mod commit_walk_test;
