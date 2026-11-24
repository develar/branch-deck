# Proposal

## Why

A run names one host, and the heavy queue of the Studio held runs for a median of 37 min (22.6 h, 144 runs, measured
2026-11-23) while a laptop beside it was free. A run cannot go where it would end first.

## What Changes

- A set of hosts, `~/.config/brd-host/sets/<name>.json` (`brd-host set <name> <alias>…`, `--idle-only <alias>`): a run
  to its name asks every member what it would meet there, ranks them, says its plan in one line and runs on the first;
  a heavy run goes to a member other than the first only with that member's measure of the command.
- A member of `idle_only` gets the start that admits at once or refuses with `busy`, and the run goes to the next
  member.
- `<run>.host` beside the run record names the member, so `wait`, `cancel`, `follow`, `fetch` and `forget` to the name
  of the set go there; `RunReport.alias` names it.
- `run <alias>` stays as it is; `--exclusive` with a set refuses with `usage`.

Capabilities: `cli/host`.
