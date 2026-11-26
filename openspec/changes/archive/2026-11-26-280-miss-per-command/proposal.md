# Proposal

## Why

The second review of develar/branch-deck#280 found that the summary of the lends could not say what a miss costs. Its
spreads mixed every command of a repository, so a reader compared the short repeats that hit with the first heavy
builds that miss. A cancel whose command exited with a code other than 130 (Bazel 8, 137 after the SIGKILL) counted as
a command that ran to its end. The counts beside each p50 were not its runs. And every miss was one `other`, though
only a slot that another checkout took since shrinks with more `branchdeck.hostSlots`.

## What Changes

- The supervisor writes `cancelled` into `exit.json` when a cancel or a stop signal came while the command ran, and the
  history line carries it; the spreads and the failures of a command read it, not the exit code.
- A lend of `idle` says why it missed the slot of its affinity (`missed`: `no_slot`, `lent`, `unavailable`), and the
  summary counts the misses by cause, `no_slot` split into the first run of the checkout in the window and a slot that
  another checkout took since.
- The spreads of the lends are per command of a repository (`commands`, the 10 with the most runs and the rest in one
  entry), each case a bucket with its runs and the p50 and p90 of its `command`, `checkout` and `prepare`; the line
  prints a p50 only from 5 runs.
- The line prints the warm share, leaves out a part of no run, and counts in saturating arithmetic.

Capabilities: `cli/host`.
