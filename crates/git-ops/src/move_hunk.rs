//! Moving the changes of one file from one commit of the checked out branch into another (`bd move-hunk`), without
//! touching the working tree or the index (docs/design/git-operations.md, "Move a hunk"). HEAD's tree never changes, by
//! construction: a move into an older commit adds the change to the trees of `[to, from)` and `from` keeps its tree (it
//! now holds only the rest); a move into a newer commit takes it out of the trees of `[from, to)` and `to` keeps its
//! tree. The commits above the newer one get new parents only. One `merge-tree --stdin` computes every new tree, one
//! `fast-import` writes the commits and their notes, one ref transaction moves the branch: a fixed number of git
//! processes, whatever the length of the range.

use crate::cat_file_batch::{BatchEntry, NO_LIMIT, read_objects};
use crate::commit_utils::empty_tree;
use crate::commit_writer::{EditedCommit, NewCommit, NewParent, RawCommit, TreeEdit, WrittenEdit, current_committer, discard_temp_refs, read_raw_commits, write_edited_commit};
use crate::history::{History, HistoryCommit};
use crate::merge_tree_output::parse_merge_tree_batch;
use crate::notes::{MappingNote, NoteSource, mapped_commit_id, mapped_source, write_commits_with_mapping_notes};
use crate::ref_transaction::RefTransaction;
use crate::revs::{checked_out_branch, short_id};
use crate::rewrite_guard::HeadMoved;
use anyhow::anyhow;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use std::fmt::Write as _;
use text_diff::apply::{LineBlock, apply_blocks};
use tracing::{info, instrument};

/// The reflog message of the branch move (`branch-deck: move hunk`)
const OPERATION: &str = "move hunk";

/// What of the file's change in `from` moves
#[derive(Debug, Clone, Copy)]
pub enum MovedChange<'a> {
  /// The whole change of the file: its content and its existence (an added or a deleted file moves as one), also of a
  /// binary file
  WholeFile,
  /// These blocks of the diff of the file's two sides in `from` (`text_diff::model::diff_texts`, as the diff views and
  /// `bd commits --patch` show them), not all of them, in any order. The file exists on both sides of `from`
  Blocks(&'a [LineBlock]),
}

#[derive(Debug, Clone, Copy)]
pub struct MoveHunkRequest<'a> {
  /// Full commit ids on the first-parent history of HEAD ahead of `baseline_branch`; different commits
  pub from: &'a str,
  pub to: &'a str,
  /// The file, relative to the repository root, as git stores it
  pub path: &'a str,
  pub change: MovedChange<'a>,
  /// The HEAD the request was made for
  pub expected_head: &'a str,
  /// The branch the commits are ahead of; the history is read down to it
  pub baseline_branch: &'a str,
  pub mode: MoveHunkMode,
}

