# Proposal

## Why

On 2026-11-10 four `brd lease get` stood behind `~/.branch-deck/pressure.lock` for over 20 minutes. The holder (the
installed brd 1.6.0, a get of accio) spent the whole time in one delete: a sample of it shows `lstat` 549 and `unlink`
547 of its samples, `rmdir` 53, `open` 104, the delete of `remove_measured`, which counts the bytes of each file as it
deletes it; no other walk. The shrink and the eviction measure the volume (one `statfs`) only before each slot, and a
slot of idea-1 is 1.4 million files, minutes to delete (52,000 files in 17.6 s on one thread at load 100 to 145). Once
the user freed space and the volume had its minimum, the holder still deleted the rest of its slot, and every get
behind it waited.

## What Changes

- The delete of a slot by the shrink or the eviction asks the volume (one `statfs`) every 256 entries on its thread,
  every 50 ms for the eviction's threads, and stops once the volume has the target of the step: the minimum and a tenth
  for the shrink, the minimum for the eviction. Neither takes another slot then.
- What it did not delete stays in the trash as the trash of a run that is gone (`left-…`, renamed at once), which the
  next get below the minimum deletes first, outside the lock, and every sweep of the pool's trash (a return, a clean).
- The waiters go on at the minimum already (lease-pressure-wait-ends-at-minimum); a waiter now tells its wait again once
  a minute, with how long the holder holds the lock and the free space against the minimum, so a wait of minutes
  reads as the deletes of disk pressure, never as a hang.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cli/lease`: Get: The shrinks and evictions of the machine take turns under one lock; Get: The shrink stops at the
  minimum and a tenth of it; Get: The eviction stops at the minimum without the margin of the shrink.

## Impact

- `crates/lease-pool/src/lease_clean.rs` (`remove_measured_until`, `Trash::empty_until`, `Trash::abandon`),
  `crates/lease-pool/src/shrink.rs`, `crates/lease-pool/src/trim.rs` (`remove_all_until`, `evict_core`);
  docs/design/git-operations.md.
