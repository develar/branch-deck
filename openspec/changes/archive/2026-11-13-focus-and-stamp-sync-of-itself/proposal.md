# Proposal

## Why

The previous change (`2026-11-13-no-focus-sync-before-the-first`) let a focus of a session run only once a sync of
another reason had been asked of it, a flag of the scheduler, and left a stamp of `brd` to sync any session a window
shows. Two deciders of one question, "does the session sync without the user": the open of a session and a window that
watches it anew read whether the worktree was synced before or the session committed a table, the focus read the flag
of the scheduler. And a stamp still made the first sync of a repository the app never synced after any mutation of
`brd`.

One predicate now, and it must know of `brd`: the app keeps `lastSyncTime` only for its own syncs, so after `brd sync`
in a terminal a session would have taken the worktree for never synced and shown the Ready to Sync card over branches
that exist.

## What Changes

- `sync_core::synced_mark`: every sync that succeeds, of the app and of `brd`, leaves `branch-deck/synced` in the git
  directory of its worktree, once; a session's host reads it without git. `SessionHost::synced_before` of the app and
  of the E2E server is the `lastSyncTime` of the app's state or that mark.
- The actor of a session (`Actor::syncs_itself`): it syncs of itself when it is `synced` (the worktree was synced
  before, or a sync of this session committed its table: what the open and a window that watches anew read already) or
  when the user asked a sync or a rewrite of it in this session (`Manual`, `AfterRewrite`), also one that failed. A
  focus and a stamp of a watched session that does not ask the host again, on the blocking pool, as `brd sync` may have
  marked the worktree since the open.
- `SyncScheduler::request` takes that predicate as an argument, beside `watched`, and keeps no flag of its own: a focus
  or a stamp of a session that does not sync of itself answers `skipped`.
- Capability `repository/onboarding`: the requirement "Before the first sync: A focus syncs nothing before the first
  sync" covers the stamp, the user's Sync and the sync of `brd`. Capability `branch/patronus`: a mutation of `brd`
  syncs the session a window shows of a repository synced before.
