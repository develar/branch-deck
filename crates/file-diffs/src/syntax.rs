//! The tree-sitter highlighting of a blob for the diff views (docs/design/diff-view.md, "Tree-sitter highlighting"):
//! `code_symbols::highlight` of its text, made once per blob and kept in memory, answered as raw bytes for the lines a
//! view asks for.
//!
//! - **The text** comes from the blobs that `get_file_diffs` and `get_blob_texts` read (the memory cache), the working
//!   tree, or one `cat-file --batch`; a view sends the text of a blob it made itself (the result of a merge) and of one
//!   answered `Missing`.
//! - **Once per blob**: the highlighting is kept by grammar and blob id, and a second request for a blob in work waits
//!   for it (`get_with`), so the views of a window never parse a blob twice. Memory only: a reopened review parses again,
//!   2–15 ms for a typical file, cheaper than a write to the object cache.
//! - **Bounded**: at most half the cores parse at once, beside the workers of the diffs.
//! - **The answer**: `u32`s in little endian, a header (`SYNTAX_FORMAT`, the status, the line count of the text, the
//!   first and the end line answered, the number of runs), then the `line_starts` of the answered lines from 0 (one more
//!   than the lines), then the runs, pairs of (UTF-16 offset in the line, style) as the worker sends them.

use crate::model::{BlobText, GetBlobSyntaxParams};
use crate::service::FileDiffs;
use crate::worktree::read_worktree_blob;
use anyhow::Result;
use code_symbols::highlight::lines::LineRuns;
use code_symbols::highlight::{grammar_of, highlight};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::is_object_id;
use std::borrow::Cow;
use std::sync::{Arc, Condvar, LazyLock, Mutex, PoisonError};
use tracing::instrument;

/// The version of the answer's layout
pub const SYNTAX_FORMAT: u32 = 1;
/// The memory of the highlightings
pub const SYNTAX_CACHE_BYTES: u64 = 64 << 20;
/// The `u32`s of the header
const HEADER: usize = 6;

/// What an answer says of its blob
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxStatus {
  /// The runs of the lines asked for follow
  Runs = 0,
  /// No highlighting here: no grammar, a binary or too large a file, a parse over its budget; the view uses Shiki
  Fallback = 1,
  /// Git has no such blob and the request no text: the view sends the text
  Missing = 2,
}

/// The memory a highlighting takes in the cache
pub(crate) fn syntax_weight(runs: &Arc<Option<LineRuns>>) -> u32 {
  let units = runs.as_ref().as_ref().map_or(0, |runs| runs.line_starts.len() + runs.runs.len());
  u32::try_from(units.saturating_mul(4).saturating_add(64)).unwrap_or(u32::MAX)
}

impl FileDiffs {
  /// The highlighting of the lines `[from_line, to_line)` of a blob, as raw bytes (the module doc)
  #[instrument(skip(self, git, params), fields(id = %params.blob.id, path = %params.file_path))]
  pub fn blob_syntax(&self, git: &GitCommandExecutor, params: &GetBlobSyntaxParams) -> Result<Vec<u8>> {
    let Some(grammar) = grammar_of(&params.file_path) else {
      return Ok(encode(SyntaxStatus::Fallback, None, 0, 0));
    };
    let key = (grammar.id(), params.blob.id.clone());
    let runs = if let Some(runs) = self.syntax.get(&key) {
      runs
    } else {
      let text = match self.syntax_text(git, params)? {
        SyntaxText::Text(text) => text,
        SyntaxText::Fallback => return Ok(encode(SyntaxStatus::Fallback, None, 0, 0)),
        SyntaxText::Missing => return Ok(encode(SyntaxStatus::Missing, None, 0, 0)),
      };
      self.syntax.get_with(key, || {
        let _permit = PARSES.acquire();
        Arc::new(highlight(grammar, &text))
      })
    };
    Ok(match runs.as_ref() {
      Some(runs) => encode(SyntaxStatus::Runs, Some(runs), params.from_line, params.to_line),
      None => encode(SyntaxStatus::Fallback, None, 0, 0),
    })
  }

