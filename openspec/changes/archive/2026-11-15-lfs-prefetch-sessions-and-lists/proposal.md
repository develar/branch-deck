# The fetch of Git LFS before a move: resolve sessions, lists of paths, a cancel, a moved store

## Why

A conflicted update fetched nothing (`UpdatePlan::worktree_move` is none), yet the start of its session may finish under
the lock and `brd update --continue` always finishes there: the smudge downloaded the upstream's objects under the lock.
A long list of paths, or one with a comma or a character of a pattern, fetched every object of the commit the store
lacked, which in a partial store can be gigabytes. A store that `lfs.storage` moves was never checked.

## What Changes

- The start of a session and each `--continue` fetch the paths that change from HEAD to the upstream tip before the
  lock (`SessionState::worktree_move`).
- The paths go in lists of at most 8 KiB, several fetches, each path a pattern of Git LFS that matches it: a character
  of a pattern and a leading `!`/`#` escaped, a comma and a blank at an end `?`.
- The fetch ends as the operation is canceled (the Stop Waiting of the app's wait for the lock).
- Where the operation read its config anyway (`ConfigSnapshot`), `lfs.storage` is a fact of its executor, and the check
  of the store takes it, with no process more.
- Capabilities `repository/update-project`, `commit/drop`.
