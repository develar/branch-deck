# Proposal

## Why

The review of 40b9f42c found a regression: a submodule and a checkout of a separate git directory have a `.git` file
whose git directory has no `commondir`, which the epoch took for a failed read (`Unknown`), so the frame of such a
repository never seeded and every start of it was cold. And two low items: an entry written between the two holds of
the mutex of the writer, of the lease now, was forgotten by the epoch read before it; and the parse of the state of a
pool was kept by a size, a time and an inode, which tell a change apart on APFS only.

## What Changes

- A `.git` file that names a git directory without a `commondir`, or not under the `worktrees` of its common directory,
  is no slot (`NotASlot`); only a file that cannot be read is `Unknown`.
- The writer forgets only an entry that still names the epoch it named when the read began.
- The parse of a state, and what a `.git` file names, are kept by their bytes.
- Capability `cli/lease` and `repository/onboarding`: the scenarios of each, with their tests.
