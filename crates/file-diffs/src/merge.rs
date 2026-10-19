//! The merges of the conflicting files of a cherry-pick, for the conflict view: git decides which files conflict and
//! with which stages (`merge-tree --write-tree`), and `text-diff` merges the three blobs of each, as IntelliJ's merge
//! view does.
//!
//! - **Processes.** One `merge-tree` for the stages, none when memory has the merge, and one `cat-file --batch` for the
//!   blobs, none when the caches have every merge.
//! - **Workers.** A conflict holds a few files, so the blobs are read first; then `workers()` threads merge the files,
//!   the files in view first, and the events go out as the results come.
//! - **Caches.** A merge is keyed by its three blobs, the policy and the languages of the scopes: memory first, then the
//!   `file_merge` store. A merge that the time cut short stays in memory only.

use crate::model::{BlobText, FileMerge, FileMergeEvent, FileMergeHeader, GetFileMergesParams, GitConflictKind, MergeContent, MergeSide, MergeSource};
use crate::service::{FileDiffs, keep_stored, options, priority_order, read_summaries, scopes, to_u32, workers, write_summaries};
use anyhow::{Result, bail};
use code_symbols::language::language_of;
use code_symbols::summary::FileSummary;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cat_file_batch::object_ids_and_types;
use git_ops::merge_tree_output::{MergeTreeOutput, parse_merge_tree_output};
use git_ops::revs::is_object_id;
use object_cache::cache::ObjectCache;
use object_cache::file_merge::FileMergeKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex, PoisonError};
use text_diff::cancellation::Deadline;
use text_diff::merge_model::{TextMerge, merge_texts};
use text_diff::model::{DiffBudget, MAX_INNER_BYTES, MAX_INNER_LINES, Scope, TextDiff, diff_texts};
use text_diff::policy::WhitespacePolicy;
use tracing::{debug, instrument};

/// The version of the merges in the `file_merge` store: bump it when `TextMerge`, the merge or the scopes change
pub(crate) const MERGE_MODEL_VERSION: u32 = 2;

/// The memory key of a merge: the options byte and the base, left and right blob ids, empty for a missing side
pub(crate) type MergeMemoryKey = (u8, String, String, String);

/// The submodule mode of a stage
const SUBMODULE_MODE: &str = "160000";

/// A conflicting path with its stages, as `merge-tree` reports it
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConflictPath {
  pub path: String,
  pub kind: GitConflictKind,
  /// The id of each stage: base, left, right
  pub ids: [Option<String>; 3],
  pub submodule: bool,
  /// The blob of the path in the copy that resolved the conflict by words
  pub resolved: Option<String>,
}

/// What the store keeps of a merge: postcard needs the default enum form, not the tagged form of the views
#[derive(Serialize, Deserialize)]
enum StoredMerge {
  Text(TextMerge),
  OneSided { deleted: MergeSide, diff: TextDiff },
}

impl StoredMerge {
  fn content(self) -> MergeContent {
    match self {
      Self::Text(merge) => MergeContent::Text(merge),
      Self::OneSided { deleted, diff } => MergeContent::OneSided { deleted, diff },
    }
  }
}

/// The weight of a merge in the memory cache, roughly in bytes
pub(crate) fn merge_weight(content: &MergeContent) -> u32 {
  let blocks = match content {
    MergeContent::Text(merge) => merge.blocks.len() * 3,
    MergeContent::OneSided { diff, .. } => diff.blocks.len(),
    _ => 0,
  };
  u32::try_from(128 + blocks * 64).unwrap_or(u32::MAX)
}

impl FileDiffs {
  /// The merge of each conflicting file of `params.source`, in the order of git, at once: `stream_merges` collected
  pub fn read_merges(&self, git: &GitCommandExecutor, cache: &ObjectCache, params: &GetFileMergesParams) -> Result<Vec<FileMerge>> {
    let mut headers = Vec::new();
    let mut contents: Vec<Option<MergeContent>> = Vec::new();
    self.stream_merges(git, cache, params, |event| match event {
      FileMergeEvent::Files { files } => {
        contents = vec![None; files.len()];
        headers = files;
      }
      FileMergeEvent::Content { index, content } => contents[index as usize] = Some(content),
      FileMergeEvent::Scopes {
        index,
        left_scopes,
        base_scopes,
        right_scopes,
      } => {
        if let Some(MergeContent::Text(merge)) = &mut contents[index as usize] {
          (merge.left_scopes, merge.base_scopes, merge.right_scopes) = (left_scopes, base_scopes, right_scopes);
        }
      }
    })?;
    Ok(
      headers
        .into_iter()
        .zip(contents)
        .map(|(header, content)| FileMerge {
          header,
          content: content.unwrap_or(MergeContent::Missing),
        })
        .collect(),
    )
  }

