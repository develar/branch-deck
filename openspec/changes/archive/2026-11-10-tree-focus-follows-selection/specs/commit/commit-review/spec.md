## ADDED Requirements

### Requirement: The sidebar and the diffs: The focus of the tree follows the selection

While the focus is on a row of the tree, a selection that the review moves (J, K, N, P, the diffs scrolled, a file
chosen from the card of a mark) MUST take the focus to the selected row, and the tab stop of the tree with it, so the
outline of the focus and the tint of the selection mark one row.

#### Scenario: J after a click on a row
- **WHEN** the user clicks a row of the tree and presses J, then K
- **THEN** the focus goes with the selection to each row J and K select, and exactly one row is marked
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should move the focus of the tree with the selection J and K move, so one row is marked`)
