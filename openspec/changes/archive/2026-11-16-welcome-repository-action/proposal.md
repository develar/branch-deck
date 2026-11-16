## Why

The first welcome step looks like an action but is plain text. New users must discover the small folder button in the toolbar to begin.

## What Changes

- Make the complete repository step open the existing folder picker, with a visible button boundary and standard pointer and keyboard feedback.
- Keep an informational step on hosts without a folder picker.

## Capabilities

### Modified Capabilities
- `repository/onboarding`: the first step opens the folder picker directly.

## Impact

Frontend presentation, onboarding requirements, regression tests and snapshots. No backend or settings changes.
