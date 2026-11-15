# A reclaimed slot never reads as an earlier free period of itself

## Why

The review of the reclaim order found that a reclaim can give its slot a number that the slot had in an earlier free
period. The reclaim takes a number below the lowest other free slot. It skipped only the number from just before the
lease.

- `disk.json` keeps the last measure of each path. On Linux and Windows `pool-disk` takes a free slot from it when the
  number matches, with no walk. So it showed the bytes of an earlier free period as current.
- The epoch `returned <n>` of the slot repeated. An app record of that earlier free period counted as current.
- A reclaim of the slot of the highest number lowered the highest number of the pool. The next return then gave a
  number of an earlier free period again, also with no reclaim record.

## What Changes

- The epoch of a free slot with a record of a reclaim or a takeover is `returned <n> reclaimed <lease id>`.
- The measure of a slot in `disk.json` names the reclaimed lease. A measure without it, or with another, is stale.
- The state of the pool keeps the highest number it gave (`last_returned`). A return takes one past it.
- Capability `cli/lease`.
