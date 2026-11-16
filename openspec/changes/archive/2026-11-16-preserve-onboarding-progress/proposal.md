## Why

The first-time welcome card replaces repository step 1 with another step 1, losing the sense of progress. Show the upcoming task and remember the completed repository selection, including across restarts.

## What Changes

- Show a two-step first-time overview, then a completed repository row and active prefix step 2.
- Persist in-progress/completed status; existing users retain standalone prefix setup.
- Continue automatically when the repository session confirms a usable prefix.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `repository/onboarding`: persistent first-time progress and accessible step transitions.

## Impact

Frontend settings, welcome card and lifecycle tracking, focused unit/E2E coverage and snapshots. No backend command, binding, dependency or domain-rule changes.
