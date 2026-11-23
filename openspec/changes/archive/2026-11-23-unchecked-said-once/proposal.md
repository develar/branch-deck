# Proposal

## Why

Agents read the answers of `brd lease pick` as `2>&1 | tail -c`: the reason of `--allow-unchecked` came twice, on
stderr and in `unchecked`, and also for a conflict that landed nothing, beside the body of the commit of the update.

## What Changes

- Only a pick that landed commits names the reason on stderr, once, and repeats it in `unchecked`; a dry run, a
  conflict and a pick with nothing to land do neither.

Capabilities: `cli/pick`, `cli/lease`.
