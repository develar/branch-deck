# Proposal

## Why

The third review of develar/branch-deck#280 found six small gaps: `cancelled` was read after the group of the command
ended, so a cancel in the seconds of that end marked a complete run; `first` and `taken` followed the starts, so a heavy
run that started first and was lent later made a first run `taken`; the outcomes listed a cancel under its exit code; a
report of an agent before the causes of a miss printed empty parentheses; the reason of an `expect` overstated; and no
test covered the relief that `cleaned` names.

## What Changes

- `run_group` answers whether the supervisor sent the SIGINT of a stop (`GroupEnd::interrupted`), which `exit.json`
  records; a cancel after the command exited leaves the run complete.
- `first` and `taken` follow the lends (the end of the phase `slot`); the outcomes key a cancel apart; a report without
  `misses` names the runs on another slot without parentheses.
- A test of the relief, the reason of the `expect`.

Capabilities: `cli/host`.
