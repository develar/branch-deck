//! `FileDiffs`: the diff of each file of a subject, and the texts of blobs, through two memory caches and the
//! `file_diff` store of the object cache.
//!
//! - **Processes.** The file list costs one git process (`diff-tree --raw --numstat -z`, or none for a commit the
//!   `commit_files` store has; `status --porcelain=v2 -z` for the working tree), and the blobs one `cat-file --batch`,
//!   none when the caches have every diff.
//! - **Workers.** The comparisons and the parses of the scopes run on `workers()` threads (the cores but one), fed by
//!   the one `cat-file`: a file goes to a worker as soon as both its sides are read, and holds its texts only until
//!   then.
//! - **Order.** The files in view (`priority_paths`) come first, then the others in the order of the tree, for the
//!   cache reads, the blob reads and the work. The events go out as the results come.
//! - **Marks.** The marks of a text (`marks.rs`) are made before its content goes out: they change the rows a view
//!   draws, so marks that came later would move a file already drawn. A block is asked a question of its characters
//!   first, and only a file with a block that may have a run is parsed for its tokens, once per blob and grammar
//!   (`TOKENS_CACHE_BYTES`). The moves of a text need no parse: they are read from the lines of every pair. The
//!   budgets below change no mark: a diff the time cut short has the runs and the moves of its blocks.
//! - **Subject.** What only the other files of the subject explain (`marks_subject`: the lines that moved between two
//!   files, the names one file declares and others use) is read from what each pair left (`leftovers`), kept beside
//!   its diff under the same key, so no blob is read for it. When the caches have every pair, it is read before the
//!   first content goes out, which carries it; else each content goes out as it is known and the marks of the files
//!   the subject says something of follow in a `marks` event, once all are. Nothing of the subject enters a cache: a
//!   pair is in other commits beside other files.
//! - **The index.** What the index of the repository says of the names a subject renamed is an answer of its own
//!   (`FileDiffs::rename_proofs`), asked by the window that shows the subject, after its diffs: every declaration of
//!   the subject in one process, a query of seconds on a large repository, so no read of the diffs waits for it and
//!   a subject read ahead of its view starts none. The pairs come from the caches again; the answer is the marks of
//!   the files it says something of, which add words and what a reader may pass, never a row. Memory keeps the
//!   answers of the last subjects (`ANSWERS_CACHE_BYTES`) by their two trees and their questions: two trees never
//!   change, and a subject shown again asks no process. Only an answer that is final for each of its pairs is kept
//!   (`RenameAnswer::is_final`): one that the budget of the query, its limit or the state of the index cut short
//!   says what that query got to, on that machine at that load, and the subject is asked again when it is shown
//!   again.
//! - **Caches.** A diff is keyed by its blob pair, the whitespace policy and how each side is read (`DiffKey`,
//!   `diff_options`): its grammar, since the scopes and the marks depend on it, `.kt` and `.kts`, `.ts`, `.tsx` and
//!   `.js` each their own; without a grammar, whether its indentation has a meaning, which the moves depend on, so a
//!   `.py` and a `.txt` file of the same blobs are two diffs. Blobs never change, so one entry serves every commit, range and window that shows the pair. Memory
//!   first (`DIFF_CACHE_BYTES`), then the store. A blob read for a diff stays in a memory cache (`BLOB_CACHE_BYTES`), so
//!   the texts that a view asks for next cost no process.
//! - **Budgets.** A side over `MAX_BLOB_BYTES` is `TooLarge`, a binary one `Binary`. Each file has `DiffLimits::file`
//!   for its inner ranges and `DiffLimits::lines` for its lines, for a pathological file only: no budget spans the
//!   subject, so the result does not depend on the load of the machine or the order of the files. A diff that the time
//!   cut short (`BudgetedDiff::out_of_time`: its inner ranges or its lines ran out) stays in memory only: the store
//!   never keeps it, and a scan never reads it (`blocks_of` compares without a deadline).

use crate::accio::{IndexUnavailable, RenameAnswer, RenamePair, SearchCanceled, SymbolIndex};
use crate::collapse::collapse_of;
use crate::leftovers::{LEFTOVERS_CACHE_BYTES, Leftovers};
use crate::marks::{TOKENS_CACHE_BYTES, pair_indented, tokens_weight};
use crate::marks_subject::{IndexAnswer, PairFacts, Subject, facts_of};
use crate::merge::{ConflictPath, MergeMemoryKey, merge_weight};
use crate::model::{
  BlobRequest, BlobText, Collapse, DiffContent, DiffFile, DiffFileHeader, DiffSource, FileDiffEvent, FileMarks, GetFileDiffsParams, GetRenameProofsParams, MergeContent,
  MergeSource,
};
use crate::range_files::RangeFilesCache;
use crate::scopes::scopes_of_summary;
use crate::syntax::{SYNTAX_CACHE_BYTES, syntax_weight};
use crate::worktree::{read_worktree_blob, uncommitted_entries};
use anyhow::{Result, bail};
use code_symbols::extract::{extractor_version, summarize};
use code_symbols::highlight::grammar_of;
use code_symbols::highlight::lines::LineRuns;
use code_symbols::language::{Language, language_of, summary_key};
use code_symbols::navigate::RULES_VERSION;
use code_symbols::summary::{FileSummary, LineSpan};
use code_symbols::tokens::{Tokens, indentation_matters, tokens_version};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cat_file_batch::{BatchEntry, for_each_object};
use git_ops::commit_files::{DiffSubject, MAX_BLOB_BYTES, read_commit_files_cached};
use git_ops::object_texts::{into_text, is_binary};
use git_ops::revs::is_object_id;
use line_diff::cancellation::Deadline;
use line_diff::model::{BlockKind, BudgetedDiff, DiffBudget, DiffMarks, LineRun, MAX_INNER_BYTES, MAX_INNER_LINES, Scope, TextDiff, diff_texts_in};
use line_diff::policy::WhitespacePolicy;
use line_diff::text_lines::TextLines;
use moka::sync::Cache;
use object_cache::cache::ObjectCache;
use object_cache::commit_files::{CommitFile, FileStatus};
use object_cache::file_diff::DiffKey;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use sync_utils::span::in_current_span;
use tracing::{debug, instrument, warn};

/// The version of the diffs in the `file_diff` store and of what they left in the `file_leftovers` store: bump it
/// when `TextDiff`, the comparison, the scopes, the marks or `Leftovers` (the hash of a line's key too) change.
/// `stored_layout_test.rs` pins the layout of both with it. 15: a diff whose lines ran out of time is not stored either
/// (`BudgetedDiff::out_of_time`), so a store of 14 may hold one block of two whole texts that the time made. 19: the
/// scopes and the blocks of the imports alone are by the lines of the diffs (`side_facts`), so a store of 18 may hold
/// those of a text with a lone `\r` by the lines of git
pub const MODEL_VERSION: u32 = 19;

/// The version the `file_diff` store keeps the diffs under: `MODEL_VERSION` with the version of the tokens the marks
/// are read from (`code_symbols::tokens::tokens_version`: the parsers of this build and the rules of the tokens) and
/// the version of the rules the names of the renames are bound by (`code_symbols::navigate::RULES_VERSION`: the
/// scopes, the locals, the sites, the clashes), so a stored diff is never read with marks of other tokens or of
/// other bindings: a rule of a scope that was fixed takes the renames and the quiet runs it had allowed with it
pub fn model_version() -> u32 {
  store_version(MODEL_VERSION, tokens_version(), RULES_VERSION)
}

/// The version of the store for a version of the model, one of the tokens and one of the rules of the names:
/// another for another of any
pub(crate) const fn store_version(model: u32, tokens: u32, names: u32) -> u32 {
  model.wrapping_mul(0x9e37_79b9) ^ tokens ^ names.wrapping_mul(0xc2b2_ae35)
}

