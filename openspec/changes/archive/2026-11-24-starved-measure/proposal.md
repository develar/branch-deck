# Proposal

## Why

A laptop host under the load of agent sessions measured `pnpm lint:backend` at 0.5 cores in 13 min 45 s and `cargo
nextest run --workspace` at 1.1 cores in 46 min 37 s, while the Studio measures 6.7 cores in 58 s. Such a measure
tells the load, not the command: the admission of the heavy turn then sees a command that needs almost nothing and
takes it beside the next load, and the estimate of the host says the wall of the load.

## What Changes

- The supervisor notes the busy cores of the whole host beside each sample of a run, less the run's own.
- A run whose mean of them and its own mean exceed the cores of the host adds no measure and no wall of its command,
  and its log says why in one line.

Capabilities: `cli/host` (modified).
