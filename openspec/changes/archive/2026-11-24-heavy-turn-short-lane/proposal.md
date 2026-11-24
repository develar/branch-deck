# Proposal

## Why

Over 22.6 h of the Studio (144 runs, 20 cores) the heavy runs waited 3189 min in the queue and ran 346 min. The queue
is first come first and only its head is admitted, so in practice one heavy run ran at a time: nextest and e2e take
about 12 of the 20 cores, a lint about 9, and they do not fit beside each other. `pnpm lint:backend` runs 30 to 58 s
and waited 50 to 70 min behind them.

## What Changes

- A short run joins the heavy queue ahead of the runs it may pass instead of at its end: a run that is not exclusive,
  whose command has a measure below the whole host, and whose last passed runs took at most 2 min (the 80th
  percentile of the walls of the last 5 runs of the command that passed, of 3 at least).
- It goes before the first entry from which every entry to the end may be passed: a run that is not short and that
  short runs passed fewer than 4 times, and no exclusive head whose drain began. The admission is unchanged: the
  queue changes in `turn.json` itself, so a supervisor of the build before, which admits its head, reads it right.
- The walls of the passed runs are `walls.json` beside `cores.json`; the classes and the passes are `passes.json`
  beside `turn.json`, which no older build rewrites. A run that supersedes a waiting one takes its passes.
- A short run counts the earliest join of the runs it passed for the load bound of 15 min.
- The log of a short run names the runs it went ahead of; `brd-host runs` marks a short run and the passes of a run.

Capabilities: `cli/host`.