/// The diffs that memory keeps, weighed by the size of their model
pub const DIFF_CACHE_BYTES: u64 = 64 << 20;
/// The blobs that memory keeps, weighed by their size
pub const BLOB_CACHE_BYTES: u64 = 128 << 20;
/// The merges whose conflicting paths memory keeps
const MAX_CACHED_CONFLICTS: u64 = 256;
/// The answers of the index that memory keeps, weighed by the uses they list: those of the subjects a window showed
/// last, each some kilobytes, one of a name used all over a repository a megabyte. Final ones alone
/// (`RenameAnswer::is_final`)
pub const ANSWERS_CACHE_BYTES: u64 = 16 << 20;
/// At most this many blobs in one `blob_texts`
pub const MAX_BLOB_REQUESTS: usize = 10_000;

/// The time of the comparison of one file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffLimits {
  /// The inner ranges; after it, the file is compared by line
  pub file: Duration,
  /// The line comparison; after it, the file is one block
  pub lines: Duration,
}

pub const DIFF_LIMITS: DiffLimits = DiffLimits {
  file: Duration::from_millis(100),
  lines: Duration::from_secs(1),
};

/// What a diff depends on beside its two blobs: the whitespace policy and how each side is read, the old one first:
/// its grammar (`Grammar::id` + 1), without one 0, or `INDENTED` when its indentation has a meaning
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DiffOptions {
  policy: u8,
  grammars: [u8; 2],
}

/// In the key of a diff, a side without a grammar whose indentation has a meaning (Python, YAML, a Makefile): its
/// moves are others than those of the same lines where it has none. No grammar has this number
const INDENTED: u8 = u8::MAX;

/// The memory key of a diff: its options and the two blob ids, empty for a missing side
pub(crate) type MemoryKey = (DiffOptions, String, String);

/// The diffs and the blobs of the views, shared by every window; cheap to clone
#[derive(Clone)]
pub struct FileDiffs {
  pub(crate) diffs: Cache<MemoryKey, Arc<DiffContent>>,
  pub(crate) merges: Cache<MergeMemoryKey, Arc<MergeContent>>,
  /// The conflicting paths of a merge with their stages, by repository and merge
  pub(crate) conflicts: Cache<(String, MergeSource), Arc<Vec<ConflictPath>>>,
  pub(crate) blobs: Cache<String, Arc<BlobText>>,
  /// The backend highlighting of blobs by grammar and id, `None` when a blob has none (`syntax.rs`)
  pub(crate) syntax: Cache<(u8, String), Arc<Option<LineRuns>>>,
  /// What the marks of the diffs keep beside them
  pub(crate) marks_memory: Arc<MarksMemory>,
  pub(crate) limits: DiffLimits,
  /// `blocks_of` keeps the blobs it reads in `blobs` (`for_scans`)
  pub(crate) keep_scan_blobs: bool,
  /// The diffs are made without their marks (`without_marks`)
  pub(crate) marks_off: bool,
  /// The files of the ranges: the list of a review (`get_range_files`) and its diffs (`stream`) run one `diff-tree`
  pub range_files: RangeFilesCache,
}

/// What the marks of the diffs keep in memory beside the diffs, behind one pointer: every reader of a review holds
/// a `FileDiffs` of its own (a clone that shares the caches), a future of a read ahead several of them
pub(crate) struct MarksMemory {
  /// The tokens of blobs by grammar and id, `None` when a blob has none (`marks.rs`)
  pub(crate) tokens: Cache<(u8, String), Option<Arc<Tokens>>>,
  /// What the diff of a text pair left to the other files of its subjects (`leftovers`), under the key of its diff
  pub(crate) leftovers: Cache<MemoryKey, Arc<Leftovers>>,
  /// What the index answered of the declarations a subject renamed (`FileDiffs::rename_proofs`)
  pub(crate) answers: Cache<AnswersKey, Arc<Vec<RenameAnswer>>>,
}

/// The key of an answer of the index: the repository, the two trees as they were asked (full object ids, so they
/// name the same trees for ever) and the declarations asked
pub(crate) type AnswersKey = (String, String, String, Vec<RenamePair>);

/// The weight of an answer of the index in memory, roughly in bytes
fn answers_weight(answers: &[RenameAnswer]) -> u32 {
  let uses: usize = answers
    .iter()
    .flat_map(|answer| [&answer.base, &answer.head])
    .flatten()
    .flat_map(|uses| &uses.uses)
    .map(|used| 80 + used.path.len())
    .sum();
  u32::try_from(256 + answers.len() * 160 + uses).unwrap_or(u32::MAX)
}

impl Default for FileDiffs {
  fn default() -> Self {
    Self::new(DIFF_LIMITS)
  }
}

/// A changed file before its diff
pub(crate) struct Entry {
  pub path: String,
  pub old_path: Option<String>,
  pub status: FileStatus,
  pub old_id: Option<String>,
  pub new_id: Option<String>,
  /// Git counts no lines for the file
  pub binary: bool,
  /// The commits of a submodule, which has no blob ids
  pub submodule: Option<(Option<String>, Option<String>)>,
  /// The new side read from the working tree
  pub worktree: Option<Arc<BlobText>>,
}

impl From<CommitFile> for Entry {
  fn from(file: CommitFile) -> Self {
    let (old_id, new_id, submodule) = if file.submodule {
      (None, None, Some((file.old_id, file.new_id)))
    } else {
      (file.old_id, file.new_id, None)
    };
    Self {
      binary: file.binary,
      path: file.path,
      old_path: file.old_path,
      status: file.status,
      old_id,
      new_id,
      submodule,
      worktree: None,
    }
  }
}

impl Entry {
  fn header(&self) -> DiffFileHeader {
    DiffFileHeader {
      path: self.path.clone(),
      old_path: self.old_path.clone(),
      status: self.status,
      old_id: self.old_id.clone(),
      new_id: self.new_id.clone(),
    }
  }

  fn old_language(&self) -> Option<Language> {
    language_of(self.old_path.as_deref().unwrap_or(&self.path))
  }
}

/// One side of a file before its work: the file does not exist there, its blob, or the id of a blob not read yet
enum Side {
  Empty,
  Blob(Arc<BlobText>),
  Pending(String),
}

/// A file whose sides are read, for a worker
struct Job {
  index: usize,
  old: Option<Arc<BlobText>>,
  new: Option<Arc<BlobText>>,
}

/// What a worker sends back
enum Outcome {
  Event(FileDiffEvent),
  /// The diff of a text and what it left, as memory keeps them: for the pass over the files of the subject
  Text {
    index: usize,
    text: KnownText,
  },
  /// A diff with what it left and summaries for the store
  Store {
    diff: Option<(MemoryKey, TextDiff, Arc<Leftovers>)>,
    summaries: Vec<((u8, String), FileSummary)>,
  },
}

/// The content of a text file and what its pair left, as the caches keep them
type KnownText = (Arc<DiffContent>, Arc<Leftovers>);

/// The two halves of a `KnownText`, each as far as a cache has it
type Halves = (Option<Arc<DiffContent>>, Option<Arc<Leftovers>>);

/// What the caches have of the files of a stream (`FileDiffs::cached`), each list by the index of a file
struct Cached {
  /// The content of a file that is no text
  known: Vec<Option<DiffContent>>,
  /// The diff of a text with what its pair left
  texts: Vec<Option<KnownText>>,
  /// The files no cache has, in the order of the work
  pending: Vec<usize>,
}

/// The files of a stream that no cache has (`FileDiffs::compare`)
#[derive(Clone, Copy)]
struct Pending<'a> {
  repository_path: &'a str,
  entries: &'a [Entry],
  keys: &'a [MemoryKey],
  policy: WhitespacePolicy,
  /// Their indices, in the order of the work
  order: &'a [usize],
}

/// The stream of a subject collected (`FileDiffs::collect`): the header of each file and its content, with why a
/// view starts it collapsed
#[derive(Default)]
pub(crate) struct Collected {
  headers: Vec<DiffFileHeader>,
  contents: Vec<Option<(DiffContent, Option<Collapse>)>>,
}

impl Collected {
  /// The files of the subject, in the order of git
  pub(crate) fn into_files(self) -> Vec<DiffFile> {
    self
      .headers
      .into_iter()
      .zip(self.contents)
      .map(|(header, content)| {
        // every file gets a content; one left out would be a bug, shown as missing rather than failing the subject
        let (content, collapse) = content.unwrap_or_else(|| (DiffContent::Missing, collapse_of(&header.path, &DiffContent::Missing)));
        DiffFile {
          path: header.path,
          old_path: header.old_path,
          status: header.status,
          old_id: header.old_id,
          new_id: header.new_id,
          content,
          collapse,
        }
      })
      .collect()
  }
}

