## Why

On Windows a get recorded no holder process (develar/branch-deck#204): the session was found by the process scan,
which Windows does not have. With no `holder_pid` a lease is never idle, so neither the takeover of a get nor
`brd lease reclaim --idle` ever freed one, the pool grew a slot per session, and the reclaim dropped those leases
without a word. A resumed session in a taken-over slot was never refused there either.

## What Changes

- The ancestors of `brd` come from one snapshot of the process list on Windows (`CreateToolhelp32Snapshot`: the pid,
  the parent and the exe of each; no memory of another process). A get records its holder there, and the refusal of a
  reclaimed slot tells its holder there too. A parent that started after its child ends the chain, and a chain cut
  so tells no ancestor and denies none. The scan of the working directories stays a non-goal.
- `brd lease reclaim --idle` names a ready lease without a holder process in `skipped` as `holder_unknown`, checked by
  nothing, for the user to return.
- `brd workspace` names `holder_alive` on Windows too.
- Edits of the main spec of `cli/lease` that are no requirement: its non-goal says the scan of the working directories
  on Windows, and its known limitations no longer say that `lease_reclaimed` refuses nothing on Windows.

## Capabilities

### Modified Capabilities

- `cli/lease`: Get: A get records the process it runs for; Reclaim: Reclaim applies the rule of the takeover to every
  idle lease; Reclaimed: The session that holds the slot is told by its process.
- `cli/workspace`: Every ready lease of the pool is listed with its holder.
