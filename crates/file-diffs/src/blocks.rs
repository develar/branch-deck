//! The blocks of many blob pairs at once, for the dependency scan of the commits (`sync_core::diff_scan`) and the
//! patches of `bd commits --patch`: the same blocks as the diff views show, under `Default`, without the scopes.
//!
//! - **Caches.** A pair that memory or the `file_diff` store has (the view showed it) costs nothing. The others are
//!   compared and not cached here: their callers cache them per commit (`commit_diff`), and the views need the scopes.
//! - **Processes.** One `cat-file --batch` for the blobs of the pairs that no cache has. A pair goes to a worker as soon as
//!   both its blobs are read, and a blob that no other pair waits for is dropped, so memory holds the pairs in flight;
//!   an instance `for_scans` keeps them in its memory cache instead, for the next pass over the same commits.

use crate::model::{BlobText, DiffContent};
use crate::service::{FileDiffs, MODEL_VERSION, blob_text, options, spawn_workers};
use anyhow::Result;
use code_symbols::language::language_of;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cat_file_batch::for_each_object;
use git_ops::commit_files::MAX_BLOB_BYTES;
use object_cache::cache::ObjectCache;
use object_cache::file_diff::DiffKey;
use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex, PoisonError};
use text_diff::cancellation::Deadline;
use text_diff::model::{DiffBudget, TextDiff, diff_texts};
use text_diff::policy::WhitespacePolicy;

/// A changed file whose blocks a scan needs: its paths (the languages of the cache key) and its blob ids, `None` for a
/// side the file does not exist on
#[derive(Debug, Clone, Copy)]
pub struct BlobPair<'a> {
  pub path: &'a str,
  pub old_path: Option<&'a str>,
  pub old_id: Option<&'a str>,
  pub new_id: Option<&'a str>,
}

/// What a scan gets of a pair
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairDiff {
  /// The diff of the texts, as the views show it (the scopes may be empty)
  Text(TextDiff),
  /// Binary, over `MAX_BLOB_BYTES`, or a blob git cannot read: no lines to compare
  NoText,
}

impl FileDiffs {
  /// The diff of each pair under `Default`, in the order of `pairs`
  pub fn blocks_of(&self, git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, pairs: &[BlobPair<'_>]) -> Result<Vec<PairDiff>> {
    let keys: Vec<(u8, String, String)> = pairs
      .iter()
      .map(|pair| {
        let old_language = language_of(pair.old_path.unwrap_or(pair.path));
        (
          options(WhitespacePolicy::Default, old_language, language_of(pair.path)),
          pair.old_id.unwrap_or_default().to_string(),
          pair.new_id.unwrap_or_default().to_string(),
        )
      })
      .collect();
    let mut results: Vec<Option<PairDiff>> = keys.iter().map(|key| self.known_diff(key)).collect();

    let missed: Vec<usize> = (0..pairs.len()).filter(|&index| results[index].is_none()).collect();
    let disk_keys: Vec<DiffKey<'_>> = missed
      .iter()
      .map(|&index| DiffKey {
        policy: keys[index].0,
        old_id: &keys[index].1,
        new_id: &keys[index].2,
      })
      .collect();
    let stored: Vec<Option<TextDiff>> = cache.get_file_diffs(MODEL_VERSION, &disk_keys);
    for (&index, diff) in missed.iter().zip(stored) {
      results[index] = diff.map(PairDiff::Text);
    }

    // the blobs of the pairs left, each read once, and the pairs that wait for each
    let pending: Vec<usize> = (0..pairs.len()).filter(|&index| results[index].is_none()).collect();
    let mut waiting: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut have: HashMap<String, Arc<BlobText>> = HashMap::new();
    // how many pending pairs still need each blob
    let mut needs: HashMap<&str, usize> = HashMap::new();
    let mut ids: Vec<String> = Vec::new();
    for &index in &pending {
      for id in [pairs[index].old_id, pairs[index].new_id].into_iter().flatten() {
        *needs.entry(id).or_default() += 1;
        if let Some(blob) = self.blobs.get(id) {
          have.insert(id.to_string(), blob);
          continue;
        }
        let files = waiting.entry(id).or_default();
        if files.is_empty() {
          ids.push(id.to_string());
        }
        files.push(index);
      }
    }

    let computed = Mutex::new(Vec::<(usize, PairDiff)>::new());
    let (sender, jobs) = channel::<(usize, [Option<Arc<BlobText>>; 2])>();
    let jobs = Mutex::new(jobs);
    std::thread::scope(|scope| -> Result<()> {
      spawn_workers(scope, pending.len().max(1), || {
        || {
          loop {
            let job = jobs.lock().unwrap_or_else(PoisonError::into_inner).recv();
            let Ok((index, [old, new])) = job else { return };
            let diff = compare(old.as_deref(), new.as_deref(), self.limits.file, self.limits.lines);
            computed.lock().unwrap_or_else(PoisonError::into_inner).push((index, diff));
          }
        }
      });
      // sends a pair whose blobs are all there, and drops the blobs no other pair needs
      let send = |index: usize, have: &mut HashMap<String, Arc<BlobText>>, needs: &mut HashMap<&str, usize>| {
        let side = |id: Option<&str>| id.map(|id| have.get(id).cloned());
        let sides = match (side(pairs[index].old_id), side(pairs[index].new_id)) {
          (Some(None), _) | (_, Some(None)) => return,
          (old, new) => [old.flatten(), new.flatten()],
        };
        let _ = sender.send((index, sides));
        for id in [pairs[index].old_id, pairs[index].new_id].into_iter().flatten() {
          if let Some(count) = needs.get_mut(id) {
            *count -= 1;
            if *count == 0 {
              have.remove(id);
            }
          }
        }
      };
      for &index in &pending {
        send(index, &mut have, &mut needs);
      }
      let read = for_each_object(git, repository_path, &ids, MAX_BLOB_BYTES, |position, entry| {
        let id = &ids[position];
        let blob = blob_text(id.clone(), entry);
        let blob = if self.keep_scan_blobs { self.keep_blob(blob) } else { Arc::new(blob) };
        have.insert(id.clone(), blob);
        for index in waiting.remove(id.as_str()).unwrap_or_default() {
          send(index, &mut have, &mut needs);
        }
        Ok(())
      });
      drop(sender);
      read
    })?;
    for (index, diff) in computed.into_inner().unwrap_or_else(PoisonError::into_inner) {
      results[index] = Some(diff);
    }
    Ok(results.into_iter().map(|diff| diff.unwrap_or(PairDiff::NoText)).collect())
  }

  /// The diff of a pair that memory has
  fn known_diff(&self, key: &(u8, String, String)) -> Option<PairDiff> {
    match self.diffs.get(key)?.as_ref() {
      DiffContent::Text(diff) => Some(PairDiff::Text(diff.clone())),
      _ => Some(PairDiff::NoText),
    }
  }
}

/// The diff of two sides, as the views compare them, within the budgets of a file
fn compare(old: Option<&BlobText>, new: Option<&BlobText>, file: std::time::Duration, lines: std::time::Duration) -> PairDiff {
  const fn text(blob: Option<&BlobText>) -> Option<&str> {
    match blob {
      None => Some(""),
      Some(BlobText::Text { text, .. }) => Some(text.as_str()),
      Some(_) => None,
    }
  }
  let (Some(old), Some(new)) = (text(old), text(new)) else {
    return PairDiff::NoText;
  };
  let budget = DiffBudget {
    inner: Some(Deadline::after(file)),
    lines: Deadline::after(lines),
  };
  PairDiff::Text(diff_texts(old, new, WhitespacePolicy::Default, &budget))
}
