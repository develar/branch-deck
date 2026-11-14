//! The one writer of the git config of each repository in this process (openspec/specs/repository/settings/spec.md, "One
//! writer of the git config of a repository"; docs/contributing/git-rules.md): every write of Settings → Repository and
//! of a companion goes to the task of its config file, the common git directory of the repository (every worktree of
//! it writes the same `config`) or the global config of the user, which runs them one after the other in the order they
//! came. So two writes of the app never race for `config.lock` and no lock is held across git; a lock of another process
//! is waited for by the write itself (`sync_core::repository_config`, about a second, the reason in the error).
//!
//! A write of a page names its page and its number among the writes of that page (`order`, `nextWriteOrder` of
//! `src/lib/field-writes.ts`):
//! the task applies it only when it is newer than the last write of the same key of that page it decided
//! (`sync_utils::page_writes`), and an older one answers `None` and writes nothing. The close of Settings sends the
//! edit a field queued while the write before it still runs, and whichever comes first, the newer value stays.

use crate::operation::run_blocking;
use crate::transport::Call;
use domain_types::error::ApiError;
use domain_types::settings::WriteOrder;
use git_ops::lease_slot::linked_worktree;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use sync_core::repository_config::{RepositoryConfig, WriteScope};
use sync_utils::page_writes::{PageWrite, PageWrites};
use tokio::sync::{mpsc, oneshot};
use tracing::Instrument;

/// The tasks of the config files this process writes, by the file
#[derive(Default)]
pub struct ConfigWriters {
  tasks: Mutex<HashMap<PathBuf, mpsc::UnboundedSender<ConfigJob>>>,
}

/// A write for the task of a config file: its blocking step, the key it is ordered by among the writes of its page, and
/// the span of the command that asked for it
struct ConfigJob {
  order: Option<(PageWrite, String)>,
  write: Box<dyn FnOnce() -> anyhow::Result<RepositoryConfig> + Send>,
  span: tracing::Span,
  reply: oneshot::Sender<Result<Option<RepositoryConfig>, ApiError>>,
}

impl ConfigWriters {
  /// Runs `write` in the task of the config file `scope` of the repository at `repository_path` writes, after the writes
  /// that came before it; `order`, the write of a page and its key, is applied only when it is newer than the last write
  /// of that key of its page, else `None`. Answers the settings `write` read again
  ///
  /// # Errors
  /// The write failed, or the task of the file went away
  pub async fn write(
    &self,
    repository_path: &str,
    scope: WriteScope,
    order: Option<(PageWrite, String)>,
    write: impl FnOnce() -> anyhow::Result<RepositoryConfig> + Send + 'static,
  ) -> Result<Option<RepositoryConfig>, ApiError> {
    let (reply, answer) = oneshot::channel();
    let job = ConfigJob {
      order,
      write: Box::new(write),
      span: tracing::Span::current(),
      reply,
    };
    self.task_of(config_file_of(repository_path, scope)).send(job).map_err(|_gone| gone())?;
    answer.await.map_err(|_gone| gone())?
  }

  /// The task of `file`, started on its first write
  fn task_of(&self, file: PathBuf) -> mpsc::UnboundedSender<ConfigJob> {
    let mut tasks = self.tasks.lock().unwrap_or_else(PoisonError::into_inner);
    let task = tasks.entry(file).or_insert_with(|| {
      let (jobs, queue) = mpsc::unbounded_channel();
      // each write runs in the span of the command that asked for it; between them, in the span of the first
      tokio::spawn(serve(queue).in_current_span());
      jobs
    });
    task.clone()
  }
}

/// The write of the page of the window of `call` that `order` names
#[must_use]
pub fn page_write(call: &Call, order: WriteOrder) -> PageWrite {
  PageWrite {
    window: call.window.to_string(),
    page: order.page,
    seq: order.seq,
  }
}

/// The writes of one config file, one after the other in the order they came
async fn serve(mut queue: mpsc::UnboundedReceiver<ConfigJob>) {
  let mut page_writes = PageWrites::default();
  while let Some(job) = queue.recv().await {
    let ConfigJob { order, write, span, reply } = job;
    if let Some((write, key)) = &order
      && !page_writes.applies(write, key)
    {
      let _ = reply.send(Ok(None));
      continue;
    }
    let written = run_blocking(write).instrument(span).await;
    let _ = reply.send(written.map(Some));
  }
}

/// The config file a write of `scope` of the repository at `repository_path` changes: the global config of the user, or
/// the `config` of the common git directory, which every worktree of the repository writes (`--local`); the path of the
/// repository for one whose git directory is not a linked worktree's
fn config_file_of(repository_path: &str, scope: WriteScope) -> PathBuf {
  match scope {
    WriteScope::Global => PathBuf::new(),
    WriteScope::Local => {
      let path = Path::new(repository_path);
      linked_worktree(path).map_or_else(|| path.join(".git"), |worktree| worktree.git_common_dir)
    }
  }
}

fn gone() -> ApiError {
  ApiError::internal("The writer of the git config of the repository went away")
}

#[cfg(test)]
#[path = "config_writer_test.rs"]
mod config_writer_test;
