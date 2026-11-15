# Proposal

## Why

The review of 209e12591 (`lease_read_again`) found three lows:

- `hide_landed` took a refusal of `Sessions::lease_of_view` for a session that knew no lease of its slot
  (`AnotherLease::ReadAgain`) as a view of another lease and answered nothing: the island had cleared what it saw, and
  the hide was lost until the island was seen again, though the session opened anew under the lease read and its view
  is the one to hide on.
- `lease_read_again` of a rewrite said "sync, then ask it again", the words of a plan that changed: the session opens
  anew under the lease read by itself, and no sync is asked.
- The docs and the spec said the session opens anew under the lease read at once; while a run is in flight it opens anew
  once that run ended (`Actor::reopen_for_lease`), as a run is never cancelled.

## What Changes

- `hide_landed` answers `lease_read_again` for `ReadAgain`; the island stays seen as it was on that refusal
  (`hideAfterSync`), so the next sync hides what was seen on the view of the lease read.
- `lease_read_again` says "ask it again" of a rewrite; `plan_changed` of a rewrite still says "sync, then ask it again"
  (`witness::lease_read_again_of`, also of the hide).
- The words of the reopen say "at once, or once the run in flight ended", with a test of the run in flight.

## Capabilities

### Modified Capabilities

- `repository/onboarding`: an intent of a session that knew no lease of its slot.
- `repository/landed-commits`: a hide of the view of such a session.

## Impact

- `crates/app-api/src/session/landed.rs`, `crates/operations/src/witness.rs`, `src/stores/landed.ts`,
  `crates/repository-model/src/session.rs` (a comment), `docs/design/git-operations.md`.
