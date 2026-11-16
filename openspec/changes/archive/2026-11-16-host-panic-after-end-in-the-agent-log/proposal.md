# Proposal

## Why

055c0c56 sends the lines of a supervisor after `exit.json` to the log of the agent, since a client forgets the run once
it ended and `supervisor.log` goes with its directory. A panic there, such as in the git of the pool that the give-back
runs, still went only to `supervisor.log` through the default hook, and so went with the forgotten directory.

## What Changes

- After `exit.json` the supervisor chains a panic hook that writes the panic as one line with the id of the run to the
  log of the agent, after the hook it had.
- Capability `cli/host`.
