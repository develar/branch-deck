# Proposal

## Why

The first sync of a repository is the user's: it creates the branches and the notes in the repository (onboarding,
"No automatic first sync"), so a repository never synced shows the Ready to Sync card until its Sync. But the scheduler
of the session ran a focus of a session that never synced at once. So the first focus of the window after it opened a
repository never synced synced it behind the user's back, with Auto-sync on Focus on by default: in the desktop app a
return to the window, in the IDE a click into the tool window. The E2E test of the welcome card in the IDE
(`tests/e2e/air/air-page.spec.ts`) failed in 19 of 20 runs under load on the Studio: the click on Save gave the frame
the focus, and when the focus reached brd after the session of the new prefix answered, it synced the repository, and
the Ready to Sync card went.

## What Changes

- `SyncScheduler` (`crates/repository-model/src/sync_scheduler.rs`): a focus of a session that was asked no sync of
  another reason yet (the open of a worktree synced before, the user's Sync, a rewrite, a stamp of `brd`) runs nothing
  and answers `skipped`, as one of a session no window shows does. From the first such sync on, a focus checks the
  inputs as before. A stamp still syncs: `brd` wrote in the repository.
- Capability `repository/onboarding`: the requirement "Before the first sync: A focus syncs nothing before the first
  sync", its tests the table test of the scheduler and an E2E test of the Ready to Sync card.
