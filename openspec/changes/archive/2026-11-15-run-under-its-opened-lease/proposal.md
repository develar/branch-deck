# A run of a session writes under the lease its session opened under

## Why

The run took the lease of the slot as it began, not the lease its session opened under: a run of a session opened
under one lease that began once the slot held the next one (returned and leased again, the stamp not heard yet) marked
the slot of the next lease synced and recorded its landed state, so the session of that lease synced of itself.

## What Changes

- `SessionHost::run` gets the lease the session opened under; the run writes the files of the lease only under it
  (`lease_began`, shared with `Sessions::lease_guard`), and a run of a slot that holds another known lease syncs
  nothing (`plan_changed`): its requests are skipped and the session opens anew for that lease.
- Capabilities `repository/onboarding`, `repository/landed-commits`.
