/// The epoch of a worktree (`lease_pool::epoch::worktree_epoch`, docs/design/git-operations.md, "The epoch of a
/// worktree"): a slot of a lease pool keeps its path from one lease to the next, so what the app records of a worktree
/// by its path names the epoch it was recorded under, and a record of another epoch is of another lease. Only a known
/// epoch decides that: an epoch the files could not tell (`Unknown`) forgets nothing and stamps nothing anew
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum WorktreeEpoch {
  /// No slot of a lease pool: the main checkout, a worktree of `git worktree add`, a path that is no git worktree
  #[default]
  NotASlot,
  /// A slot of a lease pool: the id of the lease it holds, `returned <n>` while it is free after its last return
  Lease(String),
  /// The files that tell could not be read or parsed: an error of the disk, a read during a rename on Windows, a state
  /// a later build wrote in a form this one cannot read
  Unknown,
}

impl WorktreeEpoch {
  /// The form a record keeps of the epoch (`Some(None)` outside a pool); `None` when the epoch is unknown
  #[must_use]
  pub fn recorded(&self) -> Option<Option<&str>> {
    match self {
      Self::NotASlot => Some(None),
      Self::Lease(epoch) => Some(Some(epoch)),
      Self::Unknown => None,
    }
  }

  /// Whether a record kept under `recorded` is of another epoch than this one, so it counts as absent: only for a known
  /// epoch, and only for a record that names one (a record of a path outside a pool, or of a build before the epochs,
  /// names none and is kept)
  #[must_use]
  pub fn outdates(&self, recorded: Option<&str>) -> bool {
    match (self.recorded(), recorded) {
      (Some(now), Some(kept)) => now != Some(kept),
      _ => false,
    }
  }
}
