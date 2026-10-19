//! The blocks of `bd commits --patch` and `bd move-hunk`: per changed file the blocks of the diff views under `Default`
//! (`file_diffs::patch`), each with the id that `bd move-hunk --block` takes.

use file_diffs::blocks::BlobPair;
use file_diffs::patch::BlockPatch;
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use object_cache::commit_diff::FileChange;
use schemars::JsonSchema;
use serde::Serialize;
use sync_core::diff_scan::{CommitDiff, blob_pair};
use text_diff::model::BlockKind;

/// A changed file of a commit with its blocks
#[derive(Debug, Serialize, JsonSchema)]
pub struct PatchFile {
  pub path: String,
  /// The path before a rename
  #[serde(skip_serializing_if = "Option::is_none")]
  pub old_path: Option<String>,
  /// `A`, `M`, `D`, `R` (rename) or `T` (type change)
  pub status: String,
  /// No text to compare (binary, a symlink, a submodule, a type change, over 8 MiB): no blocks, and `bd move-hunk`
  /// moves the whole file
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub binary: bool,
  /// In the order of the file. Empty when only the mode or the line ends changed
  pub blocks: Vec<PatchBlock>,
}

/// A block, as the diff views of the app show it
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PatchBlock {
  /// `12,3:12,5@1a2b3c4d`, for `bd move-hunk --block`: the lines of the block and a hash of them, so that the id of a
  /// block that changed since fails
  pub id: String,
  /// The first old line, 1-based as in a git hunk; for a block without old lines the line before them
  pub old_start: u32,
  pub old_lines: u32,
  /// The first new line, as `old_start`
  pub new_start: u32,
  pub new_lines: u32,
  pub kind: PatchBlockKind,
  /// `@@ -10,7 +10,8 @@ Commits › read`: the lines of `text`, and the declaration that holds the block
  pub header: String,
  /// The lines of the block, `-` old and `+` new, between up to three lines of context. A block is never merged with
  /// a neighbour: the context stops at the next block
  pub text: String,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PatchBlockKind {
  Changed,
  /// The lines differ in whitespace or in the final line end only
  WhitespaceOnly,
}

impl From<&BlockPatch> for PatchBlock {
  fn from(patch: &BlockPatch) -> Self {
    let block = patch.id.block;
    let line = |start: usize, end: usize| u32::try_from(if start == end { start } else { start + 1 }).unwrap_or(u32::MAX);
    let count = |start: usize, end: usize| u32::try_from(end - start).unwrap_or(u32::MAX);
    Self {
      id: patch.id.to_string(),
      old_start: line(block.old_start, block.old_end),
      old_lines: count(block.old_start, block.old_end),
      new_start: line(block.new_start, block.new_end),
      new_lines: count(block.new_start, block.new_end),
      kind: if patch.kind == BlockKind::WhitespaceOnly {
        PatchBlockKind::WhitespaceOnly
      } else {
        PatchBlockKind::Changed
      },
      header: patch.header.clone(),
      text: patch.text.clone(),
    }
  }
}

/// Whether a file has text to compare: what the scan compared (`diff_scan`)
const fn has_text(change: &FileChange) -> bool {
  !change.binary && change.status != 'T' && (change.old_blob.is_some() || change.new_blob.is_some())
}

/// The files of each commit of `diffs` with their blocks, in the order of `diffs`: one `cat-file --batch` of the blobs
pub fn read_patches(git: &GitCommandExecutor, repository: &str, cache: &ObjectCache, diffs: &[&CommitDiff]) -> anyhow::Result<Vec<Vec<PatchFile>>> {
  let pairs: Vec<BlobPair<'_>> = diffs.iter().flat_map(|diff| &diff.files).filter(|change| has_text(change)).map(blob_pair).collect();
  let mut patches = FileDiffs::default().patches_of(git, repository, cache, &pairs)?.into_iter();
  Ok(
    diffs
      .iter()
      .map(|diff| {
        diff
          .files
          .iter()
          .map(|change| {
            let blocks = if has_text(change) { patches.next().flatten() } else { None };
            PatchFile {
              path: change.path.clone(),
              old_path: change.old_path.clone(),
              status: change.status.to_string(),
              binary: blocks.is_none(),
              blocks: blocks.iter().flatten().map(PatchBlock::from).collect(),
            }
          })
          .collect()
      })
      .collect(),
  )
}
