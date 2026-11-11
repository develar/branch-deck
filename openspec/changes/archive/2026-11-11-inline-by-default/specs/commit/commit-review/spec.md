## MODIFIED Requirements

### Requirement: The sidebar and the diffs: The Inline/Unified/Split toggle switches the layout

The Inline/Unified/Split toggle MUST switch the layout; it is the setting of the Files section, Inline by default.

#### Scenario: Unified in the title bar
- **WHEN** the user chooses Unified in the toggle
- **THEN** the diffs switch to Unified, the shown layout pressed
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)

### Requirement: Opening and closing: The title bar names the list and the commit

The title bar of the review of a list SHALL name the list and the commit and end with the Inline/Unified/Split toggle;
it has no action and no "⋯". Nothing is edited there.

#### Scenario: The title bar of an unassigned commit
- **WHEN** the review of a list is shown
- **THEN** its title bar says "Unassigned › 11346cdf" (`listLabel` of the window data: the branch, or "Unassigned") and ends with the Inline/Unified/Split toggle
- **AND** it has no action and no "⋯", and nothing is edited there

## RENAMED Requirements

- FROM: `### Requirement: The sidebar and the diffs: The Split/Unified/Inline toggle switches the layout`
- TO: `### Requirement: The sidebar and the diffs: The Inline/Unified/Split toggle switches the layout`
