//! The remote of a fixture repository: a bare repository at `.git/upstream.git` that the templates name as `origin` (a
//! relative URL, so a copy of a template pushes to and fetches from its own remote, offline; inside `.git`, so no status
//! shows it), and what a test does to it behind the app's back: a push or a fetch in a terminal, a commit someone else
//! pushed, the branches it has, and the pushes it took from the app, in order (its `post-receive` hook writes each into
//! [`PUSH_LOG`]). The E2E test server serves these as its remote fixtures (`remote_git_fixtures.rs`).

use crate::git_test_utils::{append_config, disable_background_maintenance, git_command, receive_hook};
use anyhow::{Context, Result, bail, ensure};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;

/// Where the remote of a repository is, relative to its working tree: the URL of its `origin`
pub const BARE_ORIGIN: &str = ".git/upstream.git";

/// The file of the remote that its `post-receive` hook appends each push of the app to: the `<old> <new> <ref>` lines
/// git gives the hook, then an empty line
pub const PUSH_LOG: &str = "pushes.log";

/// Set for a push in a terminal ([`push_in_terminal`]): the hook of the remote, whose environment a local push passes
/// on, logs no push of a test
const PUSH_IN_TERMINAL: &str = "TEST_PUSH_IN_TERMINAL";

/// The remote of the repository at `repo_path`
pub fn bare_origin(repo_path: &Path) -> PathBuf {
  repo_path.join(BARE_ORIGIN)
}

/// Gives the repository at `repo_path` its remote, as a clone leaves it: a bare repository at [`BARE_ORIGIN`] whose
/// `branch` is the commit of `refs/remotes/origin/<branch>`, and `origin` with the usual fetch refspec; its
/// `post-receive` hook logs the pushes it takes after that ([`PUSH_LOG`]). Two processes, an `init` and a `push`
///
/// # Errors
/// When git cannot create the bare repository or push the branch into it
pub fn add_bare_origin(repo_path: &Path, branch: &str) -> Result<()> {
  run(repo_path, &["init", "-q", "--bare", &format!("--initial-branch={branch}"), "--template=", BARE_ORIGIN])?;
  let bare = bare_origin(repo_path);
  disable_background_maintenance(&bare);
  // by URL: no remote-tracking ref moves
  run(repo_path, &["push", "-q", BARE_ORIGIN, &format!("refs/remotes/origin/{branch}:refs/heads/{branch}")])?;
  // git runs the hook in the bare repository, after the refs moved: a refused push logs nothing. One write per push,
  // so two pushes at once don't interleave their lines
  receive_hook(
    &bare,
    "post-receive",
    &format!("push=$(cat)\nif [ -z \"${PUSH_IN_TERMINAL}\" ]; then printf '%s\\n\\n' \"$push\" >> {PUSH_LOG}; fi"),
  );
  append_config(
    repo_path,
    &[("remote.origin.url", BARE_ORIGIN), ("remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*")],
  );
  Ok(())
}

/// The branches of the remote of the repository at `repo_path`, by name without `refs/heads/`: one `for-each-ref`
///
/// # Errors
/// When the repository has no remote of its own
pub fn remote_branches(repo_path: &Path) -> Result<BTreeMap<String, String>> {
  let output = run(&existing_origin(repo_path)?, &["for-each-ref", "--format=%(objectname) %(refname:lstrip=2)", "refs/heads/"])?;
  Ok(
    output
      .lines()
      .filter_map(|line| line.split_once(' ').map(|(id, name)| (name.to_string(), id.to_string())))
      .collect(),
  )
}

/// Pushes `revision` of the repository at `repo_path` to `branch` of its remote as `git push --force origin
/// <commit>:refs/heads/<branch>` in a terminal does: the remote branch and the remote-tracking ref move, whatever the
/// remote had. The commit pushed
///
/// # Errors
/// When `revision` names no commit, or the remote refuses the push
pub fn push_in_terminal(repo_path: &Path, branch: &str, revision: &str) -> Result<String> {
  existing_origin(repo_path)?;
  let commit = run(repo_path, &["rev-parse", "--verify", "--end-of-options", &format!("{revision}^{{commit}}")])?;
  let commit = commit.trim();
  let output = git_command()
    .args(["push", "-q", "--force", "origin", &format!("{commit}:refs/heads/{branch}")])
    .env(PUSH_IN_TERMINAL, "1")
    .current_dir(repo_path)
    .output()?;
  ensure!(output.status.success(), "git push in a terminal failed: {}", String::from_utf8_lossy(&output.stderr));
  Ok(commit.to_string())
}

/// Fetches the remote into the repository at `repo_path` as `git fetch origin` in a terminal does: the remote-tracking
/// refs move to what the remote has, so the next sync shows a commit someone else pushed. One `fetch`
///
/// # Errors
/// When the repository has no remote of its own, or the fetch fails
pub fn fetch_in_terminal(repo_path: &Path) -> Result<()> {
  existing_origin(repo_path)?;
  run(repo_path, &["fetch", "-q", "origin"])?;
  Ok(())
}

