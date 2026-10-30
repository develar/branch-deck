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
//! - **Caches.** A diff is keyed by its blob pair, the whitespace policy and the languages of the scopes (`DiffKey`):
//!   blobs never change, so one entry serves every commit, range and window that shows the pair. Memory first
//!   (`DIFF_CACHE_BYTES`), then the store. A blob read for a diff stays in a memory cache (`BLOB_CACHE_BYTES`), so
//!   the texts that a view asks for next cost no process.
//! - **Budgets.** A side over `MAX_BLOB_BYTES` is `TooLarge`, a binary one `Binary`. Each file has `DiffLimits::file`
//!   for its inner ranges and `DiffLimits::lines` for its lines, for a pathological file only: no budget spans the
//!   subject, so the result does not depend on the load of the machine or the order of the files. A diff that the time
//!   cut short (`line_level_only` within the size limits) stays in memory only: the store never keeps it.

use crate::merge::{ConflictPath, MergeMemoryKey, merge_weight};
use crate::model::{BlobRequest, BlobText, DiffContent, DiffFile, DiffFileHeader, DiffSource, FileDiffEvent, GetFileDiffsParams, MergeContent, MergeSource};
use crate::range_files::RangeFilesCache;
use crate::scopes::scopes_of_summary;
use crate::syntax::{SYNTAX_CACHE_BYTES, syntax_weight};
use crate::worktree::{read_worktree_blob, uncommitted_entries};
use anyhow::{Result, bail};
use code_symbols::extract::{extractor_version, summarize};
use code_symbols::highlight::lines::LineRuns;
use code_symbols::language::{Language, language_of};
use code_symbols::summary::{FileSummary, LineSpan};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cat_file_batch::{BatchEntry, for_each_object};
use git_ops::commit_files::{DiffSubject, MAX_BLOB_BYTES, read_commit_files_cached};
use git_ops::object_texts::{into_text, is_binary};
use git_ops::revs::is_object_id;
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
use text_diff::cancellation::Deadline;
use text_diff::model::{BlockKind, DiffBudget, MAX_INNER_BYTES, MAX_INNER_LINES, Scope, TextDiff, diff_texts};
use text_diff::policy::WhitespacePolicy;
use tracing::{debug, instrument};

/// The version of the diffs in the `file_diff` store: bump it when `TextDiff`, the comparison or the scopes change
pub const MODEL_VERSION: u32 = 5;

/// The diffs that memory keeps, weighed by the size of their model
pub const DIFF_CACHE_BYTES: u64 = 64 << 20;
/// The blobs that memory keeps, weighed by their size
pub const BLOB_CACHE_BYTES: u64 = 128 << 20;
/// The merges whose conflicting paths memory keeps
const MAX_CACHED_CONFLICTS: u64 = 256;
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

/// The memory key of a diff: the options byte of `DiffKey` and the two blob ids, empty for a missing side
type MemoryKey = (u8, String, String);

/// The diffs and the blobs of the views, shared by every window; cheap to clone
#[derive(Clone)]
pub struct FileDiffs {
  pub(crate) diffs: Cache<MemoryKey, Arc<DiffContent>>,
  pub(crate) merges: Cache<MergeMemoryKey, Arc<MergeContent>>,
  /// The conflicting paths of a merge with their stages, by repository and merge
  pub(crate) conflicts: Cache<(String, MergeSource), Arc<Vec<ConflictPath>>>,
  pub(crate) blobs: Cache<String, Arc<BlobText>>,
  /// The tree-sitter highlighting of blobs by grammar and id, `None` when a blob has none (`syntax.rs`)
  pub(crate) syntax: Cache<(u8, String), Arc<Option<LineRuns>>>,
  pub(crate) limits: DiffLimits,
  /// `blocks_of` keeps the blobs it reads in `blobs` (`for_scans`)
  pub(crate) keep_scan_blobs: bool,
  /// The files of the ranges: the list of a review (`get_range_files`) and its diffs (`stream`) run one `diff-tree`
  pub range_files: RangeFilesCache,
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
  /// A diff and summaries for the store
  Store {
    diff: Option<(MemoryKey, TextDiff)>,
    summaries: Vec<((u8, String), FileSummary)>,
  },
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
      limits,
      keep_scan_blobs: false,
      range_files: RangeFilesCache::default(),
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

