//! The diffs of the views and the file lists of the ranges, through the memory of the app (`AppContext::file_diffs`)
//! and the object cache of the repository: the typed diff of each file of a commit, a range or the working tree, the
//! merge of each conflicting file of a cherry-pick, each told to the page on the channel of its command as it is known.
//!
//! What the index of the repository says of the names a subject renamed (`SymbolIndexes`, as a symbol search asks it)
//! is a read of its own, `get_rename_proofs`: a query of seconds on a large repository, which the window asks of the
//! subject it shows, after its diffs. One per window (`Windows::begin_rename_proofs`): a newer one of the window
//! supersedes the one in flight, whose query is killed, as is the query of a window that closed or loads again
//! (`Windows::shows`: a page that went away never says so itself); it answers nothing then, and the names stay
//! asked.

use crate::context::AppContext;
use crate::operation::run_blocking;
use crate::reads::symbols::SymbolIndexes;
use crate::session::windows::{WindowRequest, Windows};
use crate::transport::{Call, Sink};
use domain_types::error::ApiError;
use domain_types::review_request::GetRangeParams;
use file_diffs::model::{FileDiffEvent, FileMarks, FileMergeEvent, GetFileDiffsParams, GetFileMergesParams, GetRenameProofsParams};
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::{CommitFiles, DiffSubject};
use object_cache::cache::ObjectCache;
use std::sync::Arc;
use sync_utils::child_process::ChildPriority;
use tracing::{debug, instrument};

/// The diffs of the files, the files in view first (`FileDiffs::stream`), on `events`. With an index of the
/// repository on this machine, the names the subject renamed say that it is there to be asked
/// (`get_rename_proofs`); the read asks none and ends with its last file
#[instrument(skip_all, fields(repository_path = %params.repository_path, source = ?params.source))]
pub async fn get_file_diffs(ctx: &AppContext, params: GetFileDiffsParams, events: Sink<FileDiffEvent>) -> Result<(), ApiError> {
  let (git, repository_state, file_diffs) = (ctx.git.clone(), ctx.repository_state.clone(), ctx.file_diffs.clone());
  let asks_index = ctx.symbol_indexes.search(&|| false, ChildPriority::Normal).is_some();
  run_blocking(move || {
    let cache = repository_state.object_cache(&git, &params.repository_path);
    file_diffs.stream(&git, &cache, asks_index, &params, to_page(&events))
  })
  .await
}

/// What the index of the repository says of the names the subject of `params` renamed (`FileDiffs::rename_proofs`),
/// as the request of the window of `call`: the marks of each file it says something of, `None` when the request was
/// superseded or its page went away
#[instrument(skip_all, fields(repository_path = %params.repository_path, source = ?params.source))]
pub async fn get_rename_proofs(ctx: &AppContext, call: &Call, params: GetRenameProofsParams) -> Result<Option<Vec<FileMarks>>, ApiError> {
  let request = ctx.windows.begin_rename_proofs(&call.window);
  let (git, repository_state, file_diffs) = (ctx.git.clone(), ctx.repository_state.clone(), ctx.file_diffs.clone());
  let (windows, indexes, call) = (Arc::clone(&ctx.windows), Arc::clone(&ctx.symbol_indexes), call.clone());
  run_blocking(move || {
    let cache = repository_state.object_cache(&git, &params.repository_path);
    let asker = ProofAsker {
      windows: &windows,
      indexes: indexes.as_ref(),
      call: &call,
      request: &request,
    };
    asker.ask(&file_diffs, &git, &cache, &params)
  })
  .await
}

/// The request of a window for the proofs of the renames of a subject: who asks, and what ends it
pub(crate) struct ProofAsker<'a> {
  pub windows: &'a Windows,
  pub indexes: &'a dyn SymbolIndexes,
  /// The page that asked: the answer is of use to it alone
  pub call: &'a Call,
  pub request: &'a WindowRequest,
}

impl ProofAsker<'_> {
  /// Asks the index of the subject of `params`, at the priority of the app: a view waits for the answer. The query
  /// is killed once a newer request of the window superseded this one, or the window shows another page or none
  pub(crate) fn ask(&self, file_diffs: &FileDiffs, git: &GitCommandExecutor, cache: &ObjectCache, params: &GetRenameProofsParams) -> anyhow::Result<Option<Vec<FileMarks>>> {
    let canceled = || self.request.is_superseded() || !self.windows.shows(self.call);
    if canceled() {
      return Ok(None);
    }
    let index = self.indexes.search(&canceled, ChildPriority::Normal);
    let answer = file_diffs.rename_proofs(git, cache, index.as_deref(), params)?;
    // an answer that came as the page left or asked of another subject is of no use to it
    Ok(answer.filter(|_| !canceled()))
  }
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
