# Proposal

## Why

A laptop gets warm build slots and the keeper of brd-host only as a host of its own, and `setup` made a host only over
ssh. The agent listened on every address, so an agent on a laptop would take connections from the network.

## What Changes

- `brd-host setup --local [<alias>]` (alias `local` by default) makes this machine its own host without ssh. It puts
  this build at `~/.local/share/brd-host/agent/brd-host` when another build is there, runs the agent setup in process
  with that binary, and writes the host file of the loopback address. `~/.local/bin/brd-host`, the client of this
  machine, stays as it is. The certificates are pinned on both ends, as `setup <alias>` does.
- The agent setup takes a bind address, and the `LaunchAgent` passes it to `serve` as `--bind`. A local host binds the
  loopback address. `serve --bind` is no longer a hidden flag.
- The spec says what the keeper of a local host does on this machine: the relief of every pool below
  `branchdeck.leaseMinFree`, and `brew upgrade git` below the floor of the git of a host.

Capabilities: `cli/host`.
