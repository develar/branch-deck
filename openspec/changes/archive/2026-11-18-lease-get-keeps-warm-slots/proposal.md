# Proposal

## Why

On 2026-11-18 the idea-1 pool held 12 leases against a `branchdeck.leaseMaxSlots` of 10. A trim counts every slot,
leased or free, so below the minimum free space each get's trim deleted every free idea-1 slot, with its Bazel output
base, including the slot that get was about to reuse; the get then found no free slot and cloned one, 25 minutes, while
the next gets waited for that clone (up to 62 minutes). The trim deleted whole slots before the shrink took a byte of
build output, and it measured no free space: meanwhile the free slots of the branch-deck pool kept 114 GiB of `target`.

## What Changes

- The trim of a get below the minimum free space, and of the keeper, deletes no slot: only the Bazel output bases of
  gone workspaces and the expired notes. The free space comes from the shrink, then the eviction, which measure after
  each slot and keep the next free slot of the get's own pool.
- `branchdeck.leaseMaxSlots` limits only `brd lease trim`, the trim of the app and `brd-host trim`.
