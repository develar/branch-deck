# Proposal

## Why

Under load Activity showed one rewrite twice, each entry with its Undo ("Renamed docs to documentation (2 commits)",
2 of 370 runs of `move-commits.spec.ts`). The page records the entry of a rewrite in the journal of the app data
directory (`record_activity`) and adds the entry its answer names to the log it holds; the write tells every window to
read the log again (`activity-changed`), and the answers of two calls come in another order than their work on a loaded
machine: a read made after the write that answered before the record brought the entry, and the record added it a
second time. Both had the Undo of the same entry, so either undid the rewrite once. The other order lost the entry until
the next read: a read made before the write that answered after the record replaced the log without it.

## What Changes

- The page adds the entry a record answers only when its log lacks it (by id), and keeps it beside a read that may have
  been made before the write (asked before the record answered); a read asked after the record answered is the log's,
  so Clear Log still forgets it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `repository/activity`: Log: an entry the main window recorded shows once, whichever answer comes first.

## Impact

- `src/stores/activity.ts` (`setLog`, `recordActivity`), `src/test-support/fake-activity.ts` (`holdAnswer`),
  `docs/contributing/e2e-tests.md` (Pitfalls).
