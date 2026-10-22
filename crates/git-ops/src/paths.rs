//! Which paths differ between two trees, and which commits change some paths: the path facts of an update and its resolve rounds.

use anyhow::{Result, anyhow, bail};
use bstr::ByteSlice;
use domain_types::update_project::UpdateCommit;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_utils::list_cut::kept;

/// The likely conflicting files that get commits attributed, and the ones a check reports (51 when only one more:
/// `list_cut::kept`): each is a pathspec on the command line, and Windows limits it to 32K characters. A check counts
/// the rest (`file_count`)
pub(crate) const MAX_ATTRIBUTED_FILES: usize = 50;

/// `\x01` starts a commit, NUL ends a field; `%s` holds neither
pub(crate) const COMMIT_FORMAT: &str = "--format=%x01%H%x00%an%x00%at%x00%cn%x00%ct%x00%s";

/// The non-merge commits of `revs` that change any of `files` (the first `MAX_ATTRIBUTED_FILES`, cut as a check cuts
/// them), oldest first, each
/// with the files of the list it changes; the newest `limit` only, if given
pub(crate) fn commits_touching(git_executor: &GitCommandExecutor, repo_path: &str, revs: &[&str], files: &[String], limit: Option<usize>) -> Result<Vec<UpdateCommit>> {
  let max_count = limit.map(|limit| format!("--max-count={limit}"));
  // the paths are names, not globs
  let mut args = vec!["--literal-pathspecs", "log", "--reverse", "--no-merges", "--no-renames", "--name-only", "-z", COMMIT_FORMAT];
  args.extend(max_count.as_deref());
  args.extend_from_slice(revs);
  args.push("--");
  args.extend(files.iter().take(kept(files.len(), MAX_ATTRIBUTED_FILES)).map(String::as_str));
  let output = git_executor.execute_command(&args, repo_path)?;
  Ok(parse_commits(&output))
}

/// `\x01<id>\0<author>\0<author time>\0<committer>\0<committer time>\0<subject>\0`, then with `--name-only` a newline
/// and `<file>\0` per file, per commit
pub(crate) fn parse_commits(output: &str) -> Vec<UpdateCommit> {
  output
    .split('\x01')
    .filter(|record| !record.is_empty())
    .map(|record| {
      let mut fields = record.split('\0');
      let mut next = || fields.next().unwrap_or_default().to_string();
      let (id, author, time, committer, committer_time, subject) = (next(), next(), next(), next(), next(), next());
      let files = fields
        .map(|file| file.trim_start_matches('\n'))
        .filter(|file| !file.is_empty())
        .map(str::to_string)
        .collect();
      UpdateCommit {
        id,
        subject,
        author,
        time: time.parse().unwrap_or_default(),
        committer,
        committer_time: committer_time.parse().unwrap_or_default(),
        files,
      }
    })
    .collect()
}

/// A path that differs between two trees, with its side in the old tree
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChangedPath {
  /// `A`, `D`, `M` or `T`: renames are a delete and an add
  pub(crate) status: char,
  /// The mode and the blob in the old tree, `000000` and the null id for an added path
  pub(crate) old_mode: String,
  pub(crate) old_id: String,
  /// The blob in the new tree, the null id for a deleted path
  pub(crate) new_id: String,
  /// As git stores it, not necessarily UTF-8: it names the index entry and the file
  pub(crate) path: Vec<u8>,
}

impl ChangedPath {
  /// The path is new in the new tree
  pub(crate) const fn was_added(&self) -> bool {
    self.status == 'A'
  }

  /// The blob in the old tree
  pub(crate) fn old_id(&self) -> &str {
    &self.old_id
  }
}

/// Every path that differs between two tree-ish (`diff-tree -r -z --no-renames`, raw format), the paths as bytes
pub(crate) fn changed_paths(git_executor: &GitCommandExecutor, repo_path: &str, from: &str, to: &str) -> Result<Vec<ChangedPath>> {
  let output = git_executor
    .cmd(&["diff-tree", "-r", "-z", "--no-renames", "--end-of-options", from, to], repo_path)
    .bytes()?;
  let mut fields = output.split(|&b| b == 0).filter(|field| !field.is_empty());
  let mut paths = Vec::new();
  // `:<old mode> <new mode> <old id> <new id> <status>`, then the path
  while let Some(meta) = fields.next() {
    let path = fields.next().ok_or_else(|| anyhow!("Unexpected diff-tree output: no path after {:?}", meta.as_bstr()))?;
    let mut meta = meta.to_str()?.trim_start_matches(':').split(' ');
    let (Some(old_mode), Some(_), Some(old_id), Some(new_id), Some(status)) = (meta.next(), meta.next(), meta.next(), meta.next(), meta.next()) else {
      bail!("Unexpected diff-tree output: {:?}", output.as_bstr());
    };
    paths.push(ChangedPath {
      status: status.chars().next().unwrap_or('M'),
      old_mode: old_mode.to_string(),
      old_id: old_id.to_string(),
      new_id: new_id.to_string(),
      path: path.to_vec(),
    });
  }
  Ok(paths)
}
