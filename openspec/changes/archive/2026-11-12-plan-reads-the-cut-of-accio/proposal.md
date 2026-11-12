# Proposal

## Why

`ModuleEntries::cut` guessed the cut of `accio surface` from its answer: not complete with a note. accio now says what
stopped it in a `cut` field (JetBrains/accio#712): `"work"`, a bound on files read that cuts the same on every run, or
`"time"`, its `--budget` seconds, of the load of the moment; `null` when nothing was cut. The guess counted an answer a
bound on work cut as cut, so a plan that is the same on every run was not kept.

## What Changes

- `ModuleSurface::cut` reads the field, absent apart from `null`.
- `ModuleEntries::cut` is true for `"time"` and a value it does not know, false for `"work"` and `null`; an accio
  without the field keeps the old rule (not complete with a note).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-plan`: the entries of a new module are cut by the `cut` of accio's answer.

## Impact

- `crates/file-diffs/src/accio.rs`, `crates/review-plan/src/entries.rs`.
