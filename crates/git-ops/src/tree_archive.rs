//! The files of a commit's tree as a tar (`git archive`), for a caller that extracts them outside every working tree
//! (`bd check --build`).

use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::io::Write;

/// Stream the tar of the tree of `commit` (a full id), only `paths` if given, into `out`: one `git archive --format=tar`,
/// written as it arrives, never held whole
pub fn write_tree_archive(git_executor: &GitCommandExecutor, repository_path: &str, commit: &str, paths: Option<&[&str]>, out: &mut impl Write) -> Result<()> {
  let mut args = vec!["archive", "--format=tar", commit];
  if let Some(paths) = paths {
    args.push("--");
    args.extend(paths);
  }
  git_executor.execute_command_streaming(&args, repository_path, |chunk| out.write_all(chunk).map_err(anyhow::Error::from))
}

#[cfg(test)]
#[path = "tree_archive_test.rs"]
mod tree_archive_test;