  /// The diff of each changed file of `source` under `policy`, in the order of git, at once: `stream` collected
  pub fn read(&self, git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, source: &DiffSource, policy: WhitespacePolicy) -> Result<Vec<DiffFile>> {
    let mut headers = Vec::new();
    let mut contents: Vec<Option<DiffContent>> = Vec::new();
    let params = GetFileDiffsParams {
      repository_path: repository_path.to_string(),
      source: source.clone(),
      policy,
      priority_paths: Vec::new(),
    };
    self.stream(git, cache, &params, |event| match event {
      FileDiffEvent::Files { files } => {
        contents = vec![None; files.len()];
        headers = files;
      }
      FileDiffEvent::Content { index, content } => contents[index as usize] = Some(content),
      FileDiffEvent::Scopes {
        index,
        old_scopes,
        new_scopes,
        import_blocks,
      } => {
        if let Some(DiffContent::Text(diff)) = &mut contents[index as usize] {
          diff.old_scopes = old_scopes;
          diff.new_scopes = new_scopes;
          diff.import_blocks = import_blocks;
        }
      }
    })?;
    Ok(
      headers
        .into_iter()
        .zip(contents)
        .map(|(header, content)| DiffFile {
          path: header.path,
          old_path: header.old_path,
          status: header.status,
          old_id: header.old_id,
          new_id: header.new_id,
          // every file gets a content; one left out would be a bug, shown as missing rather than failing the subject
          content: content.unwrap_or(DiffContent::Missing),
        })
        .collect(),
    )
  }