/// The two trees the index is asked of the names a subject renamed, the old one first: of a commit and of a range
/// named by full object ids, which name the same trees whenever they are read (so an answer may be kept by them).
/// `None` for the working tree, which may be another by the time the index reads it, and for a name, which may
/// move: the answer would be of another text than the one the diffs show
fn revisions_of(source: &DiffSource) -> Option<(String, String)> {
  match source {
    DiffSource::Commit { id } if is_object_id(id) => Some((format!("{id}~1"), id.clone())),
    DiffSource::Range { base, head } if is_object_id(base) && is_object_id(head) => Some((base.clone(), head.clone())),
    DiffSource::Commit { .. } | DiffSource::Range { .. } | DiffSource::Uncommitted => None,
  }
}

/// What the pass over the files of a subject reads of each of `entries`: for a file whose content is a text, its
/// diff and what its pair left (`texts`, by the index of the file)
fn subject_facts<'a>(entries: &'a [Entry], texts: &'a [Option<KnownText>]) -> Vec<Option<PairFacts<'a>>> {
  entries
    .iter()
    .zip(texts)
    .map(|(entry, text)| match text.as_ref().map(|(content, left)| (content.as_ref(), left.as_ref())) {
      Some((DiffContent::Text(diff), left)) => Some(facts_of(
        entry.old_path.as_deref(),
        &entry.path,
        [entry.old_id.as_deref(), entry.new_id.as_deref()],
        diff,
        left,
      )),
      _ => None,
    })
    .collect()
}

/// The number of worker threads: the cores but one, for the thread that reads the blobs
pub(crate) fn workers() -> usize {
  std::thread::available_parallelism().map_or(1, |cores| cores.get().saturating_sub(1).max(1))
}

/// Starts a worker thread per job of `jobs`, at most `workers()`, in `scope`: each runs the closure `worker` makes for
/// it, in the span of the caller, so its lines name the operation
pub(crate) fn spawn_workers<'scope, W>(scope: &'scope std::thread::Scope<'scope, '_>, jobs: usize, mut worker: impl FnMut() -> W)
where
  W: FnOnce() + Send + 'scope,
{
  for _ in 0..workers().min(jobs) {
    scope.spawn(in_current_span(worker()));
  }
}

impl FileDiffs {
  pub fn new(limits: DiffLimits) -> Self {
    Self {
      diffs: Cache::builder()
        .max_capacity(DIFF_CACHE_BYTES)
        .weigher(|_key: &MemoryKey, diff: &Arc<DiffContent>| content_weight(diff))
        .build(),
      merges: Cache::builder()
        .max_capacity(DIFF_CACHE_BYTES)
        .weigher(|_key: &MergeMemoryKey, merge: &Arc<MergeContent>| merge_weight(merge))
        .build(),
      conflicts: Cache::builder().max_capacity(MAX_CACHED_CONFLICTS).build(),
      blobs: Cache::builder()
        .max_capacity(BLOB_CACHE_BYTES)
        .weigher(|_id: &String, blob: &Arc<BlobText>| blob.size().saturating_add(64))
        .build(),
      syntax: Cache::builder()
        .max_capacity(SYNTAX_CACHE_BYTES)
        .weigher(|_key: &(u8, String), runs: &Arc<Option<LineRuns>>| syntax_weight(runs))
        .build(),
      marks_memory: Arc::new(MarksMemory {
        tokens: Cache::builder()
          .max_capacity(TOKENS_CACHE_BYTES)
          .weigher(|_key: &(u8, String), tokens: &Option<Arc<Tokens>>| tokens_weight(tokens.as_deref()))
          .build(),
        leftovers: Cache::builder()
          .max_capacity(LEFTOVERS_CACHE_BYTES)
          .weigher(|_key: &MemoryKey, left: &Arc<Leftovers>| left.weight())
          .build(),
        answers: Cache::builder()
          .max_capacity(ANSWERS_CACHE_BYTES)
          .weigher(|_key: &AnswersKey, answers: &Arc<Vec<RenameAnswer>>| answers_weight(answers))
          .build(),
      }),
      limits,
      keep_scan_blobs: false,
      marks_off: false,
      range_files: RangeFilesCache::default(),
    }
  }

  /// An instance whose diffs have no marks, for the measurement of what the marks cost alone (`marks_measure`,
  /// `brd commits --mark-stats --no-marks`): no tokens, no runs, no renames, no moves, nothing left to the subject,
  /// each block a stop. It reads and writes no store of the diffs (`stream` gives it a cache that keeps nothing), so
  /// no view ever reads a diff of it; compare it with an instance that has no store either
  pub fn without_marks() -> Self {
    Self {
      marks_off: true,
      ..Self::default()
    }
  }

  /// An instance for the scans of one session, which read the same blobs twice: the blocks of the commits
  /// (`blocks_of`) keep their blobs in memory, so the changed lines and the patches of the same commits read them from
  /// there. A `default` instance drops each blob after its pair, so a scan holds only the pairs in flight
  pub fn for_scans() -> Self {
    Self {
      keep_scan_blobs: true,
      ..Self::default()
    }
  }

  /// The diff of each changed file of `source` under `policy`, in the order of git, at once: `stream` collected,
  /// without an index of the repository
  pub fn read(&self, git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, source: &DiffSource, policy: WhitespacePolicy) -> Result<Vec<DiffFile>> {
    self.read_checked(git, repository_path, cache, None, source, policy)
  }

  /// `read` with the index `index` of the repository, which the names the subject renamed are asked of
  /// (`rename_proofs`) once its diffs are read, as a window that shows the subject asks it
  pub(crate) fn read_checked(
    &self,
    git: &GitCommandExecutor,
    repository_path: &str,
    cache: &ObjectCache,
    index: Option<&dyn SymbolIndex>,
    source: &DiffSource,
    policy: WhitespacePolicy,
  ) -> Result<Vec<DiffFile>> {
    let params = GetFileDiffsParams {
      repository_path: repository_path.to_string(),
      source: source.clone(),
      policy,
      priority_paths: Vec::new(),
    };
    let mut read = self.collect(git, cache, index.is_some(), &params, &mut |_| {})?;
    self.answer_proofs(git, cache, index, &params, &mut read)?;
    Ok(read.into_files())
  }

  /// The stream of the subject of `params` collected: the header of each file and its content with the scopes and
  /// the marks that followed it; `seen` is told of each event as it comes
  pub(crate) fn collect(
    &self,
    git: &GitCommandExecutor,
    cache: &ObjectCache,
    asks_index: bool,
    params: &GetFileDiffsParams,
    seen: &mut dyn FnMut(&FileDiffEvent),
  ) -> Result<Collected> {
    let mut read = Collected::default();
    self.stream(git, cache, asks_index, params, |event| {
      seen(&event);
      match event {
        FileDiffEvent::Files { files } => {
          read.contents = vec![None; files.len()];
          read.headers = files;
        }
        FileDiffEvent::Content { index, content, collapse } => read.contents[index as usize] = Some((content, collapse)),
        FileDiffEvent::Marks { index, marks } => {
          if let Some((DiffContent::Text(diff), _)) = &mut read.contents[index as usize] {
            diff.marks = marks;
          }
        }
        FileDiffEvent::Scopes {
          index,
          old_scopes,
          new_scopes,
          import_blocks,
        } => {
          if let Some((DiffContent::Text(diff), _)) = &mut read.contents[index as usize] {
            diff.old_scopes = old_scopes;
            diff.new_scopes = new_scopes;
            diff.import_blocks = import_blocks;
          }
        }
      }
    })?;
    Ok(read)
  }

