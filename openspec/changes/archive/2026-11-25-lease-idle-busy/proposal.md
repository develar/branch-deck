# Proposal

## Why

`brd lease status` and `brd workspace` called a lease `idle` by the pid of its holder alone. A resumed session runs as
another process, so a slot with a live session looked free to take: on 2026-11-25 slot 32 of idea-1 had a dead holder
since the day before, while its resumed session ran `brd-host wait` and seven shells in the slot, and the status said
`idle`. `--processes` listed those processes, but the `idle` state stayed.

## What Changes

- `lease_status_core` takes a scan mode (`StatusScan::None`, `Idle`, `All`). `Idle` scans the processes once, and only
  when a lease has a gone holder; `brd lease status` and `brd workspace` use it.
- A lease whose holder is gone is `busy` while a process other than the Bazel servers of its slot works there, with
  `processes`, and it keeps the work fields a check found.
- The help of `brd help worktrees` says what an agent does on `busy`: the session lives, never return the lease.

Capabilities: `cli/lease`, `cli/workspace`.
