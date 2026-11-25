# Proposal

## Why

Only a get wrote the holder of a lease (`holder_pid`, `holder_started`). A resumed session runs as another process, so
its lease stayed idle while it worked: on 2026-11-25 slot 32 of idea-1 had a holder that was gone since the day
before, and its resumed session ran in the slot. After an hour with no move of HEAD, a get could take such a lease
over while no process of the session worked in the slot, and the live session got `lease_reclaimed`.

## What Changes

- A new `lease_pool::lease::rebind_holder` makes the session of `brd` the holder of the lease of its slot under the
  pool lock, by a compare-and-swap of the lease, when all these hold: the lease is `ready`, of an agent, its holder
  gone, no get takes it over; the slot has no `reclaimed` record; the working directory of `brd` is inside the slot;
  no process between the session and `brd` has a controlling terminal.
- `brd workspace` calls it after the check of a taken-over slot and names a rebind in `holder_rebound`. Every command
  that writes calls it after `refuse_reclaimed`. A read calls it never.
- The state is read first: a live holder costs no scan and no lock.

Capabilities: `cli/lease`, `cli/workspace`.
