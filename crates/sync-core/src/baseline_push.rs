//! The push of a copy into the branch of the baseline on its remote, which lands it where the repository lands by a
//! push (`branchdeck.landing=push`; spec/branch/landing-routes.spec.md, "Push to master"; docs/design/git-operations.md,
//! "Push into the baseline"): the commit the sync built of a piece (the tip of its virtual branch) or of a whole stack
//! (its stack branch), pushed as it is, `git push --porcelain <push URL> <commit>:refs/heads/<target>`.
//!
//! - No `+`, no lease: a commit that is no fast-forward of the target the server refuses whole, atomically, as
//!   `[rejected] (fetch first)` or `(non-fast-forward)`: the target moved since the copy was built on it (`Moved`).
//! - By the first push URL of the remote (`remote get-url --push`, with `pushurl` and `pushInsteadOf`), not by its name:
//!   a push to a named remote moves its remote-tracking ref (`refs/remotes/origin/master`), which only the fetch of the
//!   baseline writes (`baseline::fetch_baseline`, under `fetch.lock`). Git takes the configured remote whose `url` or
//!   `pushurl` is the URL (and moves its refs all the same), so the push clears both of that remote for itself
//!   (`-c remote.<name>.url= -c remote.<name>.pushurl=`: an empty value resets the list). A pre-push hook sees the URL
//!   as both its arguments; a second push URL, `remote.<name>.receivepack` and `remote.<name>.proxy` play no part.
//! - The size check of every push first (`push_size::check_push_size`, over the remote-tracking refs of the remote).
//! - Through the policy of every git that talks to a remote (`remote_command`), its progress to the caller; the
//!   report of the ref read from `--porcelain`, the server's words from its `remote:` lines.

use crate::baseline::{BaselineUpstream, resolve_baseline_upstream};
use crate::push_lease::{PushRefsReport, parse_push_porcelain, remote_messages};
use crate::push_size::{PushTooLarge, check_push_size};
use crate::remote_command::execute_remote_command_with_progress_configured;
use crate::transfer_progress::TransferProgress;
use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::{info, instrument, warn};

/// What a push into the baseline is of
#[derive(Debug, Clone, Copy)]
pub struct BaselinePush<'a> {
  pub repository_path: &'a str,
  /// The baseline as the sync named it (`origin/master`): its remote and the branch there are what the push goes to
  pub baseline: &'a str,
  /// The commit pushed: the copy as the witness of the start froze it
  pub tip: &'a str,
  /// `branchdeck.pushSizeLimit`; `None` for no check
  pub size_limit: Option<u64>,
}

/// What a push into the baseline did
#[derive(Debug)]
pub enum BaselinePushed {
  /// The server took the commit: the target is at it now (`up_to_date`: it was at it already)
  Pushed { up_to_date: bool },
  /// The target has commits the copy lacks, no fast-forward: nothing was pushed. `summary` is what git said of the ref
  Moved { summary: String },
  /// The server refused it (`[remote rejected]`: a hook, a protected branch): nothing was pushed. `message` is what git
  /// said of the ref and what the server said why
  Refused { message: String },
  /// The files it adds exceed the push size limit: it did not talk to the remote
  TooLarge(PushTooLarge),
}

/// A push into the baseline as it went, with the upstream it went to: the fetch after a refusal of no fast-forward
/// fetches that one
#[derive(Debug)]
pub struct BaselinePushReport {
  pub upstream: BaselineUpstream,
  pub pushed: BaselinePushed,
}

/// Pushes `push.tip` into the branch of `push.baseline` on its remote by the first push URL of that remote (the
/// module's header): the size check, the upstream and the URL, then one `git push --porcelain --progress`. A rejected
/// ref is an answer, never an error
///
/// # Errors
/// A local baseline, a remote without a push URL, git failing, a push whose report names no ref
#[instrument(skip(git_executor, on_progress), fields(baseline = %push.baseline, tip = %push.tip))]
pub fn push_baseline(git_executor: &GitCommandExecutor, push: &BaselinePush<'_>, on_progress: &mut dyn FnMut(TransferProgress)) -> Result<BaselinePushReport> {
  let path = push.repository_path;
  let Some(upstream) = resolve_baseline_upstream(git_executor, path, push.baseline)? else {
    bail!("The baseline {} is a local branch: no remote branch takes the push", push.baseline);
  };
  if let Some(limit) = push.size_limit
    && let Some(too_large) = check_push_size(git_executor, path, &[push.tip], &upstream.remote, limit)?
  {
    return Ok(BaselinePushReport {
      upstream,
      pushed: BaselinePushed::TooLarge(too_large),
    });
  }
  let url = push_url(git_executor, path, &upstream.remote)?;
  let refspec = format!("{}:{}", push.tip, upstream.remote_ref);
  let args = ["push", "--porcelain", "--end-of-options", url.as_str(), refspec.as_str()];
  // no configured remote for the URL: none of its remote-tracking refs moves
  let (no_url, no_push_url) = (format!("remote.{}.url=", upstream.remote), format!("remote.{}.pushurl=", upstream.remote));
  let config = ["-c", no_url.as_str(), "-c", no_push_url.as_str()];
  // exit 1: the ref was rejected, its report says why
  let output = execute_remote_command_with_progress_configured(git_executor, &config, &args, path, &[1], on_progress)?;
  let report = PushRefsReport {
    messages: remote_messages(&output.stderr),
    refs: parse_push_porcelain(&output.into_text()),
  };
  let pushed = answer_of(&report, &upstream.remote_ref)?;
  info!(?pushed, "the push into the baseline answered");
  Ok(BaselinePushReport { upstream, pushed })
}

/// The first push URL of `remote` (`pushurl`, `pushInsteadOf` applied)
fn push_url(git_executor: &GitCommandExecutor, path: &str, remote: &str) -> Result<String> {
  let url = git_executor.execute_command(&["remote", "get-url", "--push", "--end-of-options", remote], path)?;
  let url = url.lines().next().unwrap_or_default().trim();
  if url.is_empty() {
    bail!("The remote {remote} has no push URL");
  }
  Ok(url.to_owned())
}

/// What the report of `remote_ref` says: taken, up to date, no fast-forward, or refused with the server's words
fn answer_of(report: &PushRefsReport, remote_ref: &str) -> Result<BaselinePushed> {
  let Some(status) = report.refs.get(remote_ref) else {
    // git reported no ref: what it said instead is the reason
    return Err(anyhow!("The push into {remote_ref} reported nothing: {}", report.messages.join(" · ")));
  };
  if status.accepted() {
    return Ok(BaselinePushed::Pushed { up_to_date: status.flag == '=' });
  }
  let summary = status.summary.trim();
  if summary.starts_with("[rejected]") && (summary.contains("fetch first") || summary.contains("non-fast-forward")) {
    return Ok(BaselinePushed::Moved { summary: summary.to_owned() });
  }
  warn!(summary, messages = ?report.messages, "the server refused the push into the baseline");
  let mut message = summary.to_owned();
  if !report.messages.is_empty() {
    message = format!("{message}. {}", report.messages.join(" · "));
  }
  Ok(BaselinePushed::Refused { message })
}

#[cfg(test)]
#[path = "baseline_push_test.rs"]
mod baseline_push_test;
