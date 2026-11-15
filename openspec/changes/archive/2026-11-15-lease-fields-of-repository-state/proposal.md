# The fields of a lease in the state of a repository are Rust's, each under its lease

## Why

A late write of the window of the lease before carried `lastBranchCount` into the new lease of a slot: the header
showed the loading placeholder of rows ("On the way to master: checking…", busy) in the Ready to Sync card until the
user's first sync (19 of 600 runs with the review's assertion). `landedHiddenUntil` had the same flaw: the hide of the
Landed island of the lease before would hide branches of the report of the next.

## What Changes

- `LEASE_FIELDS` (`lastSyncTime`, `lastBranchCount`, `landedHiddenUntil`): the one list of the fields of an entry of
  `repositoryState` that belong to a lease; only Rust writes them, each under its lease (`set_repository_fields`), a
  write of a window keeps the writer's values for its epoch (`keep_rust_fields`), and a new lease starts without them.
- The session records the count of the branches of its sync with its time (`record_synced`); the island's hide is
  `hide_landed`, under the lease of the view it was asked on (`Sessions::lease_of_view`); the page writes neither.
- The settled layout, the durations of the steps and the time of the last fetch stay the windows'.
- Capabilities `repository/onboarding`, `repository/landed-commits`.