/// How far a move goes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveHunkMode {
  /// Write the commits and their notes, move the branch
  Apply,
  /// Compute and check the new trees (objects only), move no ref and write no note
  DryRun,
  /// Also write the new commits (objects only, without notes), and move no ref: a trial build of the rewritten history
  /// (`bd check` proving a fix) builds them
  Trial,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveHunkOutcome {
  pub previous_head: String,
  /// HEAD afterwards; `previous_head` for a dry run; for a trial the newest written commit, which no ref names
  pub new_head: String,
  /// Old -> new id of every recreated commit, oldest first: the older of `from` and `to` and every commit above it;
  /// empty for a dry run
  pub commits: Vec<(String, String)>,
  /// The tree of each commit of `commits`, in their order
  pub trees: Vec<String>,
  /// The commits whose changes differ afterwards, oldest first, by their old ids: `from`, `to` and the commits between
  /// whose diff of the file changed. They get the current user as committer and no mapping note
  pub changed: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum MoveHunkError {
  /// The request can't be applied as asked: nothing was written
  #[error("{0}")]
  Refused(String),
  /// The change doesn't apply to the file as `commit` has it (a commit between the two changes the same lines, or the
  /// file doesn't exist there); nothing was written
  #[error("Moving the change of {path} conflicts with commit {}: it changes the same lines, or the file doesn't exist there", short_id(.commit))]
  Conflict { commit: String, path: String },
  #[error(transparent)]
  HeadMoved(HeadMoved),
  #[error("{0:#}")]
  Other(#[from] anyhow::Error),
}

/// One side of the file's change in `from`: `None` where the file doesn't exist
#[derive(Debug, Clone, PartialEq, Eq)]
struct FileEntry {
  mode: String,
  id: String,
}

/// The file before and after `from`, from one `diff-tree --raw` without rename detection
fn read_file_change(git_executor: &GitCommandExecutor, repo_path: &str, from: &str, path: &str) -> Result<(Option<FileEntry>, Option<FileEntry>), MoveHunkError> {
  let output = git_executor
    .cmd(
      &[
        "--literal-pathspecs",
        "diff-tree",
        "-r",
        "-z",
        "--raw",
        "--no-renames",
        "--no-commit-id",
        "--root",
        "--end-of-options",
        from,
        "--",
        path,
      ],
      repo_path,
    )
    .bytes()?;
  let output = String::from_utf8_lossy(&output);
  // `:<old mode> <new mode> <old id> <new id> <status>\0<path>\0`
  let mut fields = output.split('\0');
  let meta = fields.next().unwrap_or_default();
  let Some(meta) = meta.strip_prefix(':') else {
    return Err(MoveHunkError::Refused(format!("Commit {} doesn't change {path}", short_id(from))));
  };
  let parts: Vec<&str> = meta.split(' ').collect();
  let [old_mode, new_mode, old_id, new_id, status] = parts[..] else {
    return Err(MoveHunkError::Other(anyhow!("Unexpected diff-tree output: {meta:?}")));
  };
  if fields.next() != Some(path) || fields.next().is_some_and(|next| next.starts_with(':')) {
    return Err(MoveHunkError::Refused(format!("{path} is not a file that commit {} changes", short_id(from))));
  }
  let what = |mode: &str| match mode {
    "120000" => Some("a symlink"),
    "160000" => Some("a submodule"),
    _ => None,
  };
  if let Some(what) = what(old_mode).or_else(|| what(new_mode)) {
    return Err(MoveHunkError::Refused(format!("{path} is {what}: only the changes of regular files move")));
  }
  if status == "T" || (old_mode != "000000" && new_mode != "000000" && old_mode != new_mode) {
    return Err(MoveHunkError::Refused(format!(
      "Commit {} changes the mode of {path} ({old_mode} -> {new_mode}): only content changes move",
      short_id(from)
    )));
  }
  let entry = |mode: &str, id: &str| {
    (mode != "000000").then(|| FileEntry {
      mode: mode.to_string(),
      id: id.to_string(),
    })
  };
  Ok((entry(old_mode, old_id), entry(new_mode, new_id)))
}

/// The bytes of blobs, with one `cat-file --batch`
fn read_blobs(git_executor: &GitCommandExecutor, repo_path: &str, ids: &[&str]) -> anyhow::Result<Vec<Vec<u8>>> {
  read_objects(git_executor, repo_path, ids, NO_LIMIT, |entry| match entry {
    BatchEntry::Object { kind: "blob", content, .. } => Some(content),
    _ => None,
  })?
  .into_iter()
  .zip(ids)
  .map(|(content, id)| content.ok_or_else(|| anyhow!("Blob {id} not found")))
  .collect()
}

/// Move the change, as `request` names it, from one commit into another, in one rewrite of the checked out branch that
/// keeps HEAD's tree. Refused, with nothing written but git objects, if the change conflicts with a commit of the range,
/// if a commit would be left without changes, or if HEAD is not `expected_head`
#[instrument(skip(git_executor, request), fields(from = %request.from, to = %request.to, path = %request.path, mode = ?request.mode))]
pub fn move_hunk(git_executor: &GitCommandExecutor, repo_path: &str, request: &MoveHunkRequest<'_>) -> Result<MoveHunkOutcome, MoveHunkError> {
  if request.from == request.to {
    return Err(MoveHunkError::Refused("The change is already in that commit: name two different commits".to_string()));
  }
  if let MovedChange::Blocks([]) = request.change {
    return Err(MoveHunkError::Refused("Name at least one block".to_string()));
  }
  let history = History::read_ahead_of(git_executor, repo_path, request.baseline_branch, true)?;
  let head = history.head().unwrap_or_default().to_string();
  let head_moved = |actual: String| {
    MoveHunkError::HeadMoved(HeadMoved {
      branch: checked_out_branch(git_executor, repo_path).unwrap_or_else(|_| "HEAD".to_string()),
      expected: request.expected_head.to_string(),
      actual,
    })
  };
  if head != request.expected_head {
    return Err(head_moved(head));
  }
  let position = |id: &str| {
    history
      .find(id)
      .ok_or_else(|| MoveHunkError::Refused(format!("Commit {} is not ahead of {} on the checked out branch", short_id(id), request.baseline_branch)))
  };
  // newest first: the older commit has the higher position
  let (from_position, to_position) = (position(request.from)?, position(request.to)?);
  let oldest_position = from_position.max(to_position);
  let target = history.check_rewrite(git_executor, repo_path, oldest_position)?;
  if target.head != head {
    return Err(head_moved(target.head));
  }

  // oldest first from here on: index 0 is the older of `from` and `to`
  let commits: Vec<&HistoryCommit> = history.commits()[..=oldest_position].iter().rev().collect();
  let ids: Vec<&str> = commits.iter().map(|commit| commit.id.as_str()).collect();
  let from_index = oldest_position - from_position;
  let to_index = oldest_position - to_position;
  let forward = to_index < from_index;
  let raw_commits = read_raw_commits(git_executor, repo_path, &ids)?;
  let raw = |index: usize| -> &RawCommit { &raw_commits[ids[index]] };

  let (old_entry, new_entry) = read_file_change(git_executor, repo_path, request.from, request.path)?;
  // forward the change is applied to the file before `from`, backward taken out of the file after it
  let content;
  let edit = match request.change {
    MovedChange::WholeFile => match if forward { &new_entry } else { &old_entry } {
      Some(entry) => TreeEdit::Blob {
        path: request.path,
        mode: &entry.mode,
        id: &entry.id,
      },
      None => TreeEdit::Delete { path: request.path },
    },
    MovedChange::Blocks(blocks) => {
      let (Some(old), Some(new)) = (&old_entry, &new_entry) else {
        return Err(MoveHunkError::Refused(format!(
          "Commit {} adds or deletes {}: move the whole file",
          short_id(request.from),
          request.path
        )));
      };
      let blobs = read_blobs(git_executor, repo_path, &[&old.id, &new.id])?;
      // backward the blocks are taken back out of the new side
      let applied = if forward {
        apply_blocks(&blobs[0], &blobs[1], blocks)
      } else {
        let reversed: Vec<LineBlock> = blocks.iter().map(|block| block.reversed()).collect();
        apply_blocks(&blobs[1], &blobs[0], &reversed)
      };
      content = applied.map_err(|error| anyhow!("{error}"))?;
      TreeEdit::Content {
        path: request.path,
        mode: &old.mode,
        content: &content,
      }
    }
  };

  // the merge base and the commits whose trees change: [to, from) forward, [from, to) backward
  let (base_index, range) = if forward {
    (from_index - 1, to_index..from_index)
  } else {
    (from_index, from_index..to_index)
  };
  let committer = current_committer(git_executor, repo_path)?;
  let theirs = write_edited_commit(
    git_executor,
    repo_path,
    &EditedCommit {
      tree: &raw(base_index).tree,
      edits: &[edit],
      parent: request.from,
      author: &committer,
      committer: &committer,
      encoding: None,
      message: b"branch-deck: move hunk\n",
    },
  )?;
  let result = rewrite(
    git_executor,
    repo_path,
    request,
    Rewrite {
      commits: &commits,
      raw: &raw_commits,
      history: &history,
      range,
      base_tree: &raw(base_index).tree,
      theirs: &theirs,
      committer: &committer,
      target_ref: &target.full_ref(),
      head: &head,
    },
  );
  if request.mode != MoveHunkMode::Apply || result.is_err() {
    discard_temp_refs(git_executor, repo_path, &theirs.temp_ref, |transaction| theirs.delete_temp_ref(transaction));
  }
  result
}

/// What `rewrite` works on: the commits from the older of `from` and `to` up to HEAD, oldest first
struct Rewrite<'a> {
  commits: &'a [&'a HistoryCommit],
  raw: &'a HashMap<String, RawCommit>,
  history: &'a History,
  /// The indexes of the commits whose trees change
  range: std::ops::Range<usize>,
  base_tree: &'a str,
  theirs: &'a WrittenEdit,
  committer: &'a str,
  target_ref: &'a str,
  head: &'a str,
}

