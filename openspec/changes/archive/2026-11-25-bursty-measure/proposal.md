# Proposal

## Why

The first run of `accio-typing-gate` measured a mean of 1.0 of the 20 cores of the Studio over 2 h: it builds, tests
and lints on 16 to 18 cores for 1 to 3 min a few times and runs on one core between. Five gates were then admitted at
1.0 each and built at once, 4 to 5 times the cores of the host; the E2E runs beside them took 39 min instead of 5 and
the timing tests of nextest failed. The mean is the wrong expectation of a bursty run, and its peak would keep the
next gate out for hours.

## What Changes

- A run measures its level of work too: the live cores at or above which 30 % of the core seconds of its samples ran
  (`level` of a measure in `cores.json`); a command is expected to use the 80th percentile of the levels of its last
  runs, each at least its mean. A measure without a level counts its mean.
- Every holder that is not exclusive counts its live use of the last 5 min once 10 min passed since its admission,
  not only one without a measure; before, the larger of its expectation and its live use as before.
- The line of a holder of a measure past its grace says `measured <cores>: it held that for its first 10 min, now its
  live use of the last 5 min`.

Capabilities: `cli/host`.
