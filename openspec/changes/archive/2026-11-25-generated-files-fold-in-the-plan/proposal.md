# Proposal

## Why

The plan of a generator's run over the monorepo (jps-bridge of idea-1: 7596 files, 7378 of them generated) was 11.7 MB
that no agent reads: 7388 modules, 7318 of them roots of generated files alone; 2809 dependency changes, 2760 of them
the entries a generator wrote; and each generated file once in `files`, `order`, the slices of its unit and the stops
and paths of its step (7367 stops in one step).

## What Changes

- A root whose changed files are all generated is no module of the review; its files have no module.
- A generated manifest has no dependency change of its own: its entries are the output of its generator.
- `brd review-plan` keeps the first 20 generated files of each of those lists and counts the rest (`files_omitted`,
  `order_omitted`, `slices_omitted`, `stopsOmitted`, `pathsOmitted`); the window reads the whole plan. `"schema": 41`.

Measured on jps-bridge: 11.7 MB, then 8.0 MB with the modules and dependencies, then 1.2 MB folded.

Capabilities: `branch/review-plan`.
