# The niceness of a child is never above the one of its parent

## Why

`sync_utils::child_process::set_niceness` set the niceness of a child with setpriority(2) between fork and exec, and
failed the start when the call failed. A process without privilege may not lower its niceness, so a child asked for a
niceness below the one of its parent never started: every run of `brd-host` asked for 10 (`--nice 0` of bt too) failed
with `spawn_failed: cannot start … Permission denied (os error 13)` under an agent above 10, such as one started from a
niced shell. The tests of `bd-host` failed the same way under a runner at nice 15 (zsh runs a job of `&` at nice 5).
The line before the command named the niceness of the spec, or 0 without one, not the one the command ran at. On
Windows a background child asked for the below-normal class of its own under an idle app, above it.

## What Changes

- `set_niceness` raises the niceness a child inherits to the one asked, and keeps a higher one: a niceness never fails
  the start of a child, and a child never runs above its parent; `child_niceness` names the niceness the child gets,
  `own_niceness` the one of this process.
- The `running in` line of a run names the niceness its command gets: the one of the spec, or the agent's when that is
  higher or the spec names none.
- On Windows a background child of an idle app asks for no class of its own and keeps the idle class it inherits.
- Capabilities `cli/host` (a command is never more urgent than the agent) and `branch/review-plan` (a child in the
  background of an app below it).
