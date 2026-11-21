# Proposal

## Why

`brd lease clean --leased --dry-run` freed 0 bytes in held slots of accio, although slot 2 had 56 GB in `target/`. In
`target/test-fast` of that slot, `incremental/` held 22 GB in 814 dirs, 16 to 25 per crate, and `deps/` 1.7 GB with
at most 4 units per package. With `keep = 8` no group of units had a stale unit, and each old kept unit kept the
incremental dir that it wrote.

## What Changes

- An incremental dir stays by itself only when it is the newest of its crate or younger than `age`.
- Only a current unit keeps the incremental dir that it wrote: the last built unit of its group, or a unit younger
  than `age`. An older unit that `keep` holds keeps no dir.

Capabilities: `cli/lease`.
