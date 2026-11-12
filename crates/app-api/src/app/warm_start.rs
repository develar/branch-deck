//! The warm start of a window of the main view (openspec/specs/repository/onboarding/spec.md, "Before the first sync";
//! docs/design/rust-domain-authority.md, "s13-warm-start"): before the page subscribes, the frame of the session of the
//! project (`repository_model::warm_start`) is read without a git process, beside the key the window's subscription
//! resolves (`AppContext::session_key_now`: the prefix the git config names now, one `git config`), the session of that
//! key is opened seeded from the frame (`Sessions::open_seeded`), and its seed, the view of the frame, is what the page
//! shows on its first frame (`CachedModel`): the app hands it in the init script of its main window
//! (`src-tauri/src/warm_start.rs`), `brd mcp` in the answer of `bd_bootstrap` (`crates/mcp-host`). No frame, a frame of
//! another prefix (saved before the prefix changed) or schema, a repository whose git directory the files don't name,
//! no prefix or no git: no model, the page starts as without a cache.

use crate::context::AppContext;
use domain_types::session::CachedModel;
use git_ops::lease_slot::linked_worktree;
use object_cache::cache::ObjectCache;
use repository_model::warm_start::{CachedSession, read_frame};
use std::path::{Path, PathBuf};
use sync_utils::span::in_current_span;
use tracing::{debug, instrument};

/// The seed of the session of `repository_path`, opened under the key of the prefix the git config names now
/// (`AppContext::session_key_now`) and seeded from its frame; none without one
#[instrument(skip(ctx))]
pub async fn seed(ctx: &AppContext, repository_path: &str) -> Option<CachedModel> {
  // the prefix of the config now beside the read of the frame: a frame of another prefix is of a session no window shows
  let path = repository_path.to_owned();
  let frame = tokio::task::spawn_blocking(in_current_span(move || project_frame(&path)));
  let (frame, key) = tokio::join!(frame, ctx.session_key_now(repository_path));
  let frame = frame.ok().flatten()?;
  let key = match key {
    Ok(key) => key,
    Err(error) => {
      debug!(code = ?error.code, "No session of the project to seed: {}", error.message);
      return None;
    }
  };
  let Some(view) = ctx.sessions.open_seeded(&key, frame).await else {
    debug!("The frame seeds no session of the prefix of now");
    return None;
  };
  Some(CachedModel {
    repository_path: repository_path.to_owned(),
    view,
  })
}

/// The frame of the session of the project at `repository_path`, from the object cache of its repository, found by the
/// files of the worktree (`git_common_dir`)
fn project_frame(repository_path: &str) -> Option<CachedSession> {
  let Some(git_common_dir) = git_common_dir(Path::new(repository_path)) else {
    debug!("The files name no git directory of the project");
    return None;
  };
  read_frame(&ObjectCache::open(&git_common_dir), repository_path)
}

/// `$GIT_COMMON_DIR` of the worktree whose top level is `worktree`, without a git process: its `.git` directory, or the
/// common directory a linked worktree's `.git` file names (`lease_slot::linked_worktree`); none for anything else
#[must_use]
pub fn git_common_dir(worktree: &Path) -> Option<PathBuf> {
  let dot_git = worktree.join(".git");
  if dot_git.is_dir() {
    return Some(dot_git);
  }
  linked_worktree(worktree).map(|linked| linked.git_common_dir)
}

#[cfg(test)]
#[path = "warm_start_test.rs"]
mod warm_start_test;
