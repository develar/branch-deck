# The cargo rule prunes under the locks of cargo and deletes the stale objects of rustc

## Why

On 2026-11-15 the leased slots of branch-deck held cargo targets of 48 to 120 GB. On macOS the dev default
`split-debuginfo=unpacked` keeps the objects of every past invocation of rustc in `out/` of a unit. rustc deletes old
incremental sessions, but their objects stay. A manual pass deleted 140,254 such files and gave back about 110 GB. The
cargo rule looked at no file inside a kept unit, so it never freed them. The rule also took no lock of cargo, so a
build that started in a checkout during `brd lease clean` raced its moves.

## What Changes

- A cargo rule that deletes takes `.cargo-build-lock` and `.cargo-lock` of each profile dir exclusive while it moves
  the stale units. It reads the units before the locks, and under them moves only a unit that no build wrote since.
  A held lock skips that profile dir with an error. A return waits up to 30 s for it, a takeover and a reclaim do not
  wait.
- In a kept unit the rule deletes each old object of an incremental compile that no session of `incremental/` holds:
  in a unit that links only an rlib, each one; in another unit, each one that no name of its newest invocation holds.
  An object of a compile without incremental stays. The objects go in place once the slot is free, never through the
  trash.
- The `keep` newest units count per package and target (`lib-bd_cli`, `test-bin-brd`), so a package with more current
  targets than `keep` keeps each of them.
- On another volume than the trash, the stale units go into `.brd-trash/` of their profile dir under the locks, and are
  deleted after the release.
- A dry run, and the due check of a host slot, count the stale objects by their inodes.
- Capability `cli/lease`.
