# The check that a companion has one primary runs with the write of its side

## Why

An add of a companion read the companion's config for its primary in the writer of the primary's config, before the
companion's `branchdeck.primary` was written in the companion's writer: two primaries that added one companion at once
both passed the check, and both named the companion, which then named the one that wrote last.

## What Changes

- An add is three steps, each in the writer of the config it reads and writes: the primary's check, which writes
  nothing (`write_companion_of_primary`); the companion's `branchdeck.primary` with the check that it has one primary
  (`write_companion_side`, `CompanionSide::Claim`); the key of the primary, with the check that the primary is still no
  companion (`finish_companion_of_primary`), whose failure takes back the primary the companion named just now
  (`CompanionSide::undo`). `brd companion` runs the same steps (`write_companion`).
- Capability `branch/declared-needs`.
