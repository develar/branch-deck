# A failed reclaim waits its turn, and a reclaim moves only its own slot

## Why

The review of the reclaim below the minimum free space found two faults.

- A reclaim that failed after the check in full recorded nothing: the check failed, the lease changed, a Bazel server
  did not exit, or the return failed. The lease stayed the one checked longest ago. So each get below the minimum
  spent its two full checks on it first: a `status` of 2 to 40 s in idea-1 and the wait for its servers. Two such
  leases kept every get from the other idle leases.
- A reclaim moved the `returned` of each free slot returned after the lease went quiet one up. Each moved slot got a
  new epoch, and `pool-disk` measured it again in full (`measured.returned == slot.returned`). In idea-1 a full walk of
  the free slots takes minutes.

## What Changes

- A reclaim that fails after the check in full records its time (`Lease::reclaim_failed_at`). It counts as a check
  for the order of the reclaim of a get. A get passes over the lease for `RECLAIM_RETRY` (one hour). The keeper and
  `brd lease reclaim` try it each time.
- The free slots are ordered by the time of their return, then by its number (`Slot::return_order`): the reserve, the
  trim, the shrink and the eviction alike. A reclaim gives its slot the next number of the pool and the time its lease
  went quiet, and no other slot changes.
- Capability `cli/lease`.
