# Proposal

## Why

The landing indicator stands right of Update in the header, which stands at its end, and was hidden while nothing was
on the way to master. When the first work came during a session (a robot or a pull request from another window, a
branch of brd), the indicator came, and Update and its Stop Waiting moved 126 px to the left from under a still pointer,
whose tooltip closed. The test of the answers of Patronus after a sync covered only an indicator that showed loading.

## What Changes

- While the repository is valid and nothing is on the way, the indicator keeps its place as an empty, disabled button
  ("Nothing on the way to master"), as wide as it will be: the first work fills it, and Update stays.
- The order of the header stays: incoming (Update), then the way into master.
- The test of the move to a branch of `repository-lock.spec.ts` asserts that the point the pointer pressed stays on the
  button: in a narrow window its spinner widens it to the left.

## Capabilities

### Modified Capabilities

- `repository/landing-overview`: the indicator keeps its place while nothing is on the way.

## Impact

- `src/components/repository/LandingIndicator.tsx`, three HTML snapshots of the header, `docs/contributing/frontend.md`.