  /// The text to highlight: the request's, else a blob of the memory cache, the working tree or git
  fn syntax_text<'a>(&self, git: &GitCommandExecutor, params: &'a GetBlobSyntaxParams) -> Result<SyntaxText<'a>> {
    if let Some(text) = &params.text {
      return Ok(SyntaxText::Text(Cow::Borrowed(text)));
    }
    let id = params.blob.id.as_str();
    if !is_object_id(id) {
      return Ok(SyntaxText::Missing);
    }
    let mut blob = self.blobs.get(id);
    if blob.is_none()
      && let Some(path) = &params.blob.path
    {
      blob = read_worktree_blob(&params.repository_path, path, id)?
        .filter(|it| it.id() == id)
        .map(|it| self.keep_blob(it));
    }
    let blob = match blob {
      Some(blob) => blob,
      None => self
        .read_blobs(git, &params.repository_path, [id])?
        .remove(id)
        .unwrap_or_else(|| Arc::new(BlobText::Missing { id: id.to_string() })),
    };
    Ok(match &*blob {
      BlobText::Text { text, .. } => SyntaxText::Text(Cow::Owned(text.clone())),
      BlobText::Binary { .. } | BlobText::TooLarge { .. } => SyntaxText::Fallback,
      BlobText::Missing { .. } => SyntaxText::Missing,
    })
  }
}

enum SyntaxText<'a> {
  Text(Cow<'a, str>),
  Fallback,
  Missing,
}

/// The bytes of an answer: the header, then for `Runs` the lines `[from, to)` clamped to the text
pub(crate) fn encode(status: SyntaxStatus, runs: Option<&LineRuns>, from: u32, to: u32) -> Vec<u8> {
  let line_count = runs.map_or(0, LineRuns::line_count);
  let from = (from as usize).min(line_count);
  let to = (to as usize).clamp(from, line_count);
  let (starts, pairs): (&[u32], &[u32]) = match runs {
    Some(runs) => {
      let starts = &runs.line_starts[from..=to];
      let first = starts[0] as usize * 2;
      let end = starts[starts.len() - 1] as usize * 2;
      (starts, &runs.runs[first..end])
    }
    None => (&[], &[]),
  };
  let base = starts.first().copied().unwrap_or(0);
  let header = [SYNTAX_FORMAT, status as u32, to_u32(line_count), to_u32(from), to_u32(to), to_u32(pairs.len() / 2)];
  let mut bytes = Vec::with_capacity((HEADER + starts.len() + pairs.len()) * 4);
  for value in header.into_iter().chain(starts.iter().map(|start| start - base)).chain(pairs.iter().copied()) {
    bytes.extend_from_slice(&value.to_le_bytes());
  }
  bytes
}

fn to_u32(value: usize) -> u32 {
  u32::try_from(value).unwrap_or(u32::MAX)
}

/// At most half the cores parse at once: the diffs of a review have their workers too
static PARSES: LazyLock<Permits> = LazyLock::new(|| Permits::new(std::thread::available_parallelism().map_or(1, |cores| (cores.get() / 2).max(1))));

struct Permits {
  free: Mutex<usize>,
  released: Condvar,
}

impl Permits {
  const fn new(count: usize) -> Self {
    Self {
      free: Mutex::new(count),
      released: Condvar::new(),
    }
  }

  fn acquire(&self) -> Permit<'_> {
    let mut free = self.free.lock().unwrap_or_else(PoisonError::into_inner);
    while *free == 0 {
      free = self.released.wait(free).unwrap_or_else(PoisonError::into_inner);
    }
    *free -= 1;
    Permit { permits: self }
  }
}

struct Permit<'a> {
  permits: &'a Permits,
}

impl Drop for Permit<'_> {
  fn drop(&mut self) {
    *self.permits.free.lock().unwrap_or_else(PoisonError::into_inner) += 1;
    self.permits.released.notify_one();
  }
}
