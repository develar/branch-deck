# Proposal

## Why

The heavy turn ran one heavy run at a time. On 2026-11-18 its queue waited 45 to 50 min, mostly behind runs that did
not fill the host: `pnpm lint:backend` used 5 to 17 of the 20 cores of the Studio while it held the turn alone. The
turn exists because two large runs at once fail on timeouts (2026-11-14: `cargo nextest run --workspace` beside a full
`pnpm e2e`, tests 3.6 times slower, 4 timeouts), not because any two heavy runs do.

## What Changes

- The supervisor of every run samples the CPU of its command group and of the Bazel servers of its slot every 10 s,
  writes `cores.json` of the run, notes the live cores in the heavy turn, and at the end adds its measure (the larger
  of the sampled CPU and `RUSAGE_CHILDREN` over the wall time) to the last 5 of its repository and command
  (`cores.json` of the agent, `lease_pool::host_cores`); a command is expected to use their 80th percentile.
- The heavy turn (`turn.json`, schema 2) has holders: the head of the queue is admitted while the expected cores of the
  holders (or their live cores, when more), the live cores of the runs that are not heavy and its own fit the cores of
  the host (`branchdeck.hostCores`, else the system's). A command without a measure, or one that measured the whole
  host, takes the whole host; no run overtakes the head of the queue. The live holder of `heavy.json` of the build
  before holds the whole host.
- The lines: the wait names what the run waits for and what each holder holds and why; `brd-host runs` says the cores
  held and each run's mean and peak cores; the end of a command says what it used.
