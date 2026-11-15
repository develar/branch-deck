# Tasks

- [x] The epoch of the view on Update Project, the undo and the dismiss, refused on another epoch or another lease of
  the slot; the guard of the lease of the view on their writes; operations tests of both refusals, of an update whose
  slot changed its lease while it waited, of a dismiss and an undo leased again right after their check, and the
  session test of the guard, with mutations (no check of the view, no check of the lease, the update, the dismiss and the
  undo under `any`, a guard of the session that always holds)
- [x] The lease of the slot at the start of `brd`; the test of `brd -C <slot>` across a return and a get, with a
  mutation (the context under `any`)
- [x] The lock of the lease across the check and the write, taken by the forget; the probe of a forget between the two,
  with mutations (a write, a forget without the lock)
- [x] `LeasePaths` and the scan of the leaf names beside `branch-deck`, with the probes of a path built at run time in
  `update-core/src/pick.rs` (`join`, `concat!`)
- [x] Git Rules and the "Lease" section of Git Operations
