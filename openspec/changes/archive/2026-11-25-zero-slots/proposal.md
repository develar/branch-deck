# Proposal

## Why

A host whose slot limit for a repository is 0 took a run and let it wait forever: on 2026-11-25 the laptop had
`branchdeck.hostSlots 0`, `run any` still chose it, and a run of a shrink limit of 0 (`brd-host slots --local --shrink
0`) took the heavy turn and then waited 1 h for `0 of 0 lent …; the host prepares its slots`, which it never would.

## What Changes

- The agent refuses the start of a run with the new code `no_slots` (exit 75) when the smaller of
  `branchdeck.hostSlots` and the shrink limit is 0, naming each limit at 0 and the command that raises it; a
  supervisor checks again before its heavy turn and while it waits for a slot of a host that has none.
- The estimate tells the limit (`Estimate::slots`), and a set chooses no member that tells 0, the first one neither.

Capabilities: `cli/host`.
