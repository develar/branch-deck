# Proposal

## Why

The gets that wait for a slot of the pool had no order. The wait for the clones lock polls a file lock every 50 ms, so
the lock fell to whichever process polled first. A get that took the lock dropped it at once when its reserve gave it
a free slot or an idle lease to check, and got back in line when the check found work. A slot freed during the wait
went to whichever waiting get reserved first, and a new get took a free slot past the gets that waited. On the idea-1
pool on 2026-11-25 a get that came 2 min later passed two gets that waited, and one get waited 56 min 34 s for the
clones lock while three later gets added slots.

## What Changes

- A get that finds no free slot enters a queue with a ticket in `leases.json`: its lease id, holder, pid and start,
  time of entry, heartbeat, phase and the slots it tried. Its phase changes in the write of its reserve.
- A free slot goes to the first waiting ticket that has not tried it, an idle lease to the first waiting ticket that
  takes idle leases over and has not tried it; a get without a ticket comes after every ticket.
- Only the first waiting ticket waits for the clones lock. A get behind it says whom it waits behind.
- A busy ticket (a check, a relief without the lock) and a stale one (no heartbeat for 60 s) are passed over and keep
  their place. A get that waits for a return under the clones lock stays waiting, so no get behind it takes that slot.
  A ticket leaves with the new slot, with its `ready` lease, or with its get; the ticket of a gone process leaves at
  the next write.
- A free slot or an idle lease that the queue gives a waiting get ends its wait at once.
- A get that holds the clones lock keeps it across the checks of each slot that its reserve under the lock gives it:
  the stop of the Bazel servers and the `status` of a free slot, at most two checks of an idle lease. It keeps its
  turn, and releases the lock once the slot is its own.
- A brd before the queue drops it from the state and passes it; each waiting get writes its ticket again.

Capabilities: `cli/lease`.
