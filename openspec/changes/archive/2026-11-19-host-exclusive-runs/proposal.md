# Proposal

## Why

A measurement on the Studio, the A/B pair of a fast lane, needs a quiet host. A heavy run of the whole host waits for
the other heavy runs only: the runs that are not heavy start beside it at once and spoil the measure. The whole-host
runs of the heavy turn are mostly new commands, not measurements, so they must not stop the other runs.

## What Changes

- `brd-host run --exclusive`, an explicit flag that implies `--heavy`: the run is admitted only without a holder on a
  quiet host, and while it holds no heavy run is admitted and each new run that is not heavy waits for its end before
  its lend, in the step `exclusive`, with a line that names the measurement.
- `brd-host runs` names an exclusive holder; the client refuses an exclusive run on an agent before the feature.
