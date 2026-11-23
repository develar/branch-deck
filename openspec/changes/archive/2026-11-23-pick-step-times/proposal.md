# Proposal

## Why

On 2026-11-23 `brd lease pick` in idea-1 had a median of 27 s and a tail of 3 to 22 minutes, and nothing said where
the time went: the checks, a witness that missed and sent the pick round again, the wait for the repository lock, the
moves of the files of two index files of 1.4M entries, or the read of the landing after them. The pick printed nothing
until its JSON, so an agent waited blind and wrapped it in `timeout`.

## What Changes

- `brd pick` and `brd lease pick` print each step on stderr as it starts, with the time since the pick began, as
  `brd lease get` does: `brd lease pick: [12 s] checking the commits against master (round 2)`.
- The answer lists the steps with when each started (`steps`, `at_ms`).
- Nothing is printed between the move of the branch and the end of the moves of the files, and a write to a closed
  stderr is ignored, so a pipe into `head -c` never stops a pick halfway.

Capabilities: `cli/pick`.
