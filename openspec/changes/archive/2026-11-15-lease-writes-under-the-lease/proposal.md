# A run writes the files of the lease only under the lease it began under

## Why

A run of the app's session across a return and a new lease of its slot still recorded the landed state
(`branch-deck/landed.json`) of the lease before into the slot: only the mark of a sync checked the epoch of the slot,
and `record_sync`, which the update check inside `sync_branches` runs, wrote at any time. The new lease then reported
what landed of a branch it never had until the report expired. Every write into a file of the lease has to go through
one place that knows the lease the work began under.

## What Changes

- `sync_core::lease_state` is the one writer of the files of a lease (`write_lease_file`, `remove_lease_file`,
  `lock_landed`), each write under the guard of its caller (`LeaseGuard`); `synced_mark` and `landed` decide what the
  files hold. `SyncOptions::lease` replaces `marks_synced`, and `SyncReport::synced_mark` goes: the sync marks the
  worktree and records the landed state under its guard, and the app's session gives it one that asks the epoch of
  the slot right before each write and holds only under the epoch the run began under.
- `lease_files_scan_test.rs` fails a name of a file of a lease, or a path of one, outside the modules of its table, and a
  write or a removal of a file in one of them but `lease_state.rs`. The paths of the landed state in the facts of a sync
  are `pub(crate)`, and `landed::STATE_FILE` is `LANDED_FILE`.
- Capabilities `repository/onboarding` (a run across a lease change writes nothing of the lease) and
  `repository/landed-commits` (a sync records the branch only under the lease it began under).
