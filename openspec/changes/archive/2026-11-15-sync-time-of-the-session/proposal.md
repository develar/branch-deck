# The time of a sync is the session's own record, of the lease it opened under

## Why

The page wrote `lastSyncTime` with the whole entry of the repository, and the writer stamped the entry with the epoch
of the worktree at the time of the write. The window of a lease wrote it after the slot was returned and leased again
(onboarding.spec.ts:298, 9 of 800 runs at 14–16 workers): the entry, the time of the sync of the lease before in it,
became the new lease's, whose session then read the slot as synced before and synced of itself, and showed no Ready to
Sync card.

## What Changes

- The session records the time of each sync it ran (`SessionEnvironment::record_synced`, `app_settings::projects::record_sync`)
  under the lease it opened under: the writer writes it in the entry of that lease and refuses it while the worktree
  holds another (`SettingsService::set_repository_field`).
- A write of a window keeps the writer's `lastSyncTime` for the epoch the entry is stamped with, whatever it carries
  (`set_of_window`, `keep_rust_fields`); the page writes it no more.
- The E2E server records it on the clock of the page (`X-Page-Now` of each command, `TestRepository::page_now_ms`).
- Capability `repository/onboarding`.
