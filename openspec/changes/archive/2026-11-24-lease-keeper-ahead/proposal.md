# Proposal

## Why

With no shared build cache, each lease's `target/` is full size, about 50 GiB for a full workspace build. On 2026-11-24
the user's Mac went from 139 to 82 GiB free in about an hour, below its `branchdeck.leaseMinFree` of 100 GiB. Nothing
freed the build dirs of leased slots in the background before the floor. Only a get below the floor did it, by a
shrink. The brd-host keeper relieved the volume below the floor too, and its log line did not show the pruned leased
slots: "nothing to delete".

## What Changes

- The keeper's relief (`host_slots::keep_free`) prunes ahead of the minimum. When the volume has the minimum but less
  than one and a half of it, it first deletes the trash that earlier deletes left. Then it runs the shrink alone, up
  to that threshold: first the leased slots by the cargo rules of their leases, then the free slots, except the one each
  pool's next get takes. It takes the lock of the shrink of the machine only when no get holds it. One that freed
  nothing makes the next one wait 10 minutes. One that ends below the minimum relieves no host slot and starts no wait:
  the next round runs the whole relief.
- The order of the leased slots in the keeper's shrink follows `disk.json`, as in a get.
- The line of a relief names the leased slots it pruned, with their holders, and the ones where cargo builds. A prune
  ahead that freed nothing writes no line.

Capabilities: `cli/lease`, `cli/host`.
