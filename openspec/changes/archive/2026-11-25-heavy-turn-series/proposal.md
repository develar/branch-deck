# Proposal

## Why

The agent keeps no series of the heavy turn. The line of a wait goes into the log of each waiting run, which goes with
the run: on 2026-11-25, 428 of the 949 waiting minutes of a day had no line left. The review of the admission design
asks for a series first, with what a backfill behind the head would start, so that one day gives its gain and the
error of its estimates before any admission changes.

## What Changes

- The sampler of `serve` writes one minute of the heavy turn a minute while a heavy run holds cores or waits, into
  `history/turn-<YYYY-MM-DD>.jsonl`: the busy cores, the holders with their count and live cores, the head and its fit,
  why no run started, and the run that the backfill of design 4.1 would start, a dry run that nothing acts on. A day
  stops at 4 MiB, and the days past 30 go.
- `brd-host stats` gives the idle cores while heavy runs waited, by cause, and the runs of the dry backfill.

Capabilities: `cli/host`.