  /// Asks `index` of the names the subject of `params` renamed (`rename_proofs`) and puts the marks it answers into
  /// the files of `read`; without an index nothing is asked: the proofs say so already
  pub(crate) fn answer_proofs(
    &self,
    git: &GitCommandExecutor,
    cache: &ObjectCache,
    index: Option<&dyn SymbolIndex>,
    params: &GetFileDiffsParams,
    read: &mut Collected,
  ) -> Result<()> {
    if index.is_none() {
      return Ok(());
    }
    let proofs = GetRenameProofsParams {
      repository_path: params.repository_path.clone(),
      source: params.source.clone(),
      policy: params.policy,
    };
    for FileMarks { path, marks } in self.rename_proofs(git, cache, index, &proofs)?.unwrap_or_default() {
      let content = read.headers.iter().position(|header| header.path == path).and_then(|index| read.contents[index].as_mut());
      if let Some((DiffContent::Text(diff), _)) = content {
        diff.marks = marks;
      }
    }
    Ok(())
  }

  /// The diffs of a subject as events (`FileDiffEvent`): the files, then the content of each file as it is known,
  /// `priority_paths` first, then the others in the order of the tree; then what the files of the subject say of each
  /// other (module doc, "Subject"), in the content itself when the caches had every pair. `asks_index`: an index of
  /// the repository is there for the names the subject renamed, so their proof is `asked` until `rename_proofs`
  /// answers; the stream itself asks no index and ends with the last of its files
  #[instrument(skip(self, git, cache, params, emit), fields(repository_path = %params.repository_path, source = ?params.source))]
  pub fn stream(&self, git: &GitCommandExecutor, cache: &ObjectCache, asks_index: bool, params: &GetFileDiffsParams, mut emit: impl FnMut(FileDiffEvent)) -> Result<()> {
    let (repository_path, policy) = (params.repository_path.as_str(), params.policy);
    // diffs without marks are for a measurement alone: no store keeps one, and none is read in their place
    let no_store = self.marks_off.then(ObjectCache::disabled);
    let cache = no_store.as_ref().unwrap_or(cache);
    let entries = list_entries(git, repository_path, cache, &self.range_files, &params.source)?;
    for blob in entries.iter().filter_map(|entry| entry.worktree.as_ref()) {
      self.blobs.insert(blob.id().to_string(), Arc::clone(blob));
    }
    emit(FileDiffEvent::Files {
      files: entries.iter().map(Entry::header).collect(),
    });
    let paths: Vec<&str> = entries.iter().map(|entry| entry.path.as_str()).collect();
    let order = priority_order(&paths, &params.priority_paths);
    let keys: Vec<MemoryKey> = entries.iter().map(|entry| memory_key(entry, policy)).collect();
    let content_event = |index: usize, content: DiffContent| FileDiffEvent::Content {
      index: to_u32(index),
      collapse: collapse_of(&entries[index].path, &content),
      content,
    };

    let Cached { mut known, mut texts, pending } = self.cached(cache, &entries, &keys, &order);
    debug!(files = entries.len(), cached = entries.len() - pending.len(), "file diffs");
    if !pending.is_empty() {
      // what the caches have goes out at once; what the files say of each other follows when all are known
      for &index in &order {
        if let Some(content) = known[index].take().or_else(|| texts[index].as_ref().map(|(content, _)| (**content).clone())) {
          emit(content_event(index, content));
        }
      }
      let files = Pending {
        repository_path,
        entries: &entries,
        keys: &keys,
        policy,
        order: &pending,
      };
      self.compare(git, cache, &files, &mut texts, &mut emit)?;
    }

    let facts = subject_facts(&entries, &texts);
    let subject = Subject::of(&facts);
    if subject.work.skipped_files > 0 || subject.work.cut {
      debug!(work = ?subject.work, "the moves between the files of the subject were not all looked for");
    }
    // the index can be asked of a subject of two trees that never change, by the declarations it renamed
    let asked = asks_index && revisions_of(&params.source).is_some() && !subject.questions().is_empty();
    let marks = subject.marks(if asked { IndexAnswer::Asked } else { IndexAnswer::Unavailable });
    if pending.is_empty() {
      // every pair was in the caches: the content of a file comes with what the subject says of it
      for &index in &order {
        let content = known[index].take().or_else(|| texts[index].as_ref().map(|(content, _)| (**content).clone()));
        let Some(mut content) = content else { continue };
        if let (DiffContent::Text(diff), Some(marks)) = (&mut content, &marks[index]) {
          diff.marks = marks.clone();
        }
        emit(content_event(index, content));
      }
    } else {
      for (index, marks) in marks.into_iter().enumerate() {
        if let Some(marks) = marks {
          emit(FileDiffEvent::Marks { index: to_u32(index), marks });
        }
      }
    }
    Ok(())
  }

  /// What `index`, the index of the repository, says of the names the subject of `params` renamed: the marks of each
  /// file it says something of, in place of those `stream` gave it (whose proofs were `asked`), by its path. Empty
  /// when the subject has nothing to ask, `None` when the query was canceled: the proofs stay as they are, for
  /// whoever asks again.
  ///
  /// Asked after the diffs of the subject: its pairs come from the caches, and a pair they lost is compared again,
  /// silently. Every declaration goes into one process, and an answer that is final for each of them
  /// (`RenameAnswer::is_final`) is kept in memory by its two trees and its questions (`ANSWERS_CACHE_BYTES`), so a
  /// subject shown again starts none; an answer that the query cut short is used for this question alone. An index
  /// that does not answer (none, one without the answer, no index of the repository, a failed query) leaves every
  /// asked proof `unavailable`: it fails no read
  #[instrument(skip(self, git, cache, index, params), fields(repository_path = %params.repository_path, source = ?params.source))]
  pub fn rename_proofs(&self, git: &GitCommandExecutor, cache: &ObjectCache, index: Option<&dyn SymbolIndex>, params: &GetRenameProofsParams) -> Result<Option<Vec<FileMarks>>> {
    let (repository_path, policy) = (params.repository_path.as_str(), params.policy);
    // a subject of the working tree, or one named by a ref, is never asked: `stream` said so
    let Some((base, head)) = revisions_of(&params.source) else {
      return Ok(Some(Vec::new()));
    };
    let entries = list_entries(git, repository_path, cache, &self.range_files, &params.source)?;
    let order: Vec<usize> = (0..entries.len()).collect();
    let keys: Vec<MemoryKey> = entries.iter().map(|entry| memory_key(entry, policy)).collect();
    let Cached { mut texts, pending, .. } = self.cached(cache, &entries, &keys, &order);
    if !pending.is_empty() {
      let files = Pending {
        repository_path,
        entries: &entries,
        keys: &keys,
        policy,
        order: &pending,
      };
      self.compare(git, cache, &files, &mut texts, &mut |_| {})?;
    }
    let facts = subject_facts(&entries, &texts);
    let subject = Subject::of(&facts);
    let questions = subject.questions();
    if questions.is_empty() {
      return Ok(Some(Vec::new()));
    }
    let key: AnswersKey = (repository_path.to_string(), base, head, questions.to_vec());
    let answers = match (self.marks_memory.answers.get(&key), index) {
      (Some(answers), _) => Some(answers),
      (None, None) => None,
      (None, Some(index)) => match index.renames(repository_path, questions, &key.1, &key.2) {
        Ok(Some(answers)) if answers.len() == questions.len() => {
          let answers = Arc::new(answers);
          // an answer the budget or the state of the index cut short is one of this query alone: the next
          // question of the subject asks the index again
          if answers.iter().all(RenameAnswer::is_final) {
            self.marks_memory.answers.insert(key, Arc::clone(&answers));
          } else {
            debug!("the index did not answer every rename of the subject to its end: the answer is not kept");
          }
          Some(answers)
        }
        Ok(_) => None,
        Err(error) if error.is::<SearchCanceled>() => return Ok(None),
        Err(error) if error.is::<IndexUnavailable>() => {
          debug!(%error, "the index of the repository answers no rename");
          None
        }
        Err(error) => {
          warn!(error = %format!("{error:#}"), "the index of the repository did not answer the renames of the subject");
          None
        }
      },
    };
    let asked = subject.marks(IndexAnswer::Asked);
    let answered = subject.marks(answers.as_deref().map_or(IndexAnswer::Unavailable, |answers| IndexAnswer::Answered(answers)));
    Ok(Some(
      entries
        .iter()
        .zip(answered)
        .zip(asked)
        .filter(|((_, marks), before)| marks != before)
        .filter_map(|((entry, marks), _)| {
          Some(FileMarks {
            path: entry.path.clone(),
            marks: marks?,
          })
        })
        .collect(),
    ))
  }

