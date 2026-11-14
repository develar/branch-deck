# Proposal

## Why

A slot of a lease pool keeps its path from one lease to the next, and the app keeps what it records of a worktree by
its path: the `lastSyncTime` of `repositoryState` (synced before, "Last synced"), the rest of that entry (the room of the
loading table, the settled layout), the frame of its session (the rows of the warm start) and a session kept idle. A
window that opened the next lease of a slot the app had synced synced it as it opened, showed "Last synced" of the
earlier lease and could seed its rows.

## What Changes

- `lease_pool::epoch::worktree_epoch`: the epoch of a worktree, the id of the lease its slot holds now (`returned <n>`
  of a free slot), none outside a pool; read without git.
- The writer of the settings names the epoch in each entry of `repositoryState` it writes (`epoch`, never the page's)
  and forgets an entry of another epoch before the key leaves it (`snapshot`, `changes_since`, `value`, the change of a
  `set`), so neither `synced_before` nor a window reads one of an earlier lease.
- A frame names its epoch (`FRAME_SCHEMA` 17) and seeds only a session of it; a window that comes back to a session
  kept under another epoch opens a new one.
- The E2E server leases a slot of the pool of a test repository as the repository a test opens
  (`POST /repositories/{id}/lease-slot`, `leaseSlot` of the setup), and the setup hands the page the writer's
  `repositoryState`, as the build of a window does.
- Capability `repository/onboarding`: what the app keeps of a slot is of its lease. `cli/lease`: the epoch of a slot.
