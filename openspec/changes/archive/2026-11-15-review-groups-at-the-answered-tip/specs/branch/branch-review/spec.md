## MODIFIED Requirements

### Requirement: The files of each reviewer group (Safe Push popover): A check that answers marks the open window

A check that answers while the window is open MUST mark its groups there too: the window's session answers it
(`knownReviewGroupsOf`, null while none answered, when the groups of the open stay). The groups are of the tip the
check asked about (`ReadinessEntry.tip`), never of the head the window holds when the answer comes.

#### Scenario: The check answers while the window is open
- **GIVEN** the review window of a branch is open before its code-owner check answers
- **WHEN** the check answers
- **THEN** the window marks the groups that need approval, from its own session (`knownReviewGroupsOf`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should mark the groups that need approval as the code-owner check answers while the window is open`)

#### Scenario: The check answered before the session of the window told it the branches
- **GIVEN** the review window of a branch opened before its code-owner check answered
- **WHEN** the check answers before the first view of the window's session, which brings the branches with the answer
- **THEN** the window marks the groups that need approval, of the tip the check asked about (`rememberReviewGroups`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should mark the groups of a code-owner check that answered before the session of the window told it the branches`)

#### Scenario: No check answered yet
- **WHEN** no check answered while the window is open (`knownReviewGroupsOf` is null)
- **THEN** the groups of the open stay
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should mark the groups that need approval as the code-owner check answers while the window is open`)
