## MODIFIED Requirements

### Requirement: Sub-windows: The row of `review` says its subject

The row of `review` MUST hold Back while it has a review it left, then what its subject, a branch, a list of commits or
a pull request, puts there, and the native title MUST name the subject.

#### Scenario: A review it left
- **WHEN** the review window has a review it left
- **THEN** its row starts with Back

#### Scenario: The review of a branch
- **WHEN** the review window shows a branch
- **THEN** its row holds the stack bar, Inline/Unified/Split and the actions on the branch
- **AND** the native title is "Review login", "Review login · a1b2c3d" when it opened at a commit
  ([Branch Review](../../branch/branch-review/spec.md))
  [@test] tests/e2e/repository/title-bar.spec.ts (`should start the row of a review window after the traffic lights, its empty parts dragging the window`)

#### Scenario: The review of a list of commits
- **WHEN** the review window shows a list of commits
- **THEN** its row says "Unassigned › 11346cdf", the list and the commit (the hash in `font-mono`), and holds
  Inline/Unified/Split ([Commit Review](../../commit/commit-review/spec.md),
  [Review Window](../../review/review-window/spec.md#one-window))

#### Scenario: The review of a pull request
- **WHEN** the review window shows a pull request
- **THEN** its row holds ↑ ↓ and "Your Turn › 2 of 5", GitHub, "⋯" and the decisions on the pull request, Approve last
- **AND** its Inline/Unified/Split stands at the end of its tabs, the header of the changes it switches
  ([Review Requests](../review-requests/spec.md#review-window))
  [@test] tests/e2e/repository/title-bar.spec.ts (`should hold the decisions of a pull request review on its one line, and its diff view at the end of the tabs`)