  /// What the caches have of the files `entries` with the keys `keys`, read in the order `order`: the content of a
  /// file that is no text, the diff of a text with what its pair left, from memory, then from the stores, whose
  /// answers memory keeps. A text is known with what its pair left or not at all: the pass over the subject reads
  /// no blob, so a pair whose leftovers no cache has is compared again
  fn cached(&self, cache: &ObjectCache, entries: &[Entry], keys: &[MemoryKey], order: &[usize]) -> Cached {
    let mut known: Vec<Option<DiffContent>> = vec![None; entries.len()];
    let mut texts: Vec<Option<KnownText>> = vec![None; entries.len()];
    // of a text, each half that memory has
    let mut halves: Vec<Halves> = vec![(None, None); entries.len()];
    for &index in order {
      let entry = &entries[index];
      match &entry.submodule {
        Some((old_commit, new_commit)) => {
          known[index] = Some(DiffContent::Submodule {
            old_commit: old_commit.clone(),
            new_commit: new_commit.clone(),
          });
        }
        None if entry.binary => known[index] = Some(DiffContent::Binary),
        None => match self.diffs.get(&keys[index]) {
          Some(content) if !matches!(*content, DiffContent::Text(_)) => known[index] = Some((*content).clone()),
          content => halves[index] = (content, self.marks_memory.leftovers.get(&keys[index])),
        },
      }
    }
    let missed = |half: fn(&Halves) -> bool| -> Vec<usize> { order.iter().copied().filter(|&index| known[index].is_none() && half(&halves[index])).collect() };
    let (no_diff, no_left) = (missed(|half| half.0.is_none()), missed(|half| half.1.is_none()));
    let disk_keys = |files: &[usize]| -> Vec<DiffKey<'_>> { files.iter().map(|&index| disk_key(&keys[index])).collect() };
    let stored: Vec<Option<TextDiff>> = cache.get_file_diffs(model_version(), &disk_keys(&no_diff));
    let stored_left: Vec<Option<Leftovers>> = cache.get_file_leftovers(model_version(), &disk_keys(&no_left));
    // the halves the stores had: memory keeps them, once the text is known whole
    let mut from_store = vec![[false; 2]; entries.len()];
    for (index, diff) in no_diff.into_iter().zip(stored) {
      from_store[index][0] = diff.is_some();
      halves[index].0 = diff.map(|diff| Arc::new(DiffContent::Text(diff)));
    }
    for (index, left) in no_left.into_iter().zip(stored_left) {
      from_store[index][1] = left.is_some();
      halves[index].1 = left.map(Arc::new);
    }
    let mut pending = Vec::new();
    for &index in order {
      if known[index].is_some() {
        continue;
      }
      match std::mem::take(&mut halves[index]) {
        (Some(content), Some(left)) => {
          if from_store[index][0] {
            self.diffs.insert(keys[index].clone(), Arc::clone(&content));
          }
          if from_store[index][1] {
            self.marks_memory.leftovers.insert(keys[index].clone(), Arc::clone(&left));
          }
          texts[index] = Some((content, left));
        }
        _ => pending.push(index),
      }
    }
    Cached { known, texts, pending }
  }