  /// The diffs of a subject as events (`FileDiffEvent`): the files, then the content of each file as it is known,
  /// `priority_paths` first, then the others in the order of the tree
  #[instrument(skip(self, git, cache, params, emit), fields(repository_path = %params.repository_path, source = ?params.source))]
  pub fn stream(&self, git: &GitCommandExecutor, cache: &ObjectCache, params: &GetFileDiffsParams, mut emit: impl FnMut(FileDiffEvent)) -> Result<()> {
    let (repository_path, policy) = (params.repository_path.as_str(), params.policy);
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
    let content_event = |index: usize, content: DiffContent| FileDiffEvent::Content { index: to_u32(index), content };

    let mut missed = Vec::new();
    for &index in &order {
      let entry = &entries[index];
      let known = match &entry.submodule {
        Some((old_commit, new_commit)) => Some(DiffContent::Submodule {
          old_commit: old_commit.clone(),
          new_commit: new_commit.clone(),
        }),
        None if entry.binary => Some(DiffContent::Binary),
        None => self.diffs.get(&keys[index]).map(|diff| (*diff).clone()),
      };
      match known {
        Some(known) => emit(content_event(index, known)),
        None => missed.push(index),
      }
    }
    let disk_keys: Vec<DiffKey<'_>> = missed.iter().map(|&index| disk_key(&keys[index])).collect();
    let stored: Vec<Option<TextDiff>> = cache.get_file_diffs(MODEL_VERSION, &disk_keys);
    let stored = stored.into_iter().map(|diff| diff.map(DiffContent::Text)).collect();
    let pending = keep_stored(missed, stored, &keys, &self.diffs, |index, known| emit(content_event(index, known)));
    debug!(files = entries.len(), cached = entries.len() - pending.len(), "file diffs");
    if pending.is_empty() {
      return Ok(());
    }

    // the sides of each pending file, and the files that wait for each blob, in the order of the work
    let mut sides: HashMap<usize, (Side, Side)> = HashMap::with_capacity(pending.len());
    let mut waiting: HashMap<String, Vec<usize>> = HashMap::new();
    let mut ids: Vec<String> = Vec::new();
    for &index in &pending {
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

    let summaries = cached_summaries(cache, &entries, &pending);
    let context = WorkContext {
      service: self,
      entries: &entries,
      keys: &keys,
      policy,
      summaries: &summaries,
    };
    let mut to_store: Vec<(MemoryKey, TextDiff)> = Vec::new();
    let mut new_summaries: Vec<((u8, String), FileSummary)> = Vec::new();
    let read = std::thread::scope(|scope| {
      let (job_sender, job_receiver) = channel::<Job>();
      let job_receiver = Arc::new(Mutex::new(job_receiver));
      let (outcome_sender, outcomes) = channel::<Outcome>();
      spawn_workers(scope, pending.len(), || {
        let (jobs, outcome_sender, context) = (Arc::clone(&job_receiver), outcome_sender.clone(), &context);
        move || work_loop(&jobs, &outcome_sender, context)
      });
      drop(outcome_sender);
      let mut deliver = |outcome: Outcome| match outcome {
        Outcome::Event(event) => emit(event),
        Outcome::Store { diff, summaries } => {
          to_store.extend(diff);
          new_summaries.extend(summaries);
        }
      };

      // the files whose sides are all known go first
      for &index in &pending {
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

    let entries_to_store: Vec<(DiffKey<'_>, &TextDiff)> = to_store.iter().map(|(key, diff)| (disk_key(key), diff)).collect();
    cache.put_file_diffs(MODEL_VERSION, &entries_to_store);
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

/// Compares one file: its content goes out first, then the scopes of a text; the diff with its scopes is cached
fn work(job: Job, outcomes: &Sender<Outcome>, context: &WorkContext<'_>) {
  let Job { index, old, new } = job;
  let entry = &context.entries[index];
  let key = &context.keys[index];
  let send = |outcome: Outcome| {
    // the receiver stays until every worker ends
    let _ = outcomes.send(outcome);
  };
  let content_event = |content: DiffContent| Outcome::Event(FileDiffEvent::Content { index: to_u32(index), content });
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
    let mut diff = diff_texts(old_text, new_text, context.policy, &budget);
    let cut_short = diff.line_level_only && fits_inner(old_text, diff.old_line_count) && fits_inner(new_text, diff.new_line_count);
    let old_side = entry.old_language().zip(entry.old_id.as_deref());
    let new_side = language_of(&entry.path).zip(entry.new_id.as_deref());
    let mut summaries = Vec::new();
    if old_side.is_some() || new_side.is_some() {
      // the blocks first: the parse of the scopes costs more than the comparison
      send(content_event(DiffContent::Text(diff.clone())));
      let old_imports = old_side.map(|(language, id)| {
        let (scopes, imports) = side_facts(context.summaries, language, id, old_text, &mut summaries);
        diff.old_scopes = scopes;
        imports
      });
      let new_imports = new_side.map(|(language, id)| {
        let (scopes, imports) = side_facts(context.summaries, language, id, new_text, &mut summaries);
        diff.new_scopes = scopes;
        imports
      });
      diff.import_blocks = import_blocks(&diff, [old_text, new_text], [old_imports.as_deref(), new_imports.as_deref()]);
      send(Outcome::Event(FileDiffEvent::Scopes {
        index: to_u32(index),
        old_scopes: diff.old_scopes.clone(),
        new_scopes: diff.new_scopes.clone(),
        import_blocks: diff.import_blocks.clone(),
      }));
      context.service.diffs.insert(key.clone(), Arc::new(DiffContent::Text(diff.clone())));
      send(Outcome::Store {
        diff: (!cut_short).then(|| (key.clone(), diff)),
        summaries,
      });
      return;
    }
    if !cut_short {
      send(Outcome::Store {
        diff: Some((key.clone(), diff.clone())),
        summaries,
      });
    }
    DiffContent::Text(diff)
  };
  context.service.diffs.insert(key.clone(), Arc::new(content.clone()));
  send(content_event(content));
}

/// The scopes of a side: from its cached summary, else from a summary made now, kept for the store unless truncated
pub(crate) fn scopes(summaries: &HashMap<(u8, String), FileSummary>, language: Language, id: &str, text: &str, made: &mut Vec<((u8, String), FileSummary)>) -> Vec<Scope> {
  side_facts(summaries, language, id, text, made).0
}

/// The scopes and the lines of the import statements of a side (`scopes`); no lines of a truncated summary
fn side_facts(
  summaries: &HashMap<(u8, String), FileSummary>,
  language: Language,
  id: &str,
  text: &str,
  made: &mut Vec<((u8, String), FileSummary)>,
) -> (Vec<Scope>, Vec<LineSpan>) {
  let key = (language.id(), id.to_string());
  if let Some(summary) = summaries.get(&key) {
    return (scopes_of_summary(summary), summary.import_lines.clone());
  }
  let summary = summarize(language, text.as_bytes());
  let facts = (scopes_of_summary(&summary), summary.import_lines.clone());
  if !summary.truncated {
    made.push((key, summary));
  }
  facts
}

/// The blocks of `diff` whose every changed line of each side is in an import statement of that side (`imports`, the
/// lines of a side's statements; `None` for a side of no known language) or blank, with a line of a statement at least
pub(crate) fn import_blocks(diff: &TextDiff, texts: [&str; 2], imports: [Option<&[LineSpan]>; 2]) -> Vec<u32> {
  let lines: [Vec<&str>; 2] = texts.map(|text| text.split('\n').collect());
  diff
    .blocks
    .iter()
    .enumerate()
    .filter(|(_, block)| block.kind != BlockKind::Ignored)
    .filter(|(_, block)| imports_only([block.old_start..block.old_end, block.new_start..block.new_end], [&lines[0], &lines[1]], imports))
    .map(|(index, _)| to_u32(index))
    .collect()
}

/// Whether the changed lines of a block (`changed`, of each side from 0, the end excluded) are each in an import
/// statement of their side (`imports`, the lines of a side's statements; `None` for a side of no known language) or
/// blank, with a line of a statement at least; `lines` the text of each side by line. The one test of a change of the
/// imports alone, for the diffs and the units of a review (`review_plan::units`)
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

/// Whether the comparison gave a text its inner ranges only for lack of time
const fn fits_inner(text: &str, line_count: u32) -> bool {
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

/// The options byte of a diff: the policy in bits 0-1, the language of the new side in bits 2-4 and of the old side
/// in bits 5-7 (`Language::id` + 1, 0 without a grammar), since the scopes depend on the language of the path
pub(crate) fn options(policy: WhitespacePolicy, old_language: Option<Language>, new_language: Option<Language>) -> u8 {
  let policy = match policy {
    WhitespacePolicy::Default => 0,
    WhitespacePolicy::Trim => 1,
    WhitespacePolicy::Ignore => 2,
    WhitespacePolicy::IgnoreBlankLines => 3,
  };
  let language = |language: Option<Language>| language.map_or(0, |language| language.id() + 1);
  policy | (language(new_language) << 2) | (language(old_language) << 5)
}

fn memory_key(entry: &Entry, policy: WhitespacePolicy) -> MemoryKey {
  let old_language = language_of(entry.old_path.as_deref().unwrap_or(&entry.path));
  (
    options(policy, old_language, language_of(&entry.path)),
    entry.old_id.clone().unwrap_or_default(),
    entry.new_id.clone().unwrap_or_default(),
  )
}

fn disk_key(key: &MemoryKey) -> DiffKey<'_> {
  DiffKey {
    policy: key.0,
    old_id: &key.1,
    new_id: &key.2,
  }
}

/// The weight of a diff in the memory cache: its blocks, ranges and scopes, roughly in bytes
fn content_weight(content: &DiffContent) -> u32 {
  let DiffContent::Text(diff) = content else { return 64 };
  let ranges: usize = diff
    .blocks
    .iter()
    .map(|block| block.old_ranges.iter().chain(&block.new_ranges).map(Vec::len).sum::<usize>())
    .sum();
  let scopes: usize = diff.old_scopes.iter().chain(&diff.new_scopes).map(|scope| scope.label.len() + 16).sum();
  u32::try_from(128 + diff.blocks.len() * 48 + ranges * 4 + scopes).unwrap_or(u32::MAX)
}