  /// The merges of the conflicting files as events (`FileMergeEvent`): the files, then the content of each file as it
  /// is known, `priority_paths` first, then the others in the order of the tree
  #[instrument(skip(self, git, cache, params, emit), fields(repository_path = %params.repository_path, source = ?params.source))]
  pub fn stream_merges(&self, git: &GitCommandExecutor, cache: &ObjectCache, params: &GetFileMergesParams, mut emit: impl FnMut(FileMergeEvent)) -> Result<()> {
    let paths = self.conflict_paths(git, &params.repository_path, &params.source)?;
    emit(FileMergeEvent::Files {
      files: paths.iter().map(header).collect(),
    });
    let names: Vec<&str> = paths.iter().map(|path| path.path.as_str()).collect();
    let order = priority_order(&names, &params.priority_paths);
    let keys: Vec<MergeMemoryKey> = paths.iter().map(|path| memory_key(path, params.policy)).collect();
    let content_event = |index: usize, content: MergeContent| FileMergeEvent::Content { index: to_u32(index), content };

    let mut missed = Vec::new();
    for &index in &order {
      let path = &paths[index];
      let known = if path.submodule {
        let [base_commit, left_commit, right_commit] = path.ids.clone();
        Some(MergeContent::Submodule {
          base_commit,
          left_commit,
          right_commit,
        })
      } else {
        self.merges.get(&keys[index]).map(|merge| (*merge).clone())
      };
      match known {
        Some(known) => emit(content_event(index, known)),
        None => missed.push(index),
      }
    }
    let disk_keys: Vec<FileMergeKey<'_>> = missed.iter().map(|&index| disk_key(&keys[index])).collect();
    let stored: Vec<Option<StoredMerge>> = cache.get_file_merges(MERGE_MODEL_VERSION, &disk_keys);
    let stored = stored.into_iter().map(|merge| merge.map(StoredMerge::content)).collect();
    let pending = keep_stored(missed, stored, &keys, &self.merges, |index, known| emit(content_event(index, known)));
    debug!(files = paths.len(), cached = paths.len() - pending.len(), "file merges");
    if pending.is_empty() {
      return Ok(());
    }

    let blobs = self.read_blobs(
      git,
      &params.repository_path,
      pending.iter().flat_map(|&index| paths[index].ids.iter().flatten().map(String::as_str)),
    )?;
    let summary_keys: Vec<(u8, &str)> = pending
      .iter()
      .flat_map(|&index| {
        let language = language_of(&paths[index].path);
        paths[index]
          .ids
          .iter()
          .flatten()
          .filter_map(move |id| language.map(|language| (language.id(), id.as_str())))
      })
      .collect();
    let summaries = read_summaries(cache, &summary_keys);

    let next = AtomicUsize::new(0);
    let stored = Mutex::new(Vec::<(MergeMemoryKey, StoredMerge)>::new());
    let made = Mutex::new(Vec::<((u8, String), FileSummary)>::new());
    std::thread::scope(|scope| {
      let (sender, events) = channel::<FileMergeEvent>();
      for _ in 0..workers().min(pending.len()) {
        let sender = sender.clone();
        let context = MergeWork {
          service: self,
          paths: &paths,
          keys: &keys,
          blobs: &blobs,
          summaries: &summaries,
          policy: params.policy,
        };
        let (next, pending, stored, made) = (&next, &pending, &stored, &made);
        scope.spawn(move || {
          while let Some(&index) = pending.get(next.fetch_add(1, Ordering::Relaxed)) {
            let (store, summaries) = context.merge(index, &mut |event| {
              // the receiver stays until every worker ends
              let _ = sender.send(event);
            });
            if let Some(store) = store {
              stored.lock().unwrap_or_else(PoisonError::into_inner).push(store);
            }
            made.lock().unwrap_or_else(PoisonError::into_inner).extend(summaries);
          }
        });
      }
      drop(sender);
      for event in events {
        emit(event);
      }
    });

    let stored = stored.into_inner().unwrap_or_else(PoisonError::into_inner);
    let entries: Vec<(FileMergeKey<'_>, &StoredMerge)> = stored.iter().map(|(key, merge)| (disk_key(key), merge)).collect();
    cache.put_file_merges(MERGE_MODEL_VERSION, &entries);
    write_summaries(cache, &made.into_inner().unwrap_or_else(PoisonError::into_inner));
    Ok(())
  }

  /// The conflicting paths of a merge: from memory, else from one `merge-tree --write-tree`
  fn conflict_paths(&self, git: &GitCommandExecutor, repository_path: &str, source: &MergeSource) -> Result<Arc<Vec<ConflictPath>>> {
    let key = (repository_path.to_string(), source.clone());
    if let Some(paths) = self.conflicts.get(&key) {
      return Ok(paths);
    }
    let MergeSource::CherryPick { parent, target, commit, resolved } = source;
    if let Some(id) = [parent, target, commit].into_iter().chain(resolved).find(|id| !is_object_id(id)) {
      bail!("Not an object id: {id}");
    }
    let args = ["merge-tree", "--write-tree", "-z", "--merge-base", parent, target, commit];
    // exit code 1 is a merge with conflicts
    let output = git.cmd(&args, repository_path).accept(&[1]).bytes()?;
    let mut paths = conflict_paths_of(&parse_merge_tree_output(&output)?);
    if let Some(copy) = resolved {
      // the blobs the sync committed: one `cat-file --batch-check`
      let names: Vec<String> = paths.iter().map(|path| format!("{copy}:{}", path.path)).collect();
      for (path, found) in paths.iter_mut().zip(object_ids_and_types(git, repository_path, &names)?) {
        path.resolved = found.filter(|(_, kind)| *kind == "blob").map(|(id, _)| id);
      }
    }
    let paths = Arc::new(paths);
    self.conflicts.insert(key, Arc::clone(&paths));
    Ok(paths)
  }
}

/// The conflicting paths of a merge in git's order, with the stages and the conflict type of each
fn conflict_paths_of(merge: &MergeTreeOutput) -> Vec<ConflictPath> {
  let mut paths: Vec<ConflictPath> = Vec::new();
  for entry in &merge.entries {
    let path = String::from_utf8_lossy(&entry.path).into_owned();
    if paths.last().is_none_or(|last| last.path != path) {
      let kind = conflict_kind(merge, &entry.path);
      paths.push(ConflictPath {
        path,
        kind,
        ids: [None, None, None],
        submodule: false,
        resolved: None,
      });
    }
    let current = paths.last_mut().expect("a path for the stage");
    current.submodule |= entry.mode == SUBMODULE_MODE;
    if let Some(slot) = current.ids.get_mut(usize::from(entry.stage).wrapping_sub(1)) {
      *slot = Some(entry.id.clone());
    }
  }
  // git types an add on both sides as a conflict of contents, without a base stage
  for path in &mut paths {
    if path.kind == GitConflictKind::Content && path.ids[0].is_none() {
      path.kind = GitConflictKind::AddAdd;
    }
  }
  paths
}

/// The type of the first conflict message about `path`
fn conflict_kind(merge: &MergeTreeOutput, path: &[u8]) -> GitConflictKind {
  let kind = merge
    .messages
    .iter()
    .filter(|message| message.kind.starts_with("CONFLICT"))
    .find(|message| message.paths.iter().any(|message_path| message_path == path))
    .map_or("", |message| message.kind.as_str());
  match kind {
    "CONFLICT (contents)" => GitConflictKind::Content,
    "CONFLICT (add/add)" => GitConflictKind::AddAdd,
    "CONFLICT (modify/delete)" => GitConflictKind::ModifyDelete,
    "CONFLICT (rename/delete)" => GitConflictKind::RenameDelete,
    "CONFLICT (rename/rename)" => GitConflictKind::RenameRename,
    "CONFLICT (binary)" => GitConflictKind::Binary,
    _ => GitConflictKind::Other,
  }
}

fn header(path: &ConflictPath) -> FileMergeHeader {
  let id = |index: usize| if path.submodule { None } else { path.ids[index].clone() };
  FileMergeHeader {
    path: path.path.clone(),
    kind: path.kind,
    base_id: id(0),
    left_id: id(1),
    right_id: id(2),
    resolved_id: path.resolved.clone(),
  }
}

fn memory_key(path: &ConflictPath, policy: WhitespacePolicy) -> MergeMemoryKey {
  let language = language_of(&path.path);
  let [base, left, right] = path.ids.clone().map(Option::unwrap_or_default);
  (options(policy, language, language), base, left, right)
}

fn disk_key(key: &MergeMemoryKey) -> FileMergeKey<'_> {
  FileMergeKey {
    options: key.0,
    base_id: &key.1,
    left_id: &key.2,
    right_id: &key.3,
  }
}