  /// Compares the files `pending.order` that no cache has: reads their blobs in one `cat-file`, hands each file to a
  /// worker as soon as its sides are read, sends the events of the workers to `emit`, keeps the diff of each text
  /// with what it left in `texts`, and writes the diffs, what they left and the summaries to the stores
  fn compare(&self, git: &GitCommandExecutor, cache: &ObjectCache, pending: &Pending<'_>, texts: &mut [Option<KnownText>], emit: &mut dyn FnMut(FileDiffEvent)) -> Result<()> {
    let Pending {
      repository_path,
      entries,
      keys,
      policy,
      order,
    } = *pending;
    // the sides of each pending file, and the files that wait for each blob, in the order of the work
    let mut sides: HashMap<usize, (Side, Side)> = HashMap::with_capacity(order.len());
    let mut waiting: HashMap<String, Vec<usize>> = HashMap::new();
    let mut ids: Vec<String> = Vec::new();
    for &index in order {
      let entry = &entries[index];
      let old = self.side(entry.old_id.as_deref(), None);
      let new = self.side(entry.new_id.as_deref(), entry.worktree.as_ref());
      for side in [&old, &new] {
        if let Side::Pending(id) = side {
          let files = waiting.entry(id.clone()).or_default();
          if files.is_empty() {
            ids.push(id.clone());
          }
          files.push(index);
        }
      }
      sides.insert(index, (old, new));
    }

    let summaries = cached_summaries(cache, entries, order);
    let context = WorkContext {
      service: self,
      entries,
      keys,
      policy,
      summaries: &summaries,
    };
    let mut to_store: Vec<(MemoryKey, TextDiff, Arc<Leftovers>)> = Vec::new();
    let mut new_summaries: Vec<((u8, String), FileSummary)> = Vec::new();
    let read = std::thread::scope(|scope| {
      let (job_sender, job_receiver) = channel::<Job>();
      let job_receiver = Arc::new(Mutex::new(job_receiver));
      let (outcome_sender, outcomes) = channel::<Outcome>();
      spawn_workers(scope, order.len(), || {
        let (jobs, outcome_sender, context) = (Arc::clone(&job_receiver), outcome_sender.clone(), &context);
        move || work_loop(&jobs, &outcome_sender, context)
      });
      drop(outcome_sender);
      let mut deliver = |outcome: Outcome| match outcome {
        Outcome::Event(event) => emit(event),
        Outcome::Text { index, text } => texts[index] = Some(text),
        Outcome::Store { diff, summaries } => {
          to_store.extend(diff);
          new_summaries.extend(summaries);
        }
      };

      // the files whose sides are all known go first
      for &index in order {
        if let Some(job) = take_job(&mut sides, index) {
          let _ = job_sender.send(job);
        }
      }
      let read = for_each_object(git, repository_path, &ids, MAX_BLOB_BYTES, |position, entry| {
        let id = &ids[position];
        let blob = self.keep_blob(blob_text(id.clone(), entry));
        for index in waiting.remove(id).unwrap_or_default() {
          if let Some((old, new)) = sides.get_mut(&index) {
            for side in [old, new] {
              if matches!(side, Side::Pending(pending) if pending == id) {
                *side = Side::Blob(Arc::clone(&blob));
              }
            }
          }
          if let Some(job) = take_job(&mut sides, index) {
            let _ = job_sender.send(job);
          }
        }
        while let Ok(outcome) = outcomes.try_recv() {
          deliver(outcome);
        }
        Ok(())
      });
      drop(job_sender);
      for outcome in outcomes {
        deliver(outcome);
      }
      read
    });
    read?;

    // what the pairs left first: a diff the store has without it is compared again, never shown without it
    let left_to_store: Vec<(DiffKey<'_>, &Leftovers)> = to_store.iter().map(|(key, _, left)| (disk_key(key), left.as_ref())).collect();
    cache.put_file_leftovers(model_version(), &left_to_store);
    let entries_to_store: Vec<(DiffKey<'_>, &TextDiff)> = to_store.iter().map(|(key, diff, _)| (disk_key(key), diff)).collect();
    cache.put_file_diffs(model_version(), &entries_to_store);
    write_summaries(cache, &new_summaries);
    Ok(())
  }

  /// A blob that `cat-file` read, kept in the memory cache unless git has no such blob (it may fetch it later)
  pub(crate) fn keep_blob(&self, blob: BlobText) -> Arc<BlobText> {
    let blob = Arc::new(blob);
    if !matches!(*blob, BlobText::Missing { .. }) {
      self.blobs.insert(blob.id().to_string(), Arc::clone(&blob));
    }
    blob
  }

  /// A side of a file: empty, a blob the cache has, the file of the working tree, or one to read
  fn side(&self, id: Option<&str>, worktree: Option<&Arc<BlobText>>) -> Side {
    if let Some(blob) = worktree {
      return Side::Blob(Arc::clone(blob));
    }
    match id {
      None => Side::Empty,
      Some(id) => self.blobs.get(id).map_or_else(|| Side::Pending(id.to_string()), Side::Blob),
    }
  }

  /// The texts of the blobs `ids`, from memory or one `cat-file --batch` of the others, which memory keeps then
  pub(crate) fn read_blobs<'a>(&self, git: &GitCommandExecutor, repository_path: &str, ids: impl IntoIterator<Item = &'a str>) -> Result<HashMap<String, Arc<BlobText>>> {
    let mut blobs: HashMap<String, Arc<BlobText>> = HashMap::new();
    let mut missing: Vec<String> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    for id in ids {
      if !seen.insert(id) {
        continue;
      }
      match self.blobs.get(id) {
        Some(blob) => drop(blobs.insert(id.to_string(), blob)),
        None => missing.push(id.to_string()),
      }
    }
    for_each_object(git, repository_path, &missing, MAX_BLOB_BYTES, |position, entry| {
      let id = missing[position].clone();
      blobs.insert(id.clone(), self.keep_blob(blob_text(id, entry)));
      Ok(())
    })?;
    Ok(blobs)
  }

  /// The texts of `ids` that the memory cache holds, by id, and the ids it holds nothing of, for a caller that reads
  /// those in a batch of its own; no git. A binary or too large blob is in neither
  pub fn texts_in_memory(&self, ids: &[String]) -> (HashMap<String, String>, Vec<String>) {
    let mut texts = HashMap::new();
    let mut unread = Vec::new();
    for id in ids {
      match self.blobs.get(id).as_deref() {
        Some(BlobText::Text { text, .. }) => {
          texts.insert(id.clone(), text.clone());
        }
        Some(_) => {}
        None => unread.push(id.clone()),
      }
    }
    (texts, unread)
  }

  /// The texts of blobs, in the order of `requests`: from the memory cache, the working tree (a request with a path,
  /// whose file must have the id), or one `cat-file --batch`
  #[instrument(skip(self, git, requests), fields(blobs = requests.len()))]
  pub fn blob_texts(&self, git: &GitCommandExecutor, repository_path: &str, requests: &[BlobRequest]) -> Result<Vec<BlobText>> {
    if requests.len() > MAX_BLOB_REQUESTS {
      bail!("Too many blobs: {} (at most {MAX_BLOB_REQUESTS})", requests.len());
    }
    if let Some(request) = requests.iter().find(|request| !is_object_id(&request.id)) {
      bail!("Not an object id: {}", request.id);
    }
    let mut texts: Vec<Option<Arc<BlobText>>> = Vec::with_capacity(requests.len());
    for request in requests {
      let mut text = self.blobs.get(&request.id);
      if text.is_none()
        && let Some(path) = &request.path
      {
        text = read_worktree_blob(repository_path, path, &request.id)?.filter(|blob| blob.id() == request.id).map(Arc::new);
      }
      if let Some(blob) = &text {
        self.blobs.insert(request.id.clone(), Arc::clone(blob));
      }
      texts.push(text);
    }
    let missing: Vec<String> = requests
      .iter()
      .zip(&texts)
      .filter(|(_, text)| text.is_none())
      .map(|(request, _)| request.id.clone())
      .collect();
    let read = self.read_blobs(git, repository_path, missing.iter().map(String::as_str))?;
    Ok(
      requests
        .iter()
        .zip(texts)
        .map(|(request, text)| {
          let blob = text.or_else(|| read.get(&request.id).cloned());
          blob.map_or_else(|| BlobText::Missing { id: request.id.clone() }, |blob| (*blob).clone())
        })
        .collect(),
    )
  }
}

/// Keeps what the store had of the `missed` files in memory and hands it to `emit`; the files it misses, in order
pub(crate) fn keep_stored<K, V>(missed: Vec<usize>, stored: Vec<Option<V>>, keys: &[K], memory: &Cache<K, Arc<V>>, mut emit: impl FnMut(usize, V)) -> Vec<usize>
where
  K: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
  V: Clone + Send + Sync + 'static,
{
  let mut pending = Vec::new();
  for (index, known) in missed.into_iter().zip(stored) {
    match known {
      Some(known) => {
        memory.insert(keys[index].clone(), Arc::new(known.clone()));
        emit(index, known);
      }
      None => pending.push(index),
    }
  }
  pending
}

/// The changed files of a subject
fn list_entries(git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, range_files: &RangeFilesCache, source: &DiffSource) -> Result<Vec<Entry>> {
  Ok(match source {
    DiffSource::Commit { id } => read_commit_files_cached(git, repository_path, cache, std::slice::from_ref(id))?
      .pop()
      .map_or_default(|files| files.files)
      .into_iter()
      .map(Entry::from)
      .collect(),
    DiffSource::Range { base, head } => {
      let subject = DiffSubject::Range {
        base: base.clone(),
        head: head.clone(),
      };
      range_files.read(git, repository_path, &subject)?.files.iter().cloned().map(Entry::from).collect()
    }
    DiffSource::Uncommitted => uncommitted_entries(git, repository_path)?,
  })
}

/// The job of a file whose sides are all read, which leaves the files in flight
fn take_job(sides: &mut HashMap<usize, (Side, Side)>, index: usize) -> Option<Job> {
  let (old, new) = sides.get(&index)?;
  if matches!(old, Side::Pending(_)) || matches!(new, Side::Pending(_)) {
    return None;
  }
  let (old, new) = sides.remove(&index)?;
  let blob = |side: Side| match side {
    Side::Blob(blob) => Some(blob),
    Side::Empty | Side::Pending(_) => None,
  };
  Some(Job {
    index,
    old: blob(old),
    new: blob(new),
  })
}

/// What the workers of one `stream` share
struct WorkContext<'a> {
  service: &'a FileDiffs,
  entries: &'a [Entry],
  keys: &'a [MemoryKey],
  policy: WhitespacePolicy,
  /// The summaries of the sides from the `blob_summary` store, by language and blob id
  summaries: &'a HashMap<(u8, String), FileSummary>,
}

/// A worker: takes the next job until the jobs end
fn work_loop(jobs: &Mutex<Receiver<Job>>, outcomes: &Sender<Outcome>, context: &WorkContext<'_>) {
  loop {
    let job = jobs.lock().unwrap_or_else(PoisonError::into_inner).recv();
    let Ok(job) = job else { return };
    work(job, outcomes, context);
  }
}

