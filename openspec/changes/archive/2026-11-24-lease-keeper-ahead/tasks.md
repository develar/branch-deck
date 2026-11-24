# Tasks

- [x] The prune ahead of the minimum in `lease::relieve_core` (`ahead_threshold`, `prune_ahead`), `ShrinkParams::keep_next`
- [x] `host_slots::keep_free` takes `KeepFreeOptions` (the sizes of `disk.json`, a volume of a test)
- [x] The line of the keeper names the threshold, the pruned leased slots and the building ones
- [x] The prune ahead deletes the trash that earlier deletes left first (`lease::delete_left_trash`), and one that ends below
  the minimum relieves no host slot and starts no wait (`ReliefRound`)
- [x] Docs: the design of host runs, Working in a Lease, the guide
