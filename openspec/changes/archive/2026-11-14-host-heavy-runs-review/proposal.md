# Proposal

## Why

The review of the heavy runs (host-heavy-runs) found gaps: an agent before heavy runs refuses every run of a commit
with `heavy`, a light one too, while the client said the run went on beside the others; a client that a tool stops with
SIGTERM left its run waiting for the turn and then running for nobody; the advice for `heavy_timeout` sent a heavy run
to the loaded client; a supervisor of another build would read a table it does not know; and the watch of a wait saw
the logs of every run.

## What Changes

- The client refuses before the start, with `bad_config` and the upgrade, a run of a commit whose `[host]` needs a
  feature that the agent lacks (`heavy` needs `heavy`, even empty).
- SIGTERM and SIGHUP of the client count as Ctrl+C: the run is cancelled on the host.
- `heavy_timeout` says to run it on the host again later, never on the client.
- `heavy.json` names its schema, and a table of another schema refuses; it moves to `~/.cache/brd-host/heavy/`, and a
  wait keeps one watch of that directory for its whole time. A holder holds the turn from the moment it took it.
- The keys that the check of `[host]` knows are the fields of its serde types.
- Capability `cli/host`.