/// What the workers of one `stream_merges` share
struct MergeWork<'a> {
  service: &'a FileDiffs,
  paths: &'a [ConflictPath],
  keys: &'a [MergeMemoryKey],
  blobs: &'a HashMap<String, Arc<BlobText>>,
  summaries: &'a HashMap<(u8, String), FileSummary>,
  policy: WhitespacePolicy,
}

impl MergeWork<'_> {
  /// Merges one file: its content goes out first, then the scopes of a text merge. Returns what the store keeps, and the
  /// summaries made for the scopes
  #[expect(clippy::type_complexity, reason = "the two lists the caller collects")]
  fn merge(&self, index: usize, send: &mut dyn FnMut(FileMergeEvent)) -> (Option<(MergeMemoryKey, StoredMerge)>, Vec<((u8, String), FileSummary)>) {
    let path = &self.paths[index];
    let key = &self.keys[index];
    let blobs = path.ids.each_ref().map(|id| id.as_ref().and_then(|id| self.blobs.get(id)).map(Arc::as_ref));
    let content_event = |content: MergeContent| FileMergeEvent::Content { index: to_u32(index), content };
    let mut made = Vec::new();

    let any = |kind: fn(&BlobText) -> bool| blobs.iter().flatten().any(|blob| kind(blob));
    if any(|blob| matches!(blob, BlobText::Missing { .. })) || path.ids.iter().flatten().any(|id| !self.blobs.contains_key(id)) {
      // git may fetch a missing blob later, so that content is not cached
      send(content_event(MergeContent::Missing));
      return (None, made);
    }
    if any(|blob| matches!(blob, BlobText::Binary { .. })) {
      self.remember(key, &MergeContent::Binary);
      send(content_event(MergeContent::Binary));
      return (None, made);
    }
    if any(|blob| matches!(blob, BlobText::TooLarge { .. })) {
      let [base_bytes, left_bytes, right_bytes] = blobs.map(|blob| blob.map_or(0, BlobText::size));
      let content = MergeContent::TooLarge {
        base_bytes,
        left_bytes,
        right_bytes,
      };
      self.remember(key, &content);
      send(content_event(content));
      return (None, made);
    }

    let texts = blobs.map(|blob| match blob {
      Some(BlobText::Text { text, .. }) => text.as_str(),
      _ => "",
    });
    let limits = self.service.limits;
    let budget = DiffBudget {
      inner: Some(Deadline::after(limits.file)),
      lines: Deadline::after(limits.lines),
    };
    let language = language_of(&path.path);
    let [base_id, left_id, right_id] = path.ids.each_ref().map(Option::as_deref);

    // one side deleted the file: the two-way diff of the base to the side that changed it
    if base_id.is_some() && (left_id.is_none() || right_id.is_none()) {
      let (deleted, remaining_id, remaining) = if left_id.is_none() {
        (MergeSide::Left, right_id, texts[2])
      } else {
        (MergeSide::Right, left_id, texts[1])
      };
      let mut diff = diff_texts(texts[0], remaining, self.policy, &budget);
      if let Some(language) = language {
        if let Some(id) = base_id {
          diff.old_scopes = scopes(self.summaries, language, id, texts[0], &mut made);
        }
        if let Some(id) = remaining_id {
          diff.new_scopes = scopes(self.summaries, language, id, remaining, &mut made);
        }
      }
      let cut_short = diff.line_level_only && fits_inner(&texts);
      let content = MergeContent::OneSided { deleted, diff: diff.clone() };
      self.remember(key, &content);
      send(content_event(content));
      return ((!cut_short).then(|| (key.clone(), StoredMerge::OneSided { deleted, diff })), made);
    }

    let mut merge = merge_texts(texts[1], texts[0], texts[2], self.policy, &budget);
    let cut_short = merge.line_level_only && fits_inner(&texts);
    if let Some(language) = language {
      send(content_event(MergeContent::Text(merge.clone())));
      let side_scopes = |id: Option<&str>, text: &str, made: &mut Vec<((u8, String), FileSummary)>| -> Vec<Scope> {
        id.map_or_else(Vec::new, |id| scopes(self.summaries, language, id, text, made))
      };
      merge.base_scopes = side_scopes(base_id, texts[0], &mut made);
      merge.left_scopes = side_scopes(left_id, texts[1], &mut made);
      merge.right_scopes = side_scopes(right_id, texts[2], &mut made);
      send(FileMergeEvent::Scopes {
        index: to_u32(index),
        left_scopes: merge.left_scopes.clone(),
        base_scopes: merge.base_scopes.clone(),
        right_scopes: merge.right_scopes.clone(),
      });
      self.remember(key, &MergeContent::Text(merge.clone()));
    } else {
      self.remember(key, &MergeContent::Text(merge.clone()));
      send(content_event(MergeContent::Text(merge.clone())));
    }
    ((!cut_short).then(|| (key.clone(), StoredMerge::Text(merge))), made)
  }

  fn remember(&self, key: &MergeMemoryKey, content: &MergeContent) {
    self.service.merges.insert(key.clone(), Arc::new(content.clone()));
  }
}

/// Whether the comparison gave the texts their inner ranges only for lack of time
fn fits_inner(texts: &[&str; 3]) -> bool {
  texts.iter().all(|text| text.len() <= MAX_INNER_BYTES && text.lines().nth(MAX_INNER_LINES).is_none())
}