/// Compares one file: its content goes out first, a text with its marks, then the scopes of a text; the diff with
/// its scopes is cached
fn work(job: Job, outcomes: &Sender<Outcome>, context: &WorkContext<'_>) {
  let Job { index, old, new } = job;
  let entry = &context.entries[index];
  let key = &context.keys[index];
  let send = |outcome: Outcome| {
    // the receiver stays until every worker ends
    let _ = outcomes.send(outcome);
  };
  let content_event = |content: DiffContent| {
    Outcome::Event(FileDiffEvent::Content {
      index: to_u32(index),
      collapse: collapse_of(&entry.path, &content),
      content,
    })
  };
  let sides = [old.as_deref(), new.as_deref()];
  let has = |kind: fn(&BlobText) -> bool| sides.iter().flatten().any(|blob| kind(blob));
  if has(|blob| matches!(blob, BlobText::Missing { .. })) {
    // git may fetch a missing blob later, so that content is not cached
    send(content_event(DiffContent::Missing));
    return;
  }
  let content = if has(|blob| matches!(blob, BlobText::Binary { .. })) {
    DiffContent::Binary
  } else if has(|blob| matches!(blob, BlobText::TooLarge { .. })) {
    DiffContent::TooLarge {
      old_bytes: sides[0].map_or(0, BlobText::size),
      new_bytes: sides[1].map_or(0, BlobText::size),
    }
  } else {
    let (old_text, new_text) = (text_of(sides[0]), text_of(sides[1]));
    let limits = context.service.limits;
    let budget = DiffBudget {
      inner: Some(Deadline::after(limits.file)),
      lines: Deadline::after(limits.lines),
    };
    let BudgetedDiff { mut diff, out_of_time } = diff_texts_in(old_text, new_text, context.policy, &budget);
    // the marks before the content: they change the rows of the file
    let paths = [entry.old_path.as_deref().unwrap_or(&entry.path), &entry.path];
    let left = if context.service.marks_off {
      // the comparison alone, each block a stop, as it gave them
      Leftovers::default()
    } else {
      let pair = context
        .service
        .marks(&diff, context.policy, paths, [entry.old_id.as_deref(), entry.new_id.as_deref()], [old_text, new_text]);
      // what the pair leaves to the other files of its subjects is read here, where its texts are; what it leaves to
      // the measurement of its proofs (`PairMarks::proven`) is read again by that measurement (`marks_measure`)
      let (marks, left) = Leftovers::of(&diff.blocks, pair, old_text, new_text, pair_indented(paths[0], paths[1]));
      diff.marks = marks;
      left
    };
    let left = Arc::new(left);
    // memory keeps the diff with what it left, and the stream gets both for the pass over its subject
    let keep = |diff: &TextDiff| {
      let content = Arc::new(DiffContent::Text(diff.clone()));
      context.service.diffs.insert(key.clone(), Arc::clone(&content));
      context.service.marks_memory.leftovers.insert(key.clone(), Arc::clone(&left));
      send(Outcome::Text {
        index,
        text: (content, Arc::clone(&left)),
      });
    };
    // a diff the time cut short, of its inner ranges or of its lines, stays in memory only
    let cut_short = out_of_time;
    let old_side = entry.old_language().zip(entry.old_id.as_deref());
    let new_side = language_of(&entry.path).zip(entry.new_id.as_deref());
    let mut summaries = Vec::new();
    if old_side.is_some() || new_side.is_some() {
      // the blocks first: the parse of the scopes costs more than the comparison, and changes no row
      send(content_event(DiffContent::Text(diff.clone())));
      let lines = [TextLines::of(old_text), TextLines::of(new_text)];
      let old_imports = old_side.map(|(language, id)| {
        let path = entry.old_path.as_deref().unwrap_or(&entry.path);
        let (scopes, imports) = side_facts(context.summaries, language, path, id, old_text, &lines[0], &mut summaries);
        diff.old_scopes = scopes;
        imports
      });
      let new_imports = new_side.map(|(language, id)| {
        let (scopes, imports) = side_facts(context.summaries, language, &entry.path, id, new_text, &lines[1], &mut summaries);
        diff.new_scopes = scopes;
        imports
      });
      diff.import_blocks = import_blocks(&diff, [&lines[0], &lines[1]], [old_imports.as_deref(), new_imports.as_deref()]);
      send(Outcome::Event(FileDiffEvent::Scopes {
        index: to_u32(index),
        old_scopes: diff.old_scopes.clone(),
        new_scopes: diff.new_scopes.clone(),
        import_blocks: diff.import_blocks.clone(),
      }));
      keep(&diff);
      send(Outcome::Store {
        diff: (!cut_short).then(|| (key.clone(), diff, left)),
        summaries,
      });
      return;
    }
    send(content_event(DiffContent::Text(diff.clone())));
    keep(&diff);
    if !cut_short {
      send(Outcome::Store {
        diff: Some((key.clone(), diff, left)),
        summaries,
      });
    }
    return;
  };
  context.service.diffs.insert(key.clone(), Arc::new(content.clone()));
  send(content_event(content));
}

/// The scopes and the lines of the import statements of a side: from its cached summary, else from a summary made
/// now, kept for the store unless truncated; no lines of a truncated summary. Both by the lines of the diffs (`lines`,
/// of `text`), which the blocks name: a summary counts the lines of git, and a lone `\r` above a statement or a
/// declaration would else give its lines to its neighbours (`TextLines::of_git`)
pub(crate) fn side_facts(
  summaries: &HashMap<(u8, String), FileSummary>,
  language: Language,
  path: &str,
  id: &str,
  text: &str,
  lines: &TextLines<'_>,
  made: &mut Vec<((u8, String), FileSummary)>,
) -> (Vec<Scope>, Vec<LineSpan>) {
  let facts = |summary: &FileSummary| {
    let imports = summary.import_lines.iter().map(|span| {
      let (start, end) = lines.of_summary(span.start, span.end);
      LineSpan { start, end }
    });
    (scopes_of_summary(summary, lines), imports.collect())
  };
  let key = (summary_key(language, path), id.to_string());
  if let Some(summary) = summaries.get(&key) {
    return facts(summary);
  }
  let summary = summarize(language, path, text.as_bytes());
  let facts = facts(&summary);
  if !summary.truncated {
    made.push((key, summary));
  }
  facts
}

