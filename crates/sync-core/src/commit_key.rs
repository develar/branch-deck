//! The identity of a commit across a rebase and an amend: the author email, the author time and the subject without the
//! `(prefix)`.
//!
//! A rebase (Safe Push, `git rebase`, `cherry-pick`) and an amend keep the author and the author time, and a virtual
//! branch commit carries the message of its local commit without the prefix. So a local commit, its copy on a virtual
//! branch, the baseline commit that landed it (`landed`, spec/repository/landed-commits.spec.md) and an older version
//! of it on the remote branch (`remote_status`: amended or restacked, docs/design/remote-status.md) share the key. Two
//! different commits of one author with the same subject in the same second share it too; both uses accept that.

use crate::commit_grouper::stripped_subject;

/// A commit identified across a rebase or an amend
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommitKey {
  pub email: String,
  pub time: u32,
  /// The subject without the `(prefix)`, as the virtual branch commit was pushed
  pub subject: String,
}

impl CommitKey {
  /// The key of a commit by its full message
  pub fn from_message(email: &str, time: u32, message: &str) -> Self {
    Self::from_subject(email, time, &subject_of(message))
  }

  /// The key of a commit by its subject as git's `%s` prints it
  pub fn from_subject(email: &str, time: u32, subject: &str) -> Self {
    Self {
      email: email.to_string(),
      time,
      subject: stripped_subject(subject).trim().to_string(),
    }
  }
}

/// The subject as git's `%s` reads it: the first paragraph of the message, its lines joined by a space
pub fn subject_of(message: &str) -> String {
  message
    .trim_start()
    .lines()
    .take_while(|line| !line.trim().is_empty())
    .map(str::trim)
    .collect::<Vec<_>>()
    .join(" ")
}
