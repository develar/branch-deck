# Proposal

## Why

A get killed during its clone leaves `<pool>/<n>/<repository>.cloning`: no state names it, nothing deletes it, and
`next_slot_path` passes over its number for ever. The idea-1 pool kept 51 GiB (by `du`) of one from 2026-11-07 until
2026-11-18, when it was found by hand.

## What Changes

- A get that takes the clones lock moves into the trash every number directory of its pool that no state of the
  machine names and that holds only `<repository>.cloning`: under that lock no clone of the repository runs. The trash
  is left to a later sweep, never deleted under the lock.
