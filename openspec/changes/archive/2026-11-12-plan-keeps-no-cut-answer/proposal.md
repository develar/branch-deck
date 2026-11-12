# Proposal

## Why

`accio surface --budget 15` stops at a deadline of the wall clock: under load it asks fewer declarations of a new
module and answers `complete: false` with the reason in its note. The entries order the group of the module
(`groups::order_by_entries`), and the app kept every plan whose `index_note` was null, so a plan a loaded machine read
stayed in its memory and every later window showed it. accio is asked for a budget of work and a field that says the
clock cut the answer (JetBrains/accio#712).

## What Changes

- `ModuleEntries::cut` (not in the JSON): accio's answer is not complete and says why in its note; a use left
  unresolved alone is no cut.
- `ReviewPlan::is_final`: `index_note` null and no entries cut. The memory of the app keeps only such a plan.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-plan`: a plan with entries accio cut short is not kept in memory.

## Impact

- `crates/review-plan/src/entries.rs`, `crates/review-plan/src/lib.rs`, `crates/app-api/src/reads/review_plan.rs`.
- `docs/design/git-operations.md` (the memory of the plans).
