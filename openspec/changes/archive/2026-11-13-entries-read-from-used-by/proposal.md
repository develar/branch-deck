# Proposal

## Why

`entry_point` decided whether a declaration of a new module is entered from the `uses` of `accio surface`, which
`--limit` cuts (20 by default) and which list the uses tied by the name and those of tests before a later resolved one:
a declaration whose first uses were such was dropped although `used_by` held a file of other code that resolves it.
`used_by` did not say which of its files is test code, so it could not be read instead. accio's answer 2 of `surface`
(JetBrains/accio#748) gives each file of `used_by` its `test`, and orders the declarations by registrations, then by
the files that resolve them, no longer by every file of a use.

## What Changes

- `SurfaceSite::test`: the field of `used_by`.
- `entry_point`: entered by a registration or a file of `used_by` that is no test and no descriptor of it.
- `AccioCli` reads `answers` of `accio versions` once for `renames` and `surface`; `surface` is asked only of an accio
  whose answer is `SURFACE_ANSWER` (2), as `renames` is of `RENAMES_ANSWER`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-plan`: what enters a new module is read from `used_by`; an accio of another `surface` revision gives
  no entries.

## Impact

- `crates/file-diffs/src/accio.rs`, `crates/review-plan/src/entries.rs`.
