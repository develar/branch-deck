//! The warm start of the app (spec/repository/onboarding.spec.md, "Before the first sync";
//! docs/design/rust-domain-authority.md, "s13-warm-start"): before the main window is built, the frame of the session
//! of the selected project (`repository_model::warm_start`) is read without a git process, beside the key the window's
//! subscription resolves (`AppContext::session_key_now`: the prefix the git config names now, one `git config`), the session of
//! that key is opened seeded from the frame (`Sessions::open_seeded`), and its seed, the view of the frame, goes into
//! the init script of the window as `window.__BD_MODEL__`, beside `__TAURI_STORE__`. The page shows its rows on its
//! first frame and subscribes as the window that holds the seed (`subscribe_view` with `have`): it gets the patch from
//! the seed to the view of the session, whose first sync runs (a project synced before) while the webview loads. No
//! frame, a frame of another prefix (saved before the prefix changed) or schema, a repository whose git directory the
//! files don't name, no prefix or no git: no model, the page starts as without a cache.
//!
//! A reload runs the same script: the page installs that seed again, and the session, which keeps it while it lives,
//! catches the window up from it; one opened since (another epoch) answers its snapshot, whose run the page doesn't take
//! for one that ended. The page ends the first sync in that catch-up only while no page of the window saw it end
//! (`src/model/store.ts`, in `sessionStorage`): a channel to a page that went away takes its patches without a failure,
//! so only the page knows what it got.

use app_api::context::AppContext;
use domain_types::session::CachedModel;
use git_ops::lease_slot::linked_worktree;
use object_cache::cache::ObjectCache;
use repository_model::warm_start::{CachedSession, read_frame};
use std::path::{Path, PathBuf};
use sync_utils::span::in_current_span;
use tracing::{debug, instrument};

/// The part of the init script of the main window that hands its page the seed of the session of `repository_path`
/// (`window.__BD_MODEL__`), opened under the key of the prefix the git config names now (`AppContext::session_key_now`)
/// and seeded from its frame; empty without one. Blocking: it waits for the key and for the actor of the session to open
#[instrument(skip(ctx))]
pub fn model_script(ctx: &AppContext, repository_path: &str) -> String {
  // the prefix of the config now beside the read of the frame: a frame of another prefix is of a session no window shows
  let (frame, key) = std::thread::scope(|scope| {
    let frame = scope.spawn(in_current_span(|| project_frame(repository_path)));
    let key = tauri::async_runtime::block_on(ctx.session_key_now(repository_path));
    (frame.join().ok().flatten(), key)
  });
  let Some(frame) = frame else {
    return String::new();
  };
  let key = match key {
    Ok(key) => key,
    Err(error) => {
      debug!(code = ?error.code, "No session of the project to seed: {}", error.message);
      return String::new();
    }
  };
  let Some(view) = tauri::async_runtime::block_on(ctx.sessions.open_seeded(&key, frame)) else {
    debug!("The frame seeds no session of the prefix of now");
    return String::new();
  };
  script(&CachedModel {
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

/// The statement that sets `window.__BD_MODEL__` to `model`: the JSON in a string that `JSON.parse` reads, which a
/// JavaScript engine parses faster than the same object as a literal
#[must_use]
pub fn script(model: &CachedModel) -> String {
  match serde_json::to_string(model) {
    Ok(json) => format!("window.__BD_MODEL__ = JSON.parse('{}');", js_string(&json)),
    Err(error) => {
      debug!("The view of the frame is not encodable: {error}");
      String::new()
    }
  }
}

/// `text` as the content of a single-quoted JavaScript string
fn js_string(text: &str) -> String {
  let mut quoted = String::with_capacity(text.len() + text.len() / 16);
  for char in text.chars() {
    match char {
      '\\' => quoted.push_str("\\\\"),
      '\'' => quoted.push_str("\\'"),
      '\n' => quoted.push_str("\\n"),
      '\r' => quoted.push_str("\\r"),
      '\u{2028}' => quoted.push_str("\\u2028"),
      '\u{2029}' => quoted.push_str("\\u2029"),
      other => quoted.push(other),
    }
  }
  quoted
}
