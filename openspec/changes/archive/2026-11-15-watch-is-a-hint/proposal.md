# Proposal

## Why

On 2026-11-15, with fseventsd at 100 % CPU for days, the watch of the operations registry told no change for 28 s and
more: a window showed the operations of another process and synced for a stamp of `brd` only at a focus, four of eight
E2E sessions heard the stamp of a new lease only in one late batch (air-page.spec.ts:214), and the waits of tests on
the watch hung until the timeout of nextest. The watches took an event for the list of what changed: an event of the
temporary name of a rename over a file, or of the directory itself, woke nothing, and nothing looked again without an
event.

## What Changes

- `sync_utils::dir_watch`: the one helper of the `notify` watches. Anything that may be a change is a wake (an event
  of another name of the directory, of the directory itself, a lost event, an error), and a waiter looks again every
  `RECHECK` (2 s) without an event, acting on what a stat says changed since its last look (`Fingerprints`).
- The watch of the operations registry looks at the registry, the stamp and the journal at every event of the
  directory and every 2 s: a window shows an operation of another process and syncs for a stamp within 2 s also when
  the watch tells nothing.
- The writer of the settings looks at its two files every 2 s while nothing is due.
- `TableWake` (the host slots, the heavy turn) wakes at any event that may be a change of its table.
- The E2E stamp fixture asks the sessions that watch the repository for a look at once, and its answer is of the
  session the window shows (the slot of a lease, else the repository).
