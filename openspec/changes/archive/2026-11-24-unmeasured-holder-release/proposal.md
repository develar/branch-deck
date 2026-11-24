# Proposal

## Why

A heavy run without a measure holds the whole host for its whole life. On 2026-11-24 the first run of the class
`accio-typing-gate` held the whole Studio for over 2 h at 0.6 of its 20 cores on average (18.3 at its peak, its late
phases on one core), and the heavy runs behind it ended with `heavy_timeout`: one slow first run of a class blocks the
host for hours.

## What Changes

- A holder without a measure holds the whole host for its first 10 min from its admission; then it counts the most of
  its live use of the last 5 min, which its supervisor notes at each sample (`recent` of its entry, the most of each
  half minute), and the head of the queue is admitted beside it while it fits. A burst counts at its next sample.
- A window that spans less than 5 min, whose last sample is more than 1 min old, or that leaves less than a fifth of
  the host beside it holds the whole host still; a supervisor of an older build drops `recent`, which costs a new window.
- A run of the queue without a measure, an exclusive run and the measure at the end of a run are unchanged.
- The line of the wait and `brd-host runs` say `no measure: it held the whole host for its first 10 min, now its live
  use of the last 5 min` of such a holder.

Capabilities: `cli/host`.
