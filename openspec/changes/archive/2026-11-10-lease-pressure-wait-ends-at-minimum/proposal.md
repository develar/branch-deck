# Proposal

## Why

On 2026-11-10 a `brd lease get` of branch-deck waited over ten minutes behind a `brd lease get --holder win-native -C
~/projects/idea-1`. The lock it waited for, `~/.branch-deck/pressure.lock`, is held only across the shrink and the
eviction of a get (`Relief::relieve`), never across the checkout or the clone, but both delete: the eviction deletes
free slots whole, and a slot of idea-1 is 1.4 million files, whose delete took minutes at load 100 (`rm -rf` of 52,000
files of a cloned `target/` took 17.6 s there, one thread; `remove_all` splits a slot by its top-level entries over 8
threads, and `community/` holds most of them). A get waited for the whole shrink and eviction of the other, though the
first slots it deleted gave the volume the minimum this get needed.

## What Changes

- A get that waits for the turn of the shrink or the eviction measures the volume at each poll of the wait (one
  `statfs`, `lock_pressure`'s `canceled`) and goes on without the turn once the volume has its minimum: it measures
  again, finds enough, and cleans nothing. The holder goes on to its own target.
- The words of the wait say it: "… the free space is measured again after it, or once it has the minimum".

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cli/lease`: Get: The shrinks and evictions of the machine take turns under one lock.

## Impact

- `crates/lease-pool/src/lease.rs` (`pressure_turn`, `PressureTurn`), `crates/lease-pool/src/shrink.rs`
  (`lock_pressure`); docs/design/git-operations.md, "Lease pool".
