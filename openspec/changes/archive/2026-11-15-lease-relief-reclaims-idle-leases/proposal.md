# A get below the minimum free space returns the idle leases whose work landed

## Why

On 2026-11-14 the volume had 164 GB free against a minimum of 200 GiB. Nine leases of dead holders stayed in the
idea-1 and accio pools. Each pool had free slots, so no get took them over, and the relief of a get only named them.
A read-only check from the main checkouts on 2026-11-15 found: accio slots 3, 4 and 11 have no commit beyond their
leased commit, accio slot 5 has all its commits in main, accio slot 7 has 2 commits that main lacks, and the four
idea-1 slots have unlanded commits. Thus up to four accio leases have only landed work, if their trees are clean. A
reclaim also stamped its slot as returned now, so the next get took its stale output first, and the shrink cleaned a
warm free slot instead.

## What Changes

- Below the minimum free space a get returns, after the trash and before the trim, the idle leases of every pool of
  the machine whose work landed (`reclaim_for_get`), by the rule of `brd lease reclaim --idle`. It reads only the
  slots on its volume, runs under `~/.branch-deck/pressure.lock`, and tells no step when no pool holds an idle lease
  there. A recorded check that holds costs one `rev-parse`; a get checks at most two leases in full, the ones checked
  longest ago first. The keeper of `brd-host` takes the same step and checks every lease.
- A reclaimed slot is returned as of when its lease went quiet (`reclaim_stamp`), so the trim and the shrink take it
  before the free slots returned since. This applies to `brd lease reclaim` too.
- The answer of the get names the reclaimed leases (`reclaimed`), and `low_disk_space` counts them.
- Capability `cli/lease`.
