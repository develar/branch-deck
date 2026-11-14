# Proposal

## Why

`brd-host follow`, `upgrade` and `trim` installed the handler of Ctrl+C and never read it, so neither Ctrl+C nor (since
the heavy runs) SIGTERM ended them: a follow that an agent started in the background and stopped hung for the length of
the run.

## What Changes

- The first Ctrl+C, SIGTERM or SIGHUP leaves a follow at once with 130; the run goes on on the host.
- The first one ends `trim` and `upgrade` at once with 130 and a line of what goes on on the host.
- Capability `cli/host`.
