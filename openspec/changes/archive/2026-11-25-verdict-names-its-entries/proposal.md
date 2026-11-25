# Proposal

## Why

The plan of jps-bridge said `verdict: "breaks"` and no more: an agent found why only by reading the 99 entries of
`api` for the five whose uses left outside can break.

## What Changes

- `brd review-plan` prints `verdict_by`, the entries of `api` that decide the verdict by their place: those with a use
  left outside that can break, else those whose uses are unknown; none when no use breaks. `"schema": 42`.

Capabilities: `branch/review-plan`.
