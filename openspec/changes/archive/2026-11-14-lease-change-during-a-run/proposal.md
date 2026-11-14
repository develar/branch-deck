# Proposal

## Why

The review of 3779eea9 found three holes of a slot leased again under a window. A stamp of the new lease that came
while a run was in flight was a recheck of the session of the lease before: it synced the worktree of the new lease
under that session. Four stores that hold something of a row by the branch's name kept it under the view of the new
lease: the confirm of a Force Push, whose `accept` pushes what the session before held, the Request Review… popover,
the popover of a start and the Create Pull Request dialog; so did the user's choice of the side stacks. And a rewrite
asked on the rows of the lease before, which may go by a branch's name, could run on the branch of the same name of the
new lease.

## What Changes

- A stamp of another lease during a run opens the session anew once the run ended (`reopen_after_run`): its table
  dropped, the requests it served told its end, no recheck and no follow-up of the session before; the scheduler of
  the facts hears the session close at the stamp. The reopen runs right after the message that asks it, before any
  other (`reopen_now`).
- The four stores and the side stacks forget the rows (`onRowsForgotten`); `pnpm lint:arch` refuses a store that names
  `branchName` without it (`row-store`), a store that only projects the view an entry of ALLOWLIST.
- A rewrite names the epoch of the view its table was seen in (`TableWitness::epoch`), and one of another epoch than the
  session's is `plan_changed` (`Sessions::view_version`). Every other intent is planned again and runs only on its
  fingerprint, so it has no plan of the new lease before its first sync.
- The session marks the worktree synced, not the sync (`SyncOptions::marks_synced` off, `SyncReport::synced_mark`), and
  only under the epoch the run began under (`GitSessionHost::run` reads it before and after): a run across a lease
  change marked the slot of the new lease synced, which then synced of itself.
- Every file a lease leaves in the git directory of its slot is listed once (`lease_state::LEASE_FILES`: the mark of a
  sync and the landed state) and forgotten at the end of the lease (`forget_lease_state`, which replaces
  `forget_synced`), so a new lease inherits none, the landed state too.
- `row-store` keys on what identifies a row (`ROW_KEYS`), not on `branchName` alone.
- Capabilities `repository/onboarding` (a window on a slot leased again) and `cli/lease` (what a lease leaves).