/// Compute the new trees and check them; unless a dry run, write the commits, and for `Apply` move the branch, deleting
/// the temporary ref of `theirs` in the same transaction
fn rewrite(git_executor: &GitCommandExecutor, repo_path: &str, request: &MoveHunkRequest<'_>, work: Rewrite<'_>) -> Result<MoveHunkOutcome, MoveHunkError> {
  let Rewrite {
    commits,
    raw,
    history,
    range,
    base_tree,
    theirs,
    committer,
    target_ref,
    head,
  } = work;
  let tree_of = |index: usize| -> &str { &raw[&commits[index].id].tree };

  // every commit of the range: the file's change applied to (or taken out of) its tree, one merge each
  let input = range.clone().fold(String::new(), |mut input, index| {
    let _ = writeln!(input, "{base_tree} -- {} {}", tree_of(index), theirs.tree);
    input
  });
  let output = git_executor
    .cmd(&["merge-tree", "--write-tree", "--stdin", "-z", "--no-messages", "-X", "no-renames"], repo_path)
    .stdin(input.as_bytes())
    .bytes()?;
  let merges = parse_merge_tree_batch(&output, range.len())?;
  if let Some((index, _)) = range.clone().zip(&merges).find(|(_, merge)| !merge.clean) {
    return Err(MoveHunkError::Conflict {
      commit: commits[index].id.clone(),
      path: request.path.to_string(),
    });
  }
  let mut new_trees: Vec<String> = (0..commits.len()).map(|index| tree_of(index).to_string()).collect();
  for (index, merge) in range.clone().zip(merges) {
    new_trees[index] = merge.output.tree;
  }

  // the tree of the commit below the oldest one, which keeps its place
  let first_parent = raw[&commits[0].id].parents.first();
  let below_tree = match first_parent {
    Some(parent) => history.tree_of(parent).ok_or_else(|| anyhow!("The tree of {} is unknown", short_id(parent)))?,
    None => empty_tree(head),
  };
  let old_parent_tree = |index: usize| if index == 0 { below_tree } else { tree_of(index - 1) };
  let new_parent_tree = |index: usize| if index == 0 { below_tree } else { new_trees[index - 1].as_str() };
  // the commits whose diff may change: the range and the commit above it
  let last = range.end;
  for index in range.start..=last {
    if new_trees[index] == new_parent_tree(index) && tree_of(index) != old_parent_tree(index) {
      let id = &commits[index].id;
      let hint = if *id == request.from { "; squash it instead" } else { "" };
      return Err(MoveHunkError::Refused(format!("Moving the change leaves commit {} without changes{hint}", short_id(id))));
    }
  }
  if new_trees.last().map(String::as_str) != Some(tree_of(commits.len() - 1)) {
    return Err(MoveHunkError::Other(anyhow!("The rewrite would change the tree of HEAD")));
  }

  // the commits between the two keep their committer and their note when their diff of the file stays the same
  let between: Vec<usize> = (range.start + 1..last).collect();
  let kept = same_file_diffs(git_executor, repo_path, request.path, &between, |index| {
    ((old_parent_tree(index), tree_of(index)), (new_parent_tree(index), new_trees[index].as_str()))
  })?;
  let changed: Vec<usize> = (range.start..=last).filter(|index| !kept.contains(index)).collect();
  let changed_ids: Vec<String> = changed.iter().map(|&index| commits[index].id.clone()).collect();
  if request.mode == MoveHunkMode::DryRun {
    return Ok(MoveHunkOutcome {
      previous_head: head.to_string(),
      new_head: head.to_string(),
      commits: Vec::new(),
      trees: Vec::new(),
      changed: changed_ids,
    });
  }

  let mut new_commits = Vec::with_capacity(commits.len());
  // (index in `new_commits`, note content)
  let mut notes = Vec::new();
  for (index, commit) in commits.iter().enumerate() {
    let raw_commit = &raw[&commit.id];
    let is_changed = changed.contains(&index);
    new_commits.push(NewCommit {
      tree: &new_trees[index],
      parent: if index == 0 {
        raw_commit.parents.first().map(|parent| NewParent::Existing(parent))
      } else {
        Some(NewParent::Written(index - 1))
      },
      author: &raw_commit.author,
      committer: if is_changed { committer } else { &raw_commit.committer },
      encoding: raw_commit.encoding.as_deref(),
      message: &raw_commit.message,
    });
    let Some(note) = commit.note.as_ref().filter(|_| request.mode == MoveHunkMode::Apply) else {
      continue;
    };
    if index > last {
      // its trees stay, so the note's source still holds
      notes.push((index, note.clone()));
    } else if !is_changed && mapped_source(note).is_none() {
      // a note without a source: the sync checks the tree of its virtual commit
      notes.push((index, note.clone()));
    } else if !is_changed && let Some(mapped) = mapped_commit_id(note) {
      // the same diff on other trees: the virtual commit stays, the source names the new trees
      let source = NoteSource {
        parent_tree: if index == 0 && first_parent.is_none() {
          String::new()
        } else {
          new_parent_tree(index).to_string()
        },
        tree: new_trees[index].clone(),
      };
      let note = MappingNote {
        original: commit.id.clone(),
        mapped: mapped.to_string(),
        source,
      };
      notes.push((index, note.content()));
    }
  }

  let written = write_commits_with_mapping_notes(git_executor, repo_path, &new_commits, notes, &[], Some(committer))?;
  let new_head = written.ids.last().cloned().ok_or_else(|| anyhow!("no commits were written"))?;
  let rewritten = commits.iter().map(|commit| commit.id.clone()).zip(written.ids.iter().cloned()).collect();
  if request.mode == MoveHunkMode::Trial {
    written.discard(git_executor, repo_path);
    return Ok(MoveHunkOutcome {
      previous_head: head.to_string(),
      new_head,
      commits: rewritten,
      trees: new_trees,
      changed: changed_ids,
    });
  }

  let mut transaction = RefTransaction::new();
  transaction.update(target_ref, &new_head, Some(head));
  theirs.delete_temp_ref(&mut transaction);
  written.delete_temp_refs(&mut transaction);
  if let Err(e) = transaction.commit(git_executor, repo_path, OPERATION) {
    written.discard(git_executor, repo_path);
    let actual = git_executor.execute_command(&["rev-parse", "HEAD"], repo_path).unwrap_or_default();
    if actual != head {
      return Err(MoveHunkError::HeadMoved(HeadMoved {
        branch: target_ref.trim_start_matches("refs/heads/").to_string(),
        expected: head.to_string(),
        actual,
      }));
    }
    return Err(MoveHunkError::Other(anyhow!(
      "The branch or a ref moved with it changed meanwhile, read it again and retry: {e:#}"
    )));
  }
  info!(commits = new_commits.len(), changed = changed.len(), "Moved the change of a file");
  Ok(MoveHunkOutcome {
    previous_head: head.to_string(),
    new_head,
    commits: rewritten,
    trees: new_trees,
    changed: changed_ids,
  })
}

