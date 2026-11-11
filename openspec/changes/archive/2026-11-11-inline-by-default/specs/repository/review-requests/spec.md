## MODIFIED Requirements

### Requirement: Review window: The tabs Overview and Changes

Below them the window MUST have the tabs Overview and Changes (`D`, with the file count) on the left; while the
changes show, the sidebar toggle and the Inline/Unified/Split toggle with Whitespace at the end of the tabs, in the
header of the block they switch.

#### Scenario: The tabs while the changes show
- **WHEN** the changes show
- **THEN** the sidebar toggle and the Inline/Unified/Split toggle with Whitespace stand at the end of the tabs
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should preview the pull request, fetch it with progress, then review its range with the file tree`)
