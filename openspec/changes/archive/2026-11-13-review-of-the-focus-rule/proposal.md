# Proposal

## Why

The review of the rule that a focus syncs nothing before the first sync found what the rule left open:

- a stamp of `brd` before the first sync is decided as a focus now, but the spec did not say so, nor that a mutation of
  `brd` that is no sync (`brd reword`, `brd lease get` beside) syncs nothing;
- the mark of a sync lives in the git directory of the worktree, which a lease slot keeps across its leases: the next
  lease would have been a worktree synced before;
- whether the mark is of a worktree or of a prefix was left unsaid;
- a run that failed after git could not read its inputs was not remembered, so every focus ran it again;
- "a prefix changed in a terminal shows on the next focus" is not so for a repository never synced.

## What Changes

- `SyncScheduler`: a failed run is remembered without inputs too, and a focus that cannot read them either answers its
  error without a run; its header says how a stamp is decided.
- `brd lease return` takes the mark of a sync out of the slot (`sync_core::synced_mark::forget_synced`).
- The stamp fixture of the E2E server answers what the session did of a stamp (`answer`).
- Capability `repository/onboarding`: the stamp before the first sync, the user's rewrite, the mark of the worktree
  across a change of the prefix, the failed sync without inputs, and the next focus that shows a prefix only of a
  session that syncs of itself. Capability `cli/lease`: the return takes out the mark.
