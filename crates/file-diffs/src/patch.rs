//! The blocks of files as `bd commits --patch` and `bd move-hunk` print them: the blocks of the diff views under
//! `Default`, each with its id (`text_diff::apply::BlockId`), a hunk header with the label of its scope, and its lines
//! between up to three lines of context. A block is never merged with its neighbour, so each id moves one block. And the
//! lines of the blocks alone, for the dangling-reference check (`sync_core::dangling_references`).

use crate::blocks::{BlobPair, PairDiff};
use crate::model::BlobText;
use crate::scopes::scopes_of_summary;
use crate::service::{FileDiffs, spawn_workers};
use anyhow::Result;
use code_symbols::extract::summarize;
use code_symbols::language::language_of;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use text_diff::apply::{BlockId, LineBlock, split_lines};
use text_diff::model::{BlockKind, DiffBlock, Scope, TextDiff};

/// The lines of context on each side of a block
const CONTEXT_LINES: usize = 3;

/// A block of a file, ready to print
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockPatch {
  pub id: BlockId,
  pub kind: BlockKind,
  /// `@@ -10,7 +10,8 @@ Commits › read`: the lines of `text` as git counts them, and the label of the innermost scope
  /// that holds the block
  pub header: String,
  /// The lines of the block, `-` old and `+` new, between up to three lines of context (` `), each without its line
  /// end. A final line end added or removed is a block on the empty last line
  pub text: String,
}

/// The lines of the blocks of a pair, each without its line end
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedLines {
  pub removed: Vec<String>,
  pub added: Vec<String>,
}

