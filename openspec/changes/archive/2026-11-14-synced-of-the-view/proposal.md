# Proposal

## Why

The review of the first-sync fix found that the page still decided "never synced" itself, by the `lastSyncTime` of
`repositoryState`, while the session decides it by `lastSyncTime` or the mark of a `brd sync`. A worktree that only an
agent's `brd sync` synced opened with the Ready to Sync card over a session that was already syncing as it opened. The
mark named no prefix, so a `brd sync --prefix X` marked the sessions of every prefix. A get that reused a free slot
kept the mark a return of an older build had left there. In Air, a subscription that joined a view whose link broke
subscribed the frame anew, as after a refusal of brd.

## What Changes

- `RepositoryView.synced` (and `TableChanges.synced`): whether the session syncs of itself (`Actor::synced`), patched
  as it changes. The Ready to Sync card, the sync after a write of Settings (`useRepositoryConfigChanges`) and the
  Sync asked again after `prefix_changed` (`refused` of `src/model/store.ts`) read it; `lastSyncTime` is read only by
  what shows "Last synced" (`last-sync-read` of `pnpm lint:arch`).
- The mark of a sync names its prefix (`branch-deck/synced`, one prefix per line); `synced_before` of a host reads the
  line of the session's prefix.
- A get that reuses a free slot takes the mark out too (`move_reused_slot`, `forget_synced`).
- The shim of Air subscribes the frame anew only after a refusal of brd; a broken link fails every subscription that
  joined the view.
- Capability `repository/onboarding`: the card and the Sync asked again read the view; the mark names the prefix; a
  worktree `brd` synced opens syncing. `cli/lease`: a get takes out the mark too. `repository/ide-embedding`: only a
  refusal makes a subscription subscribe anew.
