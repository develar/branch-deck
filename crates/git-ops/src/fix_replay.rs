//! The fixes of a shown review replayed onto its head (`sync_core::review_head::extend_review`;
//! docs/design/git-operations.md, "Review head"), each the commits of a merged Safe Push past the target, objects only.
//!
//! - **A fix at once** where it applies: one `git replay` of `<source> ^<target>`, which drops a commit whose change the
//!   head has.
//! - **Commit by commit** where it conflicts: the commits of the fix and of the head past the target (one `rev-list
//!   --left-right`) and the commits of the target since the fork point of the head by their authors (one more), then a
//!   binary search finds the commit `c` that conflicts (`first_conflict`). One whose identity the head has already is a
//!   repeat after a crash: it is left out, so a repeat adds nothing though a later fix changed the files it took. Its landed commit `u` is the one of the target with its identity (`CommitKey`: the author, the
//!   author time, the subject), and each path git cannot merge (the stages of `merge-tree` and the paths of its
//!   `CONFLICT` messages, the `<path>~<side>` copies of a file/directory conflict among them) takes `u:<path>` (one
//!   `cat-file --batch-check`), or goes where `u` lacks it: the review shows the file as master has it after the fix,
//!   with what master changed in it since the review. The commit keeps the author and the message of `c`
//!   (`write_edited_commit`, unsigned: the caller signs the range once); one that changes nothing then is dropped, as
//!   `git replay` drops one that becomes empty. The rest of the fix goes on by `git replay` of `<source> ^<c>`.
//! - **A fix that doesn't apply**: the commit that conflicts has no landed commit of its identity (an autosquash folded
//!   it, a reword renamed it), or a path git cannot merge is not UTF-8: the head stays where the fix found it, and the
//!   next fixes go on from there.
//! - No temporary ref is left: one `RefTransaction` deletes those of the commits written, also on a failure.

use crate::cat_file_batch::object_ids_and_modes;
use crate::commit_signing::Signing;
use crate::commit_writer::{EditedCommit, TreeEdit, WrittenEdit, current_committer, discard_temp_refs, read_raw_commits, write_edited_commit};
use crate::landed_pairing::{CommitKey, WalkedCommit, ids_by_key, walk_commits};
use crate::merge_tree_output::{MergeTreeOutput, parse_merge_tree_output};
use crate::replay::{first_conflict, replay_of, replay_range};
use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{HashMap, HashSet};
use tracing::{debug, instrument};

/// A commit of a fix that conflicted and took the files git cannot merge from the commit that landed it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakenFiles {
  /// The commit of the fix
  pub commit: String,
  /// The commit of the target that landed it, whose files it took
  pub landed: String,
  /// The paths it took from `landed`, in git's order; the paths it deleted, which show nothing, are not named
  pub paths: Vec<String>,
}

/// What became of one fix
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixReplay {
  /// On the head; `taken` names the commits that took files from what landed them (none: it applied as it is)
  Applied { taken: Vec<TakenFiles> },
  /// It doesn't apply on the head (module docs): `commit` is its commit that conflicts
  NotApplied { commit: String },
}

/// The head after the fixes, and what became of each
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixesReplay {
  pub tip: String,
  /// One per source, in their order
  pub fixes: Vec<FixReplay>,
}

/// Replay the fixes `sources` (the source revisions of merged Safe Pushes, oldest first) past `target_ref` (the
/// remote-tracking ref of the target) onto `onto`, the head of a shown review (module docs). No temporary ref is left
///
/// # Errors
/// The failure of a git process
#[instrument(skip(git_executor))]
pub fn replay_fixes(git_executor: &GitCommandExecutor, repo_path: &str, onto: &str, sources: &[String], target_ref: &str) -> Result<FixesReplay> {
  let mut fixes = FixReplayer {
    git_executor,
    repo_path,
    onto,
    target_ref,
    committer: None,
    written: Vec::new(),
  };
  let replay = fixes.replay_all(sources);
  if let Some(first) = fixes.written.first() {
    discard_temp_refs(git_executor, repo_path, &first.temp_ref, |transaction| {
      for edit in &fixes.written {
        edit.delete_temp_ref(transaction);
      }
    });
  }
  replay
}

