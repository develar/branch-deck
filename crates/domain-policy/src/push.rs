//! What a push of a branch does to its remote branch, by its remote status (`RemoteStatusUpdate`, compared by content:
//! a branch only rebased onto a newer baseline has nothing to push), for the app and `bd` alike
//! (docs/design/rust-domain-authority.md, R1 and D4):
//!
//! - `action`: create, push (a fast-forward), force (rebased, older versions replaced, remote commits dropped) or up to
//!   date; `sync_core::remote_status` stores it on the status, so the page and `bd` read this one answer
//! - `needs_push`: the push changes the remote branch (the lack of it, commits it lacks, versions it replaces or drops)
//! - `replaces_older_versions`: the remote has older versions of commits of the branch, of its stack ancestors or of the
//!   baseline, or an older piece of a stacked branch
//! - `drops_remote_commits`: the push drops remote commits the branch lacks, someone's work: it asks first
//! - `over_moved_remote`: a push refused because the remote branch moved since its status goes again, once, over what
//!   the remote has now, only when nothing of it is lost

use domain_types::{PushAction, RemoteStatusUpdate};

/// What a push of the branch does over `remote`. The status compares content: a branch only rebased onto a newer
/// baseline is up to date, the remote has its work; one reworded since is not, a robot would land the remote's
/// messages. A push that is not a fast-forward (the remote has commits the branch lacks, older versions of its
/// commits, or the branch was rebased) is a force push
#[must_use]
pub const fn action(remote: &RemoteStatusUpdate) -> PushAction {
  if !remote.remote_exists {
    return PushAction::Create;
  }
  if !needs_push(remote) {
    return PushAction::UpToDate;
  }
  let drops = drops_remote_commits(remote);
  let replaces = replaces_older_versions(remote);
  if drops || replaces || !remote.fast_forward {
    PushAction::Force { drops, replaces }
  } else {
    PushAction::Push
  }
}

/// A push of the branch would change its remote branch: the remote lacks it or commits of it, or has commits a push
/// replaces or drops
#[must_use]
pub const fn needs_push(remote: &RemoteStatusUpdate) -> bool {
  !remote.remote_exists || !remote.unpushed_commits.is_empty() || drops_remote_commits(remote) || replaces_older_versions(remote)
}

/// The remote has older versions of commits of the branch (amended here, or reworded: the same changes with another
/// message or author), of its stack ancestors (restacked) or of the baseline (landed), or an older piece: it lacks new
/// commits of the branches it is built on, which a Safe Push of the branch lands with it
#[must_use]
pub const fn replaces_older_versions(remote: &RemoteStatusUpdate) -> bool {
  remote.commits_amended > 0 || remote.commits_restacked > 0 || remote.commits_landed > 0 || remote.commits_reworded > 0 || remote.ancestor_commits_ahead > 0
}

/// The push drops remote commits the branch lacks that are no older version of a commit it has: someone's work, which
/// the push asks for first (the app's Force Push, `bd push --force`)
#[must_use]
pub const fn drops_remote_commits(remote: &RemoteStatusUpdate) -> bool {
  remote.commits_behind > 0
}

/// Whether a push the remote refused because the remote branch moved since the status the user saw goes again, once,
/// over `moved`, the status fetched after the refusal: only when the remote has no commits the branch lacks (nobody's
/// work is lost) and the branch has work for it. A remote with someone's commits, or one that has the branch as it is,
/// is told instead; so is one that moves again
#[must_use]
pub const fn over_moved_remote(moved: &RemoteStatusUpdate) -> bool {
  !drops_remote_commits(moved) && needs_push(moved)
}