impl FileDiffs {
  /// The blocks of each pair, in the order of `pairs`; `None` for a pair without text (binary, over `MAX_BLOB_BYTES`, a
  /// blob git cannot read). One `cat-file --batch` of the blobs that memory lacks
  pub fn patches_of(&self, git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, pairs: &[BlobPair<'_>]) -> Result<Vec<Option<Vec<BlockPatch>>>> {
    self.with_texts(git, repository_path, cache, pairs, file_patch)
  }

  /// The lines of the blocks of each pair, as `patches_of` has them, without the context
  pub fn changed_lines_of(&self, git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, pairs: &[BlobPair<'_>]) -> Result<Vec<Option<ChangedLines>>> {
    self.with_texts(git, repository_path, cache, pairs, |_, diff, old, new| changed_lines(diff, old, new))
  }

  /// `work` on the diff and the texts of each pair with text, on the workers, in the order of `pairs`
  fn with_texts<T: Send>(
    &self,
    git: &GitCommandExecutor,
    repository_path: &str,
    cache: &ObjectCache,
    pairs: &[BlobPair<'_>],
    work: impl Fn(&BlobPair<'_>, &TextDiff, &str, &str) -> T + Sync,
  ) -> Result<Vec<Option<T>>> {
    let texts = self.read_blobs(git, repository_path, pairs.iter().flat_map(|pair| [pair.old_id, pair.new_id]).flatten())?;
    // the blobs are in memory now, so the diffs that no cache has read none again
    let diffs = self.blocks_of(git, repository_path, cache, pairs)?;
    let text = |id: Option<&str>| match id.and_then(|id| texts.get(id)).map(AsRef::as_ref) {
      None if id.is_none() => Some(""),
      Some(BlobText::Text { text, .. }) => Some(text.as_str()),
      _ => None,
    };

    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<T>>> = Mutex::new(std::iter::repeat_with(|| None).take(pairs.len()).collect());
    std::thread::scope(|scope| {
      spawn_workers(scope, pairs.len(), || {
        || {
          loop {
            let index = next.fetch_add(1, Ordering::Relaxed);
            let Some(pair) = pairs.get(index) else { return };
            let PairDiff::Text(diff) = &diffs[index] else { continue };
            let (Some(old), Some(new)) = (text(pair.old_id), text(pair.new_id)) else { continue };
            let result = work(pair, diff, old, new);
            results.lock().unwrap_or_else(PoisonError::into_inner)[index] = Some(result);
          }
        }
      });
    });
    Ok(results.into_inner().unwrap_or_else(PoisonError::into_inner))
  }
}

/// The lines of the blocks of the diff of `old` and `new`
fn changed_lines(diff: &TextDiff, old: &str, new: &str) -> ChangedLines {
  let (old_lines, new_lines) = (split_lines(old.as_bytes()), split_lines(new.as_bytes()));
  let mut lines = ChangedLines {
    removed: Vec::new(),
    added: Vec::new(),
  };
  for block in diff.blocks.iter().filter(|block| block.kind != BlockKind::Ignored).map(LineBlock::from) {
    lines.removed.extend(old_lines[block.old_start..block.old_end].iter().map(|line| line_text(line)));
    lines.added.extend(new_lines[block.new_start..block.new_end].iter().map(|line| line_text(line)));
  }
  lines
}

/// A line without its line end
fn line_text(line: &[u8]) -> String {
  String::from_utf8_lossy(line).trim_end_matches(['\n', '\r']).to_string()
}

/// The blocks of one pair, with the scopes of its texts when the diff has none
fn file_patch(pair: &BlobPair<'_>, diff: &TextDiff, old: &str, new: &str) -> Vec<BlockPatch> {
  if !diff.old_scopes.is_empty() || !diff.new_scopes.is_empty() {
    return block_patches(old, new, diff, &diff.old_scopes, &diff.new_scopes);
  }
  let parse = |path: &str, text: &str| language_of(path).map_or_else(Vec::new, |language| scopes_of_summary(&summarize(language, text.as_bytes())));
  let old_scopes = parse(pair.old_path.unwrap_or(pair.path), old);
  block_patches(old, new, diff, &old_scopes, &parse(pair.path, new))
}

/// The blocks of the diff of `old` and `new`, in order
pub(crate) fn block_patches(old: &str, new: &str, diff: &TextDiff, old_scopes: &[Scope], new_scopes: &[Scope]) -> Vec<BlockPatch> {
  let old_lines = split_lines(old.as_bytes());
  let new_lines = split_lines(new.as_bytes());
  let blocks: Vec<&DiffBlock> = diff.blocks.iter().filter(|block| block.kind != BlockKind::Ignored).collect();
  blocks
    .iter()
    .enumerate()
    .map(|(index, block)| {
      let lines = LineBlock::from(*block);
      // the context stops at a neighbour block; the lines between two blocks are the same on both sides
      let previous_end = index.checked_sub(1).map_or(0, |previous| blocks[previous].old_end as usize);
      let next_start = blocks.get(index + 1).map_or(old_lines.len(), |next| next.old_start as usize);
      let before = CONTEXT_LINES.min(lines.old_start - previous_end);
      let mut after = CONTEXT_LINES.min(next_start - lines.old_end);
      // the empty last line after a final line end is no context
      if after > 0 && lines.new_end + after == new_lines.len() && new_lines.last().is_some_and(|line| line.is_empty()) {
        after -= 1;
      }

      let mut text = String::new();
      let mut push = |prefix: char, line: &[u8]| {
        let _ = writeln!(text, "{prefix}{}", line_text(line));
      };
      new_lines[lines.new_start - before..lines.new_start].iter().for_each(|line| push(' ', line));
      old_lines[lines.old_start..lines.old_end].iter().for_each(|line| push('-', line));
      new_lines[lines.new_start..lines.new_end].iter().for_each(|line| push('+', line));
      new_lines[lines.new_end..lines.new_end + after].iter().for_each(|line| push(' ', line));
      text.pop();

      let range = |start: usize, end: usize| {
        let count = end - start;
        format!("{},{count}", if count == 0 { start } else { start + 1 })
      };
      let mut header = format!(
        "@@ -{} +{} @@",
        range(lines.old_start - before, lines.old_end + after),
        range(lines.new_start - before, lines.new_end + after)
      );
      let label = if lines.new_start < lines.new_end {
        scope_label(new_scopes, lines.new_start)
      } else {
        scope_label(old_scopes, lines.old_start)
      };
      if let Some(label) = label {
        header.push(' ');
        header.push_str(label);
      }
      BlockPatch {
        id: BlockId::of_lines(&old_lines, &new_lines, lines),
        kind: block.kind,
        header,
        text,
      }
    })
    .collect()
}

/// The label of the innermost scope that holds the line `line`, as git labels a hunk by the function it starts in
fn scope_label(scopes: &[Scope], line: usize) -> Option<&str> {
  scopes
    .iter()
    .filter(|scope| (scope.start_line as usize..scope.end_line as usize).contains(&line))
    .min_by_key(|scope| scope.end_line - scope.start_line)
    .map(|scope| scope.label.as_str())
}