/// The indexes of `indexes` whose diff of `path` is the same in both pairs `pairs(index)` gives, `((old parent tree, old
/// tree), (new parent tree, new tree))`: one `diff-tree --stdin -p -U0` of all pairs, one `patch-id --stable` (line
/// numbers and whitespace don't count, so a change that only moved keeps its id)
fn same_file_diffs<'a>(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  path: &str,
  indexes: &[usize],
  pairs: impl Fn(usize) -> ((&'a str, &'a str), (&'a str, &'a str)),
) -> anyhow::Result<Vec<usize>> {
  let mut kept = Vec::with_capacity(indexes.len());
  // the pairs to diff, two per index; an index whose pairs are equal keeps its diff without one
  let mut headers: Vec<String> = Vec::new();
  let mut compared = Vec::new();
  for &index in indexes {
    let (old, new) = pairs(index);
    if old == new {
      kept.push(index);
      continue;
    }
    compared.push(index);
    headers.push(format!("{} {}", old.0, old.1));
    headers.push(format!("{} {}", new.0, new.1));
  }
  if compared.is_empty() {
    return Ok(kept);
  }
  let mut input = headers.join("\n");
  input.push('\n');
  let output = git_executor
    .cmd(
      &[
        "--literal-pathspecs",
        "diff-tree",
        "--stdin",
        "-p",
        "-U0",
        "--no-renames",
        "--no-ext-diff",
        "--no-textconv",
        "--",
        path,
      ],
      repo_path,
    )
    .stdin(input.as_bytes())
    .bytes()?;

  // each pair prints its `<tree> <tree>` line, then its patch (none for no change); a patch line never is one
  let hash_length = headers[0].find(' ').unwrap_or(40);
  let mut patches = Vec::with_capacity(output.len() + headers.len() * (hash_length + 1));
  let mut next = 0;
  for line in output.split_inclusive(|&b| b == b'\n') {
    let text = line.strip_suffix(b"\n").unwrap_or(line);
    if let Some(offset) = headers[next..].iter().position(|header| header.as_bytes() == text) {
      next += offset + 1;
      // a header of its own per pair: `patch-id` names each patch by the first id of its header line
      patches.extend_from_slice(format!("{next:0>hash_length$x}\n").as_bytes());
      continue;
    }
    patches.extend_from_slice(line);
  }
  let ids = git_executor.cmd(&["patch-id", "--stable"], repo_path).stdin(&patches).bytes()?;
  // `<patch id> <pair number>` per pair with a change
  let ids = String::from_utf8_lossy(&ids);
  let patch_of: HashMap<usize, &str> = ids
    .lines()
    .filter_map(|line| line.split_once(' '))
    .filter_map(|(patch, pair)| Some((usize::from_str_radix(pair, 16).ok()?, patch)))
    .collect();
  for (position, &index) in compared.iter().enumerate() {
    // pair numbers count from 1
    if patch_of.get(&(position * 2 + 1)) == patch_of.get(&(position * 2 + 2)) {
      kept.push(index);
    }
  }
  kept.sort_unstable();
  Ok(kept)
}