struct FixReplayer<'a> {
  git_executor: &'a GitCommandExecutor,
  repo_path: &'a str,
  /// The head the fixes go onto, whose fork point on the target is where their landed commits are looked for
  onto: &'a str,
  target_ref: &'a str,
  /// The committer of the commits written, read at the first
  committer: Option<String>,
  /// The commits written, each on a temporary ref
  written: Vec<WrittenEdit>,
}

impl FixReplayer<'_> {
  fn replay_all(&mut self, sources: &[String]) -> Result<FixesReplay> {
    let exclusion = format!("^{}", self.target_ref);
    let mut tip = self.onto.to_owned();
    let mut fixes = Vec::with_capacity(sources.len());
    for source in sources {
      let revs = [source.as_str(), exclusion.as_str()];
      if let Some(replayed) = replay_range(self.git_executor, self.repo_path, &tip, &revs).map_err(|error| anyhow!("Cannot replay {source} onto {tip}: {error}"))? {
        tip = replayed;
        fixes.push(FixReplay::Applied { taken: Vec::new() });
        continue;
      }
      let (fix, replayed) = self.replay_by_commits(source, &exclusion, &tip)?;
      if let Some(replayed) = replayed {
        tip = replayed;
      }
      fixes.push(fix);
    }
    Ok(FixesReplay { tip, fixes })
  }

  /// The fix `source` that conflicts onto `tip`, commit by commit (module docs); the new tip of one that applies
  fn replay_by_commits(&mut self, source: &str, exclusion: &str, tip: &str) -> Result<(FixReplay, Option<String>)> {
    let (git, repo_path) = (self.git_executor, self.repo_path);
    // the commits of the fix (`<`) and of the head (`>`) past the target in one walk
    let both = format!("{source}...{}", self.onto);
    let (commits, on_head): (Vec<WalkedCommit>, Vec<WalkedCommit>) = walk_commits(git, repo_path, &["--left-right", "--reverse"], &[&both, exclusion])?
      .into_iter()
      .partition(|commit| commit.mark == '<');
    let on_head: HashSet<CommitKey> = on_head.into_iter().map(|commit| commit.key).collect();
    if commits.is_empty() {
      bail!("The replay of {source} conflicts, but it has no commit past {}", self.target_ref);
    }
    let since_fork = format!("^{}", self.onto);
    let landed: HashMap<CommitKey, String> = ids_by_key(git, repo_path, commits.iter().map(|commit| commit.key.email.as_str()), &[self.target_ref, &since_fork])?;
    let ids: Vec<String> = commits.iter().map(|commit| commit.id.clone()).collect();
    let mut replay = replay_of(git, repo_path, &ids, &[]);
    let mut taken = Vec::new();
    let mut tip = tip.to_owned();
    let mut next = 0;
    loop {
      // the commits from `next` on conflict onto `tip`
      let (index, clean) = first_conflict(next, ids.len() - 1, tip, &mut replay)?;
      let commit = &commits[index];
      if on_head.contains(&commit.key) {
        // a repeat: the head took it already, with the files of what landed it
        debug!(commit = %commit.id, source, "a commit of a fix that conflicts is on the head already");
        tip = clean;
      } else if let Some(landed) = landed.get(&commit.key) {
        let Some((written, paths)) = self.take_landed(&commit.id, &clean, landed)? else {
          return Ok((FixReplay::NotApplied { commit: commit.id.clone() }, None));
        };
        tip = if written.tree == written.parent_tree { clean } else { written.commit.clone() };
        self.written.push(written);
        if !paths.is_empty() {
          taken.push(TakenFiles {
            commit: commit.id.clone(),
            landed: landed.clone(),
            paths,
          });
        }
      } else {
        debug!(commit = %commit.id, source, "a commit of a fix conflicts and the target has no commit of its identity");
        return Ok((FixReplay::NotApplied { commit: commit.id.clone() }, None));
      }
      next = index + 1;
      if next == ids.len() {
        return Ok((FixReplay::Applied { taken }, Some(tip)));
      }
      let after = format!("^{}", commit.id);
      if let Some(replayed) = replay_range(git, repo_path, &tip, &[source, &after]).map_err(|error| anyhow!("Cannot replay {source} onto {tip}: {error}"))? {
        return Ok((FixReplay::Applied { taken }, Some(replayed)));
      }
    }
  }

  /// The cherry-pick of `commit` onto `onto` with every path git cannot merge as `landed` has it (module docs), written on
  /// a temporary ref, and the paths it took; `None` when a path is not UTF-8
  fn take_landed(&mut self, commit: &str, onto: &str, landed: &str) -> Result<Option<(WrittenEdit, Vec<String>)>> {
    let (git, repo_path) = (self.git_executor, self.repo_path);
    let parent = format!("{commit}^");
    let output = git
      .cmd(&["merge-tree", "--write-tree", "-z", "--merge-base", &parent, onto, commit], repo_path)
      .accept(&[1])
      .output()?;
    let merge = parse_merge_tree_output(&output.stdout)?;
    let Some(paths) = unmerged_paths(&merge) else {
      debug!(commit, "a path git cannot merge is not UTF-8: the fix doesn't apply");
      return Ok(None);
    };
    let names: Vec<String> = paths.iter().map(|path| format!("{landed}:{path}")).collect();
    let entries = object_ids_and_modes(git, repo_path, &names)?;
    // the deletes first: a path that `landed` has under one that goes stays
    let mut edits: Vec<TreeEdit<'_>> = paths
      .iter()
      .zip(&entries)
      .filter(|(_, entry)| entry.is_none())
      .map(|(path, _)| TreeEdit::Delete { path })
      .collect();
    edits.extend(
      paths
        .iter()
        .zip(&entries)
        .filter_map(|(path, entry)| entry.as_ref().map(|(id, mode)| TreeEdit::Blob { path, mode, id })),
    );
    let raw = read_raw_commits(git, repo_path, &[commit])?.remove(commit).ok_or_else(|| anyhow!("no commit {commit}"))?;
    let committer = match &self.committer {
      Some(committer) => committer.clone(),
      None => self.committer.insert(current_committer(git, repo_path)?).clone(),
    };
    let written = write_edited_commit(
      git,
      repo_path,
      &EditedCommit {
        tree: &merge.tree,
        edits: &edits,
        parent: onto,
        author: &raw.author,
        committer: &committer,
        encoding: raw.encoding.as_deref(),
        message: &raw.message,
      },
      Signing::Off,
    )?;
    let taken = paths.iter().zip(&entries).filter(|(_, entry)| entry.is_some()).map(|(path, _)| path.clone()).collect();
    debug!(commit, landed, "a commit of a fix took the files it conflicts in from the commit that landed it");
    Ok(Some((written, taken)))
  }
}

/// The paths git cannot merge (module docs), each once, the stages first in git's order; `None` when one is not UTF-8
fn unmerged_paths(merge: &MergeTreeOutput) -> Option<Vec<String>> {
  let conflicts = merge.messages.iter().filter(|message| message.kind.starts_with("CONFLICT"));
  let mut paths: Vec<String> = Vec::new();
  for path in merge
    .conflicted_paths()
    .into_iter()
    .chain(conflicts.flat_map(|message| message.paths.iter().map(Vec::as_slice)))
  {
    let path = std::str::from_utf8(path).ok()?;
    if !paths.iter().any(|known| known == path) {
      paths.push(path.to_owned());
    }
  }
  Some(paths)
}
