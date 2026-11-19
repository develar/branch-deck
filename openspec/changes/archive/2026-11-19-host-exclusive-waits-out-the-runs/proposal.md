# Proposal

## Why

Twice on 2026-11-19 an exclusive run took the host in a lull of one sample of a bursty `scripts/bench ab-run` that had
started before it (runs 1795114056189-5191 and 1795103574230-82667): the bench used 0 to 1.8 cores in its lulls and 10
to 11 at its peaks, ran beside the measurement, and the load median of the measured lane doubled.

## What Changes

- An exclusive run waits until no run that is not heavy is in its command, of any repository: a run is in `light` of
  the turn from the start of its command to its end, not from its first sample to its give-back.
- Then it waits until every sample of the host of the last 30 s is quiet: `serve` keeps the samples of 30 s and
  `host.json` their most; a shorter window admits it not.
- Its wait names each run it waits for and the most busy cores of the last 30 s. The timeout stays 2 h.
