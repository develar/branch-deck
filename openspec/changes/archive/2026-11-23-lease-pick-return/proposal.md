# Proposal

## Why

Agents end almost every lease with `brd lease pick` and then `brd lease return`. The follow of that pick moves a lease
that the return frees at once: in the IntelliJ monorepo it costs two of the four full unpack_trees of a pick (the dry
run on a copy of the lease's index and its `read-tree -m -u`) and both refreshes of that index, 7.6 s each without
fsmonitor, and the return then proves again what the pick just proved.

## What Changes

- `brd lease pick --return` picks without the follow and returns the lease in the same process, after the pick released
  the repository lock. The commits of the pick's range are landed by the pick (`proof: picked`); a commit made after the
  pick read the HEAD of the lease goes through the check of any return. Every refusal of a return keeps the lease
  (`return_refused`), a conflict or a refusal of the pick too. `--stop-own` passes to the return.
- The answer names the HEAD the pick read (`source_head`): the undo token has no `+follow`, so `brd undo` moves only the
  home branch back, and that HEAD holds the commits after it.
- The `next` of a get and of `brd workspace` ends a lease with a home branch with `brd -C <path> lease pick --return`.

Capabilities: `cli/lease`.
