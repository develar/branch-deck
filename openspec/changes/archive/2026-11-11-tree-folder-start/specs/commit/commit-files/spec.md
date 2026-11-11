## ADDED Requirements

### Requirement: Files section: A directory row starts its path at its icon, a long one cut from its start

The path of a directory row MUST start right after its folder icon, however wide its row; a path wider than its row
MUST give way from its start ("…"), so that its last folder stays in view by the counts.

#### Scenario: A short and a long directory
- **WHEN** the tree shows a directory one word long and a directory whose path is wider than its row
- **THEN** the short path starts right after its icon, the room after it empty up to the counts
- **AND** the long path is cut at its start, its end in view at the right of its box
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should start the path of a directory at its icon and cut a long one from its start, its last folder kept`)
