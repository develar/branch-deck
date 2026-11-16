## Why

With no saved repositories, the selector opens a popup with no content. On a fresh install it looks like a rendering failure.

## What Changes

- Explain the empty recent-repository list and how to populate it using the folder button.
- Give hosts without a folder dialog appropriate IDE guidance.
- Name the selector for assistive technology and associate the empty explanation while its popup is open.

## Capabilities

### Modified Capabilities
- `repository/title-bar`: the repository selector explains its empty state.

## Impact

Repository selector presentation, regression tests and snapshots. No backend or settings changes.
