# Proposal

## Why

The fix of a conflict of `bt-class-shards` (two commits of `(bt-class-shards)`, the last one needing a commit of
`bt-plan-age`, whose Safe Push ran) moved that commit to `bt-plan-age`, rewriting its prefix, while the user then
stacked `bt-class-shards` on `bt-plan-age` by hand and the sync applied it cleanly. The choice counted a restack as a
change of every commit of its branch and a move as one commit, so a move of one commit out of a branch of two won; the
rewrite of the user's prefix came only after. A dependency is an order of landing, which a stack says: a move splits the
piece the user made and lands it in more robots (the running robot lands the old branch, the moved commit is a tail for
another one).

## What Changes

- The score of a verified candidate: the failing branches, the emptied branches, the commits that leave the group the
  user gave them, the rewritten subjects, the commits whose base changes (a restacked branch and the branches built on
  it, the unassigned commits a branch takes). A restack moves no commit out of its group, so it comes before any move.
- The direct answer restacks every conflicting branch whose needed commits have owners, not only a branch that would
  move whole, but not onto a stack with a conflict that the answer leaves; of a partly verified answer only the
  restacks of the fixed branches stay.
- The candidates of a conflict are tried restack first, then the pull in, then the move; a change a trial built is not
  built again.
- A restack of the fix replaces the unstack of the `--plan` it builds on: the plan was invalid ("both stacked and
  unstacked"), so a fix on top of an unstack never offered the restack.

Capabilities: `branch/conflict-fix`.
