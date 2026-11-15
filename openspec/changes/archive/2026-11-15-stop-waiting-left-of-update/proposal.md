# Proposal

## Why

Stop Waiting came on the right of the Update button as the update began to wait for the repository lock. The header
stands at its end, so the button moved about 110 px from under a still pointer, and the tooltip with the steps closed
(`update-project.spec.ts`, "should show one progress from the click to the end of its sync").

## What Changes

- Stop Waiting comes on the left of the Update button: the button and what stands after it keep their places.

## Capabilities

### Modified Capabilities

- `repository/update-project`: where Stop Waiting comes.

## Impact

- `src/components/repository/UpdateProjectButton.tsx`, `tests/e2e/update-project/update-project.spec.ts`.
