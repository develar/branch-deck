# Proposal

## Why

The choice of a member of a set of hosts (change host-sets) needs what each host would do with a run, and a laptop
that a person works on must take a run only when it has room now, never queue it, so several clients never meet on it.

## What Changes

- `Request::Estimate` behind the feature `estimate`: `has_commit`, `warm`, `cores`, `expected_cores`,
  `expected_wall_ms`, `start_in_ms`, `unknown_ahead`, `os`, by the queue played forward (`lease_pool::host_estimate`).
- `RunSpec::admit_now`: the supervisor takes the heavy turn in one look or ends the run with the new code `busy` (exit 75)
  before its lend; the start answers the decision.
- `branchdeck.hostJobs` of a host: `CARGO_BUILD_JOBS`, `NEXTEST_TEST_THREADS` and `BRD_HOST_JOBS` of each command.
- `Request::Runs` takes an optional `run`, which an older agent ignores.
- The schema of the protocol stays 1; `heavy_timeout` names a set of hosts.

Capabilities: `cli/host`.
