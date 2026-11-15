//! The one writer of the git config of each repository in this process (openspec/specs/repository/settings/spec.md, "One
//! writer of the git config of a repository"; docs/contributing/git-rules.md): every write of Settings → Repository and
//! of a companion goes to the task of its config file, the common git directory of the repository (every worktree of
//! it writes the same `config`) or the global config of the user, which runs them one after the other in the order they
//! came. So two writes of the app never race for `config.lock` and no lock is held across git; a lock of another process
//! is waited for by the write itself (`sync_core::repository_config`, about a second, the reason in the error).
//!
//! A write of several configs (a companion: the key of the primary, the `branchdeck.primary` of the companion) holds the
//! tasks of all of them for its whole span (`write_together`), taken in the order of their files, so it never interleaves
//! with another write of either file and two of them never wait for each other.
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
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use sync_core::repository_config::WriteScope;
use sync_utils::page_writes::{PageWrite, PageWrites};
use tokio::sync::{mpsc, oneshot};
use tracing::Instrument;

/// The tasks of the config files this process writes, by the file
#[derive(Default)]
pub struct ConfigWriters {
  tasks: Mutex<HashMap<PathBuf, mpsc::UnboundedSender<ConfigJob>>>,
  /// The jobs sent to the tasks that have not started yet: a test queues writes in a known order by it
  queued: Arc<AtomicUsize>,
}

/// A write for the task of a config file: the key it is ordered by among the writes of its page, its run (its blocking
/// step when it applies, its answer to its caller either way) and the span of the command that asked for it
struct ConfigJob {
  order: Option<(PageWrite, String)>,
  run: JobRun,
  span: tracing::Span,
}

/// The run of a job: given whether it applies, the future that writes (or not) and answers its caller
type JobRun = Box<dyn FnOnce(bool) -> JobFuture + Send>;
type JobFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

impl ConfigWriters {
  /// Runs `write` in the task of the config file `scope` of the repository at `repository_path` writes, after the writes
  /// that came before it; `order`, the write of a page and its key, is applied only when it is newer than the last write
  /// of that key of its page, else `None`. Answers what `write` answered (the settings it read again, the write it leaves
  /// for the config of another repository)
  ///
  /// # Errors
  /// The write failed, or the task of the file went away
  pub async fn write<T: Send + 'static>(
    &self,
    repository_path: &str,
    scope: WriteScope,
    order: Option<(PageWrite, String)>,
    write: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
  ) -> Result<Option<T>, ApiError> {
    let (reply, answer) = oneshot::channel();
    let run = move |applies: bool| -> JobFuture {
      Box::pin(async move {
        let written = if applies { run_blocking(write).await.map(Some) } else { Ok(None) };
        let _ = reply.send(written);
      })
    };
    let job = ConfigJob {
      order,
      run: Box::new(run),
      span: tracing::Span::current(),
    };
    self.send(config_file_of(repository_path, scope), job)?;
    answer.await.map_err(|_gone| gone())?
  }

  /// Runs `write` while it holds the tasks of the local config files of every repository of `repositories`, one write of
  /// several configs (a companion, `app::repository_config::write_companion`): each task is taken after the writes that
  /// came before it there and held until `write` ends, so no write of this process to any of the files runs inside it.
  /// The tasks are taken one by one in the order of their files, the one global order of every write that holds more
  /// than one, so two such writes never hold one task each while they wait for the other's: no deadlock
  ///
  /// # Errors
  /// The write failed, or the task of a file went away
  pub fn write_together<T: Send + 'static>(
    &self,
    repositories: &[&str],
    write: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
  ) -> impl Future<Output = Result<T, ApiError>> + Send + '_ {
    let mut files: Vec<PathBuf> = repositories.iter().map(|repository| config_file_of(repository, WriteScope::Local)).collect();
    files.sort();
    files.dedup();
    self.hold_and_write(files, write)
  }

  /// `write_together` of the files it resolved
  async fn hold_and_write<T: Send + 'static>(&self, files: Vec<PathBuf>, write: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> Result<T, ApiError> {
    // each a release of a task this write holds, sent as it ends; dropped on an error or a drop of the write, which
    // releases the task as well
    let mut held = Vec::with_capacity(files.len());
    for file in files {
      let (taken, on_taken) = oneshot::channel::<()>();
      let (release, on_release) = oneshot::channel::<()>();
      let run = move |_applies: bool| -> JobFuture {
        Box::pin(async move {
          if taken.send(()).is_ok() {
            let _ = on_release.await;
          }
        })
      };
      let job = ConfigJob {
        order: None,
        run: Box::new(run),
        span: tracing::Span::current(),
      };
      self.send(file, job)?;
      on_taken.await.map_err(|_gone| gone())?;
      held.push(release);
    }
    let written = run_blocking(write).await;
    for release in held {
      let _ = release.send(());
    }
    written
  }

  /// Sends `job` to the task of `file`, started on its first write
  fn send(&self, file: PathBuf, job: ConfigJob) -> Result<(), ApiError> {
    let task = {
      let mut tasks = self.tasks.lock().unwrap_or_else(PoisonError::into_inner);
      let task = tasks.entry(file).or_insert_with(|| {
        let (jobs, queue) = mpsc::unbounded_channel();
        // each write runs in the span of the command that asked for it; between them, in the span of the first
        tokio::spawn(serve(queue, Arc::clone(&self.queued)).in_current_span());
        jobs
      });
      task.clone()
    };
    self.queued.fetch_add(1, Ordering::SeqCst);
    task.send(job).map_err(|_gone| {
      self.queued.fetch_sub(1, Ordering::SeqCst);
      gone()
    })
  }

  /// The jobs sent to the tasks that have not started yet
  #[cfg(test)]
  fn queued(&self) -> usize {
    self.queued.load(Ordering::SeqCst)
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
async fn serve(mut queue: mpsc::UnboundedReceiver<ConfigJob>, queued: Arc<AtomicUsize>) {
  let mut page_writes = PageWrites::default();
  while let Some(job) = queue.recv().await {
    queued.fetch_sub(1, Ordering::SeqCst);
    let ConfigJob { order, run, span } = job;
    let applies = order.as_ref().is_none_or(|(write, key)| page_writes.applies(write, key));
    run(applies).instrument(span).await;
  }
}

/// The config file a write of `scope` of the repository at `repository_path` changes, canonical, so every spelling of
/// one repository (`/var/…` and `/private/var/…`, a symlink) is one task: the global config of the user, or the `config`
/// of the common git directory, which every worktree of the repository writes (`--local`); the `.git` of the repository
/// for one that is no linked worktree, as given when it cannot be resolved
fn config_file_of(repository_path: &str, scope: WriteScope) -> PathBuf {
  match scope {
    WriteScope::Global => PathBuf::new(),
    WriteScope::Local => {
      let path = Path::new(repository_path);
      linked_worktree(path).map_or_else(
        || {
          let git_dir = path.join(".git");
          std::fs::canonicalize(&git_dir).unwrap_or(git_dir)
        },
        |worktree| worktree.git_common_dir,
      )
    }
  }
}

fn gone() -> ApiError {
  ApiError::internal("The writer of the git config of the repository went away")
}

#[cfg(test)]
#[path = "config_writer_test.rs"]
mod config_writer_test;
