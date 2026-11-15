# The cargo rule joins a classic unit by its hash and keeps the objects of a compile under way

## Why

A review of the cargo rule found three faults. In the classic layout of stable cargo, `deps/` names a unit by the
crate of its target and `.fingerprint/` by its package. So a bin named apart from its package (`brd` of `bd-cli`) was
two units, and the files of a current unit could go while its fingerprint stayed: the next build compiled it again.
The object pass after the release of the locks of cargo assumed that no crate compiles longer than 10 minutes. Below
that `age` it ran under the locks, 48.7 s on one slot, against a return that waits 30 s. The requirement said that
the objects go once the slot is free, but the prune of a leased slot deletes them while the lease holds. Also, a
`low_disk_space` counted the leased slots that freed nothing.

## What Changes

- A unit of the classic layout is its hash: the files of `deps/`, `.fingerprint/` and `build/` with that hash go
  together, and the group is the package that the fingerprint dir names and its target.
- An object stays while it is not older than the newest rlib, rmeta or executable of its unit, as a running rustc
  writes it after these. A unit with no such output keeps every object. So the object pass always runs after the
  release of the locks.
- The requirement on the objects states when they go: when the caller empties its trash, also in a leased slot.
- A `low_disk_space` counts only the slots of the shrink that freed bytes.
- Capability `cli/lease`.
