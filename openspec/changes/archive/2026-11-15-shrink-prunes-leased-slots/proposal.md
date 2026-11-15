# A get below the minimum prunes the stale build output of the leased slots first

## Why

On 2026-11-15 the leased slots of branch-deck held cargo targets of 48 to 120 GB, and the free space fell by about
190 GB in a few hours. The clean rules ran only at a return, a clean of a free slot and the shrink of a free slot, so
a long session grew without limit. The shrink of a get deleted the warm build output of the free slots instead.

## What Changes

- Below the minimum free space the shrink first prunes each slot with a `ready` lease of an agent by the cargo rules of
  its lease, with their own `keep` and `age`, the largest build output first (as `disk.json` last measured it). Then it
  cleans the free slots as before. A host slot and a lease in another state are passed over.
- The prune takes the locks of cargo of a profile dir first, and then checks the lease under the pool lock: a return
  or a get that came between keeps the slot. It applies no `delete` rule, sets no marker and stamps the lease
  (`pruned_at`). The shrink passes over a slot within the smallest `age` of its cargo rules after a prune.
- A slot in which cargo builds stays, named in `shrunk.building`.
- `brd lease clean --leased` and `brd lease clean <dir>` of a leased slot apply the same prune on demand (`leased_by`);
  `--keep` and `--age` never apply to a leased slot.
- Capability `cli/lease`.
