# Proposal

## Why

The review of the first change of develar/branch-deck#280 found that an `affinity` hit counted as warm also when a
clean (the clean after a run, the idle clean, a relief that may drop the output bases) had emptied the slot since the
last run of that affinity: `HostSlot::affinity` survives a clean, so a cold build went into the p50 of the hits, the
number that says what a warm slot is worth. `waited` read as a wait for busy slots while it counts every wait of the
lend, the spreads took cancelled, refused and lost runs, and `RunInfo::repo` was spelled as the client wrote it while
`stats` keys the expanded path.

## What Changes

- A lend says whether a clean came since the last run of its slot (`Lend::cleaned`: no `returned` of a give-back since
  the last lend, and a `cleaned_at` after it), carried as `cleaned` in `slot.json`, the history line and `RunInfo`.
  The choice of the slot does not change.
- The summary keeps the hits on a cleaned slot apart: `cleaned` of `SlotStats`, and the command of the warm hits, of the
  cleaned hits and of the others (`warm_command_ms`, `cleaned_command_ms`, `other_command_ms`), each of the commands
  that ran to their end; its line leaves out a part of no run.
- `waited` says that it counts every wait of the lend; `RunInfo::repo` is the expanded path, as in the line and `stats`.

Capabilities: `cli/host`.
