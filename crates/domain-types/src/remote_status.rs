//! What a push of a branch does over its remote status, by the counts of the status alone (compared by content: a
//! branch only rebased onto a newer baseline has nothing to push), for the app and `bd` alike: derived fields of the
//! status, not a rule over the facts of a repository, so the sync that computes the status sets its action
//! (`sync_core::remote_status`) without reaching the rules, and the push orders of `domain_policy::push` read the same
//! answers (docs/design/rust-domain-authority.md, R1, D4 and s16-1; spec/branch/branch-row.spec.md,
//! spec/repository/outgoing.spec.md):
//!
//! - `push_action`: create, push (a fast-forward), force (rebased, older versions replaced, remote commits dropped) or
//!   up to date; the sync stores it on the status (`RemoteStatusUpdate::action`), so the page and `bd` read this one
//!   answer
//! - `needs_push`: the push changes the remote branch (the lack of it, commits it lacks, versions it replaces or drops)
//! - `replaces_older_versions`: the remote has older versions of commits of the branch, of its stack ancestors or of the
//!   baseline, or an older piece of a stacked branch
//! - `drops_remote_commits`: the push drops remote commits the branch lacks, someone's work: it asks first
//! - `pushes_again`: a push refused because the remote branch moved since its status goes again, once, over what the
//!   remote has now, only when nothing of it is lost

use crate::{PushAction, RemoteStatusUpdate};

impl RemoteStatusUpdate {
  /// What a push of the branch does over this status. The status compares content: a branch only rebased onto a newer
  /// baseline is up to date, the remote has its work; one reworded since is not, a robot would land the remote's
  /// messages. A push that is not a fast-forward (the remote has commits the branch lacks, older versions of its
  /// commits, or the branch was rebased) is a force push
  #[must_use]
  pub const fn push_action(&self) -> PushAction {
    if !self.remote_exists {
      return PushAction::Create;
    }
    if !self.needs_push() {
      return PushAction::UpToDate;
    }
    let drops = self.drops_remote_commits();
    let replaces = self.replaces_older_versions();
    if drops || replaces || !self.fast_forward {
      PushAction::Force { drops, replaces }
    } else {
      PushAction::Push
    }
  }

  /// A push of the branch would change its remote branch: the remote lacks it or commits of it, or has commits a push
  /// replaces or drops
  #[must_use]
  pub const fn needs_push(&self) -> bool {
    !self.remote_exists || !self.unpushed_commits.is_empty() || self.drops_remote_commits() || self.replaces_older_versions()
  }

  /// The remote has older versions of commits of the branch (amended here, or reworded: the same changes with another
  /// message or author), of its stack ancestors (restacked) or of the baseline (landed), or an older piece: it lacks new
  /// commits of the branches it is built on, which a Safe Push of the branch lands with it
  #[must_use]
  pub const fn replaces_older_versions(&self) -> bool {
    self.commits_amended > 0 || self.commits_restacked > 0 || self.commits_landed > 0 || self.commits_reworded > 0 || self.ancestor_commits_ahead > 0
  }

  /// The push drops remote commits the branch lacks that are no older version of a commit it has: someone's work, which
  /// the push asks for first (the app's Force Push, `bd push --force`)
  #[must_use]
  pub const fn drops_remote_commits(&self) -> bool {
    self.commits_behind > 0
  }

  /// Whether a push the remote refused because the remote branch moved since the status the user saw goes again, once,
  /// over this status, the one fetched after the refusal: only when the remote has no commits the branch lacks (nobody's
  /// work is lost) and the branch has work for it. A remote with someone's commits, or one that has the branch as it is,
  /// is told instead; so is one that moves again
  #[must_use]
  pub const fn pushes_again(&self) -> bool {
    !self.drops_remote_commits() && self.needs_push()
  }
}

#[cfg(test)]
#[path = "remote_status_test.rs"]
mod remote_status_test;
