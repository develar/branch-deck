//! The diffs of the views and the file lists of the ranges, through the memory of the app (`AppContext::file_diffs`)
//! and the object cache of the repository: the typed diff of each file of a commit, a range or the working tree, the
//! merge of each conflicting file of a cherry-pick, each told to the page on the channel of its command as it is known.
//! The diffs of a subject end with what the index of the repository says of the names it renamed
//! (`SymbolIndexes`, as a symbol search asks it): its query is killed once the page that asked went away, the page of
//! its window being another by then (`Windows::shows`) or a send to it having failed

use crate::context::AppContext;
use crate::operation::run_blocking;
use crate::transport::{Call, Sink};
use domain_types::error::ApiError;
use domain_types::review_request::GetRangeParams;
use file_diffs::model::{FileDiffEvent, FileMergeEvent, GetFileDiffsParams, GetFileMergesParams};
use git_ops::commit_files::{CommitFiles, DiffSubject};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use sync_utils::child_process::ChildPriority;
use tracing::{debug, instrument};

/// The diffs of the files, the files in view first (`FileDiffs::stream`), on `events`, for the page of `call`
#[instrument(skip_all, fields(repository_path = %params.repository_path, source = ?params.source))]
pub async fn get_file_diffs(ctx: &AppContext, call: &Call, params: GetFileDiffsParams, events: Sink<FileDiffEvent>) -> Result<(), ApiError> {
  let (git, repository_state, file_diffs) = (ctx.git.clone(), ctx.repository_state.clone(), ctx.file_diffs.clone());
  let (windows, indexes, call) = (Arc::clone(&ctx.windows), Arc::clone(&ctx.symbol_indexes), call.clone());
  run_blocking(move || {
    let cache = repository_state.object_cache(&git, &params.repository_path);
    // a Tauri channel of a page that went away never fails its send: the page of the window tells
    let gone = AtomicBool::new(false);
    let canceled = || gone.load(Ordering::Relaxed) || !windows.shows(&call);
    let index = indexes.search(&canceled, ChildPriority::Normal);
    file_diffs.stream(&git, &cache, index.as_deref(), &params, |event| {
      if !gone.load(Ordering::Relaxed) && events.send(event).is_err() {
        gone.store(true, Ordering::Relaxed);
        debug!("the page of the diffs went away");
      }
    })
  })
  .await
}

/// The merges of the conflicting files, the files in view first (`FileDiffs::stream_merges`), on `events`
#[instrument(skip_all, fields(repository_path = %params.repository_path, source = ?params.source))]
pub async fn get_file_merges(ctx: &AppContext, params: GetFileMergesParams, events: Sink<FileMergeEvent>) -> Result<(), ApiError> {
  let (git, repository_state, file_diffs) = (ctx.git.clone(), ctx.repository_state.clone(), ctx.file_diffs.clone());
  run_blocking(move || {
    let cache = repository_state.object_cache(&git, &params.repository_path);
    file_diffs.stream_merges(&git, &cache, &params, to_page(&events))
  })
  .await
}

/// The files of `base..head` from the memory of the app, else one `diff-tree` (`RangeFilesCache::read`)
#[instrument(skip(ctx))]
pub async fn get_range_files(ctx: &AppContext, params: GetRangeParams) -> Result<Arc<CommitFiles>, ApiError> {
  let (git, range_files) = (ctx.git.clone(), ctx.file_diffs.range_files.clone());
  let subject = DiffSubject::Range {
    base: params.base,
    head: params.head,
  };
  run_blocking(move || range_files.read(&git, &params.repository_path, &subject)).await
}

/// Sends each event to the page of `events`; once the page went away, the rest of the read is heard by no one
fn to_page<E>(events: &Sink<E>) -> impl FnMut(E) + '_ {
  let mut gone = false;
  move |event| {
    if !gone && events.send(event).is_err() {
      gone = true;
      debug!("the page of the diffs went away");
    }
  }
}