/// The blocks of `diff` whose every changed line of each side is in an import statement of that side (`imports`, the
/// lines of a side's statements as `side_facts` gives them; `None` for a side of no known language) or blank, with a
/// line of a statement at least; `lines` the lines of each side as the blocks count them
pub(crate) fn import_blocks(diff: &TextDiff, lines: [&TextLines<'_>; 2], imports: [Option<&[LineSpan]>; 2]) -> Vec<u32> {
  diff
    .blocks
    .iter()
    .enumerate()
    .filter(|(_, block)| block.kind != BlockKind::Ignored)
    .filter(|(_, block)| imports_only([block.old_start..block.old_end, block.new_start..block.new_end], lines.map(TextLines::all), imports))
    .map(|(index, _)| to_u32(index))
    .collect()
}

/// Whether the changed lines of a block (`changed`, of each side from 0, the end excluded) are each in an import
/// statement of their side (`imports`, the lines of a side's statements; `None` for a side of no known language) or
/// blank, with a line of a statement at least; `lines` the text of each side by line. The blocks, the statements and
/// the text count the lines of the diffs (`line_diff::text_lines`), never those of git. The one test of a change of
/// the imports alone, for the diffs and the units of a review (`review_plan::units`)
pub fn imports_only(changed: [Range<u32>; 2], lines: [&[&str]; 2], imports: [Option<&[LineSpan]>; 2]) -> bool {
  // per side, by 0-based line: in a statement, blank, else code
  let kind = |side: usize, line: u32| -> Option<bool> {
    let text = lines[side].get(line as usize).copied().unwrap_or_default();
    if imports[side]?.iter().any(|span| span.start <= line + 1 && line < span.end) {
      return Some(true);
    }
    text.trim().is_empty().then_some(false)
  };
  let kinds: Option<Vec<bool>> = changed
    .into_iter()
    .enumerate()
    .flat_map(|(side, range)| range.map(move |line| (side, line)))
    .map(|(side, line)| kind(side, line))
    .collect();
  kinds.is_some_and(|kinds| kinds.contains(&true))
}

/// The indices of `paths` in the order of the work: the `priority_paths` that the subject has, in their order, then the
/// others in the order of the tree of the views (`src/lib/file-tree.ts`)
pub(crate) fn priority_order(paths: &[&str], priority_paths: &[String]) -> Vec<usize> {
  let by_path: HashMap<&str, usize> = paths.iter().enumerate().map(|(index, path)| (*path, index)).collect();
  let mut order: Vec<usize> = Vec::with_capacity(paths.len());
  let mut taken = vec![false; paths.len()];
  for path in priority_paths {
    if let Some(&index) = by_path.get(path.as_str())
      && !taken[index]
    {
      taken[index] = true;
      order.push(index);
    }
  }
  let mut rest: Vec<usize> = (0..paths.len()).filter(|&index| !taken[index]).collect();
  rest.sort_by(|&a, &b| compare_tree_paths(paths[a], paths[b]));
  order.extend(rest);
  order
}

/// The order of two paths in the tree: at the first directory level where they differ, a directory before a file,
/// then the names as a person reads them (`file2` before `file10`, case-insensitive); also the tiebreak of the
/// dependency order of a review (`review_plan::order`)
pub fn compare_tree_paths(a: &str, b: &str) -> Ordering {
  let (mut a_parts, mut b_parts) = (a.split('/').peekable(), b.split('/').peekable());
  loop {
    match (a_parts.next(), b_parts.next()) {
      (Some(a_name), Some(b_name)) => {
        let (a_directory, b_directory) = (a_parts.peek().is_some(), b_parts.peek().is_some());
        if a_name != b_name || a_directory != b_directory {
          return b_directory.cmp(&a_directory).then_with(|| compare_names(a_name, b_name));
        }
      }
      (a_name, b_name) => return a_name.is_some().cmp(&b_name.is_some()),
    }
  }
}

/// Names as a person reads them: digit runs by their value, the rest without case
fn compare_names(a: &str, b: &str) -> Ordering {
  let (mut a_chars, mut b_chars) = (a.chars().peekable(), b.chars().peekable());
  loop {
    match (a_chars.peek().copied(), b_chars.peek().copied()) {
      (None, None) => return a.cmp(b),
      (None, Some(_)) => return Ordering::Less,
      (Some(_), None) => return Ordering::Greater,
      (Some(a_char), Some(b_char)) if a_char.is_ascii_digit() && b_char.is_ascii_digit() => {
        let (a_run, b_run) = (digit_run(&mut a_chars), digit_run(&mut b_chars));
        let (a_value, b_value) = (a_run.trim_start_matches('0'), b_run.trim_start_matches('0'));
        let order = a_value.len().cmp(&b_value.len()).then_with(|| a_value.cmp(b_value));
        if order != Ordering::Equal {
          return order;
        }
      }
      (Some(a_char), Some(b_char)) => {
        let order = a_char.to_lowercase().cmp(b_char.to_lowercase());
        if order != Ordering::Equal {
          return order;
        }
        a_chars.next();
        b_chars.next();
      }
    }
  }
}

fn digit_run(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
  let mut run = String::new();
  while let Some(digit) = chars.next_if(char::is_ascii_digit) {
    run.push(digit);
  }
  run
}

/// A file index or a line index; the subjects and the texts are far below 4 G
pub(crate) fn to_u32(value: usize) -> u32 {
  u32::try_from(value).expect("an index below 4 G")
}

/// The summaries that the `blob_summary` store has for the sides of the `pending` files, which the sync of the
/// branches made already for most of them
fn cached_summaries(cache: &ObjectCache, entries: &[Entry], pending: &[usize]) -> HashMap<(u8, String), FileSummary> {
  let keys: Vec<(u8, &str)> = pending
    .iter()
    .flat_map(|&index| {
      let entry = &entries[index];
      let old = language_of(entry.old_path.as_deref().unwrap_or(&entry.path)).zip(entry.old_id.as_deref());
      let new = language_of(&entry.path).zip(entry.new_id.as_deref());
      [old, new].into_iter().flatten().map(|(language, id)| (language.id(), id))
    })
    .collect();
  read_summaries(cache, &keys)
}

/// The summaries that the `blob_summary` store has of the blobs `keys` (language, blob id)
pub(crate) fn read_summaries(cache: &ObjectCache, keys: &[(u8, &str)]) -> HashMap<(u8, String), FileSummary> {
  if keys.is_empty() {
    return HashMap::new();
  }
  keys
    .iter()
    .zip(cache.get_blob_summaries::<FileSummary>(extractor_version(), keys))
    .filter_map(|(&(language, id), summary)| summary.map(|summary| ((language, id.to_string()), summary)))
    .collect()
}

/// Writes the summaries that a read made, in one transaction
pub(crate) fn write_summaries(cache: &ObjectCache, summaries: &[((u8, String), FileSummary)]) {
  let entries: Vec<((u8, &str), &FileSummary)> = summaries.iter().map(|((language, id), summary)| ((*language, id.as_str()), summary)).collect();
  cache.put_blob_summaries(extractor_version(), &entries);
}

/// The text of a side; an empty side is the empty text
fn text_of(blob: Option<&BlobText>) -> &str {
  match blob {
    Some(BlobText::Text { text, .. }) => text,
    _ => "",
  }
}

/// Whether a text is within the size the inner ranges are compared for (`diff_texts`): a diff by lines alone of two
/// such texts is one the time cut short, and only such texts are read for the runs of their marks
pub(crate) const fn fits_inner(text: &str, line_count: u32) -> bool {
  text.len() <= MAX_INNER_BYTES && line_count as usize <= MAX_INNER_LINES
}

/// A blob as `cat-file --batch` gave it
pub(crate) fn blob_text(id: String, entry: BatchEntry<'_>) -> BlobText {
  match entry {
    BatchEntry::Object { kind: "blob", content, .. } if is_binary(&content) => BlobText::Binary { id },
    BatchEntry::Object { kind: "blob", content, .. } => BlobText::Text { id, text: into_text(content) },
    BatchEntry::TooLarge { kind: "blob", size, .. } => BlobText::TooLarge {
      id,
      size: u32::try_from(size).unwrap_or(u32::MAX),
    },
    _ => BlobText::Missing { id },
  }
}

/// The number of a policy in a cache key
pub(crate) const fn policy_id(policy: WhitespacePolicy) -> u8 {
  match policy {
    WhitespacePolicy::Default => 0,
    WhitespacePolicy::Trim => 1,
    WhitespacePolicy::Ignore => 2,
    WhitespacePolicy::IgnoreBlankLines => 3,
  }
}

/// The options of the diff of a file at `path`, `old_path` before a rename, under `policy`: what the key of its diff
/// holds beside the blobs, in memory and in the store. The one function of every reader of a diff (the views, and the
/// scans of `blocks_of`, which find the entry a view made only under the same key). The grammar of each side, not its
/// language: the scopes and the marks of the same bytes differ as a `.ts` and a `.tsx`, a `.kt` and a `.kts`, a
/// `BUILD` and a `.bzl` file. And of a side without a grammar whether its indentation has a meaning
/// (`code_symbols::tokens::indentation_matters`, which a grammar decides for its files): the moves of the same lines
/// differ as a `.py` and a `.txt` file
pub(crate) fn diff_options(policy: WhitespacePolicy, old_path: Option<&str>, path: &str) -> DiffOptions {
  let grammar = |path: &str| grammar_of(path).map_or_else(|| if indentation_matters(path) { INDENTED } else { 0 }, |grammar| grammar.id() + 1);
  DiffOptions {
    policy: policy_id(policy),
    grammars: [grammar(old_path.unwrap_or(path)), grammar(path)],
  }
}

fn memory_key(entry: &Entry, policy: WhitespacePolicy) -> MemoryKey {
  (
    diff_options(policy, entry.old_path.as_deref(), &entry.path),
    entry.old_id.clone().unwrap_or_default(),
    entry.new_id.clone().unwrap_or_default(),
  )
}

pub(crate) fn disk_key(key: &MemoryKey) -> DiffKey<'_> {
  DiffKey {
    policy: key.0.policy,
    grammars: key.0.grammars,
    old_id: &key.1,
    new_id: &key.2,
  }
}

/// The weight of a diff in the memory cache: its blocks, ranges, scopes and marks, roughly in bytes
fn content_weight(content: &DiffContent) -> u32 {
  let DiffContent::Text(diff) = content else { return 64 };
  let ranges: usize = diff
    .blocks
    .iter()
    .map(|block| block.old_ranges.iter().chain(&block.new_ranges).map(Vec::len).sum::<usize>())
    .sum();
  let scopes: usize = diff.old_scopes.iter().chain(&diff.new_scopes).map(|scope| scope.label.len() + 16).sum();
  u32::try_from(128 + diff.blocks.len() * 48 + ranges * 4 + scopes + marks_weight(&diff.marks)).unwrap_or(u32::MAX)
}

/// The weight of the marks of a diff, roughly in bytes
fn marks_weight(marks: &DiffMarks) -> usize {
  let moves: usize = marks
    .moves
    .iter()
    .map(|it| 96 + it.edited.len() * 4 + it.old_ranges.iter().chain(&it.new_ranges).map(|ranges| 24 + ranges.len() * 4).sum::<usize>())
    .sum();
  let renames: usize = marks
    .renames
    .iter()
    .map(|it| 88 + it.old.len() + it.new.len() + (it.declared.len() + it.old_sites.len() + it.new_sites.len()) * 4)
    .sum();
  marks.runs.len() * size_of::<LineRun>() + moves + renames + marks.starts.len() * 4
}