/// The pushes of the app the remote of the repository at `repo_path` took, in order, each the branches it moved (by
/// name without `refs/heads/`) with the commit it moved them to, `None` for a deleted one; a push in a terminal is
/// not one. Reads [`PUSH_LOG`]
///
/// # Errors
/// When the repository has no remote of its own, or the log has a line that is not `<old> <new> <ref>`
pub fn remote_pushes(repo_path: &Path) -> Result<Vec<BTreeMap<String, Option<String>>>> {
  let log = match std::fs::read_to_string(existing_origin(repo_path)?.join(PUSH_LOG)) {
    Ok(log) => log,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(error) => return Err(error.into()),
  };
  log
    .split("\n\n")
    .filter(|push| !push.trim().is_empty())
    .map(|push| {
      push
        .lines()
        .filter_map(|line| {
          let [_, new, name] = line.split(' ').collect::<Vec<_>>()[..] else {
            return Some(Err(anyhow::anyhow!("an unexpected line of the push log: {line:?}")));
          };
          let branch = name.strip_prefix("refs/heads/")?;
          let moved_to = (new.bytes().any(|byte| byte != b'0')).then(|| new.to_string());
          Some(Ok((branch.to_string(), moved_to)))
        })
        .collect()
    })
    .collect()
}

/// A copy of the commits of `revision` after `base` onto `base`, each with its author, message and tree, committed a day
/// earlier: the same commits by content under other ids, as an earlier build of a virtual branch that the sync since
/// rebuilt. One `log`, then a `commit-tree` per commit; the tip of the copy
///
/// # Errors
/// When `base..revision` has no commit, or git cannot write the copy
pub fn earlier_build(repo_path: &Path, base: &str, revision: &str) -> Result<String> {
  const FIELDS: &str = "%T%x00%an%x00%ae%x00%at%x00%ct%x00%B%x01";
  let log = run(
    repo_path,
    &["log", "--reverse", &format!("--format={FIELDS}"), "--end-of-options", &format!("{base}..{revision}")],
  )?;
  let mut parent = run(repo_path, &["rev-parse", "--verify", "--end-of-options", &format!("{base}^{{commit}}")])?
    .trim()
    .to_string();
  let mut copied = 0;
  for entry in log.split('\u{1}').map(|entry| entry.trim_start_matches('\n')).filter(|entry| !entry.is_empty()) {
    let [tree, name, email, author_time, committer_time, message] = entry.splitn(6, '\0').collect::<Vec<_>>()[..] else {
      bail!("an unexpected log entry: {entry:?}");
    };
    let earlier = committer_time.parse::<i64>()? - 86_400;
    let mut child = git_command()
      .args(["commit-tree", tree, "-p", &parent, "-F", "-"])
      .env("GIT_AUTHOR_NAME", name)
      .env("GIT_AUTHOR_EMAIL", email)
      .env("GIT_AUTHOR_DATE", format!("{author_time} +0000"))
      .env("GIT_COMMITTER_DATE", format!("{earlier} +0000"))
      .current_dir(repo_path)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .context("cannot start git commit-tree")?;
    child.stdin.take().context("piped stdin")?.write_all(message.as_bytes())?;
    let output = child.wait_with_output()?;
    ensure!(output.status.success(), "git commit-tree failed: {}", String::from_utf8_lossy(&output.stderr));
    parent = String::from_utf8(output.stdout)?.trim().to_string();
    copied += 1;
  }
  ensure!(copied > 0, "{base}..{revision} has no commit to copy");
  Ok(parent)
}

/// A commit someone else pushed: its subject, its author (`Name <email>`) and time (Unix seconds, also the committer's),
/// and the files it writes
#[derive(Debug, Clone)]
pub struct RemoteCommit {
  pub subject: String,
  pub author: String,
  pub timestamp: i64,
  pub files: Vec<(String, String)>,
}

/// Writes `commit` onto `branch` of the remote of the repository at `repo_path`, as someone else's push does: the
/// remote-tracking ref stays where it was, until a fetch. One `fast-import` in the remote; the new commit
///
/// # Errors
/// When the remote has no `branch`, or `fast-import` refuses the commit
pub fn commit_on_remote(repo_path: &Path, branch: &str, commit: &RemoteCommit) -> Result<String> {
  ensure!(!commit.files.is_empty(), "a commit on the remote writes a file: `git commit` refuses an empty one");
  let when = format!("{} +0000", commit.timestamp);
  let message = format!("{}\n", commit.subject);
  let mut stream = format!(
    "commit refs/heads/{branch}\nmark :1\nauthor {author} {when}\ncommitter {author} {when}\ndata {}\n{message}from refs/heads/{branch}^0\n",
    message.len(),
    author = commit.author,
  );
  for (path, content) in &commit.files {
    let _ = write!(stream, "M 100644 inline {path}\ndata {}\n{content}\n", content.len());
  }
  stream.push_str("\nget-mark :1\ndone\n");
  let mut child = git_command()
    .args(["fast-import", "--quiet", "--done", "--date-format=raw"])
    .current_dir(existing_origin(repo_path)?)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .context("cannot start git fast-import")?;
  child.stdin.take().context("piped stdin")?.write_all(stream.as_bytes())?;
  let output = child.wait_with_output()?;
  if !output.status.success() {
    bail!("git fast-import failed: {}", String::from_utf8_lossy(&output.stderr));
  }
  Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

/// The remote of the repository at `repo_path`, which its template gave it
fn existing_origin(repo_path: &Path) -> Result<PathBuf> {
  let origin = bare_origin(repo_path);
  ensure!(origin.is_dir(), "the repository at {} has no remote of its own ({BARE_ORIGIN})", repo_path.display());
  Ok(origin)
}

/// `git <args>` in `dir`: its stdout
fn run(dir: &Path, args: &[&str]) -> Result<String> {
  let output = git_command().args(args).current_dir(dir).output()?;
  ensure!(output.status.success(), "git {} failed: {}", args.join(" "), String::from_utf8_lossy(&output.stderr));
  Ok(String::from_utf8(output.stdout)?)
}

#[cfg(test)]
#[path = "remote_git_test.rs"]
mod remote_git_test;
