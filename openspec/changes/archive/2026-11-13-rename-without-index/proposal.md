# Proposal

## Why

A check of the real app found a rename whose proof never comes. The read ahead of Your Turn took the index to be there
whenever the accio CLI was on the machine (`SymbolIndexes::search(...).is_some()`) and never asked whether this
repository has an index. In a repository without one a private member renamed waited for the index
(`RenamesPending`), and its row said "checking renames" for ever. The review window then asked the index, got "no index
of this repository" and said "1 file not proven", but that reading was not final (an index was "there"), so every
listing (some three minutes apart) read the request again, its renames waiting again.

## What Changes

- `SymbolIndex::has_index`: whether the repository has an index the queries may answer from, or one is being built;
  accio answers by `accio stats --no-build` (tens of milliseconds without an index), the fixture index by its fixture.
- `range_readings::repository_indexed`: no accio, none; else what the index says, kept for ten minutes per
  repository (`ReviewPlans::indexed`); an index that does not say may answer. The read ahead asks it before it reads,
  and a rename waits only where it is true; the window asks it of a reading its index left unanswered.
- Such a reading is final: kept in memory and on disk, so no listing reads it again; dropped from both once the
  repository has an index.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `review/range-reading`: the read ahead waits only for an index of the repository; a reading of a repository without
  one is kept until it has one.
- `repository/review-requests`: a reading no index can answer is final, not read again.

## Impact

- `crates/file-diffs/src/accio.rs`, `crates/test-server/src/state.rs`, `crates/app-api/src/reads/{range_readings,
  review_plan,review_prefetch}.rs`; the `stacked_reading` template (a private member renamed in `ledger/Ledger.kt`).
