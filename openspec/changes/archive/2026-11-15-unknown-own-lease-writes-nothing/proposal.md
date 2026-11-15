# A session of an unknown lease writes nothing of one, and the lease refusal of a run is its own outcome

## Why

A session opened while the epoch of its slot could not be told acted under the lease its slot read later
(`lease_began(Unknown, lease-b)`), so its run marked lease-b. The refusal of a run under another lease was told by the
code `plan_changed`, which another refusal could share and so reopen the session on every run, and its race with a
stamp that found the same lease was guarded only by a probe.

## What Changes

- A session that opened under an epoch the files could not tell, or whose slot reads none now, writes no file of a lease
  and no field of one (`lease_writes`, `set_repository_fields`).
- The lease refusal of a run is `Ran::AnotherLease`, an outcome of its own; a stamp that found the lease first keeps its
  waiter for the one reopen (the race test of the review).
- Capability `repository/onboarding`.
