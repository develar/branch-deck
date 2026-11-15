# Proposal

## Why

`brd-host run` forgets its run on the host as soon as the follow ended, and the agent removes the run directory once
`exit.json` exists. The supervisor writes `exit.json` before it gives the slot back, so its give-back lines went to
`supervisor.log` of a removed directory: why a give-back failed or kept a slot in quarantine, and whether the heavy turn
went back, could not be read anywhere on the host, only the state that `brd-host slots` shows.

## What Changes

- `serve` gives each supervisor its own stderr, the log of the agent, as the stdout. Each line of the supervisor after
  `exit.json` goes to `supervisor.log` and, with the id of the run, to the log of the agent.
- Capability `cli/host`.
