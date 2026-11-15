# A reclaimed slot is the coldest free slot of its pool, and no clock orders a pool

## Why

The review of the reclaim order found two faults.

- A reclaim gave its slot the next number of the pool and the old time of its lease. The order by time first put the
  slot before the free slots returned since. A brd that orders the reserve and the trim by the number alone took
  the cold slot first for a get and kept it over a warm one. One state file serves every installed brd.
- The order by time first let a step back of the clock change the order of one pool.
- No test failed when the trim ordered by the number again.

## What Changes

- A reclaim gives its slot a number below every other free slot of the pool, never its number before the lease, and
  a time not after any of theirs. Every brd orders it last for a get and first for the trim and the shrink. With no
  number left below, the other free slots move up by `RECLAIM_ROOM` once.
- A return records a time not before the last return of the pool. Within one pool the time follows the number.
- The number alone orders the free slots of one pool for the reserve, `next_free` and the trim. The time orders only
  the shrink across pools. `Slot::return_order` is gone.
- Capability `cli/lease`.
