# Proposal

## Why

Per repository, 14, 11 and 32 local checkouts share the 4 host slots of the repository on a host shared by several
agent sessions. To choose `branchdeck.hostSlots`, and to decide whether more work can move into host slots, one needs
the share of runs that got the slot their affinity used last, and what a miss costs in the command. Neither could be
read: `RunInfo` named no slot and no repository, the history line no slot, and the slot was only in the log line
`running in <slot> (slot <n>)`, which a forget cuts (develar/branch-deck#280).

## What Changes

- A lend says how it chose its slot (`host_slots::Lend::choice`: `affinity` or `idle`) and whether its ticket waited in
  the queue of the slots first (`Lend::waited`); the supervisor writes it with the number of the slot to `slot.json` of
  the run (`host_history::SlotRecord`).
- `RunInfo` of `brd-host runs --json` names the repository (`repo`) and the slot (`slot`), and the history line the
  slot (`slot`), as optional fields that an older agent or line leaves out.
- `brd-host stats` prints one line per repository with the share of runs that got the slot of their affinity and the
  command p50 of those that did and of the others; `--json` has the same numbers as `slots`.

Capabilities: `cli/host`.
