# Proposal

## Why

On 2026-11-25 an agent of the idea-1 lease `idea-15` sent 4 exclusive heavy runs to the Studio, then returned the
lease while the runs waited. No client was left, and nobody could fetch their results. The runs kept their place at
the head of the heavy queue for 2 h and ended with `heavy_timeout`. Exclusive runs at the head of the queue were the
largest cause of waits on the Studio.

## What Changes

- Every end of a lease (`brd lease return`, `brd lease pick --return`, the takeover of a get, `brd lease reclaim`, the
  reclaim of a get or of the keeper below the minimum free space) cancels each run of brd-host of the slot that has no
  end, before it changes the slot.
- The new command `brd-host cancel-unfinished` cancels these runs on their hosts, the hosts in parallel, each one for
  at most `--max`, and forgets the record of each cancelled run. brd runs the `brd-host` beside its binary, as brd
  links no client of the remote runs.
- A host that does not answer holds the end of a lease for a few seconds at most. The end goes on, and its answer
  names the runs that it could not cancel (`uncancelled_runs`) beside the cancelled ones (`cancelled_runs`).

Capabilities: `cli/lease`, `cli/host`.
